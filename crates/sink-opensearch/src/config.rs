//! Sink configuration: CAC keys plus environment overrides.
//!
//! Settings live in a CAC file (see [`crate::cac`]), one key per setting,
//! named `section.name`, e.g. `batch.max_docs` or `opensearch.index`. The file
//! path comes from `SINK_CONFIG` (default `cac.toml`).
//!
//! Endpoints and credentials can be overridden from the environment, using the
//! same Kafka variable names as the event collector:
//!
//! - `KAFKA_BOOTSTRAP_SERVERS`: overrides `kafka.client_config["bootstrap.servers"]`
//! - `KAFKA_CLIENT_CONFIG`: JSON object merged into `kafka.client_config`;
//!   `KAFKA_BOOTSTRAP_SERVERS` still wins over it
//! - `OPENSEARCH_URL`: overrides `opensearch.url`
//! - `OPENSEARCH_USERNAME` / `OPENSEARCH_PASSWORD`: basic auth credentials,
//!   accepted only from the environment
//!
//! Empty or whitespace-only values are treated as unset.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::cac::{Cac, CacError};

const CONFIG_PATH_ENV: &str = "SINK_CONFIG";
const DEFAULT_CONFIG_PATH: &str = "cac.toml";
const KAFKA_BOOTSTRAP_SERVERS_ENV: &str = "KAFKA_BOOTSTRAP_SERVERS";
const KAFKA_CLIENT_CONFIG_ENV: &str = "KAFKA_CLIENT_CONFIG";
const OPENSEARCH_URL_ENV: &str = "OPENSEARCH_URL";
const OPENSEARCH_USERNAME_ENV: &str = "OPENSEARCH_USERNAME";
const OPENSEARCH_PASSWORD_ENV: &str = "OPENSEARCH_PASSWORD";

const BOOTSTRAP_SERVERS: &str = "bootstrap.servers";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Cac(#[from] CacError),

    #[error("invalid config: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("invalid JSON in `{key}` environment variable: {source}")]
    EnvJson {
        key: &'static str,
        source: serde_json::Error,
    },

    #[error("invalid config: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub kafka: KafkaConfig,
    pub opensearch: OpenSearchConfig,
    #[serde(default)]
    pub batch: BatchConfig,
    #[serde(default)]
    pub retry: RetryConfig,
    pub dlq: DlqConfig,
    #[serde(default)]
    pub commit: CommitConfig,
    #[serde(default)]
    pub shutdown: ShutdownConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KafkaConfig {
    pub topics: Vec<String>,
    pub group_id: String,
    /// Raw librdkafka properties (brokers, security, timeouts). The sink always
    /// overrides `group.id`, `enable.auto.commit` and `enable.auto.offset.store`.
    #[serde(default)]
    pub client_config: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenSearchConfig {
    pub url: String,
    /// Where documents are written. `{tenant_id}` is replaced with each
    /// event's tenant, giving every tenant its own index or data stream.
    /// This is the default; CAC overrides can change it per tenant.
    pub index: IndexTemplate,
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(skip)]
    pub username: Option<String>,
    #[serde(skip)]
    pub password: Option<Secret>,
}

impl OpenSearchConfig {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }
}

/// A value that is never printed by `Debug`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

const TENANT_PLACEHOLDER: &str = "{tenant_id}";

/// An index, alias or data stream name, optionally containing `{tenant_id}`,
/// e.g. `events-{tenant_id}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct IndexTemplate(String);

impl IndexTemplate {
    /// Checks the fixed parts of the template against OpenSearch's naming
    /// rules. Each tenant's name is checked again when its events arrive.
    pub fn parse(template: &str) -> Result<Self, String> {
        if template
            .replace(TENANT_PLACEHOLDER, "")
            .contains(['{', '}'])
        {
            return Err(format!(
                "`opensearch.index` `{template}`: the only placeholder is `{TENANT_PLACEHOLDER}`"
            ));
        }
        let sample = template.replace(TENANT_PLACEHOLDER, "tenant");
        validate_index_name(&sample)
            .map_err(|reason| format!("`opensearch.index` `{template}` {reason}"))?;
        Ok(Self(template.to_owned()))
    }

    /// The index for one tenant's events. Fails if the tenant can't be part of
    /// an index name, e.g. it has uppercase letters or a comma. Tenant IDs are
    /// never rewritten, because two tenants could then share an index.
    pub fn render(&self, tenant_id: &str) -> Result<Cow<'_, str>, String> {
        if !self.0.contains(TENANT_PLACEHOLDER) {
            return Ok(Cow::Borrowed(&self.0));
        }
        if tenant_id.is_empty() {
            return Err("tenant_id is empty".to_owned());
        }
        let index = self.0.replace(TENANT_PLACEHOLDER, tenant_id);
        validate_index_name(&index).map_err(|reason| {
            format!("tenant_id `{tenant_id}` gives index `{index}`, which {reason}")
        })?;
        Ok(Cow::Owned(index))
    }
}

