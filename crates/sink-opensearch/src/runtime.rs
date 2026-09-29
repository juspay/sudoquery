//! Sink-agnostic consumer runtime.
//!
//! Consumes Kafka partitions, buffers records per partition, hands batches to a
//! [`Writer`], and commits a partition's offsets only after every record in a
//! batch is written or confirmed in the dead letter queue. Nothing here knows
//! about OpenSearch; this module moves to `sq-sink-runtime` once a second sink
//! needs it.
//!
//! Each partition has at most one batch in flight, and a batch finishes only
//! when all its records are resolved, so offsets always complete in order.

mod backoff;
mod batch;
mod context;
mod dlq;
mod offsets;
mod write_task;

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use metrics::{counter, gauge};
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::error::{KafkaError, KafkaResult};
use rdkafka::message::{BorrowedMessage, Message};
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::{BatchConfig, RetryConfig};
use crate::health::Health;
use backoff::Backoff;
use batch::{Entry, PartitionBuffer};
use context::SinkContext;
use dlq::Dlq;
use offsets::{OffsetTracker, TopicPartition, lock};
use write_task::{BatchOutcome, BatchResult, WriteTask};

/// Why a record can't be written. It goes to the dead letter queue.
#[derive(Clone, Debug)]
pub struct Rejection {
    /// Short, fixed category, used as a metric label and DLQ header.
    pub class: &'static str,
    pub reason: String,
    pub status: Option<u16>,
}

impl Rejection {
    pub fn new(class: &'static str, reason: impl Into<String>) -> Self {
        Self {
            class,
            reason: reason.into(),
            status: None,
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }
}

/// The result for one document in a write.
#[derive(Debug)]
pub enum ItemOutcome {
    Done,
    /// Temporary failure; the document is written again after a backoff.
    /// `alert` marks failures that need an operator, such as a missing index.
    Retry {
        reason: String,
        alert: bool,
    },
    Reject(Rejection),
}

/// The whole write failed and is retried after a backoff.
#[derive(Debug)]
pub struct WriteError {
    pub reason: String,
    /// Logged at error level: retrying alone won't fix it.
    pub alert: bool,
}

/// The destination-specific half of a sink.
pub trait Writer: Send + Sync + 'static {
    type Doc: Clone + Send + Sync + 'static;

    /// Turns a record's payload into a document, or rejects it. May look up
    /// settings for the record, such as its tenant's index.
    fn prepare(
        &self,
        payload: Option<&[u8]>,
    ) -> impl Future<Output = Result<Self::Doc, Rejection>> + Send;

    /// Bytes the document adds to a write request.
    fn doc_size(doc: &Self::Doc) -> usize;

    /// Writes the documents. On success, returns one outcome per document, in
    /// the same order.
    fn write(
        &self,
        docs: Vec<Self::Doc>,
    ) -> impl Future<Output = Result<Vec<ItemOutcome>, WriteError>> + Send;
}

pub struct RuntimeConfig {
    pub topics: Vec<String>,
    pub group_id: String,
    pub client_config: HashMap<String, String>,
    pub batch: BatchConfig,
    pub retry: RetryConfig,
    pub dlq_topic: String,
    pub commit_interval: Duration,
    pub shutdown_grace: Duration,
}

/// How shutdown went.
#[derive(Debug, PartialEq, Eq)]
pub enum Drained {
    /// Every consumed record was written or dead-lettered, and committed.
    Complete,
    /// The grace period ran out; unwritten records are replayed on restart.
    Incomplete,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("failed to set up Kafka: {0}")]
    Kafka(#[from] KafkaError),

    #[error("fatal consumer error: {0}")]
    FatalConsumer(KafkaError),
}

