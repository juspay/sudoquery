//! Black-box integration tests for the externally-run `kafka_to_s3` service.
//!
//! Every test serializes on [`serial`] (one shared topic, one shared service
//! instance), isolates its S3 namespace behind a `unique_org` prefix, and
//! asserts on the objects the service archives. See the library docs for the
//! contract under test and the `K2S_TEST_*` environment variables.
//!
//! Object file names are content hashes (`{sha256-16}.jsonl.zst`), so they
//! carry no offset or ordering information. Ordering is therefore asserted
//! on file contents (production order), and cross-object relationships on
//! disjoint event-id sets.
//!
//! The timer-dependent tests (`flushes_partial_batch_on_timeout`,
//! `flushes_surplus_after_exact_n_cycle`) are `#[ignore]`d: run them with
//! `cargo test -p kafka-to-s3-tests -- --ignored`. The idempotency test
//! exercises S3 semantics directly against MinIO and needs neither Kafka
//! nor the service.

// allow: SIZE_OK — the task spec mandates a single integration test file
// holding every black-box test.

use std::collections::HashSet;
use std::time::Duration;

use canonical_event::CanonicalEvent;
use chrono::{DateTime, Utc};
use kafka_to_s3_tests::config::TestConfig;
use kafka_to_s3_tests::events::{event_id, make_event, unique_org};
use kafka_to_s3_tests::launcher::StubLauncher;
use kafka_to_s3_tests::producer::{EventProducer, ProducedEvent, ensure_topic};
use kafka_to_s3_tests::s3::{ArchiveStore, ArchivedEvent, is_hash_file_name};
use kafka_to_s3_tests::serial;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Fixed archive date every test produces events for.
const DATE: &str = "2026-09-23";
/// Hours used by the tests. Only two-digit hours are exercised: the
/// single-digit-hour zero-padding format is unverified.
const HOUR_13: u32 = 13;
const HOUR_14: u32 = 14;
/// Deadline for count-triggered flushes (objects should appear in seconds).
const FLUSH_DEADLINE: Duration = Duration::from_secs(60);
/// Deadline for timer-driven flushes: the flush interval is 1 minute and
/// the window timer resets after each flush completes.
const TIMER_DEADLINE: Duration = Duration::from_secs(120);
/// Stability wait after an expected object appears (double-write guard).
const STABILITY_WAIT: Duration = Duration::from_secs(5);

struct TestHarness {
    config: &'static TestConfig,
    producer: EventProducer,
    store: ArchiveStore,
}

/// Ensures the shared topic and bucket exist and builds the harness clients.
async fn setup() -> TestHarness {
    let config = TestConfig::load();
    ensure_topic(&config.kafka_brokers, &config.topic).await;
    let producer = EventProducer::new(&config.kafka_brokers, &config.topic);
    let store = ArchiveStore::new(config);
    store.ensure_bucket().await;
    TestHarness {
        config,
        producer,
        store,
    }
}

/// An org/proj/hour group the tests segregate events into.
struct Group {
    org: String,
    proj: String,
    hour: u32,
}

impl Group {
    /// Builds `count` events for the group, arriving `seq` seconds past the
    /// hour start, with `seq` markers starting at `seq_start`.
    fn events(&self, seq_start: u64, count: usize) -> Vec<CanonicalEvent> {
        let base = hour_base(self.hour);
        (0..count as u64)
            .map(|i| {
                let seq = seq_start + i;
                make_event(
                    &self.org,
                    &self.proj,
                    base + chrono::Duration::seconds(seq as i64),
                    seq,
                )
            })
            .collect()
    }

    /// The S3 prefix the service must archive this group under.
    fn prefix(&self, config: &TestConfig) -> String {
        config.expected_prefix(&self.org, &self.proj, DATE, self.hour)
    }
}

