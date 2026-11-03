//! Per-workspace infrastructure: the workspace's Kafka topic, its ClickHouse
//! database and user (created by clickhouse-proxy, which owns them), and its
//! OpenSearch data stream.
//! The admin API provisions them after the workspace is created (see
//! `routes/admin.rs`); deleting the workspace drops them all.
//!
//! Every step is idempotent in both directions, so provisioning or
//! deprovisioning can be re-run after a partial failure. Every step is
//! bounded by [`STEP_TIMEOUT`], so an attempt always ends before
//! [`STALE_CREATING_AFTER`] lets another attempt take the workspace over.

use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use canonical_event::{OrgId, WorkspaceId};
use chrono::{DateTime, Utc};
use futures_util::FutureExt;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::error::RDKafkaErrorCode;
use serde::Serialize;

use crate::AppState;
use clickhouse_proxy::client::ProxyError;
use opensearch_proxy::{self as opensearch, OpenSearchError};

const ORG_ID_PLACEHOLDER: &str = "{org_id}";
const WORKSPACE_ID_PLACEHOLDER: &str = "{workspace_id}";
/// Kafka's limit on topic name length.
const MAX_TOPIC_LEN: usize = 249;
const ADMIN_TIMEOUT: Duration = Duration::from_secs(10);

/// How long one provisioning or deprovisioning step may take.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// A `CREATING` workspace whose attempt started this long ago lost it (e.g.
/// the server restarted mid-way), so it may be provisioned or deleted.
pub const STALE_CREATING_AFTER: Duration = Duration::from_secs(15 * 60);

// A live attempt must end before it can be taken over.
const _: () =
    assert!(STEP_TIMEOUT.as_secs() * (Step::ALL.len() as u64) < STALE_CREATING_AFTER.as_secs());

#[derive(Debug, thiserror::Error)]
pub enum ProvisionError {
    #[error("failed to {action} Kafka topic {topic}: {reason}")]
    Topic {
        action: &'static str,
        topic: String,
        reason: String,
    },
    #[error("clickhouse-proxy: {0}")]
    ClickHouse(#[from] ProxyError),
    #[error(transparent)]
    OpenSearch(#[from] OpenSearchError),
    #[error("timed out after {0:?}")]
    TimedOut(Duration),
    #[error("panicked: {0}")]
    Panicked(String),
}

/// One provisioning step. Serialized names appear in `provisioning_error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    KafkaTopic,
    ClickhouseDatabase,
    ClickhouseUser,
    OpensearchIndex,
}

impl Step {
    /// The order steps run in: the topic before the database whose Kafka
    /// table reads it, and the database before the user, so the user never
    /// sees a half-built database.
    pub const ALL: [Self; 4] = [
        Self::KafkaTopic,
        Self::ClickhouseDatabase,
        Self::ClickhouseUser,
        Self::OpensearchIndex,
    ];
}

/// Why provisioning stopped; stored as the workspace's `provisioning_error`.
#[derive(Debug, Serialize)]
pub struct ProvisioningFailure {
    pub failed_step: Step,
    pub error: String,
    /// Steps that succeeded before it. They stay in place for the retry.
    pub completed_steps: Vec<Step>,
    /// Steps that were not attempted.
    pub skipped_steps: Vec<Step>,
    pub failed_at: DateTime<Utc>,
}

/// Where and how workspace infrastructure is created, from the environment.
#[derive(Clone, Debug)]
pub struct ProvisioningConfig {
    /// Brokers as dashboard-server reaches them (`KAFKA_BROKER_URL`).
    pub kafka_brokers: String,
    /// Topic name template (`WORKSPACE_TOPIC_TEMPLATE`); must render the same
    /// name as the events-collector's `kafka_connector.topic`.
    pub topic_template: String,
    pub topic_partitions: i32,
    pub topic_replication: i32,
}

impl ProvisioningConfig {
    /// Reads and validates the configuration.
    ///
    /// # Errors
    /// Names the first missing or invalid variable.
    pub fn from_env(kafka_brokers: String) -> Result<Self, String> {
        let var = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default.to_owned())
        };
        let positive = |name: &str, default: &str| {
            let raw = var(name, default);
            raw.parse::<i32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer, got {raw:?}"))
        };

