//! Sink configuration: CAC keys plus environment overrides.
//!
//! Settings live in a CAC file (see [`crate::cac`]), one key per setting,
//! named `section.name`, e.g. `batch.max_docs` or `opensearch.index`. The file
//! path comes from `SINK_CONFIG` (default `cac.toml`).
//!
//! Every setting can also come from the environment, and the environment wins
//! over the file. The variable is the key in upper case with `.` replaced by
//! `_`, e.g. `BATCH_MAX_DOCS` for `batch.max_docs` (see `ENV_SETTINGS`):
//!
//! - numbers are written as they are: `BATCH_MAX_DOCS=500`
//! - `KAFKA_TOPICS` is comma-separated: `events.a,events.b`
//! - `KAFKA_CLIENT_CONFIG` is a JSON object merged into `kafka.client_config`
//! - `OPENSEARCH_INDEX` applies to every org, so CAC overrides of
//!   `opensearch.index` are ignored while it is set
//!
//! Besides those, using the same names as the event collector and dashboard:
//!
//! - `KAFKA_BOOTSTRAP_SERVERS`: overrides `kafka.client_config["bootstrap.servers"]`,
//!   winning over `KAFKA_CLIENT_CONFIG` too
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
const OPENSEARCH_USERNAME_ENV: &str = "OPENSEARCH_USERNAME";
const OPENSEARCH_PASSWORD_ENV: &str = "OPENSEARCH_PASSWORD";

const BOOTSTRAP_SERVERS: &str = "bootstrap.servers";

/// The CAC key resolved per org.
pub(crate) const INDEX_KEY: &str = "opensearch.index";

/// How a variable's text becomes a setting's value.
#[derive(Clone, Copy, Debug)]
enum EnvKind {
    /// Taken as it is.
    Text,
    /// A whole number, e.g. `1000`.
    Number,
    /// Comma-separated, e.g. `events.a,events.b`.
    List,
    /// A JSON object of strings, merged into the file's object key by key.
    Object,
}

