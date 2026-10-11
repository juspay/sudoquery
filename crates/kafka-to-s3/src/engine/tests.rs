use super::*;
use crate::buffer::Entry;
use std::time::Duration;
use tokio::sync::mpsc;

fn event(id: u32) -> String {
    format!(
        r#"{{"id":"{id}","org_id":"o1","project_id":"p1","occured_at":"2026-09-24T10:00:00Z","arrived_at":"2026-09-24T10:00:01Z"}}"#
    )
}

fn consumed(offset: i64, payload: &str) -> Consumed {
    Consumed {
        topic: "events".to_string(),
        partition: 0,
        offset,
        payload: payload.as_bytes().to_vec(),
    }
}

struct ScriptedSource {
    rx: mpsc::UnboundedReceiver<Consumed>,
}

impl Source for ScriptedSource {
    async fn recv(&mut self) -> Option<Consumed> {
        self.rx.recv().await
    }
}

enum Recorded {
    Cycle(Batch),
    Final(Batch),
}

struct RecordingSink {
    tx: mpsc::UnboundedSender<Recorded>,
}

impl Sink for RecordingSink {
    async fn flush(&mut self, batch: Batch) -> Result<(), FlushError> {
        // Send fails only after the test has stopped reading; safe to drop.
        let _ = self.tx.send(Recorded::Cycle(batch));
        Ok(())
    }

    async fn flush_final(&mut self, batch: Batch) -> Result<(), FlushError> {
        let _ = self.tx.send(Recorded::Final(batch));
        Ok(())
    }
}

struct Harness {
    source: mpsc::UnboundedSender<Consumed>,
    records: mpsc::UnboundedReceiver<Recorded>,
    shutdown: watch::Sender<bool>,
    engine: tokio::task::JoinHandle<Result<(), FlushError>>,
}

fn spawn_engine(max_events: usize, interval_secs: u64) -> Harness {
    let (source_tx, source_rx) = mpsc::unbounded_channel();
    let (record_tx, record_rx) = mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let policy = WindowPolicy {
        max_events,
        interval: Duration::from_secs(interval_secs),
    };
    let engine = tokio::spawn(run_engine(
        ScriptedSource { rx: source_rx },
        RecordingSink { tx: record_tx },
        policy,
        shutdown_rx,
    ));
    Harness {
        source: source_tx,
        records: record_rx,
        shutdown: shutdown_tx,
        engine,
    }
}

impl Harness {
    fn send(&self, offset: i64, payload: &str) {
        self.source.send(consumed(offset, payload)).unwrap();
    }

    fn send_event(&self, offset: i64, id: u32) {
        self.send(offset, &event(id));
    }

    async fn next_cycle(&mut self) -> Batch {
        match self.records.recv().await.unwrap() {
            Recorded::Cycle(batch) => batch,
            Recorded::Final(batch) => panic!("expected cycle flush, got final: {batch:?}"),
        }
    }

    fn assert_no_more_records(&mut self) {
        assert!(
            self.records.try_recv().is_err(),
            "expected no further flush records"
        );
    }

    /// Asserts (not just signals): panics if any record was flushed after shutdown.
    async fn finish(mut self) {
        self.shutdown.send(true).unwrap();
        self.engine.await.unwrap().unwrap();
        assert!(
            self.records.try_recv().is_err(),
            "expected no records after shutdown"
        );
    }
}

/// Single-partition tests: the sole commit entry, asserted to be
/// `("events", 0) -> value`.
fn commit_of(batch: &Batch) -> i64 {
    assert_eq!(batch.commits.len(), 1, "{:?}", batch.commits);
    let (key, next) = batch.commits.iter().next().unwrap();
    assert_eq!(key, &("events".to_string(), 0));
    *next
}

fn cycle_ids(batch: &Batch) -> Vec<String> {
    batch
        .groups
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(id_of))
        .collect()
}

/// Extract the `id` field from a raw JSON entry for ordering assertions.
fn id_of(entry: &Entry) -> String {
    let value: serde_json::Value = serde_json::from_slice(&entry.raw).unwrap();
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_string()
}

