//! Test configuration, loaded once from the environment.

use std::sync::OnceLock;

/// Harness configuration resolved from `K2S_TEST_*` environment variables.
///
/// See the [crate documentation](crate) for the full variable table. The
/// values must mirror the configuration of the externally-run service under
/// test: the topic it consumes, its batch size `n`, and the bucket it
/// archives into.
#[derive(Debug)]
pub struct TestConfig {
    /// Kafka broker list (`K2S_TEST_KAFKA`).
    pub kafka_brokers: String,
    /// Topic the service consumes (`K2S_TEST_TOPIC`).
    pub topic: String,
    /// The service's configured batch size `n` (`K2S_TEST_BATCH_SIZE`).
    pub batch_size: usize,
    /// MinIO/S3 endpoint (`K2S_TEST_S3_ENDPOINT`).
    pub s3_endpoint: String,
    /// Bucket the service archives into (`K2S_TEST_S3_BUCKET`).
    pub s3_bucket: String,
    /// S3 region (`K2S_TEST_S3_REGION`).
    pub s3_region: String,
    /// S3 access key (`K2S_TEST_S3_ACCESS_KEY`).
    pub s3_access_key: String,
    /// S3 secret key (`K2S_TEST_S3_SECRET_KEY`).
    pub s3_secret_key: String,
    /// The service's `SOURCE_ID` (`K2S_TEST_SOURCE_ID`), when the operator
    /// mirrored it. The archived objects' `source` metadata is asserted only
    /// when this is set.
    pub source_id: Option<String>,
}

impl TestConfig {
    /// Returns the process-wide configuration, reading the environment on
    /// first use and caching it for the rest of the test run.
    pub fn load() -> &'static TestConfig {
        static CONFIG: OnceLock<TestConfig> = OnceLock::new();
        CONFIG.get_or_init(TestConfig::from_env)
    }

    fn from_env() -> TestConfig {
        let batch_size = env_or("K2S_TEST_BATCH_SIZE", "100")
            .parse::<usize>()
            .unwrap_or_else(|err| {
                panic!("K2S_TEST_BATCH_SIZE must be a positive integer (got {err})")
            });
        TestConfig {
            kafka_brokers: env_or("K2S_TEST_KAFKA", "localhost:19092"),
            topic: env_or("K2S_TEST_TOPIC", "canonical-events"),
            batch_size,
            s3_endpoint: env_or("K2S_TEST_S3_ENDPOINT", "http://localhost:9000"),
            s3_bucket: env_or("K2S_TEST_S3_BUCKET", "canonical-events"),
            s3_region: env_or("K2S_TEST_S3_REGION", "us-east-1"),
            s3_access_key: env_or("K2S_TEST_S3_ACCESS_KEY", "minioadmin"),
            s3_secret_key: env_or("K2S_TEST_S3_SECRET_KEY", "minioadmin"),
            source_id: env_optional("K2S_TEST_SOURCE_ID"),
        }
    }

    /// Builds the S3 key prefix the service must archive a group under:
    /// `{org}/{proj}/dt={date}/hour={hour}`.
    pub fn expected_prefix(&self, org: &str, proj: &str, date: &str, hour: u32) -> String {
        format!("{org}/{proj}/dt={date}/hour={hour}")
    }
}

/// Reads `name` from the environment, falling back to `default` when it is
/// unset or not valid Unicode.
fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Reads `name` from the environment, mapping an unset or empty value to
/// `None`.
fn env_optional(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_prefix_joins_group_parts() {
        let config = TestConfig {
            kafka_brokers: String::new(),
            topic: String::new(),
            batch_size: 1,
            s3_endpoint: String::new(),
            s3_bucket: String::new(),
            s3_region: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            source_id: None,
        };

        let prefix = config.expected_prefix("org-a", "proj-1", "2026-09-23", 13);

        assert_eq!(prefix, "org-a/proj-1/dt=2026-09-23/hour=13");
    }
}