        let topic_template = var("WORKSPACE_TOPIC_TEMPLATE", "events.{workspace_id}");
        if !topic_template.contains(WORKSPACE_ID_PLACEHOLDER) {
            return Err(format!(
                "WORKSPACE_TOPIC_TEMPLATE must contain {WORKSPACE_ID_PLACEHOLDER}, got {topic_template:?}"
            ));
        }
        // Slugs are legal topic characters, so checking the template with a
        // sample slug checks every rendered name.
        let sample = render_topic(&topic_template, "sample_org", "sample_org-sample_workspace");
        validate_topic(&sample).map_err(|reason| {
            format!(
                "WORKSPACE_TOPIC_TEMPLATE renders invalid topics ({reason}): {topic_template:?}"
            )
        })?;

        Ok(Self {
            kafka_brokers,
            topic_template,
            topic_partitions: positive("WORKSPACE_TOPIC_PARTITIONS", "1")?,
            topic_replication: positive("WORKSPACE_TOPIC_REPLICATION", "1")?,
        })
    }

    /// The Kafka topic carrying `workspace`'s events.
    pub fn topic(&self, org: &OrgId, workspace: &WorkspaceId) -> String {
        render_topic(&self.topic_template, org.as_ref(), workspace.as_ref())
    }
}

/// Runs every step of [`Step::ALL`] for `workspace`. Safe to run again for a
/// workspace that already has some or all of its infrastructure.
///
/// # Errors
/// Stops at the first step that fails, times out or panics; earlier steps
/// stay in place.
pub async fn provision_workspace(
    state: &AppState,
    org: &OrgId,
    workspace: &WorkspaceId,
) -> Result<(), ProvisioningFailure> {
    run_steps(STEP_TIMEOUT, |step| run_step(state, step, org, workspace))
        .await
        .map_err(|mut failure| {
            failure.error = stored_error(&failure.error);
            failure
        })
}

async fn run_steps<F, Fut>(step_timeout: Duration, mut run: F) -> Result<(), ProvisioningFailure>
where
    F: FnMut(Step) -> Fut,
    Fut: Future<Output = Result<(), ProvisionError>>,
{
    for (index, step) in Step::ALL.into_iter().enumerate() {
        // A hang or a panic fails the step like an error does, so the
        // attempt always records how it ended.
        let outcome = AssertUnwindSafe(run(step)).catch_unwind();
        let outcome = match tokio::time::timeout(step_timeout, outcome).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(panic)) => Err(ProvisionError::Panicked(panic_message(panic.as_ref()))),
            Err(_) => Err(ProvisionError::TimedOut(step_timeout)),
        };
        if let Err(error) = outcome {
            return Err(ProvisioningFailure {
                failed_step: step,
                error: error.to_string(),
                completed_steps: Step::ALL[..index].to_vec(),
                skipped_steps: Step::ALL[index + 1..].to_vec(),
                failed_at: Utc::now(),
            });
        }
    }
    Ok(())
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

/// `error` as `provisioning_error` stores it, cut to a bounded length.
/// clickhouse-proxy already leaves the workspace password out of its errors.
fn stored_error(error: &str) -> String {
    opensearch::excerpt(error).to_owned()
}

/// Runs a deprovisioning step, failing it after [`STEP_TIMEOUT`].
async fn bounded<T, E>(step: impl Future<Output = Result<T, E>>) -> Result<T, ProvisionError>
where
    ProvisionError: From<E>,
{
    match tokio::time::timeout(STEP_TIMEOUT, step).await {
        Ok(outcome) => outcome.map_err(ProvisionError::from),
        Err(_) => Err(ProvisionError::TimedOut(STEP_TIMEOUT)),
    }
}