/// The UTC instant at [`DATE`] and the given hour.
fn hour_base(hour: u32) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&format!("{DATE}T{hour:02}:00:00Z"))
        .unwrap_or_else(|err| panic!("invalid fixed test timestamp: {err}"))
        .with_timezone(&Utc)
}

/// What the harness produced for one event, mirrored from the built event.
struct ExpectedEvent {
    id: Uuid,
    org_id: String,
    proj_id: String,
    arrived_at: DateTime<Utc>,
}

/// Mirrors `events` into assertion-friendly records, in input order.
fn expected_of(events: &[CanonicalEvent]) -> Vec<ExpectedEvent> {
    events
        .iter()
        .map(|event| ExpectedEvent {
            id: event_id(event),
            org_id: event.org_id.clone(),
            proj_id: event
                .proj_id
                .clone()
                .expect("harness events always set proj_id"),
            arrived_at: event
                .arrived_at
                .expect("harness events always set arrived_at"),
        })
        .collect()
}

/// The set of event `id`s the harness produced for `expected`.
fn expected_ids(expected: &[ExpectedEvent]) -> HashSet<Uuid> {
    expected.iter().map(|event| event.id).collect()
}

/// The set of event `id`s archived in `archived`.
fn archived_ids(archived: &[ArchivedEvent]) -> HashSet<Uuid> {
    archived.iter().map(|event| event.id).collect()
}

/// Asserts an archived line matches the event the harness produced for it.
fn assert_event_matches(actual: &ArchivedEvent, expected: &ExpectedEvent) {
    assert_eq!(actual.id, expected.id, "archived event id mismatch");
    assert_eq!(
        actual.org_id, expected.org_id,
        "archived org_id mismatch for event {}",
        expected.id
    );
    assert_eq!(
        actual.proj_id.as_deref(),
        Some(expected.proj_id.as_str()),
        "archived proj_id mismatch for event {}",
        expected.id
    );
    assert_eq!(
        actual.arrived_at, expected.arrived_at,
        "archived arrived_at mismatch for event {}",
        expected.id
    );
}

/// Asserts the archived file holds exactly the expected events (by id set),
/// with every line matching its produced event.
fn assert_contents(archived: &[ArchivedEvent], expected: &[ExpectedEvent]) {
    assert_eq!(
        archived.len(),
        expected.len(),
        "archived file must contain exactly the expected {} event(s)",
        expected.len()
    );
    assert_eq!(
        archived_ids(archived),
        expected_ids(expected),
        "archived file must contain exactly the expected event ids"
    );
    let expected_by_id: std::collections::HashMap<Uuid, &ExpectedEvent> =
        expected.iter().map(|event| (event.id, event)).collect();
    for event in archived {
        assert_event_matches(event, expected_by_id[&event.id]);
    }
}

/// Asserts the object key is the group prefix plus a hash file name: 16
/// lowercase hex chars (sha256 of the uncompressed JSONL, first 16 hex
/// chars) followed by `.jsonl.zst`.
fn assert_hash_file_name(key: &str, prefix: &str) {
    let file_name = key
        .strip_prefix(&format!("{prefix}/"))
        .unwrap_or_else(|| panic!("object key {key:?} must sit under the group prefix {prefix:?}"));
    assert!(
        is_hash_file_name(file_name),
        "object file name {file_name:?} must be the content-hash form \
         16-lowercase-hex-chars.jsonl.zst (sha256 of the uncompressed JSONL, \
         first 16 hex chars)"
    );
}