/// Runs the sink until `shutdown` is cancelled or the consumer fails fatally.
pub async fn run<W: Writer>(
    config: RuntimeConfig,
    writer: W,
    health: Arc<Health>,
    shutdown: CancellationToken,
) -> Result<Drained, RuntimeError> {
    let tracker = Arc::new(Mutex::new(OffsetTracker::default()));
    let consumer: StreamConsumer<SinkContext> =
        consumer_config(&config).create_with_context(SinkContext::new(Arc::clone(&tracker)))?;
    let topics: Vec<&str> = config.topics.iter().map(String::as_str).collect();
    consumer.subscribe(&topics)?;
    let dlq = Dlq::new(&config.client_config, &config.dlq_topic)?;

    info!(topics = ?config.topics, group_id = %config.group_id, dlq_topic = %config.dlq_topic, "sink started");

    let (results_tx, mut results_rx) = mpsc::unbounded_channel();
    let mut event_loop = EventLoop {
        backoff: Backoff::from_config(&config.retry),
        semaphore: Arc::new(Semaphore::new(config.batch.max_in_flight)),
        config,
        writer: Arc::new(writer),
        consumer: Arc::new(consumer),
        tracker,
        dlq,
        results_tx,
        partitions: HashMap::new(),
        abort: CancellationToken::new(),
        health,
        last_commit: Instant::now(),
        draining: false,
    };

    let result = event_loop.consume(&shutdown, &mut results_rx).await;
    let drained = match &result {
        Ok(()) => event_loop.drain(&mut results_rx).await,
        Err(error) => {
            error!(%error, "stopping after a fatal error");
            event_loop.abort.cancel();
            Drained::Incomplete
        }
    };
    event_loop.close().await;

    result.map(|()| drained)
}

fn consumer_config(config: &RuntimeConfig) -> ClientConfig {
    let mut client_config = ClientConfig::new();
    client_config.set("auto.offset.reset", "earliest");
    for (key, value) in &config.client_config {
        client_config.set(key, value);
    }
    client_config
        .set("group.id", &config.group_id)
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false");
    client_config
}

/// A consumed message, copied out of librdkafka's buffer so it can be held
/// across an `.await`.
struct Record {
    tp: TopicPartition,
    offset: i64,
    key: Option<Bytes>,
    payload: Option<Bytes>,
}

impl Record {
    fn copy_from(message: &BorrowedMessage<'_>) -> Self {
        Self {
            tp: TopicPartition::new(message.topic(), message.partition()),
            offset: message.offset(),
            key: message.key().map(Bytes::copy_from_slice),
            payload: message.payload().map(Bytes::copy_from_slice),
        }
    }
}

struct PartitionState<D> {
    epoch: u64,
    buffer: PartitionBuffer<D>,
    /// Cancels the partition's in-flight batch, if there is one.
    in_flight: Option<CancellationToken>,
    paused: bool,
}

impl<D> PartitionState<D> {
    fn new(epoch: u64) -> Self {
        Self {
            epoch,
            buffer: PartitionBuffer::default(),
            in_flight: None,
            paused: false,
        }
    }

    fn cancel_in_flight(&mut self) {
        if let Some(token) = self.in_flight.take() {
            token.cancel();
        }
    }
}

/// Owns every partition's buffer and in-flight state. Only the offset tracker
/// is shared, with the consumer callbacks.
struct EventLoop<W: Writer> {
    config: RuntimeConfig,
    writer: Arc<W>,
    consumer: Arc<StreamConsumer<SinkContext>>,
    tracker: Arc<Mutex<OffsetTracker>>,
    dlq: Dlq,
    backoff: Backoff,
    semaphore: Arc<Semaphore>,
    results_tx: UnboundedSender<BatchResult>,
    partitions: HashMap<TopicPartition, PartitionState<W::Doc>>,
    /// Cancels every in-flight batch when the shutdown grace period runs out.
    abort: CancellationToken,
    health: Arc<Health>,
    last_commit: Instant,
    /// Set during shutdown: flush buffers without waiting for `linger`.
    draining: bool,
}

