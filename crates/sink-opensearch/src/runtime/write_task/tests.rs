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

    async fn prepare(&self, _payload: Option<&[u8]>) -> Result<u32, Rejection> {
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
async fn documents_already_written_are_not_retried() {
    let writer = FakeWriter::new(vec![Ok(vec![
        ItemOutcome::AlreadyWritten,
        ItemOutcome::Done,
    ])]);
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
