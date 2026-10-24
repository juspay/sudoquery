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
    /// Where documents are written. `{org_id}` and `{workspace_id}` are
    /// replaced with each event's org and workspace, giving every workspace its
    /// own data stream. This is the default; CAC overrides can change it per
    /// org or workspace.
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
const WORKSPACE_PLACEHOLDER: &str = "{workspace_id}";

/// An index, alias or data stream name, optionally containing `{org_id}`
/// and `{workspace_id}`, e.g. `events-{org_id}-{workspace_id}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct IndexTemplate(String);

/// The event ID that couldn't be part of an index name, with the reason.
#[derive(Debug, PartialEq, Eq)]
pub enum InvalidId {
    Org(String),
    Workspace(String),
}

impl IndexTemplate {
    /// Checks the fixed parts of the template against OpenSearch's naming
    /// rules. Each event's name is checked again when it arrives.
    pub fn parse(template: &str) -> Result<Self, String> {
        let fixed = template
            .replace(ORG_PLACEHOLDER, "")
            .replace(WORKSPACE_PLACEHOLDER, "");
        if fixed.contains(['{', '}']) {
            return Err(format!(
                "`opensearch.index` `{template}`: the only placeholders are `{ORG_PLACEHOLDER}` and `{WORKSPACE_PLACEHOLDER}`"
            ));
        }
        let sample = template
            .replace(ORG_PLACEHOLDER, "org")
            .replace(WORKSPACE_PLACEHOLDER, "workspace");
        validate_index_name(&sample)
            .map_err(|reason| format!("`opensearch.index` `{template}` {reason}"))?;
        Ok(Self(template.to_owned()))
    }

    /// The index for one event. Fails if its org or workspace can't be part of
    /// an index name, e.g. it is empty or has uppercase letters or a comma.
    /// The org is checked first, so a bad workspace is never blamed on it. IDs
    /// are never rewritten, because two orgs or workspaces could then share an
    /// index.
    pub fn render(&self, org_id: &str, workspace_id: &str) -> Result<Cow<'_, str>, InvalidId> {
        let has_org = self.0.contains(ORG_PLACEHOLDER);
        let has_workspace = self.0.contains(WORKSPACE_PLACEHOLDER);
        if !has_org && !has_workspace {
            return Ok(Cow::Borrowed(&self.0));
        }

        let mut index = self.0.clone();
        if has_org {
            if org_id.is_empty() {
                return Err(InvalidId::Org("org_id is empty".to_owned()));
            }
            index = index.replace(ORG_PLACEHOLDER, org_id);
            validate_index_name(&index).map_err(|reason| {
                InvalidId::Org(format!(
                    "org_id `{org_id}` gives index `{index}`, which {reason}"
                ))
            })?;
        }
        if has_workspace {
            if workspace_id.is_empty() {
                return Err(InvalidId::Workspace("workspace_id is empty".to_owned()));
            }
            index = index.replace(WORKSPACE_PLACEHOLDER, workspace_id);
            validate_index_name(&index).map_err(|reason| {
                InvalidId::Workspace(format!(
                    "workspace_id `{workspace_id}` gives index `{index}`, which {reason}"
                ))
            })?;
        }
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
        for topic in &self.kafka.topics {
            // librdkafka treats a topic starting with `^` as a regex.
            let consumed = match topic.strip_prefix('^') {
                Some(_) => regex_lite::Regex::new(topic)
                    .map_err(|e| {
                        ConfigError::Invalid(format!("`kafka.topics` pattern `{topic}`: {e}"))
                    })?
                    .is_match(&self.dlq.topic),
                None => *topic == self.dlq.topic,
            };
            if consumed {
                return invalid(format!(
                    "`dlq.topic` `{}` is matched by `kafka.topics` entry `{topic}`; the sink would consume its own dead letters",
                    self.dlq.topic
                ));
            }
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
mod tests;