impl<W: Writer> EventLoop<W> {
    async fn consume(
        &mut self,
        shutdown: &CancellationToken,
        results_rx: &mut UnboundedReceiver<BatchResult>,
    ) -> Result<(), RuntimeError> {
        let consumer = Arc::clone(&self.consumer);
        let mut tick = tokio::time::interval(tick_period(&self.config.batch));
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            let record = tokio::select! {
                biased;
                () = shutdown.cancelled() => return Ok(()),
                Some(result) = results_rx.recv() => {
                    self.on_result(result);
                    continue;
                }
                _ = tick.tick() => {
                    self.on_tick();
                    continue;
                }
                message = consumer.recv() => match message {
                    Ok(message) => Record::copy_from(&message),
                    Err(KafkaError::MessageConsumptionFatal(code)) => {
                        return Err(RuntimeError::FatalConsumer(KafkaError::MessageConsumptionFatal(code)));
                    }
                    Err(error) => {
                        warn!(%error, "consumer error");
                        continue;
                    }
                },
            };
            self.on_record(record).await;
        }
    }

    async fn on_record(&mut self, record: Record) {
        let Record {
            tp,
            offset,
            key,
            payload,
        } = record;
        let Some(epoch) = lock(&self.tracker).epoch(&tp) else {
            debug!(topic = %tp.topic, partition = tp.partition, "skipping a record for a partition that is no longer assigned");
            return;
        };
        counter!("sink_records_consumed_total", "topic" => tp.topic.clone()).increment(1);

        let doc = self.writer.prepare(payload.as_deref()).await;
        if let Err(rejection) = &doc {
            debug!(topic = %tp.topic, partition = tp.partition, offset, class = rejection.class, reason = %rejection.reason, "record rejected before writing");
        }
        let entry = Entry {
            offset,
            key,
            payload,
            size: doc.as_ref().map_or(0, W::doc_size),
            doc,
            received: Instant::now(),
        };

        let state = self
            .partitions
            .entry(tp.clone())
            .or_insert_with(|| PartitionState::new(epoch));
        if state.epoch != epoch {
            // Revoked and assigned again since this state was created.
            state.cancel_in_flight();
            *state = PartitionState::new(epoch);
        }
        state.buffer.push(entry);

        self.flush_if_ready(&tp);
        self.pause_if_full(&tp);
    }

    fn on_result(&mut self, result: BatchResult) {
        let Some(state) = self.partitions.get_mut(&result.tp) else {
            return;
        };
        if state.epoch != result.epoch {
            return;
        }
        state.in_flight = None;
        if result.outcome == BatchOutcome::Completed {
            lock(&self.tracker).complete(&result.tp, result.epoch, result.last_offset);
        }

        self.flush_if_ready(&result.tp);
        self.resume_if_drained(&result.tp);
    }

    fn on_tick(&mut self) {
        self.health.tick();
        self.forget_revoked_partitions();

        let partitions: Vec<TopicPartition> = self.partitions.keys().cloned().collect();
        for tp in &partitions {
            self.flush_if_ready(tp);
        }

        let paused = self
            .partitions
            .values()
            .filter(|state| state.paused)
            .count();
        gauge!("sink_partitions_paused").set(paused as f64);

        if self.last_commit.elapsed() >= self.config.commit_interval {
            self.commit_async();
            self.last_commit = Instant::now();
        }
    }

    /// Drops state for partitions the consumer callbacks have revoked, and
    /// abandons their in-flight batches.
    fn forget_revoked_partitions(&mut self) {
        let tracker = lock(&self.tracker);
        self.partitions.retain(|tp, state| {
            let assigned = tracker.epoch(tp) == Some(state.epoch);
            if !assigned {
                state.cancel_in_flight();
            }
            assigned
        });
    }

    /// Starts a write for the partition if it has no batch in flight and its
    /// buffer is ready.
    fn flush_if_ready(&mut self, tp: &TopicPartition) {
        let Some(state) = self.partitions.get_mut(tp) else {
            return;
        };
        if state.in_flight.is_some() || state.buffer.is_empty() {
            return;
        }
        if !self.draining && !state.buffer.is_ready(Instant::now(), &self.config.batch) {
            return;
        }

        let entries = state.buffer.take_batch(&self.config.batch);
        let Some(last_offset) = entries.last().map(|entry| entry.offset) else {
            return;
        };
        let token = self.abort.child_token();
        state.in_flight = Some(token.clone());

        let task = WriteTask {
            writer: Arc::clone(&self.writer),
            dlq: self.dlq.clone(),
            semaphore: Arc::clone(&self.semaphore),
            backoff: self.backoff,
            tp: tp.clone(),
            epoch: state.epoch,
            last_offset,
            entries,
            results: self.results_tx.clone(),
            token,
        };
        tokio::spawn(task.run());
    }

    /// Stops fetching a partition whose buffer is full while its batch is
    /// still being written, which bounds memory during an outage.
    fn pause_if_full(&mut self, tp: &TopicPartition) {
        let Some(state) = self.partitions.get_mut(tp) else {
            return;
        };
        if state.paused || state.in_flight.is_none() || !state.buffer.is_full(&self.config.batch) {
            return;
        }
        match self.consumer.pause(&single_partition(tp)) {
            Ok(()) => {
                state.paused = true;
                debug!(topic = %tp.topic, partition = tp.partition, "paused partition");
            }
            Err(error) => {
                warn!(%error, topic = %tp.topic, partition = tp.partition, "failed to pause partition")
            }
        }
    }

    fn resume_if_drained(&mut self, tp: &TopicPartition) {
        let Some(state) = self.partitions.get_mut(tp) else {
            return;
        };
        if !state.paused || state.buffer.is_full(&self.config.batch) {
            return;
        }
        match self.consumer.resume(&single_partition(tp)) {
            Ok(()) => {
                state.paused = false;
                debug!(topic = %tp.topic, partition = tp.partition, "resumed partition");
            }
            Err(error) => {
                warn!(%error, topic = %tp.topic, partition = tp.partition, "failed to resume partition")
            }
        }
    }

    /// Starts committing advanced offsets. Confirmation arrives through the
    /// consumer's commit callback; failures are retried on the next tick.
    fn commit_async(&self) {
        let pending = lock(&self.tracker).pending_commits();
        if pending.is_empty() {
            return;
        }
        let result = commit_list(&pending)
            .and_then(|offsets| self.consumer.commit(&offsets, CommitMode::Async));
        if let Err(error) = result {
            warn!(%error, "failed to start an offset commit");
        }
    }

    /// Writes out every buffered record, waiting at most the grace period.
    async fn drain(&mut self, results_rx: &mut UnboundedReceiver<BatchResult>) -> Drained {
        self.draining = true;
        let deadline = Instant::now() + self.config.shutdown_grace;
        info!(grace = ?self.config.shutdown_grace, "shutting down: writing buffered records");

        let partitions: Vec<TopicPartition> = self.partitions.keys().cloned().collect();
        for tp in &partitions {
            self.flush_if_ready(tp);
        }

        loop {
            let idle = self
                .partitions
                .values()
                .all(|state| state.in_flight.is_none() && state.buffer.is_empty());
            if idle {
                info!("all consumed records are written");
                return Drained::Complete;
            }

            tokio::select! {
                Some(result) = results_rx.recv() => self.on_result(result),
                () = tokio::time::sleep_until(deadline) => break,
            }
        }

        self.abort.cancel();
        warn!(
            "shutdown grace period expired; records that were not written will be replayed on restart"
        );
        Drained::Incomplete
    }

    /// Commits final offsets, leaves the consumer group and flushes the DLQ.
    async fn close(self) {
        let Self {
            consumer,
            tracker,
            dlq,
            ..
        } = self;
        let pending = lock(&tracker).pending_commits();

        // Committing synchronously and closing the consumer both block.
        let closing = tokio::task::spawn_blocking(move || {
            if !pending.is_empty() {
                let result = commit_list(&pending)
                    .and_then(|offsets| consumer.commit(&offsets, CommitMode::Sync));
                match result {
                    Ok(()) => {
                        let mut tracker = lock(&tracker);
                        for (tp, offset) in &pending {
                            tracker.mark_committed(tp, *offset);
                        }
                        info!(partitions = pending.len(), "committed final offsets");
                    }
                    Err(error) => error!(
                        %error,
                        "final offset commit failed; records since the last commit will be replayed on restart"
                    ),
                }
            }

            // Dropping the last handle closes the consumer: it leaves the
            // group, which runs the revoke callback.
            drop(consumer);
            if let Err(error) = dlq.flush(Duration::from_secs(10)) {
                warn!(%error, "failed to flush the DLQ producer");
            }
        });

        if let Err(error) = closing.await {
            error!(%error, "shutdown task failed");
        }
    }
}

/// How often buffers are checked for `linger` and commits are considered.
fn tick_period(batch: &BatchConfig) -> Duration {
    batch
        .linger()
        .clamp(Duration::from_millis(10), Duration::from_millis(100))
}

fn single_partition(tp: &TopicPartition) -> TopicPartitionList {
    let mut list = TopicPartitionList::new();
    list.add_partition(&tp.topic, tp.partition);
    list
}

fn commit_list(pending: &[(TopicPartition, i64)]) -> KafkaResult<TopicPartitionList> {
    let mut list = TopicPartitionList::new();
    for (tp, offset) in pending {
        list.add_partition_offset(&tp.topic, tp.partition, Offset::Offset(*offset))?;
    }
    Ok(list)
}
