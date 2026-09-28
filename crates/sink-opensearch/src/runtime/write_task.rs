//! Writes one partition's batch until every record is written or
//! dead-lettered, then reports back to the event loop.

use std::sync::Arc;

use bytes::Bytes;
use futures::future::join_all;
use metrics::{counter, histogram};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::backoff::Backoff;
use super::batch::Entry;
use super::dlq::Dlq;
use super::offsets::TopicPartition;
use super::{ItemOutcome, Rejection, Writer};

#[derive(Debug, PartialEq, Eq)]
pub enum BatchOutcome {
    /// Every record was written or dead-lettered; its offsets may be committed.
    Completed,
    /// Abandoned on revoke or when the shutdown grace period ran out.
    Cancelled,
}

#[derive(Debug)]
pub struct BatchResult {
    pub tp: TopicPartition,
    pub epoch: u64,
    pub last_offset: i64,
    pub outcome: BatchOutcome,
}

/// A record on its way to the dead letter queue.
#[derive(Debug)]
pub struct DeadLetter {
    pub offset: i64,
    pub key: Option<Bytes>,
    pub payload: Option<Bytes>,
    pub rejection: Rejection,
    /// Write attempts made before the record was rejected.
    pub attempts: u32,
}

pub struct WriteTask<W: Writer> {
    pub writer: Arc<W>,
    pub dlq: Dlq,
    pub semaphore: Arc<Semaphore>,
    pub backoff: Backoff,
    pub tp: TopicPartition,
    pub epoch: u64,
    pub last_offset: i64,
    pub entries: Vec<Entry<W::Doc>>,
    pub results: UnboundedSender<BatchResult>,
    pub token: CancellationToken,
}

impl<W: Writer> WriteTask<W> {
    pub async fn run(self) {
        let outcome = tokio::select! {
            () = self.token.cancelled() => BatchOutcome::Cancelled,
            () = self.process() => BatchOutcome::Completed,
        };

        // The receiver only disappears after shutdown, when nobody is left to
        // act on the result.
        let _ = self.results.send(BatchResult {
            tp: self.tp,
            epoch: self.epoch,
            last_offset: self.last_offset,
            outcome,
        });
    }

    async fn process(&self) {
        let letters = write_with_retry(
            self.writer.as_ref(),
            &self.entries,
            &self.semaphore,
            self.backoff,
            &self.tp,
        )
        .await;

        if !letters.is_empty() {
            dead_letter(&self.dlq, &self.tp, letters, self.backoff).await;
        }
    }
}

/// Writes every prepared document, retrying failed requests and failed items
/// with backoff until each one is written or rejected. Returns the records
/// that belong in the dead letter queue, in offset order.
pub async fn write_with_retry<W: Writer>(
    writer: &W,
    entries: &[Entry<W::Doc>],
    semaphore: &Semaphore,
    backoff: Backoff,
    tp: &TopicPartition,
) -> Vec<DeadLetter> {
    let mut letters = Vec::new();
    let mut pending = Vec::new();
    for entry in entries {
        match &entry.doc {
            Ok(doc) => pending.push((entry, doc.clone())),
            Err(rejection) => letters.push(dead_letter_for(entry, rejection.clone(), 0)),
        }
    }

    let mut attempt: u32 = 0;
    while !pending.is_empty() {
        attempt += 1;
        let docs = pending.iter().map(|(_, doc)| doc.clone()).collect();
        let result = {
            let _permit = semaphore.acquire().await.ok();
            let started = Instant::now();
            let result = writer.write(docs).await;
            histogram!("sink_bulk_duration_seconds").record(started.elapsed().as_secs_f64());
            result
        };

        let retry_reason = match result {
            Ok(outcomes) if outcomes.len() == pending.len() => {
                let mut retry = Vec::new();
                let mut reason = None;
                let mut alert = false;
                let mut written: u64 = 0;

                for ((entry, doc), outcome) in pending.into_iter().zip(outcomes) {
                    match outcome {
                        ItemOutcome::Done => written += 1,
                        ItemOutcome::Retry {
                            reason: item_reason,
                            alert: item_alert,
                        } => {
                            alert |= item_alert;
                            reason.get_or_insert(item_reason);
                            retry.push((entry, doc));
                        }
                        ItemOutcome::Reject(rejection) => {
                            letters.push(dead_letter_for(entry, rejection, attempt));
                        }
                    }
                }

                counter!("sink_docs_written_total").increment(written);
                if !retry.is_empty() {
                    counter!("sink_retries_total", "class" => "item").increment(retry.len() as u64);
                }
                pending = retry;
                reason.map(|reason| {
                    (
                        format!("{} documents failed: {reason}", pending.len()),
                        alert,
                    )
                })
            }
            Ok(outcomes) => {
                counter!("sink_retries_total", "class" => "request").increment(1);
                Some((
                    format!(
                        "writer returned {} results for {} documents",
                        outcomes.len(),
                        pending.len()
                    ),
                    false,
                ))
            }
            Err(error) => {
                counter!("sink_retries_total", "class" => "request").increment(1);
                Some((error.reason, error.alert))
            }
        };

        if let Some((reason, alert)) = retry_reason {
            let delay = backoff.delay(attempt);
            if alert {
                error!(topic = %tp.topic, partition = tp.partition, attempt, ?delay, %reason, "write failed; retrying");
            } else {
                warn!(topic = %tp.topic, partition = tp.partition, attempt, ?delay, %reason, "write failed; retrying");
            }
            tokio::time::sleep(delay).await;
        }
    }

    letters.sort_by_key(|letter| letter.offset);
    debug!(topic = %tp.topic, partition = tp.partition, records = entries.len(), rejected = letters.len(), "batch written");
    letters
}