impl TryFrom<String> for IndexTemplate {
    type Error = String;

    fn try_from(template: String) -> Result<Self, String> {
        Self::parse(&template)
    }
}

impl fmt::Display for IndexTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BatchConfig {
    /// Flush a partition's buffer once it holds this many records.
    pub max_docs: usize,
    /// Flush once buffered documents reach this many bytes. A single bulk
    /// request is never larger, and a larger document goes to the DLQ.
    pub max_bytes: usize,
    /// Maximum time a record waits in a buffer before it is flushed.
    pub linger_ms: u64,
    /// Concurrent bulk requests across all partitions.
    pub max_in_flight: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_docs: 1000,
            max_bytes: 5 * 1024 * 1024,
            linger_ms: 1000,
            max_in_flight: 8,
        }
    }
}

impl BatchConfig {
    pub fn linger(&self) -> Duration {
        Duration::from_millis(self.linger_ms)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RetryConfig {
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_backoff_ms: 100,
            max_backoff_ms: 30_000,
        }
    }
}

impl RetryConfig {
    pub fn initial_backoff(&self) -> Duration {
        Duration::from_millis(self.initial_backoff_ms)
    }

    pub fn max_backoff(&self) -> Duration {
        Duration::from_millis(self.max_backoff_ms)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DlqConfig {
    pub topic: String,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CommitConfig {
    pub interval_ms: u64,
}

impl Default for CommitConfig {
    fn default() -> Self {
        Self { interval_ms: 5000 }
    }
}

impl CommitConfig {
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShutdownConfig {
    /// Maximum time to finish writing buffered records after SIGTERM.
    pub grace_ms: u64,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self { grace_ms: 25_000 }
    }
}

impl ShutdownConfig {
    pub fn grace(&self) -> Duration {
        Duration::from_millis(self.grace_ms)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Address for `/health` and `/metrics`.
    pub addr: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from(([0, 0, 0, 0], 9464)),
        }
    }
}

fn default_request_timeout_ms() -> u64 {
    30_000
}

/// Values read from the environment that take priority over the config file.
#[derive(Debug, Default)]
pub struct EnvOverrides {
    pub kafka_bootstrap_servers: Option<String>,
    pub kafka_client_config: Option<HashMap<String, String>>,
    pub opensearch_url: Option<String>,
    pub opensearch_username: Option<String>,
    pub opensearch_password: Option<String>,
}

impl EnvOverrides {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            kafka_bootstrap_servers: non_empty_env(KAFKA_BOOTSTRAP_SERVERS_ENV),
            kafka_client_config: non_empty_env(KAFKA_CLIENT_CONFIG_ENV)
                .map(|raw| parse_json_map(KAFKA_CLIENT_CONFIG_ENV, &raw))
                .transpose()?,
            opensearch_url: non_empty_env(OPENSEARCH_URL_ENV),
            opensearch_username: non_empty_env(OPENSEARCH_USERNAME_ENV),
            opensearch_password: non_empty_env(OPENSEARCH_PASSWORD_ENV),
        })
    }
}

/// The CAC file named by `SINK_CONFIG`.
pub fn cac_path() -> PathBuf {
    PathBuf::from(non_empty_env(CONFIG_PATH_ENV).unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned()))
}

impl Config {
    /// Resolves the process-wide settings from CAC, applies environment
    /// overrides and validates the result.
    pub async fn load(cac: &Cac) -> Result<Self, ConfigError> {
        Self::from_cac(cac.resolve_defaults().await?, EnvOverrides::from_env()?)
    }

    /// Builds the config from resolved CAC keys such as `batch.max_docs`.
    pub fn from_cac(
        values: Map<String, Value>,
        overrides: EnvOverrides,
    ) -> Result<Self, ConfigError> {
        let mut config: Self = serde_json::from_value(nest(values)?)?;
        config.apply(overrides);
        config.validate()?;
        Ok(config)
    }