#[tokio::test(start_paused = true)]
async fn n_trigger_flushes_exactly_n_in_fifo_order() {
    let mut harness = spawn_engine(3, 60);

    for id in 0..3u32 {
        harness.send_event(i64::from(id), id);
    }
    let batch = harness.next_cycle().await;

    assert_eq!(cycle_ids(&batch), vec!["0", "1", "2"]);
    assert_eq!(commit_of(&batch), 3);
    harness.assert_no_more_records();
    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn surplus_carries_over_and_drains_in_consecutive_cycles() {
    let mut harness = spawn_engine(2, 60);

    for id in 0..5u32 {
        harness.send_event(i64::from(id), id);
    }
    let first = harness.next_cycle().await;
    assert_eq!(cycle_ids(&first), vec!["0", "1"]);
    assert_eq!(commit_of(&first), 2);

    // Surplus (3 events) >= n: the next cycle fires immediately, no timer.
    let second = harness.next_cycle().await;
    assert_eq!(cycle_ids(&second), vec!["2", "3"]);
    assert_eq!(commit_of(&second), 4);

    // One leftover event (< n) waits for the timeout window.
    tokio::time::sleep(Duration::from_secs(61)).await;
    let third = harness.next_cycle().await;
    assert_eq!(cycle_ids(&third), vec!["4"]);
    assert_eq!(commit_of(&third), 5);

    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn timeout_takes_all_when_below_n() {
    let mut harness = spawn_engine(10, 30);

    for id in 0..3u32 {
        harness.send_event(i64::from(id), id);
    }
    tokio::time::sleep(Duration::from_secs(31)).await;

    let batch = harness.next_cycle().await;
    assert_eq!(cycle_ids(&batch), vec!["0", "1", "2"]);
    assert_eq!(commit_of(&batch), 3);

    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn window_tumbles_from_cycle_completion() {
    let mut harness = spawn_engine(2, 10);

    // First event arrives at T0; the second at T0+7 fires the n-cycle.
    harness.send_event(0, 0);
    tokio::time::sleep(Duration::from_secs(7)).await;
    harness.send_event(1, 1);
    let cycle = harness.next_cycle().await;
    assert_eq!(cycle_ids(&cycle), vec!["0", "1"]);

    // A late event buffered at T0+7. If the deadline had NOT reset at cycle
    // completion, the stale pre-cycle deadline (T0+10) would have flushed it
    // already by T0+11; the tumbled deadline is T0+17.
    harness.send_event(2, 2);
    tokio::time::sleep(Duration::from_secs(4)).await; // now at T0+11
    harness.assert_no_more_records();

    tokio::time::sleep(Duration::from_secs(7)).await; // past T0+17
    let late = harness.next_cycle().await;
    assert_eq!(cycle_ids(&late), vec!["2"]);

    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn zero_event_window_is_a_noop() {
    let mut harness = spawn_engine(5, 10);

    // Three full empty windows elapse: no flush, no commit (D2).
    tokio::time::sleep(Duration::from_secs(35)).await;
    harness.assert_no_more_records();
    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn quarantine_counts_toward_n() {
    let mut harness = spawn_engine(2, 60);

    harness.send_event(0, 0);
    harness.send(1, "not json at all");
    let batch = harness.next_cycle().await;

    assert_eq!(
        batch
            .groups
            .iter()
            .map(|(_, entries)| entries.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(batch.quarantine.len(), 1);
    assert_eq!(commit_of(&batch), 2);

    harness.finish().await;
}

#[tokio::test(start_paused = true)]
async fn shutdown_flushes_remaining_as_final_batch() {
    let harness = spawn_engine(5, 60);

    harness.send_event(0, 0);
    harness.send_event(1, 1);
    tokio::time::sleep(Duration::from_secs(1)).await; // let the engine consume

    harness.shutdown.send(true).unwrap();
    let Harness {
        source: _,
        mut records,
        shutdown: _,
        engine,
    } = harness;
    let batch = match records.recv().await.unwrap() {
        Recorded::Final(batch) => batch,
        Recorded::Cycle(batch) => panic!("expected final flush, got cycle: {batch:?}"),
    };
    assert_eq!(cycle_ids(&batch), vec!["0", "1"]);
    assert_eq!(commit_of(&batch), 2);

    engine.await.unwrap().unwrap();
    assert!(records.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn source_end_flushes_remaining_as_final_batch() {
    let harness = spawn_engine(5, 60);

    harness.send_event(0, 0);
    let Harness {
        source,
        mut records,
        shutdown: _,
        engine,
    } = harness;
    drop(source); // recv drains the message, then yields None

    let batch = match records.recv().await.unwrap() {
        Recorded::Final(batch) => batch,
        Recorded::Cycle(batch) => panic!("expected final flush, got cycle: {batch:?}"),
    };
    assert_eq!(cycle_ids(&batch), vec!["0"]);
    assert_eq!(commit_of(&batch), 1);

    engine.await.unwrap().unwrap();
    assert!(records.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn empty_shutdown_skips_final_flush() {
    let mut harness = spawn_engine(5, 10);

    tokio::time::sleep(Duration::from_secs(3)).await;
    harness.assert_no_more_records();
    harness.finish().await;
}