/// Asserts archived ids appear in file order == production order, with each
/// line matching its produced event.
///
/// The producer pins partition 0, so the service's consumption order — and
/// therefore the file order — equals the delivery-offset order. Offsets
/// serve as production-order markers here, never as file-name material.
fn assert_ordered_by_production(
    archived: &[ArchivedEvent],
    expected: &[ExpectedEvent],
    produced: &[ProducedEvent],
) {
    assert_eq!(
        expected.len(),
        produced.len(),
        "one delivered ack per produced event"
    );
    assert_eq!(
        archived.len(),
        produced.len(),
        "archived file must contain exactly the {} produced event(s)",
        produced.len()
    );
    let mut by_offset: Vec<usize> = (0..produced.len()).collect();
    by_offset.sort_by_key(|&i| produced[i].offset);
    let file_order: Vec<Uuid> = archived.iter().map(|event| event.id).collect();
    let production_order: Vec<Uuid> = by_offset.iter().map(|&i| produced[i].id).collect();
    assert_eq!(
        file_order, production_order,
        "archived event ids must appear in file order == production order"
    );
    for (event, &i) in archived.iter().zip(by_offset.iter()) {
        assert_event_matches(event, &expected[i]);
    }
}

/// Asserts the object count under `prefix` does not grow after a wait
/// (guards against double-writes).
async fn assert_stable(store: &ArchiveStore, prefix: &str, keys: &[String]) {
    tokio::time::sleep(STABILITY_WAIT).await;
    let after = store.list_keys(prefix).await;
    assert_eq!(
        after.len(),
        keys.len(),
        "object count under {prefix} changed after {STABILITY_WAIT:?} (double-write?); \
         objects now: {after:?}"
    );
}

/// Verifies redpanda and MinIO are reachable and the shared topic and bucket
/// can be ensured. Produces no events; the service under test is not needed.
#[tokio::test]
async fn harness_connects_to_infrastructure() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    setup().await;
}

/// One org+proj, `n` events in hour 13: exactly one object appears
/// under the group prefix with a hash file name, holding all `n` events in
/// production order with matching org/proj/arrived_at.
#[tokio::test]
async fn flushes_exactly_n_events_for_single_org_proj() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let group = Group {
        org: unique_org("k2s-flush-n"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let events = group.events(0, n);
    let expected = expected_of(&events);
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n, "every event must be delivered");

    let prefix = group.prefix(harness.config);
    let keys = harness
        .store
        .wait_for_objects(&prefix, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        1,
        "expected exactly one object under {prefix}, found {keys:?}"
    );
    assert_hash_file_name(&keys[0], &prefix);
    assert_stable(&harness.store, &prefix, &keys).await;

    let archived = harness.store.fetch_jsonl(&keys[0]).await;
    assert_ordered_by_production(&archived, &expected, &produced);
}

/// `n` events across two org+proj groups in one batch: each group
/// gets exactly one hash-named object under its own prefix holding exactly
/// its own events; the union of both files covers all `n` ids without
/// duplicates.
#[tokio::test]
async fn segregates_events_by_org_and_proj() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let org = unique_org("k2s-segregate");
    let ws1 = Group {
        org: org.clone(),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let ws2 = Group {
        org,
        proj: "proj-2".to_string(),
        hour: HOUR_13,
    };
    let events_ws1 = ws1.events(0, n / 2);
    let events_ws2 = ws2.events(0, n - n / 2);
    let expected_ws1 = expected_of(&events_ws1);
    let expected_ws2 = expected_of(&events_ws2);

    // Interleave the groups so adjacent deliveries belong to different groups.
    let mut events = Vec::with_capacity(n);
    let mut ws1_events = events_ws1.into_iter().peekable();
    let mut ws2_events = events_ws2.into_iter().peekable();
    while ws1_events.peek().is_some() || ws2_events.peek().is_some() {
        if let Some(event) = ws1_events.next() {
            events.push(event);
        }
        if let Some(event) = ws2_events.next() {
            events.push(event);
        }
    }
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n, "every event must be delivered");

    let prefix_ws1 = ws1.prefix(harness.config);
    let prefix_ws2 = ws2.prefix(harness.config);
    let keys_ws1 = harness
        .store
        .wait_for_objects(&prefix_ws1, 1, FLUSH_DEADLINE)
        .await;
    let keys_ws2 = harness
        .store
        .wait_for_objects(&prefix_ws2, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys_ws1.len(),
        1,
        "expected exactly one object under {prefix_ws1}, found {keys_ws1:?}"
    );
    assert_eq!(
        keys_ws2.len(),
        1,
        "expected exactly one object under {prefix_ws2}, found {keys_ws2:?}"
    );
    assert_hash_file_name(&keys_ws1[0], &prefix_ws1);
    assert_hash_file_name(&keys_ws2[0], &prefix_ws2);
    assert_stable(&harness.store, &prefix_ws1, &keys_ws1).await;
    assert_stable(&harness.store, &prefix_ws2, &keys_ws2).await;

    let archived_ws1 = harness.store.fetch_jsonl(&keys_ws1[0]).await;
    let archived_ws2 = harness.store.fetch_jsonl(&keys_ws2[0]).await;
    assert_contents(&archived_ws1, &expected_ws1);
    assert_contents(&archived_ws2, &expected_ws2);

    let union: HashSet<Uuid> = archived_ids(&archived_ws1)
        .union(&archived_ids(&archived_ws2))
        .copied()
        .collect();
    assert_eq!(
        union.len(),
        n,
        "union of both files must cover all {n} produced ids without duplicates"
    );
    assert_eq!(
        union,
        expected_ids(&expected_ws1)
            .union(&expected_ids(&expected_ws2))
            .copied()
            .collect()
    );
}