    fn apply(&mut self, overrides: EnvOverrides) {
        let EnvOverrides {
            kafka_bootstrap_servers,
            kafka_client_config,
            opensearch_url,
            opensearch_username,
            opensearch_password,
        } = overrides;

        if let Some(client_config) = kafka_client_config {
            self.kafka.client_config.extend(client_config);
        }
        if let Some(bootstrap_servers) = kafka_bootstrap_servers {
            self.kafka
                .client_config
                .insert(BOOTSTRAP_SERVERS.to_owned(), bootstrap_servers);
        }
        if let Some(url) = opensearch_url {
            self.opensearch.url = url;
        }
        self.opensearch.username = opensearch_username;
        self.opensearch.password = opensearch_password.map(Secret);
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |message: String| Err(ConfigError::Invalid(message));

        if self.kafka.topics.is_empty() || self.kafka.topics.iter().any(|t| t.trim().is_empty()) {
            return invalid("`kafka.topics` must list at least one non-empty topic".into());
        }
        if self.kafka.group_id.trim().is_empty() {
            return invalid("`kafka.group_id` must not be empty".into());
        }
        if self
            .kafka
            .client_config
            .get(BOOTSTRAP_SERVERS)
            .is_none_or(|servers| servers.trim().is_empty())
        {
            return invalid(format!(
                "`kafka.client_config.\"{BOOTSTRAP_SERVERS}\"` or `{KAFKA_BOOTSTRAP_SERVERS_ENV}` must be set"
            ));
        }
        if self.dlq.topic.trim().is_empty() {
            return invalid("`dlq.topic` must not be empty".into());
        }
        if self.kafka.topics.contains(&self.dlq.topic) {
            return invalid(format!(
                "`dlq.topic` `{}` is also in `kafka.topics`; the sink would consume its own dead letters",
                self.dlq.topic
            ));
        }

        validate_url(&self.opensearch.url)?;
        if self.opensearch.username.is_some() != self.opensearch.password.is_some() {
            return invalid(format!(
                "set both `{OPENSEARCH_USERNAME_ENV}` and `{OPENSEARCH_PASSWORD_ENV}`, or neither"
            ));
        }
        if self.opensearch.request_timeout_ms == 0 {
            return invalid("`opensearch.request_timeout_ms` must be greater than 0".into());
        }

        let batch = &self.batch;
        if batch.max_docs == 0 || batch.max_bytes == 0 || batch.max_in_flight == 0 {
            return invalid(
                "`batch.max_docs`, `batch.max_bytes` and `batch.max_in_flight` must be greater than 0"
                    .into(),
            );
        }
        if batch.linger_ms == 0 {
            return invalid("`batch.linger_ms` must be at least 1".into());
        }

        let retry = &self.retry;
        if retry.initial_backoff_ms == 0 || retry.max_backoff_ms < retry.initial_backoff_ms {
            return invalid(
                "`retry.initial_backoff_ms` must be greater than 0 and at most `retry.max_backoff_ms`"
                    .into(),
            );
        }
        if self.commit.interval_ms == 0 {
            return invalid("`commit.interval_ms` must be greater than 0".into());
        }

        Ok(())
    }
}

/// Turns `{"batch.max_docs": 10}` into `{"batch": {"max_docs": 10}}`.
fn nest(values: Map<String, Value>) -> Result<Value, ConfigError> {
    let mut sections = Map::new();
    for (key, value) in values {
        let Some((section, name)) = key.split_once('.') else {
            return Err(ConfigError::Invalid(format!(
                "`{key}` is not a sink setting; keys look like `section.name`, e.g. `batch.max_docs`"
            )));
        };
        if let Some(section) = sections
            .entry(section)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        {
            section.insert(name.to_owned(), value);
        }
    }
    Ok(Value::Object(sections))
}

fn validate_url(url: &str) -> Result<(), ConfigError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| ConfigError::Invalid(format!("`opensearch.url` `{url}`: {error}")))?;

    if !matches!(parsed.scheme(), "http" | "https") || parsed.cannot_be_a_base() {
        return Err(ConfigError::Invalid(format!(
            "`opensearch.url` must be an http or https URL, got `{url}`"
        )));
    }
    Ok(())
}

