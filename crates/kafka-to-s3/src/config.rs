//! Environment-driven configuration with fail-fast validation.

use std::str::FromStr;
use std::time::Duration;

/// Default `BATCH_MAX_EVENTS` (D1 `n`).
const DEFAULT_BATCH_MAX_EVENTS: usize = 10_000;
/// Default `FLUSH_INTERVAL_SECS` (window length).
const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 60;

/// Boot configuration, parsed from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_bootstrap_servers: String,
    pub kafka_topics: Vec<String>,
    pub kafka_group_id: String,
    pub s3_bucket: String,
    pub s3_prefix: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_force_path_style: bool,
    pub batch_max_events: usize,
    pub flush_interval: Duration,
    pub source_id: Option<String>,
}

/// Configuration failure, naming the offending variable.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{variable} is required but not set")]
    Missing { variable: &'static str },
    #[error("{variable} must contain at least one non-empty topic")]
    MissingTopics { variable: &'static str },
    #[error("{variable} must be {expected}, got {value:?}")]
    Invalid {
        variable: &'static str,
        expected: &'static str,
        value: String,
    },
    #[error("{variable} must be greater than zero, got {value}")]
    NotPositive {
        variable: &'static str,
        value: String,
    },
}

impl Config {
    /// Parse from the process environment. Empty or whitespace-only values are unset.
    ///
    /// # Errors
    /// Returns a [`ConfigError`] naming the first missing or invalid variable.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Parse from a lookup function (test seam; keeps tests free of process-env mutation).
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let value = |name: &str| {
            lookup(name)
                .map(|raw| raw.trim().to_string())
                .filter(|trimmed| !trimmed.is_empty())
        };

        let kafka_bootstrap_servers = required(&value, "KAFKA_BOOTSTRAP_SERVERS")?;
        let kafka_topics = parse_topics(&required(&value, "KAFKA_TOPICS")?)?;
        let kafka_group_id = required(&value, "KAFKA_GROUP_ID")?;
        let s3_bucket = required(&value, "S3_BUCKET")?;
        let s3_prefix = value("S3_PREFIX")
            .map(|prefix| prefix.trim_matches('/').to_string())
            .filter(|prefix| !prefix.is_empty());
        let s3_endpoint = value("S3_ENDPOINT");
        let s3_force_path_style = match value("S3_FORCE_PATH_STYLE") {
            Some(raw) => parse_bool("S3_FORCE_PATH_STYLE", &raw)?,
            None => false,
        };
        let batch_max_events = match value("BATCH_MAX_EVENTS") {
            Some(raw) => parse_positive("BATCH_MAX_EVENTS", &raw)?,
            None => DEFAULT_BATCH_MAX_EVENTS,
        };
        let flush_interval_secs = match value("FLUSH_INTERVAL_SECS") {
            Some(raw) => parse_positive("FLUSH_INTERVAL_SECS", &raw)?,
            None => DEFAULT_FLUSH_INTERVAL_SECS,
        };
        let source_id = value("SOURCE_ID");

        Ok(Self {
            kafka_bootstrap_servers,
            kafka_topics,
            kafka_group_id,
            s3_bucket,
            s3_prefix,
            s3_endpoint,
            s3_force_path_style,
            batch_max_events,
            flush_interval: Duration::from_secs(flush_interval_secs),
            source_id,
        })
    }
}

fn required(
    lookup: &impl Fn(&str) -> Option<String>,
    variable: &'static str,
) -> Result<String, ConfigError> {
    lookup(variable).ok_or(ConfigError::Missing { variable })
}

fn parse_topics(raw: &str) -> Result<Vec<String>, ConfigError> {
    let topics: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|topic| !topic.is_empty())
        .map(str::to_string)
        .collect();
    if topics.is_empty() {
        return Err(ConfigError::MissingTopics {
            variable: "KAFKA_TOPICS",
        });
    }
    Ok(topics)
}

fn parse_bool(variable: &'static str, raw: &str) -> Result<bool, ConfigError> {
    match raw.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::Invalid {
            variable,
            expected: "\"true\" or \"false\"",
            value: raw.to_string(),
        }),
    }
}