/// `n` events for one org+proj, half in hour 13 and half in hour 14:
/// each hour prefix gets exactly one hash-named object holding exactly that
/// hour's events.
#[tokio::test]
async fn segregates_events_by_arrived_at_hour() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let org = unique_org("k2s-by-hour");
    let h13 = Group {
        org: org.clone(),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let h14 = Group {
        org,
        proj: "proj-1".to_string(),
        hour: HOUR_14,
    };
    let events_h13 = h13.events(0, n / 2);
    let events_h14 = h14.events(0, n - n / 2);
    let expected_h13 = expected_of(&events_h13);
    let expected_h14 = expected_of(&events_h14);

    // Interleave the hours so adjacent deliveries belong to different hours.
    let mut events = Vec::with_capacity(n);
    let mut h13_events = events_h13.into_iter().peekable();
    let mut h14_events = events_h14.into_iter().peekable();
    while h13_events.peek().is_some() || h14_events.peek().is_some() {
        if let Some(event) = h13_events.next() {
            events.push(event);
        }
        if let Some(event) = h14_events.next() {
            events.push(event);
        }
    }
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n, "every event must be delivered");

    let prefix_h13 = h13.prefix(harness.config);
    let prefix_h14 = h14.prefix(harness.config);
    let keys_h13 = harness
        .store
        .wait_for_objects(&prefix_h13, 1, FLUSH_DEADLINE)
        .await;
    let keys_h14 = harness
        .store
        .wait_for_objects(&prefix_h14, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys_h13.len(),
        1,
        "expected exactly one object under {prefix_h13}, found {keys_h13:?}"
    );
    assert_eq!(
        keys_h14.len(),
        1,
        "expected exactly one object under {prefix_h14}, found {keys_h14:?}"
    );
    assert_hash_file_name(&keys_h13[0], &prefix_h13);
    assert_hash_file_name(&keys_h14[0], &prefix_h14);
    assert_stable(&harness.store, &prefix_h13, &keys_h13).await;
    assert_stable(&harness.store, &prefix_h14, &keys_h14).await;

    let archived_h13 = harness.store.fetch_jsonl(&keys_h13[0]).await;
    let archived_h14 = harness.store.fetch_jsonl(&keys_h14[0]).await;
    assert_contents(&archived_h13, &expected_h13);
    assert_contents(&archived_h14, &expected_h14);
}