async fn run_step(
    state: &AppState,
    step: Step,
    org: &OrgId,
    workspace: &WorkspaceId,
) -> Result<(), ProvisionError> {
    let config = &state.provisioning;
    match step {
        Step::KafkaTopic => create_topic(config, &config.topic(org, workspace)).await,
        Step::ClickhouseDatabase => Ok(state
            .clickhouse
            .create_database(workspace.as_ref(), &config.topic(org, workspace))
            .await?),
        Step::ClickhouseUser => Ok(state.clickhouse.create_user(workspace.as_ref()).await?),
        Step::OpensearchIndex => Ok(state
            .opensearch
            .create_data_stream(org.as_ref(), workspace.as_ref())
            .await?),
    }
}

/// Drops `workspace`'s ClickHouse database (with its Kafka table and views),
/// its ClickHouse user, its topic and its data stream, permanently deleting
/// its events. Safe to run for a workspace that has some or none of them.
///
/// # Errors
/// Every step is attempted even when an earlier one fails, so as much as
/// possible is removed; the first failure is returned.
pub async fn deprovision_workspace(
    state: &AppState,
    org: &OrgId,
    workspace: &WorkspaceId,
) -> Result<(), ProvisionError> {
    let config = &state.provisioning;
    // The database first: dropping its Kafka table stops ClickHouse
    // consuming before the topic disappears. The topic before the data
    // stream, so sink-opensearch has nothing left to write to it.
    let database = bounded(state.clickhouse.drop_database(workspace.as_ref())).await;
    let user = bounded(state.clickhouse.drop_user(workspace.as_ref())).await;
    let topic = bounded(delete_topic(config, &config.topic(org, workspace))).await;
    let data_stream = bounded(
        state
            .opensearch
            .delete_data_stream(org.as_ref(), workspace.as_ref()),
    )
    .await;
    database.and(user).and(topic).and(data_stream)
}

fn kafka_admin(
    config: &ProvisioningConfig,
) -> Result<AdminClient<DefaultClientContext>, rdkafka::error::KafkaError> {
    ClientConfig::new()
        .set("bootstrap.servers", &config.kafka_brokers)
        .create()
}

/// Creates `topic`, treating "already exists" as success.
async fn create_topic(config: &ProvisioningConfig, topic: &str) -> Result<(), ProvisionError> {
    let error = |reason: String| ProvisionError::Topic {
        action: "create",
        topic: topic.to_owned(),
        reason,
    };
    let admin = kafka_admin(config).map_err(|e| error(e.to_string()))?;
    let results = admin
        .create_topics(
            &[NewTopic::new(
                topic,
                config.topic_partitions,
                TopicReplication::Fixed(config.topic_replication),
            )],
            &AdminOptions::new().operation_timeout(Some(ADMIN_TIMEOUT)),
        )
        .await
        .map_err(|e| error(e.to_string()))?;
    for result in results {
        match result {
            Ok(_) => {}
            Err((_, RDKafkaErrorCode::TopicAlreadyExists)) => {
                check_existing_topic(config, topic).await.map_err(error)?;
            }
            Err((_, code)) => return Err(error(code.to_string())),
        }
    }
    Ok(())
}