fn dead_letter_for<D>(entry: &Entry<D>, rejection: Rejection, attempts: u32) -> DeadLetter {
    DeadLetter {
        offset: entry.offset,
        key: entry.key.clone(),
        payload: entry.payload.clone(),
        rejection,
        attempts,
    }
}

/// Sends every letter to the dead letter queue, retrying until Kafka confirms
/// each one. The partition makes no progress until this finishes.
async fn dead_letter(dlq: &Dlq, tp: &TopicPartition, letters: Vec<DeadLetter>, backoff: Backoff) {
    let mut remaining = letters;
    let mut attempt: u32 = 0;

    while !remaining.is_empty() {
        let results = join_all(remaining.iter().map(|letter| dlq.send(tp, letter))).await;

        let mut failed = Vec::new();
        let mut last_error = None;
        let mut sent = Vec::new();
        for (letter, result) in remaining.into_iter().zip(results) {
            match result {
                Ok(()) => {
                    counter!("sink_dlq_records_total", "class" => letter.rejection.class)
                        .increment(1);
                    sent.push(letter);
                }
                Err(error) => {
                    last_error = Some(error);
                    failed.push(letter);
                }
            }
        }

        if let Some(first) = sent.first() {
            info!(
                topic = %tp.topic,
                partition = tp.partition,
                records = sent.len(),
                first_offset = first.offset,
                class = first.rejection.class,
                reason = %first.rejection.reason,
                "sent records to the DLQ"
            );
        }
        if let Some(error) = last_error {
            attempt += 1;
            let delay = backoff.delay(attempt);
            error!(
                topic = %tp.topic,
                partition = tp.partition,
                records = failed.len(),
                %error,
                ?delay,
                "failed to write to the DLQ; the partition stays blocked until it succeeds"
            );
            tokio::time::sleep(delay).await;
        }
        remaining = failed;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;
    use crate::runtime::WriteError;

    type Script = VecDeque<Result<Vec<ItemOutcome>, WriteError>>;

    /// Returns scripted results and records which documents each call wrote.
    struct FakeWriter {
        script: Mutex<Script>,
        calls: Mutex<Vec<Vec<u32>>>,
    }

    impl FakeWriter {
        fn new(script: Vec<Result<Vec<ItemOutcome>, WriteError>>) -> Self {
            Self {
                script: Mutex::new(script.into()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<u32>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Writer for FakeWriter {
        type Doc = u32;

        fn prepare(&self, _payload: Option<&[u8]>) -> Result<u32, Rejection> {
            unreachable!("tests build entries directly")
        }

        fn doc_size(_doc: &u32) -> usize {
            1
        }

        fn write(
            &self,
            docs: Vec<u32>,
        ) -> impl Future<Output = Result<Vec<ItemOutcome>, WriteError>> + Send {
            let result = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(docs.iter().map(|_| ItemOutcome::Done).collect()));
            self.calls.lock().unwrap().push(docs);
            std::future::ready(result)
        }
    }

    fn entry(offset: i64, doc: Result<u32, Rejection>) -> Entry<u32> {
        Entry {
            offset,
            key: None,
            payload: Some(Bytes::from(format!("payload-{offset}"))),
            doc,
            size: 1,
            received: Instant::now(),
        }
    }

    fn retry(reason: &str) -> ItemOutcome {
        ItemOutcome::Retry {
            reason: reason.into(),
            alert: false,
        }
    }

    async fn write(writer: &FakeWriter, entries: &[Entry<u32>]) -> Vec<DeadLetter> {
        let backoff = Backoff::new(Duration::from_millis(1), Duration::from_millis(2));
        write_with_retry(
            writer,
            entries,
            &Semaphore::new(1),
            backoff,
            &TopicPartition::new("events", 0),
        )
        .await
    }

    #[tokio::test]
    async fn writes_all_documents_in_one_call() {
        let writer = FakeWriter::new(vec![]);
        let entries = vec![entry(0, Ok(10)), entry(1, Ok(11))];

        let letters = write(&writer, &entries).await;

        assert!(letters.is_empty());
        assert_eq!(writer.calls(), vec![vec![10, 11]]);
    }

    #[tokio::test]
    async fn retries_the_whole_request_after_a_request_error() {
        let writer = FakeWriter::new(vec![Err(WriteError {
            reason: "HTTP 503".into(),
            alert: false,
        })]);
        let entries = vec![entry(0, Ok(10)), entry(1, Ok(11))];

        let letters = write(&writer, &entries).await;

        assert!(letters.is_empty());
        assert_eq!(writer.calls(), vec![vec![10, 11], vec![10, 11]]);
    }

    #[tokio::test]
    async fn resends_only_failed_items_and_dead_letters_rejects() {
        let writer = FakeWriter::new(vec![Ok(vec![
            ItemOutcome::Done,
            retry("429"),
            ItemOutcome::Reject(
                Rejection::new("rejected", "mapper_parsing_exception").with_status(400),
            ),
        ])]);
        let entries = vec![entry(0, Ok(10)), entry(1, Ok(11)), entry(2, Ok(12))];

        let letters = write(&writer, &entries).await;

        assert_eq!(writer.calls(), vec![vec![10, 11, 12], vec![11]]);
        assert_eq!(letters.len(), 1);
        assert_eq!(letters[0].offset, 2);
        assert_eq!(letters[0].attempts, 1);
        assert_eq!(letters[0].rejection.status, Some(400));
        assert_eq!(letters[0].payload.as_deref(), Some(&b"payload-2"[..]));
    }

    #[tokio::test]
    async fn keeps_retrying_until_items_succeed() {
        let writer = FakeWriter::new(vec![
            Ok(vec![retry("429")]),
            Ok(vec![retry("429")]),
            Ok(vec![retry("503")]),
        ]);
        let entries = vec![entry(0, Ok(10))];

        let letters = write(&writer, &entries).await;

        assert!(letters.is_empty());
        assert_eq!(writer.calls().len(), 4);
    }

    #[tokio::test]
    async fn records_rejected_before_writing_are_dead_lettered_without_a_write() {
        let writer = FakeWriter::new(vec![]);
        let entries = vec![
            entry(0, Err(Rejection::new("decode", "not a canonical event"))),
            entry(1, Ok(11)),
        ];

        let letters = write(&writer, &entries).await;

        assert_eq!(writer.calls(), vec![vec![11]]);
        assert_eq!(letters.len(), 1);
        assert_eq!(letters[0].offset, 0);
        assert_eq!(letters[0].attempts, 0);
        assert_eq!(letters[0].rejection.class, "decode");
    }

    #[tokio::test]
    async fn a_batch_of_only_rejects_makes_no_write() {
        let writer = FakeWriter::new(vec![]);
        let entries = vec![entry(0, Err(Rejection::new("decode", "empty payload")))];

        let letters = write(&writer, &entries).await;

        assert!(writer.calls().is_empty());
        assert_eq!(letters.len(), 1);
    }

    #[tokio::test]
    async fn retries_when_the_writer_returns_the_wrong_number_of_results() {
        let writer = FakeWriter::new(vec![Ok(vec![ItemOutcome::Done])]);
        let entries = vec![entry(0, Ok(10)), entry(1, Ok(11))];

        let letters = write(&writer, &entries).await;

        assert!(letters.is_empty());
        assert_eq!(writer.calls(), vec![vec![10, 11], vec![10, 11]]);
    }

    #[tokio::test]
    async fn dead_letters_are_returned_in_offset_order() {
        let writer = FakeWriter::new(vec![Ok(vec![
            ItemOutcome::Reject(Rejection::new("rejected", "bad")),
            ItemOutcome::Done,
        ])]);
        let entries = vec![
            entry(5, Ok(15)),
            entry(6, Ok(16)),
            entry(7, Err(Rejection::new("decode", "bad"))),
        ];

        let letters = write(&writer, &entries).await;

        let offsets: Vec<i64> = letters.iter().map(|letter| letter.offset).collect();
        assert_eq!(offsets, vec![5, 7]);
    }
}
