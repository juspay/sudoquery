//! Kafka access for the collector suite: creates per-test topics, reads
//! them back from the beginning, and deletes them afterwards.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, bail};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer, StreamConsumer};
use rdkafka::error::RDKafkaErrorCode;
use rdkafka::{Message as _, Offset, TopicPartitionList};
use serde_json::Value;
use tokio::time::Instant;

/// Brokers used when `KAFKA_BOOTSTRAP_SERVERS` is unset: the compose Redpanda.
const DEFAULT_BOOTSTRAP_SERVERS: &str = "localhost:19092";
const ADMIN_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait for expected messages to arrive.
const ARRIVAL_TIMEOUT: Duration = Duration::from_secs(15);
/// How long to keep listening after the expected messages, to catch extras.
const LINGER: Duration = Duration::from_millis(500);
/// How long a topic must stay empty to count as "nothing was published".
const QUIET_PERIOD: Duration = Duration::from_secs(3);

/// A consumed record: its key and JSON payload.
pub(crate) struct Record {
    pub(crate) key: Option<String>,
    pub(crate) value: Value,
}

pub(crate) struct Kafka {
    bootstrap_servers: String,
    admin: AdminClient<DefaultClientContext>,
    created: Mutex<Vec<String>>,
}

impl Kafka {
    /// Connects to `KAFKA_BOOTSTRAP_SERVERS` (default: the compose Redpanda)
    /// and checks the brokers answer.
    pub(crate) async fn connect() -> anyhow::Result<Self> {
        let bootstrap_servers = std::env::var("KAFKA_BOOTSTRAP_SERVERS")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BOOTSTRAP_SERVERS.to_owned());

        let probe: BaseConsumer = client_config(&bootstrap_servers).create()?;
        tokio::task::spawn_blocking(move || probe.fetch_metadata(None, ADMIN_TIMEOUT))
            .await?
            .with_context(|| {
                format!(
                    "Kafka at {bootstrap_servers} is unreachable \
                     (hint: `cargo xtask setup redpanda`)"
                )
            })?;
        println!("e2e kafka: {bootstrap_servers}");

        Ok(Self {
            admin: client_config(&bootstrap_servers).create()?,
            bootstrap_servers,
            created: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn bootstrap_servers(&self) -> &str {
        &self.bootstrap_servers
    }

    /// Creates a single-partition topic, remembered for [`Self::delete_created`].
    pub(crate) async fn create_topic(&self, name: &str) -> anyhow::Result<()> {
        let results = self
            .admin
            .create_topics(
                &[NewTopic::new(name, 1, TopicReplication::Fixed(1))],
                &AdminOptions::new().operation_timeout(Some(ADMIN_TIMEOUT)),
            )
            .await
            .with_context(|| format!("failed to create topic {name}"))?;
        for result in results {
            match result {
                Ok(_) | Err((_, RDKafkaErrorCode::TopicAlreadyExists)) => {}
                Err((topic, code)) => bail!("failed to create topic {topic}: {code}"),
            }
        }
        self.created.lock().unwrap().push(name.to_owned());
        Ok(())
    }

    /// Deletes every topic created by this run.
    pub(crate) async fn delete_created(&self) -> anyhow::Result<()> {
        let topics = std::mem::take(&mut *self.created.lock().unwrap());
        if topics.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = topics.iter().map(String::as_str).collect();
        let results = self
            .admin
            .delete_topics(
                &names,
                &AdminOptions::new().operation_timeout(Some(ADMIN_TIMEOUT)),
            )
            .await
            .context("failed to delete e2e topics")?;
        let failed: Vec<String> = results
            .into_iter()
            .filter_map(|result| result.err())
            .map(|(topic, code)| format!("{topic} ({code})"))
            .collect();
        if !failed.is_empty() {
            bail!("failed to delete e2e topics: {}", failed.join(", "));
        }
        Ok(())
    }

    /// Reads `topic` from the beginning until `expected` records arrived (plus
    /// a short linger to catch extras), or for a quiet period when `expected`
    /// is 0. Returns everything read; callers assert on the count.
    pub(crate) async fn read(&self, topic: &str, expected: usize) -> anyhow::Result<Vec<Record>> {
        let consumer: StreamConsumer = client_config(&self.bootstrap_servers)
            .set("group.id", format!("xtask-e2e-{}", uuid::Uuid::new_v4()))
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()?;
        let mut partitions = TopicPartitionList::new();
        partitions.add_partition_offset(topic, 0, Offset::Beginning)?;
        consumer.assign(&partitions)?;

        let mut deadline = Instant::now()
            + if expected == 0 {
                QUIET_PERIOD
            } else {
                ARRIVAL_TIMEOUT
            };
        let mut records = Vec::new();
        while let Ok(message) = tokio::time::timeout_at(deadline, consumer.recv()).await {
            let message = message.with_context(|| format!("failed to read {topic}"))?;
            let payload = message.payload().unwrap_or_default();
            records.push(Record {
                key: message
                    .key()
                    .map(|key| String::from_utf8_lossy(key).into_owned()),
                value: serde_json::from_slice(payload).with_context(|| {
                    format!(
                        "non-JSON record on {topic}: {}",
                        String::from_utf8_lossy(payload)
                    )
                })?,
            });
            if records.len() == expected {
                deadline = deadline.min(Instant::now() + LINGER);
            }
        }
        Ok(records)
    }
}

fn client_config(bootstrap_servers: &str) -> ClientConfig {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", bootstrap_servers);
    config
}