/// After a count-triggered flush the service keeps consuming: a second batch
/// for the same group archives as a second object. The two objects' event-id
/// sets are disjoint, each object holds exactly one batch's events, and
/// together they cover all `2n` produced ids without duplicates.
#[tokio::test]
async fn continues_consuming_after_flush() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let group = Group {
        org: unique_org("k2s-continue"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let prefix = group.prefix(harness.config);

    let first_events = group.events(0, n);
    let first_expected = expected_of(&first_events);
    let first_produced = harness.producer.produce(first_events).await;
    assert_eq!(first_produced.len(), n, "every event must be delivered");

    let keys = harness
        .store
        .wait_for_objects(&prefix, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        1,
        "expected exactly one object after the first batch, found {keys:?}"
    );
    assert_hash_file_name(&keys[0], &prefix);
    assert_stable(&harness.store, &prefix, &keys).await;

    let second_events = group.events(n as u64, n);
    let second_expected = expected_of(&second_events);
    let second_produced = harness.producer.produce(second_events).await;
    assert_eq!(second_produced.len(), n, "every event must be delivered");

    let keys = harness
        .store
        .wait_for_objects(&prefix, 2, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        2,
        "expected exactly two objects after the second batch, found {keys:?}"
    );
    for key in &keys {
        assert_hash_file_name(key, &prefix);
    }
    assert_stable(&harness.store, &prefix, &keys).await;

    let first_ids = expected_ids(&first_expected);
    let second_ids = expected_ids(&second_expected);
    let archived_a = harness.store.fetch_jsonl(&keys[0]).await;
    let archived_b = harness.store.fetch_jsonl(&keys[1]).await;
    let ids_a = archived_ids(&archived_a);
    let ids_b = archived_ids(&archived_b);
    assert!(
        ids_a.is_disjoint(&ids_b),
        "the two objects' event ids must be disjoint (no event archived twice): \
         {ids_a:?} vs {ids_b:?}"
    );

    // Hash file names carry no ordering, so match each object to its batch
    // by content.
    let (first_archived, second_archived) = if ids_a == first_ids {
        assert_eq!(
            ids_b, second_ids,
            "the second object must hold exactly the second batch's events"
        );
        (archived_a, archived_b)
    } else {
        assert_eq!(
            ids_a, second_ids,
            "each object must hold exactly one batch's events"
        );
        assert_eq!(
            ids_b, first_ids,
            "the second object must hold exactly the first batch's events"
        );
        (archived_b, archived_a)
    };
    assert_contents(&first_archived, &first_expected);
    assert_contents(&second_archived, &second_expected);

    let union: HashSet<Uuid> = ids_a.union(&ids_b).copied().collect();
    assert_eq!(
        union.len(),
        2 * n,
        "both objects together must hold all {} produced ids without duplicates",
        2 * n
    );
    assert_eq!(union, first_ids.union(&second_ids).copied().collect());
}

/// slow: exercises the 1-minute flush timer; run with -- --ignored
///
/// `n - 1` events (below the count trigger): no object may exist under the
/// prefix after a 30s wait (the timer must not fire early), then the timer
/// flushes exactly the partial batch with matching ids.
#[tokio::test]
#[ignore]
async fn flushes_partial_batch_on_timeout() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let group = Group {
        org: unique_org("k2s-timeout"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let events = group.events(0, n - 1);
    let expected = expected_of(&events);
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n - 1, "every event must be delivered");

    // The 1-minute timer must not fire early: nothing may be archived for
    // 30s after production.
    tokio::time::sleep(Duration::from_secs(30)).await;
    let prefix = group.prefix(harness.config);
    let early = harness.store.list_keys(&prefix).await;
    assert!(
        early.is_empty(),
        "no object may exist under {prefix} before the 1-minute flush timer; found \
         {early:?} after 30s — the service flushes sooner than spec'd"
    );

    let keys = harness
        .store
        .wait_for_objects(&prefix, 1, TIMER_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        1,
        "expected exactly one object under {prefix}, found {keys:?}"
    );
    assert_hash_file_name(&keys[0], &prefix);
    assert_stable(&harness.store, &prefix, &keys).await;

    let archived = harness.store.fetch_jsonl(&keys[0]).await;
    assert_contents(&archived, &expected);
}

/// slow: exercises the exact-n cycle followed by the 1-minute flush timer;
/// run with -- --ignored
///
/// `n + k` events (`k < n`) for one org: the count trigger flushes an
/// `n`-line object immediately (exactly `n`, FIFO), the surplus `k` events
/// stay buffered and flush as a `k`-line object when the timer fires. The
/// two objects' id sets are disjoint and together cover all `n + k` ids.
#[tokio::test]
#[ignore]
async fn flushes_surplus_after_exact_n_cycle() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;
    let k = (n / 4).max(1);
    assert!(
        k < n,
        "K2S_TEST_BATCH_SIZE must be at least 2 to exercise a surplus cycle"
    );

    let group = Group {
        org: unique_org("k2s-surplus"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let prefix = group.prefix(harness.config);

    let events = group.events(0, n + k);
    let expected_first = expected_of(&events[..n]);
    let expected_surplus = expected_of(&events[n..]);
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n + k, "every event must be delivered");

    // The exact-n trigger fires as soon as `n` events are buffered, long
    // before the timer.
    let keys = harness
        .store
        .wait_for_objects(&prefix, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        1,
        "expected exactly one object after the exact-n cycle, found {keys:?}"
    );
    let first_key = keys[0].clone();
    assert_hash_file_name(&first_key, &prefix);
    let first_archived = harness.store.fetch_jsonl(&first_key).await;
    assert_ordered_by_production(&first_archived, &expected_first, &produced[..n]);

    // The surplus (< n) stays buffered until the timer fires. The window
    // timer reset when the exact-n flush completed, so the surplus object
    // appears about a minute after the first one.
    let keys = harness
        .store
        .wait_for_objects(&prefix, 2, TIMER_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        2,
        "expected exactly two objects after the surplus flush, found {keys:?}"
    );
    let surplus_key = keys
        .iter()
        .find(|key| **key != first_key)
        .expect("the surplus object must differ from the exact-n object");
    assert_hash_file_name(surplus_key, &prefix);
    assert_stable(&harness.store, &prefix, &keys).await;

    let surplus_archived = harness.store.fetch_jsonl(surplus_key).await;
    assert_ordered_by_production(&surplus_archived, &expected_surplus, &produced[n..]);

    let first_ids = archived_ids(&first_archived);
    let surplus_ids = archived_ids(&surplus_archived);
    assert!(
        first_ids.is_disjoint(&surplus_ids),
        "the exact-n object and the surplus object must hold disjoint event ids"
    );
    let union: HashSet<Uuid> = first_ids.union(&surplus_ids).copied().collect();
    assert_eq!(
        union.len(),
        n + k,
        "both objects together must hold all {} produced ids without duplicates",
        n + k
    );
    assert_eq!(
        union,
        expected_ids(&expected_first)
            .union(&expected_ids(&expected_surplus))
            .copied()
            .collect()
    );
}

/// The archived object carries the service's provenance metadata: the
/// flushed event count, the consumed offset `ranges`, and — when the
/// operator mirrored the service's `SOURCE_ID` into `K2S_TEST_SOURCE_ID` —
/// the `source` id.
#[tokio::test]
async fn attaches_provenance_metadata_to_archived_objects() {
    let _guard = serial().await;
    let _service = StubLauncher.launch();
    let harness = setup().await;
    let n = harness.config.batch_size;

    let group = Group {
        org: unique_org("k2s-metadata"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let events = group.events(0, n);
    let produced = harness.producer.produce(events).await;
    assert_eq!(produced.len(), n, "every event must be delivered");

    let prefix = group.prefix(harness.config);
    let keys = harness
        .store
        .wait_for_objects(&prefix, 1, FLUSH_DEADLINE)
        .await;
    assert_eq!(
        keys.len(),
        1,
        "expected exactly one object under {prefix}, found {keys:?}"
    );
    assert_hash_file_name(&keys[0], &prefix);

    let metadata = harness.store.head_object(&keys[0]).await;
    assert_eq!(
        metadata.event_count,
        Some(n.to_string()),
        "the object's event-count metadata must equal the flushed batch size"
    );

    // All n events form one group in one cycle, so ranges must be the single
    // `topic:partition:first:last` entry of the dequeued batch. Offsets are
    // production-order markers here, never file-name material.
    let first = produced
        .iter()
        .map(|event| event.offset)
        .min()
        .expect("the batch is non-empty");
    let last = produced
        .iter()
        .map(|event| event.offset)
        .max()
        .expect("the batch is non-empty");
    let expected_ranges = format!("{}:0:{first}:{last}", harness.config.topic);
    assert_eq!(
        metadata.ranges.as_deref(),
        Some(expected_ranges.as_str()),
        "the object's ranges metadata must name the harness topic, partition 0, \
         and the dequeued batch's offset span"
    );

    if let Some(expected_source) = &harness.config.source_id {
        assert_eq!(
            metadata.source.as_deref(),
            Some(expected_source.as_str()),
            "the object's source metadata must equal the mirrored SOURCE_ID"
        );
    }
}

/// Direct S3-level check of the hash-named overwrite semantics the service
/// relies on for idempotency: re-putting identical content under the same
/// content-hash key hits `412 PreconditionFailed` (which the service treats
/// as success), leaves exactly one object, and does not corrupt it.
///
/// A true restart test — stop the service mid-window before it commits,
/// restart it, and assert no duplicate objects appear — needs the operator
/// to orchestrate the service lifecycle: the [`StubLauncher`] is a no-op
/// seam and the service runs externally. That scenario is deferred to
/// operator orchestration; this test pins the S3-level semantics it depends
/// on. It needs the compose stack's MinIO but neither Kafka nor the
/// service, so it does not use `setup()` or launch the service.
#[tokio::test]
async fn identical_content_reputs_stay_idempotent() {
    let _guard = serial().await;
    let config = TestConfig::load();
    let store = ArchiveStore::new(config);
    store.ensure_bucket().await;

    let group = Group {
        org: unique_org("k2s-idempotent"),
        proj: "proj-1".to_string(),
        hour: HOUR_13,
    };
    let events = group.events(0, 2);
    let expected = expected_of(&events);
    let prefix = group.prefix(config);

    // Build the object exactly as the service would: raw event bytes, one
    // per line, zstd-compressed, keyed by the sha256 of the uncompressed
    // JSONL truncated to the first 16 hex chars.
    let mut jsonl: Vec<u8> = Vec::new();
    for event in &events {
        let payload =
            serde_json::to_string(event).expect("failed to serialize canonical event to JSON");
        jsonl.extend_from_slice(payload.as_bytes());
        jsonl.push(b'\n');
    }
    let digest = Sha256::digest(&jsonl);
    let key = format!("{prefix}/{}.jsonl.zst", hex::encode(&digest[..8]));
    let body = zstd::encode_all(&jsonl[..], 3).expect("failed to zstd-compress the JSONL");

    assert!(
        store.put_if_absent(&key, body.clone()).await,
        "the first conditional PUT of new content must succeed"
    );
    assert!(
        !store.put_if_absent(&key, body).await,
        "re-putting identical content must hit 412 PreconditionFailed (which the \
         service treats as success) instead of erroring or duplicating"
    );

    let keys = store.list_keys(&prefix).await;
    assert_eq!(
        keys,
        vec![key.clone()],
        "re-putting identical content must not create a second object"
    );

    let archived = store.fetch_jsonl(&key).await;
    assert_contents(&archived, &expected);
}