/// Checks that the existing `topic` has at least the configured partitions
/// and replication. A topic the broker auto-created from early traffic has
/// the broker's defaults instead, which provisioning must not adopt silently.
async fn check_existing_topic(config: &ProvisioningConfig, topic: &str) -> Result<(), String> {
    let config = config.clone();
    let topic = topic.to_owned();
    // `fetch_metadata` blocks.
    tokio::task::spawn_blocking(move || {
        let admin = kafka_admin(&config).map_err(|e| e.to_string())?;
        let metadata = admin
            .inner()
            .fetch_metadata(Some(&topic), ADMIN_TIMEOUT)
            .map_err(|e| e.to_string())?;
        let partitions = metadata
            .topics()
            .iter()
            .find(|found| found.name() == topic)
            .map(|found| found.partitions())
            .unwrap_or_default();
        let replication = partitions
            .iter()
            .map(|partition| partition.replicas().len())
            .min()
            .unwrap_or(0);
        check_topic_shape(&config, partitions.len(), replication)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn check_topic_shape(
    config: &ProvisioningConfig,
    partitions: usize,
    replication: usize,
) -> Result<(), String> {
    let wanted_partitions = usize::try_from(config.topic_partitions).unwrap_or(usize::MAX);
    let wanted_replication = usize::try_from(config.topic_replication).unwrap_or(usize::MAX);
    if partitions < wanted_partitions || replication < wanted_replication {
        return Err(format!(
            "it already exists with {partitions} partitions and replication {replication}, \
             below the configured {wanted_partitions} and {wanted_replication}; delete it and \
             provision again"
        ));
    }
    Ok(())
}

/// Deletes `topic`, treating "does not exist" as success.
async fn delete_topic(config: &ProvisioningConfig, topic: &str) -> Result<(), ProvisionError> {
    let error = |reason: String| ProvisionError::Topic {
        action: "delete",
        topic: topic.to_owned(),
        reason,
    };
    let admin = kafka_admin(config).map_err(|e| error(e.to_string()))?;
    let results = admin
        .delete_topics(
            &[topic],
            &AdminOptions::new().operation_timeout(Some(ADMIN_TIMEOUT)),
        )
        .await
        .map_err(|e| error(e.to_string()))?;
    for result in results {
        match result {
            Ok(_) | Err((_, RDKafkaErrorCode::UnknownTopicOrPartition)) => {}
            Err((_, code)) => return Err(error(code.to_string())),
        }
    }
    Ok(())
}

/// Mirrors the events-collector's topic rendering (`kafka_connector.rs`).
fn render_topic(template: &str, org: &str, workspace: &str) -> String {
    template
        .replace(ORG_ID_PLACEHOLDER, org)
        .replace(WORKSPACE_ID_PLACEHOLDER, workspace)
}

fn validate_topic(topic: &str) -> Result<(), String> {
    if topic.is_empty() || topic.len() > MAX_TOPIC_LEN {
        return Err(format!("length must be 1..={MAX_TOPIC_LEN}"));
    }
    if topic == "." || topic == ".." {
        return Err("`.` and `..` are reserved".to_owned());
    }
    if let Some(bad) = topic
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        return Err(format!("{bad:?} is not allowed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn config() -> ProvisioningConfig {
        ProvisioningConfig {
            kafka_brokers: "localhost:19092".to_owned(),
            topic_template: "events.{workspace_id}".to_owned(),
            topic_partitions: 1,
            topic_replication: 1,
        }
    }

    fn ids() -> (OrgId, WorkspaceId) {
        (
            OrgId::from_str("acme_retail").unwrap(),
            WorkspaceId::from_str("acme_retail-web_shop").unwrap(),
        )
    }

    #[test]
    fn topic_renders_the_workspace_id() {
        let (org, workspace) = ids();
        assert_eq!(
            config().topic(&org, &workspace),
            "events.acme_retail-web_shop"
        );
    }

    /// Runs the steps, failing at `failing` (if any); returns the steps run.
    async fn run_failing_at(failing: Option<Step>) -> (Vec<Step>, Result<(), ProvisioningFailure>) {
        let mut ran = Vec::new();
        let outcome = run_steps(STEP_TIMEOUT, |step| {
            ran.push(step);
            async move {
                if Some(step) == failing {
                    return Err(ProvisionError::Topic {
                        action: "create",
                        topic: "events.acme".to_owned(),
                        reason: "broker down".to_owned(),
                    });
                }
                Ok(())
            }
        })
        .await;
        (ran, outcome)
    }

    #[tokio::test]
    async fn every_step_runs_in_order() {
        let (ran, outcome) = run_failing_at(None).await;

        assert!(outcome.is_ok());
        assert_eq!(ran, Step::ALL);
    }

    #[tokio::test]
    async fn a_failure_records_completed_and_skipped_steps() {
        let (ran, outcome) = run_failing_at(Some(Step::ClickhouseDatabase)).await;
        let failure = outcome.unwrap_err();

        assert_eq!(ran, [Step::KafkaTopic, Step::ClickhouseDatabase]);
        let mut json = serde_json::to_value(&failure).unwrap();
        assert!(json["failed_at"].as_str().is_some());
        json.as_object_mut().unwrap().remove("failed_at");
        assert_eq!(
            json,
            serde_json::json!({
                "failed_step": "clickhouse_database",
                "error": "failed to create Kafka topic events.acme: broker down",
                "completed_steps": ["kafka_topic"],
                "skipped_steps": ["clickhouse_user", "opensearch_index"],
            })
        );
    }

    #[tokio::test]
    async fn a_failure_at_the_first_or_last_step() {
        let first = run_failing_at(Some(Step::KafkaTopic)).await.1.unwrap_err();
        assert!(first.completed_steps.is_empty());
        assert_eq!(first.skipped_steps, &Step::ALL[1..]);

        let last = run_failing_at(Some(Step::OpensearchIndex))
            .await
            .1
            .unwrap_err();
        assert_eq!(last.completed_steps, &Step::ALL[..3]);
        assert!(last.skipped_steps.is_empty());
    }

    #[tokio::test]
    async fn a_hanging_step_times_out() {
        let timeout = Duration::from_millis(10);
        let failure = run_steps(timeout, |step| async move {
            if step == Step::ClickhouseUser {
                std::future::pending::<()>().await;
            }
            Ok(())
        })
        .await
        .unwrap_err();

        assert_eq!(failure.failed_step, Step::ClickhouseUser);
        assert_eq!(failure.error, "timed out after 10ms");
        assert_eq!(failure.skipped_steps, [Step::OpensearchIndex]);
    }

    #[tokio::test]
    async fn a_panicking_step_fails_it() {
        let failure = run_steps(STEP_TIMEOUT, |step| async move {
            assert_ne!(step, Step::ClickhouseDatabase, "broken step");
            Ok(())
        })
        .await
        .unwrap_err();

        assert_eq!(failure.failed_step, Step::ClickhouseDatabase);
        assert!(failure.error.starts_with("panicked: "), "{}", failure.error);
        assert!(failure.error.contains("broken step"), "{}", failure.error);
        assert_eq!(failure.completed_steps, [Step::KafkaTopic]);
    }

    #[test]
    fn stored_errors_are_bounded() {
        assert_eq!(stored_error("plain"), "plain");
        let long = "x".repeat(10_000);
        assert!(stored_error(&long).len() < 1_000);
    }

    #[test]
    fn an_existing_topic_needs_the_configured_shape() {
        let mut config = config();
        config.topic_partitions = 3;
        config.topic_replication = 2;

        assert!(check_topic_shape(&config, 3, 2).is_ok());
        assert!(check_topic_shape(&config, 6, 3).is_ok());
        let error = check_topic_shape(&config, 1, 2).unwrap_err();
        assert!(error.contains("1 partitions"), "{error}");
        assert!(check_topic_shape(&config, 3, 1).is_err());
    }

    #[test]
    fn topic_validation_rejects_illegal_names() {
        assert!(validate_topic("events.acme-k3x9qa.web-shop-9x2k1a").is_ok());
        assert!(validate_topic("events/acme").is_err());
        assert!(validate_topic("..").is_err());
        assert!(validate_topic(&"a".repeat(250)).is_err());
    }
}