fn parse_positive<T>(variable: &'static str, raw: &str) -> Result<T, ConfigError>
where
    T: FromStr + Default + PartialOrd,
    T::Err: std::fmt::Display,
{
    let parsed = raw.parse::<T>().map_err(|_| ConfigError::Invalid {
        variable,
        expected: "a positive integer",
        value: raw.to_string(),
    })?;
    if parsed <= T::default() {
        return Err(ConfigError::NotPositive {
            variable,
            value: raw.to_string(),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&'static str, &str)]) -> impl Fn(&str) -> Option<String> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        move |name| env.get(name).cloned()
    }

    fn required_env() -> Vec<(&'static str, String)> {
        vec![
            (
                "KAFKA_BOOTSTRAP_SERVERS",
                "broker1:9092,broker2:9092".to_string(),
            ),
            ("KAFKA_TOPICS", "events".to_string()),
            ("KAFKA_GROUP_ID", "k2s-prod".to_string()),
            ("S3_BUCKET", "events-archive".to_string()),
        ]
    }

    fn parse(pairs: &[(&'static str, String)]) -> Result<Config, ConfigError> {
        let static_pairs: Vec<(&'static str, &str)> =
            pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Config::from_lookup(env_of(&static_pairs))
    }

    #[test]
    fn parses_all_fields_when_fully_set() {
        let config = parse(&[
            ("KAFKA_BOOTSTRAP_SERVERS", "redpanda:9092".to_string()),
            ("KAFKA_TOPICS", "events , other_events".to_string()),
            ("KAFKA_GROUP_ID", "archiver-7".to_string()),
            ("S3_BUCKET", "bucket-x".to_string()),
            ("S3_PREFIX", "env=prod".to_string()),
            ("S3_ENDPOINT", "http://localhost:9000".to_string()),
            ("S3_FORCE_PATH_STYLE", "true".to_string()),
            ("BATCH_MAX_EVENTS", "25".to_string()),
            ("FLUSH_INTERVAL_SECS", "7".to_string()),
            ("SOURCE_ID", "aws-prod-a".to_string()),
        ])
        .unwrap();

        assert_eq!(config.kafka_bootstrap_servers, "redpanda:9092");
        assert_eq!(config.kafka_topics, vec!["events", "other_events"]);
        assert_eq!(config.kafka_group_id, "archiver-7");
        assert_eq!(config.s3_bucket, "bucket-x");
        assert_eq!(config.s3_prefix.as_deref(), Some("env=prod"));
        assert_eq!(config.s3_endpoint.as_deref(), Some("http://localhost:9000"));
        assert!(config.s3_force_path_style);
        assert_eq!(config.batch_max_events, 25);
        assert_eq!(config.flush_interval, Duration::from_secs(7));
        assert_eq!(config.source_id.as_deref(), Some("aws-prod-a"));
    }

    #[test]
    fn applies_defaults_when_optional_vars_unset() {
        let config = parse(&required_env()).unwrap();

        assert_eq!(config.s3_prefix, None);
        assert_eq!(config.s3_endpoint, None);
        assert!(!config.s3_force_path_style);
        assert_eq!(config.batch_max_events, 10_000);
        assert_eq!(config.flush_interval, Duration::from_secs(60));
        assert_eq!(config.source_id, None);
    }

    #[test]
    fn missing_required_variable_is_named_in_error() {
        let mut pairs = required_env();
        pairs.retain(|(name, _)| *name != "KAFKA_GROUP_ID");

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("KAFKA_GROUP_ID"), "{error}");
    }

    #[test]
    fn blank_required_variable_is_treated_as_missing() {
        let mut pairs = required_env();
        pairs.push(("S3_BUCKET", "   ".to_string()));

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("S3_BUCKET"), "{error}");
    }

    #[test]
    fn topics_list_without_entries_fails() {
        let mut pairs = required_env();
        pairs.push(("KAFKA_TOPICS", " , , ".to_string()));

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("KAFKA_TOPICS"), "{error}");
    }

    #[test]
    fn zero_batch_max_events_is_rejected() {
        let mut pairs = required_env();
        pairs.push(("BATCH_MAX_EVENTS", "0".to_string()));

        let error = parse(&pairs).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("BATCH_MAX_EVENTS"), "{message}");
        assert!(message.contains('0'), "{message}");
    }

    #[test]
    fn zero_flush_interval_is_rejected() {
        let mut pairs = required_env();
        pairs.push(("FLUSH_INTERVAL_SECS", "0".to_string()));

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("FLUSH_INTERVAL_SECS"), "{error}");
    }

    #[test]
    fn non_numeric_batch_max_events_is_rejected() {
        let mut pairs = required_env();
        pairs.push(("BATCH_MAX_EVENTS", "many".to_string()));

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("BATCH_MAX_EVENTS"), "{error}");
    }

    #[test]
    fn invalid_force_path_style_is_rejected() {
        let mut pairs = required_env();
        pairs.push(("S3_FORCE_PATH_STYLE", "yes".to_string()));

        let error = parse(&pairs).unwrap_err();
        assert!(error.to_string().contains("S3_FORCE_PATH_STYLE"), "{error}");
    }

    #[test]
    fn force_path_style_is_case_insensitive() {
        for (raw, expected) in [("TRUE", true), ("False", false)] {
            let mut pairs = required_env();
            pairs.push(("S3_FORCE_PATH_STYLE", raw.to_string()));
            let config = parse(&pairs).unwrap();
            assert_eq!(config.s3_force_path_style, expected, "{raw}");
        }
    }

    #[test]
    fn prefix_slashes_are_trimmed() {
        let mut pairs = required_env();
        pairs.push(("S3_PREFIX", "/env=prod/".to_string()));
        let config = parse(&pairs).unwrap();
        assert_eq!(config.s3_prefix.as_deref(), Some("env=prod"));

        let mut pairs = required_env();
        pairs.push(("S3_PREFIX", "/".to_string()));
        let config = parse(&pairs).unwrap();
        assert_eq!(config.s3_prefix, None);
    }
}
