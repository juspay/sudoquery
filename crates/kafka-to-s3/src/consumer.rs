//! rdkafka consumer wiring (D14, D16) and the engine source adapter.

use crate::buffer::{CommitMap, Consumed};
use crate::config::Config;
use crate::engine::Source;
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, StreamConsumer};
use rdkafka::error::{KafkaError, KafkaResult};
use rdkafka::message::Message;
use rdkafka::topic_partition_list::{Offset, TopicPartitionList};
use rdkafka::{ClientConfig, ClientContext};
use std::time::Duration;

/// Pause between receive retries so a broken broker cannot hot-loop the engine.
const RECV_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Consumer setup or commit failure.
#[derive(Debug, thiserror::Error)]
pub enum ConsumerError {
    #[error("failed to create Kafka consumer: {0}")]
    Create(#[from] KafkaError),
    #[error("failed to subscribe to {topics:?}: {source}")]
    Subscribe {
        topics: Vec<String>,
        source: KafkaError,
    },
    #[error("failed to commit offsets: {0}")]
    Commit(KafkaError),
}

/// Consumer context that surfaces async commit failures as warnings (D14:
/// best-effort commits, dupes accepted over blocking the pipeline).
#[derive(Debug)]
pub struct ArchiverContext;

impl ClientContext for ArchiverContext {}

impl ConsumerContext for ArchiverContext {
    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        if let Err(error) = result {
            tracing::warn!(%error, "async offset commit failed (best-effort; events may be reprocessed)");
        }
    }
}

pub struct EventConsumer {
    inner: StreamConsumer<ArchiverContext>,
}

impl EventConsumer {
    /// Create and subscribe (D16): manual commits, `earliest` reset, ALL topics.
    ///
    /// # Errors
    /// Returns a [`ConsumerError`] when client creation or subscription fails.
    pub fn new(config: &Config) -> Result<Self, ConsumerError> {
        let inner: StreamConsumer<ArchiverContext> = ClientConfig::new()
            .set("bootstrap.servers", config.kafka_bootstrap_servers.as_str())
            .set("group.id", config.kafka_group_id.as_str())
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("enable.partition.eof", "false")
            .create_with_context(ArchiverContext)?;
        let topics: Vec<&str> = config.kafka_topics.iter().map(String::as_str).collect();
        inner
            .subscribe(&topics)
            .map_err(|source| ConsumerError::Subscribe {
                topics: config.kafka_topics.clone(),
                source,
            })?;
        Ok(Self { inner })
    }

    /// Receive the next consumed message with its payload bytes verbatim (D7).
    ///
    /// # Errors
    /// Returns the rdkafka error; transient failures are retried by [`KafkaSource`].
    pub async fn recv(&self) -> Result<Consumed, KafkaError> {
        let message = self.inner.recv().await?;
        Ok(Consumed {
            topic: message.topic().to_owned(),
            partition: message.partition(),
            offset: message.offset(),
            payload: message.payload().map(<[u8]>::to_vec).unwrap_or_default(),
        })
    }

    /// Enqueue an async commit of next-offsets, best-effort (D14); failures are
    /// logged by [`ArchiverContext::commit_callback`].
    ///
    /// # Errors
    /// Returns a [`ConsumerError`] when the commit cannot be enqueued.
    pub fn commit_async(&self, commits: &CommitMap) -> Result<(), ConsumerError> {
        self.commit(commits, CommitMode::Async)
    }

    /// Commit next-offsets synchronously (D15 shutdown path).
    ///
    /// # Errors
    /// Returns a [`ConsumerError`] when the commit fails or cannot be enqueued.
    pub fn commit_sync(&self, commits: &CommitMap) -> Result<(), ConsumerError> {
        self.commit(commits, CommitMode::Sync)
    }

    fn commit(&self, commits: &CommitMap, mode: CommitMode) -> Result<(), ConsumerError> {
        let mut tpl = TopicPartitionList::new();
        for ((topic, partition), next_offset) in commits {
            tpl.add_partition_offset(topic, *partition, Offset::Offset(*next_offset))
                .map_err(ConsumerError::Commit)?;
        }
        self.inner.commit(&tpl, mode).map_err(ConsumerError::Commit)
    }
}

/// Engine source over the Kafka consumer: retries transient receive failures and
/// never yields `None` (only shutdown or source end stops the engine).
pub struct KafkaSource<'a> {
    consumer: &'a EventConsumer,
}

impl<'a> KafkaSource<'a> {
    pub fn new(consumer: &'a EventConsumer) -> Self {
        Self { consumer }
    }
}

impl Source for KafkaSource<'_> {
    async fn recv(&mut self) -> Option<Consumed> {
        loop {
            match self.consumer.recv().await {
                Ok(consumed) => return Some(consumed),
                Err(error) => {
                    tracing::error!(%error, "kafka receive failed; retrying");
                    tokio::time::sleep(RECV_RETRY_DELAY).await;
                }
            }
        }
    }
}
