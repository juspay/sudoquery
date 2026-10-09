use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;
use superposition_provider::data_source::file::FileDataSource;
use superposition_provider::{
    AllFeatureProvider, EvaluationContext, LocalResolutionProvider, RefreshStrategy,
};

use crate::collector_event::CollectorEvent;
use crate::enrichment::EnrichmentConfig;
use crate::kafka_connector::{KafkaConnectorConfig, render_topic};
use crate::result;

/// Kafka settings can be overridden via environment variables, taking priority
/// over the values resolved from `cac.toml`:
///
/// - `KAFKA_TOPIC`: overrides `kafka_connector.topic` (may use the same
///   `{org_id}` / `{project_id}` placeholders)
/// - `KAFKA_BOOTSTRAP_SERVERS`: overrides `kafka_connector.client_config["bootstrap.servers"]`
/// - `KAFKA_CLIENT_CONFIG`: JSON object merged into `kafka_connector.client_config`
///   using the same keys as `cac.toml` (e.g. `{"message.timeout.ms": "10000"}`);
///   entries here win over `cac.toml`, but `KAFKA_BOOTSTRAP_SERVERS` wins over both
///
/// Empty or whitespace-only values are treated as unset.
const KAFKA_TOPIC_ENV: &str = "KAFKA_TOPIC";
const KAFKA_BOOTSTRAP_SERVERS_ENV: &str = "KAFKA_BOOTSTRAP_SERVERS";
const KAFKA_CLIENT_CONFIG_ENV: &str = "KAFKA_CLIENT_CONFIG";

#[derive(Debug, Default)]
struct KafkaEnvOverrides {
    topic: Option<String>,
    bootstrap_servers: Option<String>,
    client_config: Option<HashMap<String, String>>,
}

impl KafkaEnvOverrides {
    fn from_env() -> result::Result<Self> {
        Ok(Self {
            topic: non_empty_env(KAFKA_TOPIC_ENV),
            bootstrap_servers: non_empty_env(KAFKA_BOOTSTRAP_SERVERS_ENV),
            client_config: parse_env_json_map(KAFKA_CLIENT_CONFIG_ENV)?,
        })
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_env_json_map(key: &str) -> result::Result<Option<HashMap<String, String>>> {
    non_empty_env(key)
        .map(|raw| parse_json_map(key, &raw))
        .transpose()
}

fn parse_json_map(key: &str, raw: &str) -> result::Result<HashMap<String, String>> {
    serde_json::from_str(raw).map_err(|error| {
        result::AppError::Config(format!(
            "invalid JSON in `{}` environment variable: {}",
            key, error
        ))
    })
}

fn apply_kafka_overrides(mut config: Config, overrides: KafkaEnvOverrides) -> Config {
    let KafkaEnvOverrides {
        topic,
        bootstrap_servers,
        client_config,
    } = overrides;

    if let Some(client_config) = client_config {
        config.kafka_connector.client_config.extend(client_config);
    }

    if let Some(bootstrap_servers) = bootstrap_servers {
        config
            .kafka_connector
            .client_config
            .insert("bootstrap.servers".to_owned(), bootstrap_servers);
    }

    if let Some(topic) = topic {
        config.kafka_connector.topic = topic;
    }

    config
}

#[derive(Deserialize)]
pub struct Config {
    pub kafka_connector: KafkaConnectorConfig,
    #[serde(default)]
    allowed_events: Option<HashSet<String>>,
    pub enrichment: Option<EnrichmentConfig>,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub addr: String,
    pub accept_cors: bool,
}

const SERVER_ADDR_ENV: &str = "SERVER_ADDR";
const SERVER_ACCEPT_CORS_ENV: &str = "SERVER_ACCEPT_CORS";
const DEFAULT_SERVER_ADDR: &str = "0.0.0.0:3000";

impl ServerConfig {
    /// Server settings are read from environment variables instead of `cac.toml`:
    ///
    /// - `SERVER_ADDR`: socket address to listen on; defaults to `0.0.0.0:3000`
    /// - `SERVER_ACCEPT_CORS`: `true` or `false` (case-insensitive), enabling
    ///   permissive CORS; defaults to `false`
    ///
    /// Empty or whitespace-only values are treated as unset.
    pub fn from_env() -> result::Result<Self> {
        Ok(Self {
            addr: server_addr_from(non_empty_env(SERVER_ADDR_ENV).as_deref())?,
            accept_cors: accept_cors_from(non_empty_env(SERVER_ACCEPT_CORS_ENV).as_deref())?,
        })
    }
}

fn server_addr_from(raw: Option<&str>) -> result::Result<String> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_SERVER_ADDR.to_string());
    };

    raw.parse::<SocketAddr>()
        .map(|_| raw.to_string())
        .map_err(|_| {
            result::AppError::Config(format!(
                "`{}` must be a valid socket address (e.g. `0.0.0.0:3000`), got `{}`",
                SERVER_ADDR_ENV, raw
            ))
        })
}

