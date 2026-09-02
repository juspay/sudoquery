use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Deserialize;
use superposition_provider::data_source::file::FileDataSource;
use superposition_provider::{
    AllFeatureProvider, EvaluationContext, LocalResolutionProvider, RefreshStrategy,
};

use crate::collector_event::CollectorEvent;
use crate::enrichment::EnrichmentConfig;
use crate::kafka_connector::KafkaConnectorConfig;
use crate::result;

/// Kafka settings can be overridden via environment variables, taking priority
/// over the values resolved from `cac.toml`:
///
/// - `KAFKA_TOPIC`: overrides `kafka_connector.topic`
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
    #[serde(default)]
    pub server_config: ServerConfig,
    pub kafka_connector: KafkaConnectorConfig,
    #[serde(default)]
    allowed_events: Option<HashSet<String>>,
    pub enrichment: Option<EnrichmentConfig>,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub addr: String,
    pub accept_cors: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:3000".to_string(),
            accept_cors: false,
        }
    }
}

impl Config {
    pub fn new(kafka_connector: KafkaConnectorConfig) -> Self {
        Config {
            server_config: ServerConfig::default(),
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
    server_config: Option<ServerConfig>,
    kafka_connector: KafkaConnectorConfig,
    allowed_events: Option<HashSet<String>>,
    enrichment: Option<EnrichmentConfig>,
}

impl ConfigBuilder {
    pub fn server_config(mut self, server_config: ServerConfig) -> Self {
        self.server_config = Some(server_config);
        self
    }

    pub fn allowed_events(mut self, events: HashSet<String>) -> Self {
        self.allowed_events = Some(events);
        self
    }

    pub fn build(self) -> Config {
        Config {
            server_config: self.server_config.unwrap_or_default(),
            kafka_connector: self.kafka_connector,
            allowed_events: self.allowed_events,
            enrichment: self.enrichment,
        }
    }
}

pub async fn get_config_from_local_file(
    tenant_id: String,
    workspace_id: Option<String>,
) -> result::Result<Config> {
    let mut evaluation_context = EvaluationContext::default();
    evaluation_context.add_custom_field("tenant_id", tenant_id);
    if let Some(workspace_id) = workspace_id {
        evaluation_context.add_custom_field("workspace_id", workspace_id);
    }

    get_config_from_local_file_for_context(evaluation_context).await
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

    #[tokio::test]
    async fn loads_config_from_cac_toml() {
        let config = get_config_from_local_file("tenant-1".to_string(), None)
            .await
            .unwrap();

        assert_eq!(config.kafka_connector.topic, "events");
        assert_eq!(
            config
                .kafka_connector
                .client_config
                .get("bootstrap.servers"),
            Some(&"host.docker.internal:19092".to_string())
        );
        assert!(config.allowed_events.is_none());
    }

    #[tokio::test]
    async fn loads_default_config_from_cac_toml() {
        let config = get_default_config_from_local_file().await.unwrap();

        assert_eq!(config.kafka_connector.topic, "events");
        assert_eq!(config.server_config.addr, "0.0.0.0:3000");
        assert!(config.server_config.accept_cors);
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
            config.kafka_connector.client_config.get("bootstrap.servers"),
            Some(&"env-bootstrap:9092".to_owned())
        );
        assert_eq!(
            config.kafka_connector.client_config.get("message.timeout.ms"),
            Some(&"9000".to_owned())
        );
        assert_eq!(
            config.kafka_connector.client_config.get("socket.timeout.ms"),
            Some(&"4000".to_owned())
        );
    }

    #[test]
    fn kafka_env_overrides_absent_keeps_cac_config() {
        let config = apply_kafka_overrides(cac_config(), KafkaEnvOverrides::default());

        assert_eq!(config.kafka_connector.topic, "cac-topic");
        assert_eq!(
            config.kafka_connector.client_config.get("bootstrap.servers"),
            Some(&"cac-broker:9092".to_owned())
        );
        assert_eq!(
            config.kafka_connector.client_config.get("message.timeout.ms"),
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
            config.kafka_connector.client_config.get("bootstrap.servers"),
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