/// Applies OpenSearch's index naming rules, so a bad name fails before a
/// bulk request does.
fn validate_index_name(index: &str) -> Result<(), String> {
    const FORBIDDEN: &[char] = &['\\', '/', '*', '?', '"', '<', '>', '|', ' ', ',', '#', ':'];
    const MAX_BYTES: usize = 255;

    let valid = !index.is_empty()
        && index.len() <= MAX_BYTES
        && index != "."
        && index != ".."
        && !index.starts_with(['_', '-', '+'])
        && !index.contains(FORBIDDEN)
        && index.to_lowercase() == index;

    if valid {
        Ok(())
    } else {
        Err(format!(
            "is not a valid index name: use lowercase, at most {MAX_BYTES} bytes, don't start with `_`, `-` or `+`, and avoid spaces and \\ / * ? \" < > | , # :"
        ))
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_json_map(key: &'static str, raw: &str) -> Result<HashMap<String, String>, ConfigError> {
    serde_json::from_str(raw).map_err(|source| ConfigError::EnvJson { key, source })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn minimal() -> Map<String, Value> {
        let value = json!({
            "kafka.topics": ["events.generic"],
            "kafka.group_id": "sink-opensearch",
            "kafka.client_config": { "bootstrap.servers": "cac-broker:9092" },
            "opensearch.url": "http://localhost:9200",
            "opensearch.index": "events",
            "dlq.topic": "events.generic.dlq",
        });
        value.as_object().cloned().unwrap()
    }

    fn config_with(changes: &[(&str, Value)]) -> Result<Config, ConfigError> {
        let mut values = minimal();
        for (key, value) in changes {
            values.insert((*key).to_owned(), value.clone());
        }
        Config::from_cac(values, EnvOverrides::default())
    }

    fn config_without(key: &str) -> Result<Config, ConfigError> {
        let mut values = minimal();
        values.remove(key);
        Config::from_cac(values, EnvOverrides::default())
    }

    fn assert_invalid(result: Result<Config, ConfigError>, needle: &str) {
        match result {
            Err(ConfigError::Invalid(message)) => assert!(
                message.contains(needle),
                "expected `{needle}` in `{message}`"
            ),
            other => panic!("expected an invalid config error, got {other:?}"),
        }
    }

    fn assert_rejected(result: Result<Config, ConfigError>, needle: &str) {
        match result {
            Err(ConfigError::Parse(error)) => assert!(
                error.to_string().contains(needle),
                "expected `{needle}` in `{error}`"
            ),
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn minimal_config_gets_defaults() {
        let config = config_with(&[]).unwrap();

        assert_eq!(config.batch.max_docs, 1000);
        assert_eq!(config.batch.max_bytes, 5 * 1024 * 1024);
        assert_eq!(config.batch.linger(), Duration::from_secs(1));
        assert_eq!(config.batch.max_in_flight, 8);
        assert_eq!(config.retry.initial_backoff(), Duration::from_millis(100));
        assert_eq!(config.retry.max_backoff(), Duration::from_secs(30));
        assert_eq!(config.commit.interval(), Duration::from_secs(5));
        assert_eq!(config.shutdown.grace(), Duration::from_secs(25));
        assert_eq!(config.opensearch.request_timeout(), Duration::from_secs(30));
        assert_eq!(config.server.addr, "0.0.0.0:9464".parse().unwrap());
        assert!(config.opensearch.username.is_none());
    }

    #[test]
    fn dotted_keys_fill_their_sections() {
        let config = config_with(&[
            ("batch.max_docs", json!(10)),
            ("opensearch.request_timeout_ms", json!(2000)),
        ])
        .unwrap();

        assert_eq!(config.batch.max_docs, 10);
        assert_eq!(config.batch.linger_ms, 1000);
        assert_eq!(config.opensearch.request_timeout_ms, 2000);
    }

    #[test]
    fn keys_need_a_section() {
        assert_invalid(
            config_with(&[("kafka_connector", json!({}))]),
            "keys look like `section.name`",
        );
    }

    #[test]
    fn env_overrides_take_priority() {
        let config = Config::from_cac(
            minimal(),
            EnvOverrides {
                kafka_bootstrap_servers: Some("env-broker:9092".into()),
                kafka_client_config: Some(HashMap::from([
                    ("bootstrap.servers".into(), "json-broker:9092".into()),
                    ("security.protocol".into(), "SSL".into()),
                ])),
                opensearch_url: Some("https://search.internal:443".into()),
                opensearch_username: Some("sink".into()),
                opensearch_password: Some("hunter2".into()),
            },
        )
        .unwrap();

        let client_config = &config.kafka.client_config;
        assert_eq!(client_config["bootstrap.servers"], "env-broker:9092");
        assert_eq!(client_config["security.protocol"], "SSL");
        assert_eq!(config.opensearch.url, "https://search.internal:443");
        assert_eq!(config.opensearch.username.as_deref(), Some("sink"));
        assert_eq!(
            config.opensearch.password.as_ref().map(Secret::expose),
            Some("hunter2")
        );
    }

    #[test]
    fn debug_output_hides_the_password() {
        let config = Config::from_cac(
            minimal(),
            EnvOverrides {
                opensearch_username: Some("sink".into()),
                opensearch_password: Some("hunter2".into()),
                ..EnvOverrides::default()
            },
        )
        .unwrap();

        assert!(!format!("{config:?}").contains("hunter2"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert_rejected(config_with(&[("batch.max_doc", json!(10))]), "max_doc");
    }

    #[test]
    fn credentials_are_not_accepted_from_cac() {
        assert_rejected(
            config_with(&[("opensearch.password", json!("hunter2"))]),
            "password",
        );
    }

    #[test]
    fn dlq_topic_must_not_be_consumed() {
        assert_invalid(
            config_with(&[("dlq.topic", json!("events.generic"))]),
            "dead letters",
        );
    }

    #[test]
    fn bootstrap_servers_are_required() {
        assert_invalid(config_without("kafka.client_config"), "bootstrap.servers");
    }

    #[test]
    fn index_names_follow_opensearch_rules() {
        for index in ["Events", "_events", "events,logs", "my events", ""] {
            assert_rejected(
                config_with(&[("opensearch.index", json!(index))]),
                "not a valid index name",
            );
        }
        for index in ["events", "events-v1", ".events", "logs.2026"] {
            assert!(
                config_with(&[("opensearch.index", json!(index))]).is_ok(),
                "`{index}` should be valid"
            );
        }
    }

    #[test]
    fn index_can_be_per_tenant() {
        let config = config_with(&[("opensearch.index", json!("events-{tenant_id}"))]).unwrap();

        assert_eq!(
            config.opensearch.index.render("merchant-1").unwrap(),
            "events-merchant-1"
        );
    }

    #[test]
    fn index_template_checks_its_fixed_parts() {
        for (index, needle) in [
            ("Events-{tenant_id}", "not a valid index name"),
            ("_{tenant_id}", "not a valid index name"),
            ("events-{tenant}", "the only placeholder is `{tenant_id}`"),
            ("events-{date}-{tenant_id}", "the only placeholder"),
        ] {
            assert_rejected(config_with(&[("opensearch.index", json!(index))]), needle);
        }
    }

    #[test]
    fn fixed_index_ignores_the_tenant() {
        let template = IndexTemplate::parse("events").unwrap();

        assert_eq!(template.render("Anything, really").unwrap(), "events");
    }

    #[test]
    fn tenants_that_cannot_form_an_index_name_are_refused() {
        let template = IndexTemplate::parse("events-{tenant_id}").unwrap();

        for tenant in [
            "",
            "Merchant-1",
            "a,b",
            "a b",
            "a*",
            "a/b",
            &"x".repeat(250),
        ] {
            assert!(
                template.render(tenant).is_err(),
                "`{tenant}` should be refused"
            );
        }
        assert_eq!(template.render("m_1.eu").unwrap(), "events-m_1.eu");
    }

    #[test]
    fn url_must_be_http() {
        assert_invalid(
            config_with(&[("opensearch.url", json!("ftp://localhost"))]),
            "http or https",
        );
    }

    #[test]
    fn username_requires_password() {
        let result = Config::from_cac(
            minimal(),
            EnvOverrides {
                opensearch_username: Some("sink".into()),
                ..EnvOverrides::default()
            },
        );

        assert_invalid(result, "OPENSEARCH_PASSWORD");
    }

    #[test]
    fn backoff_bounds_are_checked() {
        assert_invalid(
            config_with(&[
                ("retry.initial_backoff_ms", json!(500)),
                ("retry.max_backoff_ms", json!(100)),
            ]),
            "retry.initial_backoff_ms",
        );
    }

    #[test]
    fn zero_linger_is_rejected() {
        assert_invalid(config_with(&[("batch.linger_ms", json!(0))]), "linger_ms");
    }

    #[test]
    fn invalid_client_config_json_names_the_variable() {
        let error = parse_json_map(KAFKA_CLIENT_CONFIG_ENV, "not-json").unwrap_err();

        assert!(error.to_string().contains(KAFKA_CLIENT_CONFIG_ENV));
    }

    #[tokio::test]
    async fn the_shipped_cac_file_is_valid() {
        let cac = Cac::load(concat!(env!("CARGO_MANIFEST_DIR"), "/cac.toml"))
            .await
            .unwrap();

        let config = Config::from_cac(
            cac.resolve_defaults().await.unwrap(),
            EnvOverrides::default(),
        )
        .unwrap();

        assert_eq!(config.kafka.topics, vec!["events.generic"]);
        assert_eq!(
            config.opensearch.index,
            IndexTemplate::parse("events-{tenant_id}").unwrap()
        );
        cac.close().await;
    }
}