fn accept_cors_from(raw: Option<&str>) -> result::Result<bool> {
    let Some(raw) = raw else {
        return Ok(false);
    };

    match raw.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(result::AppError::Config(format!(
            "`{}` must be `true` or `false`, got `{}`",
            SERVER_ACCEPT_CORS_ENV, raw
        ))),
    }
}

impl Config {
    pub fn new(kafka_connector: KafkaConnectorConfig) -> Self {
        Config {
            kafka_connector,
            allowed_events: None,
            enrichment: None,
        }
    }

    pub fn is_event_allowed(&self, event: &CollectorEvent) -> bool {
        match &self.allowed_events {
            None => true,
            Some(allowed) => allowed.contains(&event.name),
        }
    }
}

pub struct ConfigBuilder {
    kafka_connector: KafkaConnectorConfig,
    allowed_events: Option<HashSet<String>>,
    enrichment: Option<EnrichmentConfig>,
}

impl ConfigBuilder {
    pub fn allowed_events(mut self, events: HashSet<String>) -> Self {
        self.allowed_events = Some(events);
        self
    }

    pub fn build(self) -> Config {
        Config {
            kafka_connector: self.kafka_connector,
            allowed_events: self.allowed_events,
            enrichment: self.enrichment,
        }
    }
}

pub async fn get_config_from_local_file(
    org_id: String,
    project_id: String,
) -> result::Result<Config> {
    let mut evaluation_context = EvaluationContext::default();
    evaluation_context.add_custom_field("org_id", org_id.clone());
    evaluation_context.add_custom_field("project_id", project_id.clone());

    let mut config = get_config_from_local_file_for_context(evaluation_context).await?;
    config.kafka_connector.topic =
        render_topic(&config.kafka_connector.topic, &org_id, &project_id)?;
    Ok(config)
}

pub async fn get_default_config_from_local_file() -> result::Result<Config> {
    get_config_from_local_file_for_context(EvaluationContext::default()).await
}

