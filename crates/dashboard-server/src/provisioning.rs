//! Per-project infrastructure: the project's Kafka topic, its ClickHouse
//! database (`ckh/project.sql`) and user, and its OpenSearch data stream.
//! The admin API provisions them after the project is created (see
//! `routes/admin.rs`); deleting the project drops all but the data stream.
//!
//! Every step is idempotent in both directions, so provisioning or
//! deprovisioning can be re-run after a partial failure.

use std::future::Future;
use std::time::Duration;

use canonical_event::{OrgId, ProjectId};
use chrono::{DateTime, Utc};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::error::RDKafkaErrorCode;
use reqwest::Client;
use serde::Serialize;

use crate::AppState;
use crate::clickhouse::{self, ClickHouseError};
use crate::opensearch::OpenSearchError;

/// Per-project ClickHouse objects; see the file header for placeholders.
const PROJECT_SQL: &str = include_str!("../../../ckh/project.sql");

/// The tables in a project's database that its ClickHouse user may read.
pub const PROJECT_TABLES: [&str; 3] = ["events_v2", "user_events_v2", "event_schema_catalog"];

const ORG_ID_PLACEHOLDER: &str = "{org_id}";
const PROJECT_ID_PLACEHOLDER: &str = "{project_id}";
/// Kafka's limit on topic name length.
const MAX_TOPIC_LEN: usize = 249;
const ADMIN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum ProvisionError {
    #[error("failed to {action} Kafka topic {topic}: {reason}")]
    Topic {
        action: &'static str,
        topic: String,
        reason: String,
    },
    #[error(transparent)]
    ClickHouse(#[from] ClickHouseError),
    #[error(transparent)]
    OpenSearch(#[from] OpenSearchError),
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

/// Why provisioning stopped; stored as the project's `provisioning_error`.
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

/// Where and how project infrastructure is created, from the environment.
#[derive(Clone, Debug)]
pub struct ProvisioningConfig {
    /// Brokers as dashboard-server reaches them (`KAFKA_BROKER_URL`).
    pub kafka_brokers: String,
    /// Brokers as ClickHouse reaches them (`CLICKHOUSE_KAFKA_BROKERS`).
    pub clickhouse_kafka_brokers: String,
    /// Topic name template (`PROJECT_TOPIC_TEMPLATE`); must render the same
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

        let topic_template = var("PROJECT_TOPIC_TEMPLATE", "events.{org_id}.{project_id}");
        if !topic_template.contains(PROJECT_ID_PLACEHOLDER) {
            return Err(format!(
                "PROJECT_TOPIC_TEMPLATE must contain {PROJECT_ID_PLACEHOLDER}, got {topic_template:?}"
            ));
        }
        // Slugs are legal topic characters, so checking the template with a
        // sample slug checks every rendered name.
        let sample = render_topic(
            &topic_template,
            "sample-org-aaaaaa",
            "sample-project-aaaaaa",
        );
        validate_topic(&sample).map_err(|reason| {
            format!("PROJECT_TOPIC_TEMPLATE renders invalid topics ({reason}): {topic_template:?}")
        })?;

        Ok(Self {
            kafka_brokers,
            clickhouse_kafka_brokers: var("CLICKHOUSE_KAFKA_BROKERS", "redpanda:9092"),
            topic_template,
            topic_partitions: positive("PROJECT_TOPIC_PARTITIONS", "1")?,
            topic_replication: positive("PROJECT_TOPIC_REPLICATION", "1")?,
        })
    }

    /// The Kafka topic carrying `project`'s events.
    pub fn topic(&self, org: &OrgId, project: &ProjectId) -> String {
        render_topic(&self.topic_template, org.as_ref(), project.as_ref())
    }
}

/// ClickHouse connection used for DDL.
pub struct ClickHouseAdmin<'a> {
    pub url: &'a str,
    pub user: &'a str,
    pub password: &'a str,
}

impl<'a> ClickHouseAdmin<'a> {
    pub fn of(state: &'a AppState) -> Self {
        Self {
            url: &state.clickhouse_admin_url,
            user: &state.clickhouse_admin_user,
            password: &state.clickhouse_admin_password,
        }
    }
}

/// Runs every step of [`Step::ALL`] for `project`. Safe to run again for a
/// project that already has some or all of its infrastructure.
///
/// # Errors
/// Stops at the first step that fails; earlier steps stay in place.
pub async fn provision_project(
    state: &AppState,
    org: &OrgId,
    project: &ProjectId,
) -> Result<(), ProvisioningFailure> {
    run_steps(|step| run_step(state, step, org, project)).await
}

async fn run_steps<F, Fut>(mut run: F) -> Result<(), ProvisioningFailure>
where
    F: FnMut(Step) -> Fut,
    Fut: Future<Output = Result<(), ProvisionError>>,
{
    for (index, step) in Step::ALL.into_iter().enumerate() {
        if let Err(error) = run(step).await {
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

async fn run_step(
    state: &AppState,
    step: Step,
    org: &OrgId,
    project: &ProjectId,
) -> Result<(), ProvisionError> {
    let config = &state.provisioning;
    let admin = ClickHouseAdmin::of(state);
    match step {
        Step::KafkaTopic => create_topic(config, &config.topic(org, project)).await,
        Step::ClickhouseDatabase => {
            let topic = config.topic(org, project);
            let client = Client::new();
            for statement in project_statements(config, project, &topic) {
                clickhouse::execute_admin_query(
                    &client,
                    admin.url,
                    admin.user,
                    admin.password,
                    &statement,
                )
                .await?;
            }
            Ok(())
        }
        Step::ClickhouseUser => Ok(clickhouse::create_project_user(
            admin.url,
            admin.user,
            admin.password,
            &state.clickhouse_project_password,
            project,
        )
        .await?),
        Step::OpensearchIndex => Ok(state
            .opensearch
            .create_data_stream(org.as_ref(), project.as_ref())
            .await?),
    }
}

/// Drops `project`'s ClickHouse database (with its Kafka table and views),
/// its ClickHouse user and its topic, permanently deleting its events. Safe
/// to run for a project that has some or none of them.
///
/// # Errors
/// Every step is attempted even when an earlier one fails, so as much as
/// possible is removed; the first failure is returned.
pub async fn deprovision_project(
    config: &ProvisioningConfig,
    admin: &ClickHouseAdmin<'_>,
    org: &OrgId,
    project: &ProjectId,
) -> Result<(), ProvisionError> {
    // The database first: dropping its Kafka table stops ClickHouse
    // consuming before the topic disappears.
    let drop_database = format!("DROP DATABASE IF EXISTS `{}` SYNC", project.as_ref());
    let database = clickhouse::execute_admin_query(
        &Client::new(),
        admin.url,
        admin.user,
        admin.password,
        &drop_database,
    )
    .await
    .map(drop)
    .map_err(ProvisionError::from);
    let user = clickhouse::drop_project_user(admin.url, admin.user, admin.password, project)
        .await
        .map_err(ProvisionError::from);
    let topic = delete_topic(config, &config.topic(org, project)).await;
    database.and(user).and(topic)
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
            Ok(_) | Err((_, RDKafkaErrorCode::TopicAlreadyExists)) => {}
            Err((_, code)) => return Err(error(code.to_string())),
        }
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

/// `ckh/project.sql` for `project`, one statement per element.
fn project_statements(
    config: &ProvisioningConfig,
    project: &ProjectId,
    topic: &str,
) -> Vec<String> {
    let database = project.as_ref();
    let consumer_group = format!("clickhouse.{topic}");
    // Slugs and topics are restricted to characters that are safe inside
    // backticks and quotes; the broker list comes from configuration, so it
    // is escaped.
    let sql = strip_comments(PROJECT_SQL)
        .replace("{database}", database)
        .replace("{project_id}", database)
        .replace("{topic}", topic)
        .replace("{consumer_group}", &consumer_group)
        .replace(
            "{kafka_brokers}",
            &escape_sql_string(&config.clickhouse_kafka_brokers),
        );
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(str::to_owned)
        .collect()
}

fn strip_comments(sql: &str) -> String {
    sql.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Mirrors the events-collector's topic rendering (`kafka_connector.rs`).
fn render_topic(template: &str, org: &str, project: &str) -> String {
    template
        .replace(ORG_ID_PLACEHOLDER, org)
        .replace(PROJECT_ID_PLACEHOLDER, project)
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
            clickhouse_kafka_brokers: "redpanda:9092".to_owned(),
            topic_template: "events.{org_id}.{project_id}".to_owned(),
            topic_partitions: 1,
            topic_replication: 1,
        }
    }

    fn ids() -> (OrgId, ProjectId) {
        (
            OrgId::from_str("acme-k3x9qa").unwrap(),
            ProjectId::from_str("web-shop-9x2k1a").unwrap(),
        )
    }

    #[test]
    fn topic_renders_org_and_project() {
        let (org, project) = ids();
        assert_eq!(
            config().topic(&org, &project),
            "events.acme-k3x9qa.web-shop-9x2k1a"
        );
    }

    #[test]
    fn statements_fill_every_placeholder() {
        let (org, project) = ids();
        let config = config();
        let statements = project_statements(&config, &project, &config.topic(&org, &project));

        assert_eq!(
            statements[0],
            "CREATE DATABASE IF NOT EXISTS `web-shop-9x2k1a`"
        );
        assert!(statements.iter().all(|s| s.contains("IF NOT EXISTS")));
        for placeholder in [
            "{database}",
            "{project_id}",
            "{topic}",
            "{kafka_brokers}",
            "{consumer_group}",
        ] {
            assert!(
                statements.iter().all(|s| !s.contains(placeholder)),
                "{placeholder} left"
            );
        }
        let kafka = statements
            .iter()
            .find(|s| s.contains("ENGINE = Kafka"))
            .expect("a Kafka table");
        assert!(
            kafka.contains("kafka_topic_list           = 'events.acme-k3x9qa.web-shop-9x2k1a'")
        );
        assert!(kafka.contains(
            "kafka_group_name           = 'clickhouse.events.acme-k3x9qa.web-shop-9x2k1a'"
        ));
        assert!(kafka.contains("kafka_broker_list          = 'redpanda:9092'"));
        assert!(
            statements
                .iter()
                .filter(|s| s.contains("WHERE project_id"))
                .all(|s| s.contains("WHERE project_id = 'web-shop-9x2k1a'"))
        );
    }

    #[test]
    fn broker_list_is_escaped() {
        let (org, project) = ids();
        let mut config = config();
        config.clickhouse_kafka_brokers = "evil', x='y".to_owned();
        let statements = project_statements(&config, &project, &config.topic(&org, &project));
        assert!(statements.iter().any(|s| s.contains(r"'evil\', x=\'y'")));
    }

    /// Runs the steps, failing at `failing` (if any); returns the steps run.
    async fn run_failing_at(failing: Option<Step>) -> (Vec<Step>, Result<(), ProvisioningFailure>) {
        let mut ran = Vec::new();
        let outcome = run_steps(|step| {
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

    #[test]
    fn topic_validation_rejects_illegal_names() {
        assert!(validate_topic("events.acme-k3x9qa.web-shop-9x2k1a").is_ok());
        assert!(validate_topic("events/acme").is_err());
        assert!(validate_topic("..").is_err());
        assert!(validate_topic(&"a".repeat(250)).is_err());
    }
}