/// Every setting, with the variable that overrides it.
const ENV_SETTINGS: &[(&str, &str, EnvKind)] = &[
    ("kafka.topics", "KAFKA_TOPICS", EnvKind::List),
    ("kafka.group_id", "KAFKA_GROUP_ID", EnvKind::Text),
    (
        "kafka.client_config",
        "KAFKA_CLIENT_CONFIG",
        EnvKind::Object,
    ),
    ("opensearch.url", "OPENSEARCH_URL", EnvKind::Text),
    (INDEX_KEY, "OPENSEARCH_INDEX", EnvKind::Text),
    (
        "opensearch.request_timeout_ms",
        "OPENSEARCH_REQUEST_TIMEOUT_MS",
        EnvKind::Number,
    ),
    ("batch.max_docs", "BATCH_MAX_DOCS", EnvKind::Number),
    ("batch.max_bytes", "BATCH_MAX_BYTES", EnvKind::Number),
    ("batch.linger_ms", "BATCH_LINGER_MS", EnvKind::Number),
    (
        "batch.max_in_flight",
        "BATCH_MAX_IN_FLIGHT",
        EnvKind::Number,
    ),
    (
        "retry.initial_backoff_ms",
        "RETRY_INITIAL_BACKOFF_MS",
        EnvKind::Number,
    ),
    (
        "retry.max_backoff_ms",
        "RETRY_MAX_BACKOFF_MS",
        EnvKind::Number,
    ),
    ("dlq.topic", "DLQ_TOPIC", EnvKind::Text),
    ("commit.interval_ms", "COMMIT_INTERVAL_MS", EnvKind::Number),
    ("shutdown.grace_ms", "SHUTDOWN_GRACE_MS", EnvKind::Number),
    ("server.addr", "SERVER_ADDR", EnvKind::Text),
];

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

    #[error("invalid `{key}` environment variable: {message}")]
    EnvValue { key: &'static str, message: String },

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
    /// Where documents are written. `{org_id}` is replaced with each
    /// event's org, giving every org its own index or data stream.
    /// This is the default; CAC overrides can change it per org.
    pub index: IndexTemplate,
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
    /// Set when `index` came from `OPENSEARCH_INDEX`: it then applies to
    /// every org, and CAC overrides of `opensearch.index` are ignored.
    #[serde(skip)]
    pub index_from_env: bool,
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

const ORG_PLACEHOLDER: &str = "{org_id}";

/// An index, alias or data stream name, optionally containing `{org_id}`,
/// e.g. `events-{org_id}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct IndexTemplate(String);

impl IndexTemplate {
    /// Checks the fixed parts of the template against OpenSearch's naming
    /// rules. Each org's name is checked again when its events arrive.
    pub fn parse(template: &str) -> Result<Self, String> {
        if template.replace(ORG_PLACEHOLDER, "").contains(['{', '}']) {
            return Err(format!(
                "`opensearch.index` `{template}`: the only placeholder is `{ORG_PLACEHOLDER}`"
            ));
        }
        let sample = template.replace(ORG_PLACEHOLDER, "org");
        validate_index_name(&sample)
            .map_err(|reason| format!("`opensearch.index` `{template}` {reason}"))?;
        Ok(Self(template.to_owned()))
    }

    /// The index for one org's events. Fails if the org can't be part of
    /// an index name, e.g. it has uppercase letters or a comma. Org IDs are
    /// never rewritten, because two orgs could then share an index.
    pub fn render(&self, org_id: &str) -> Result<Cow<'_, str>, String> {
        if !self.0.contains(ORG_PLACEHOLDER) {
            return Ok(Cow::Borrowed(&self.0));
        }
        if org_id.is_empty() {
            return Err("org_id is empty".to_owned());
        }
        let index = self.0.replace(ORG_PLACEHOLDER, org_id);
        validate_index_name(&index)
            .map_err(|reason| format!("org_id `{org_id}` gives index `{index}`, which {reason}"))?;
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
    /// Settings by CAC key, e.g. `batch.max_docs` from `BATCH_MAX_DOCS`.
    pub settings: Map<String, Value>,
    pub kafka_bootstrap_servers: Option<String>,
    pub opensearch_username: Option<String>,
    pub opensearch_password: Option<String>,
}

impl EnvOverrides {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the overrides through `lookup`, which returns a variable's value.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };

        let mut settings = Map::new();
        for &(key, name, kind) in ENV_SETTINGS {
            if let Some(raw) = get(name) {
                settings.insert(key.to_owned(), env_value(name, kind, &raw)?);
            }
        }

        Ok(Self {
            settings,
            kafka_bootstrap_servers: get(KAFKA_BOOTSTRAP_SERVERS_ENV),
            opensearch_username: get(OPENSEARCH_USERNAME_ENV),
            opensearch_password: get(OPENSEARCH_PASSWORD_ENV),
        })
    }
}

fn env_value(name: &'static str, kind: EnvKind, raw: &str) -> Result<Value, ConfigError> {
    match kind {
        EnvKind::Text => Ok(Value::String(raw.to_owned())),
        EnvKind::Number => raw
            .parse::<u64>()
            .map(Value::from)
            .map_err(|_| ConfigError::EnvValue {
                key: name,
                message: format!("expected a whole number, got `{raw}`"),
            }),
        EnvKind::List => Ok(raw
            .split(',')
            .map(|item| Value::String(item.trim().to_owned()))
            .collect()),
        EnvKind::Object => Ok(parse_json_map(name, raw)?
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect()),
    }
}