async fn get_config_from_local_file_for_context(
    evaluation_context: EvaluationContext,
) -> result::Result<Config> {
    let file_path = PathBuf::from("cac.toml");
    let data_source = FileDataSource::new(file_path).map_err(result::AppError::Config)?;

    let provider =
        LocalResolutionProvider::new(Box::new(data_source), None, RefreshStrategy::Manual);

    provider.init(evaluation_context.clone()).await?;
    let resolved_config = provider.resolve_all_features(evaluation_context).await?;

    let config = serde_json::from_value(serde_json::Value::Object(resolved_config))?;
    let overrides = KafkaEnvOverrides::from_env()?;

    Ok(apply_kafka_overrides(config, overrides))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test binaries run with the package root as their working directory,
    /// but `cac.toml` is resolved from the current directory, as at runtime
    /// (repo root locally, `/app` in the container). Point the working
    /// directory at the workspace root so the bundled `cac.toml` is found.
    fn chdir_to_workspace_root() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::env::set_current_dir(&workspace_root).expect("failed to chdir to workspace root");
    }

    #[tokio::test]
    async fn loads_config_from_cac_toml() {
        chdir_to_workspace_root();

        let config =
            get_config_from_local_file("acme-k3x9qa".to_string(), "blue-ocean-7".to_string())
                .await
                .unwrap();

        assert_eq!(config.kafka_connector.topic, "events.generic");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"breeze-c2-kafka-brokers.kafka-cluster-v2:9092".to_string())
        );
        assert!(config.allowed_events.is_none());
    }

    #[tokio::test]
    async fn loads_default_config_from_cac_toml() {
        chdir_to_workspace_root();

        let config = get_default_config_from_local_file().await.unwrap();

        assert_eq!(config.kafka_connector.topic, "events.generic");
        assert!(config.allowed_events.is_none());
        assert!(
            config
                .enrichment
                .as_ref()
                .and_then(|enrichment| enrichment.ip_address.as_ref())
                .is_some()
        );
        assert!(
            config
                .enrichment
                .as_ref()
                .is_some_and(|enrichment| enrichment.arrived_at.enabled)
        );
    }

    #[test]
    fn server_addr_defaults_when_env_missing() {
        assert_eq!(server_addr_from(None).unwrap(), "0.0.0.0:3000");
    }

    #[test]
    fn server_addr_accepts_valid_socket_addr() {
        assert_eq!(
            server_addr_from(Some("127.0.0.1:8080")).unwrap(),
            "127.0.0.1:8080"
        );
        assert_eq!(server_addr_from(Some("[::]:3000")).unwrap(), "[::]:3000");
    }

    #[test]
    fn invalid_server_addr_is_error() {
        assert!(matches!(
            server_addr_from(Some("not-an-addr")),
            Err(result::AppError::Config(_))
        ));
    }

    #[test]
    fn accept_cors_defaults_false_when_env_missing() {
        assert!(!accept_cors_from(None).unwrap());
    }

    #[test]
    fn accept_cors_parses_true_and_false_case_insensitively() {
        assert!(accept_cors_from(Some("true")).unwrap());
        assert!(accept_cors_from(Some("TRUE")).unwrap());
        assert!(!accept_cors_from(Some("false")).unwrap());
        assert!(!accept_cors_from(Some("False")).unwrap());
    }

    #[test]
    fn invalid_accept_cors_is_error() {
        assert!(matches!(
            accept_cors_from(Some("yes")),
            Err(result::AppError::Config(_))
        ));
    }

    fn cac_config() -> Config {
        let mut client_config = HashMap::new();
        client_config.insert("bootstrap.servers".to_owned(), "cac-broker:9092".to_owned());
        client_config.insert("message.timeout.ms".to_owned(), "5000".to_owned());

        Config::new(KafkaConnectorConfig {
            topic: "cac-topic".to_owned(),
            client_config,
        })
    }

    fn json_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn kafka_env_overrides_take_priority_over_cac_config() {
        let config = apply_kafka_overrides(
            cac_config(),
            KafkaEnvOverrides {
                topic: Some("env-topic".to_owned()),
                bootstrap_servers: Some("env-bootstrap:9092".to_owned()),
                client_config: Some(json_map(&[
                    ("bootstrap.servers", "env-json-broker:9092"),
                    ("message.timeout.ms", "9000"),
                    ("socket.timeout.ms", "4000"),
                ])),
            },
        );

        assert_eq!(config.kafka_connector.topic, "env-topic");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"env-bootstrap:9092".to_owned())
        );
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("message.timeout.ms"),
            Some(&"9000".to_owned())
        );
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("socket.timeout.ms"),
            Some(&"4000".to_owned())
        );
    }

    #[test]
    fn kafka_env_overrides_absent_keeps_cac_config() {
        let config = apply_kafka_overrides(cac_config(), KafkaEnvOverrides::default());

        assert_eq!(config.kafka_connector.topic, "cac-topic");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"cac-broker:9092".to_owned())
        );
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("message.timeout.ms"),
            Some(&"5000".to_owned())
        );
    }

    #[test]
    fn kafka_topic_env_overrides_topic_only() {
        let config = apply_kafka_overrides(
            cac_config(),
            KafkaEnvOverrides {
                topic: Some("env-topic".to_owned()),
                ..KafkaEnvOverrides::default()
            },
        );

        assert_eq!(config.kafka_connector.topic, "env-topic");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"cac-broker:9092".to_owned())
        );
    }

    #[test]
    fn kafka_client_config_env_rejects_invalid_json() {
        let error = parse_json_map(KAFKA_CLIENT_CONFIG_ENV, "not-json").unwrap_err();

        assert!(matches!(error, result::AppError::Config(_)));
        assert!(error.to_string().contains("KAFKA_CLIENT_CONFIG"));
    }
}