/// Puts an environment value over the file's: objects are merged key by key,
/// anything else replaces the file's value.
fn override_setting(values: &mut Map<String, Value>, key: String, value: Value) {
    let current = values.entry(key).or_insert(Value::Null);
    match (current, value) {
        (Value::Object(current), Value::Object(entries)) => current.extend(entries),
        (current, value) => *current = value,
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

    /// Builds the config from resolved CAC keys such as `batch.max_docs`,
    /// with the environment's values taking priority.
    pub fn from_cac(
        mut values: Map<String, Value>,
        overrides: EnvOverrides,
    ) -> Result<Self, ConfigError> {
        let EnvOverrides {
            settings,
            kafka_bootstrap_servers,
            opensearch_username,
            opensearch_password,
        } = overrides;

        let index_from_env = settings.contains_key(INDEX_KEY);
        for (key, value) in settings {
            override_setting(&mut values, key, value);
        }

        let mut config: Self = serde_json::from_value(nest(values)?)?;
        if let Some(bootstrap_servers) = kafka_bootstrap_servers {
            config
                .kafka
                .client_config
                .insert(BOOTSTRAP_SERVERS.to_owned(), bootstrap_servers);
        }
        config.opensearch.index_from_env = index_from_env;
        config.opensearch.username = opensearch_username;
        config.opensearch.password = opensearch_password.map(Secret);
        config.validate()?;
        Ok(config)
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

    fn env(vars: &[(&str, &str)]) -> Result<EnvOverrides, ConfigError> {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        EnvOverrides::from_lookup(|name| vars.get(name).map(|value| (*value).to_owned()))
    }

    fn config_from_env(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        Config::from_cac(minimal(), env(vars)?)
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
        let config = config_from_env(&[
            ("KAFKA_BOOTSTRAP_SERVERS", "env-broker:9092"),
            (
                "KAFKA_CLIENT_CONFIG",
                r#"{"bootstrap.servers": "json-broker:9092", "security.protocol": "SSL"}"#,
            ),
            ("OPENSEARCH_URL", "https://search.internal:443"),
            ("OPENSEARCH_USERNAME", "sink"),
            ("OPENSEARCH_PASSWORD", "hunter2"),
        ])
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
    fn every_setting_can_come_from_the_environment() {
        let config = config_from_env(&[
            ("KAFKA_TOPICS", "events.a, events.b"),
            ("KAFKA_GROUP_ID", "env-group"),
            ("KAFKA_CLIENT_CONFIG", r#"{"security.protocol": "SSL"}"#),
            ("OPENSEARCH_URL", "https://search.internal"),
            ("OPENSEARCH_INDEX", "events-env-{org_id}"),
            ("OPENSEARCH_REQUEST_TIMEOUT_MS", "2000"),
            ("BATCH_MAX_DOCS", "10"),
            ("BATCH_MAX_BYTES", "2048"),
            ("BATCH_LINGER_MS", "50"),
            ("BATCH_MAX_IN_FLIGHT", "2"),
            ("RETRY_INITIAL_BACKOFF_MS", "5"),
            ("RETRY_MAX_BACKOFF_MS", "500"),
            ("DLQ_TOPIC", "events.env.dlq"),
            ("COMMIT_INTERVAL_MS", "700"),
            ("SHUTDOWN_GRACE_MS", "900"),
            ("SERVER_ADDR", "127.0.0.1:9000"),
        ])
        .unwrap();

        assert_eq!(config.kafka.topics, vec!["events.a", "events.b"]);
        assert_eq!(config.kafka.group_id, "env-group");
        // Merged into the file's client config, not replacing it.
        assert_eq!(
            config.kafka.client_config["bootstrap.servers"],
            "cac-broker:9092"
        );
        assert_eq!(config.kafka.client_config["security.protocol"], "SSL");
        assert_eq!(config.opensearch.url, "https://search.internal");
        assert_eq!(
            config.opensearch.index,
            IndexTemplate::parse("events-env-{org_id}").unwrap()
        );
        assert!(config.opensearch.index_from_env);
        assert_eq!(config.opensearch.request_timeout_ms, 2000);
        assert_eq!(config.batch.max_docs, 10);
        assert_eq!(config.batch.max_bytes, 2048);
        assert_eq!(config.batch.linger_ms, 50);
        assert_eq!(config.batch.max_in_flight, 2);
        assert_eq!(config.retry.initial_backoff_ms, 5);
        assert_eq!(config.retry.max_backoff_ms, 500);
        assert_eq!(config.dlq.topic, "events.env.dlq");
        assert_eq!(config.commit.interval_ms, 700);
        assert_eq!(config.shutdown.grace_ms, 900);
        assert_eq!(config.server.addr, "127.0.0.1:9000".parse().unwrap());
    }

    #[test]
    fn env_names_are_the_keys_in_upper_case() {
        for (key, name, _) in ENV_SETTINGS {
            assert_eq!(*name, key.to_uppercase().replace('.', "_"));
        }
    }

    #[test]
    fn the_index_from_the_file_is_not_pinned() {
        let config = config_from_env(&[("BATCH_MAX_DOCS", "10")]).unwrap();

        assert!(!config.opensearch.index_from_env);
    }

    #[test]
    fn empty_env_values_are_ignored() {
        let config = config_from_env(&[("BATCH_MAX_DOCS", "  "), ("DLQ_TOPIC", "")]).unwrap();

        assert_eq!(config.batch.max_docs, 1000);
        assert_eq!(config.dlq.topic, "events.generic.dlq");
    }

    #[test]
    fn env_numbers_must_be_whole_numbers() {
        for value in ["ten", "-1", "1.5"] {
            let error = env(&[("BATCH_MAX_DOCS", value)]).unwrap_err();

            assert!(
                error.to_string().contains("BATCH_MAX_DOCS"),
                "expected the variable in `{error}`"
            );
        }
    }

    #[test]
    fn env_values_are_validated_like_file_values() {
        assert_invalid(
            config_from_env(&[("DLQ_TOPIC", "events.generic")]),
            "dead letters",
        );
        assert_invalid(config_from_env(&[("KAFKA_TOPICS", "a,,b")]), "kafka.topics");
    }

    #[test]
    fn debug_output_hides_the_password() {
        let config = config_from_env(&[
            ("OPENSEARCH_USERNAME", "sink"),
            ("OPENSEARCH_PASSWORD", "hunter2"),
        ])
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
    fn index_can_be_per_org() {
        let config = config_with(&[("opensearch.index", json!("events-{org_id}"))]).unwrap();

        assert_eq!(
            config.opensearch.index.render("merchant-1").unwrap(),
            "events-merchant-1"
        );
    }

    #[test]
    fn index_template_checks_its_fixed_parts() {
        for (index, needle) in [
            ("Events-{org_id}", "not a valid index name"),
            ("_{org_id}", "not a valid index name"),
            ("events-{org}", "the only placeholder is `{org_id}`"),
            ("events-{date}-{org_id}", "the only placeholder"),
        ] {
            assert_rejected(config_with(&[("opensearch.index", json!(index))]), needle);
        }
    }

    #[test]
    fn fixed_index_ignores_the_org() {
        let template = IndexTemplate::parse("events").unwrap();

        assert_eq!(template.render("Anything, really").unwrap(), "events");
    }

    #[test]
    fn orgs_that_cannot_form_an_index_name_are_refused() {
        let template = IndexTemplate::parse("events-{org_id}").unwrap();

        for org in [
            "",
            "Merchant-1",
            "a,b",
            "a b",
            "a*",
            "a/b",
            &"x".repeat(250),
        ] {
            assert!(template.render(org).is_err(), "`{org}` should be refused");
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
        assert_invalid(
            config_from_env(&[("OPENSEARCH_USERNAME", "sink")]),
            "OPENSEARCH_PASSWORD",
        );
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
        let error = env(&[("KAFKA_CLIENT_CONFIG", "not-json")]).unwrap_err();

        assert!(error.to_string().contains("KAFKA_CLIENT_CONFIG"));
    }

    #[tokio::test]
    async fn the_shipped_cac_file_is_valid() {
        let cac = Cac::load(concat!(env!("CARGO_MANIFEST_DIR"), "/cac.toml"))
            .await
            .unwrap();

        let values = cac.resolve_defaults().await.unwrap();
        for key in values.keys() {
            assert!(
                ENV_SETTINGS.iter().any(|(setting, _, _)| setting == key),
                "`{key}` has no environment variable in ENV_SETTINGS"
            );
        }
        let config = Config::from_cac(values, EnvOverrides::default()).unwrap();

        assert_eq!(config.kafka.topics, vec!["events.generic"]);
        assert_eq!(
            config.opensearch.index,
            IndexTemplate::parse("events-{org_id}").unwrap()
        );
        cac.close().await;
    }
}
