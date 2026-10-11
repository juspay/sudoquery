//! Archiving through a running kafka-to-s3 into MinIO.
//!
//! Each run gets its own topic and bucket. A shared archiver consumes the
//! run topic with a small batch size and an hour-long window, so only the
//! count trigger fires and every flush is deterministic. Cases that exercise
//! the window timer start their own archiver on their own topic with a
//! short window, after producing their events, so the whole backlog lands
//! in one window.
//!
//! The archiver writes the raw Kafka payloads verbatim and names each object
//! by the hash of its body, so cases compute the exact keys and bodies they
//! expect from what they produced.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, ensure};
use canonical_event::CanonicalEvent;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::kafka::Kafka;
use super::s3::S3;
use super::server::Service;
use super::{Case, run_cases, select};

const PACKAGE: &str = "kafka-to-s3";
/// The archiver's batch size `n`.
const BATCH: usize = 10;
/// Window of the shared archiver: long enough that it never fires.
const SHARED_WINDOW_SECS: u64 = 3600;
/// Window of the archivers that cases start to exercise the timer.
const SHORT_WINDOW_SECS: u64 = 3;
const SOURCE_ID: &str = "xtask-e2e";
/// Archive date of every produced event, as it appears in object keys.
const DATE: &str = "2026-09-23";
const DATE_YMD: (i32, u32, u32) = (2026, 9, 23);
/// How long to wait for expected objects; covers the consumer group join.
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the object count must stay put after the expected objects
/// appeared (guards against double writes).
const SETTLE: Duration = Duration::from_secs(2);

pub(crate) struct Ctx {
    kafka: Kafka,
    s3: S3,
    run_id: String,
    /// The topic the shared archiver consumes.
    topic: String,
}

/// A record produced to Kafka: where it landed and its exact bytes.
#[derive(Clone)]
struct Produced {
    topic: String,
    offset: i64,
    payload: Vec<u8>,
}

/// An org/project/hour group the archiver segregates events into.
struct Group {
    org_id: String,
    project_id: String,
    hour: u32,
}

impl Group {
    /// The key prefix the archiver files this group's objects under.
    fn prefix(&self) -> String {
        format!(
            "{}/{}/dt={DATE}/hour={:02}/",
            self.org_id, self.project_id, self.hour
        )
    }

    /// `count` events for this group, arriving one second apart.
    fn events(&self, count: usize) -> Vec<CanonicalEvent> {
        (0..count)
            .map(|second| {
                let at = self.at(second as u32);
                CanonicalEvent::builder()
                    .name("archiver_test_event".to_owned())
                    .org_id(self.org_id.clone())
                    .project_id(self.project_id.clone())
                    .anon_id(format!("anon-{}", short_id()))
                    .occured_at(at)
                    .arrived_at(Some(at))
                    .properties(Some(json!({ "source": "xtask-e2e" })))
                    .build()
            })
            .collect()
    }

    /// `second` seconds into this group's hour on [`DATE`].
    fn at(&self, second: u32) -> DateTime<Utc> {
        let (year, month, day) = DATE_YMD;
        Utc.with_ymd_and_hms(year, month, day, self.hour, 0, second)
            .single()
            .expect("valid test timestamp")
    }
}

impl Ctx {
    /// A group in a fresh org, so its prefix is empty.
    fn group(&self, hour: u32) -> Group {
        self.group_in(&format!("e2e-org-{}", short_id()), hour)
    }

    /// A fresh project in `org_id`.
    fn group_in(&self, org_id: &str, hour: u32) -> Group {
        Group {
            org_id: org_id.to_owned(),
            project_id: format!("e2e-proj-{}", short_id()),
            hour,
        }
    }

    /// Produces `events` to `topic` in order.
    async fn produce(
        &self,
        topic: &str,
        events: &[CanonicalEvent],
    ) -> anyhow::Result<Vec<Produced>> {
        let mut produced = Vec::with_capacity(events.len());
        for event in events {
            let payload = serde_json::to_string(event)?;
            produced.push(self.produce_raw(topic, &event.anon_id, payload).await?);
        }
        Ok(produced)
    }

    async fn produce_raw(
        &self,
        topic: &str,
        key: &str,
        payload: String,
    ) -> anyhow::Result<Produced> {
        let offset = self.kafka.produce(topic, key, payload.as_bytes()).await?;
        Ok(Produced {
            topic: topic.to_owned(),
            offset,
            payload: payload.into_bytes(),
        })
    }

    /// A fresh topic for a case-owned archiver.
    async fn case_topic(&self) -> anyhow::Result<String> {
        let topic = format!("{}-{}", self.run_id, short_id());
        self.kafka.create_topic(&topic).await?;
        Ok(topic)
    }

    /// Starts an archiver consuming `topics` with its own consumer group.
    async fn archiver(&self, topics: &[&str], window_secs: u64) -> anyhow::Result<Service> {
        let mut envs = vec![
            (
                "KAFKA_BOOTSTRAP_SERVERS",
                self.kafka.bootstrap_servers().to_owned(),
            ),
            ("KAFKA_TOPICS", topics.join(",")),
            ("KAFKA_GROUP_ID", topics.join("+")),
            ("BATCH_MAX_EVENTS", BATCH.to_string()),
            ("FLUSH_INTERVAL_SECS", window_secs.to_string()),
            ("SOURCE_ID", SOURCE_ID.to_owned()),
            ("RUST_LOG", "warn".to_owned()),
        ];
        envs.extend(self.s3.service_env());
        Service::spawn(PACKAGE, None, &envs).await
    }

    /// Waits until exactly one object per batch exists under `prefix`, each
    /// holding that batch's payloads in production order under its
    /// content-hash key, with matching provenance metadata.
    async fn expect_archived(&self, prefix: &str, batches: &[&[Produced]]) -> anyhow::Result<()> {
        self.s3
            .wait_for(prefix, batches.len(), ARCHIVE_TIMEOUT)
            .await?;
        tokio::time::sleep(SETTLE).await;
        let keys = self.s3.list(prefix).await?;

        let mut expected: Vec<String> = batches
            .iter()
            .map(|batch| object_key(prefix, &jsonl(batch)))
            .collect();
        expected.sort_unstable();
        if keys != expected {
            let mut found = Vec::new();
            for key in &keys {
                let lines = self.s3.get(key).await?.lines()?.len();
                found.push(format!("{key} ({lines} line(s))"));
            }
            anyhow::bail!(
                "expected objects {expected:?} under {prefix}, found {found:?} \
                 (a different key means different contents or order)"
            );
        }

        for batch in batches {
            let body = jsonl(batch);
            let key = object_key(prefix, &body);
            let object = self.s3.get(&key).await?;
            ensure!(
                object.body == body,
                "{key} does not hold the produced payloads verbatim"
            );
            expect_metadata(&key, &object.metadata, batch)?;
        }
        Ok(())
    }
}

/// Builds kafka-to-s3, starts the shared archiver against Kafka and MinIO,
/// runs the selected cases, and deletes the run's topics and bucket.
/// Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;
    Service::build(PACKAGE);

    let run_id = format!("e2e-k2s-{}", short_id());
    let kafka = Kafka::connect().await?;
    let s3 = S3::connect(run_id.clone()).await?;
    let ctx = Ctx {
        kafka,
        s3,
        topic: run_id.clone(),
        run_id,
    };

    let outcome = run_with_infra(&ctx, &cases).await;
    let topics = ctx.kafka.delete_created().await;
    let bucket = ctx.s3.delete_bucket().await;
    let passed = outcome?;
    topics?;
    bucket?;
    Ok(passed)
}

async fn run_with_infra(ctx: &Ctx, cases: &[&Case<Ctx>]) -> anyhow::Result<bool> {
    ctx.kafka.create_topic(&ctx.topic).await?;
    let archiver = ctx.archiver(&[&ctx.topic], SHARED_WINDOW_SECS).await?;
    Ok(run_cases(ctx, cases, |_| {}, || archiver.print_log_tail()).await)
}

cases![Ctx:
    archives_a_full_batch_as_one_object,
    segregates_events_by_org_and_project,
    segregates_events_by_arrival_hour,
    keeps_consuming_after_a_flush,
    quarantines_unroutable_payloads,
    flushes_a_partial_batch_when_the_window_ends,
    carries_the_surplus_into_the_next_window,
    merges_every_subscribed_topic_into_one_object,
    identical_content_reputs_are_idempotent,
];

async fn archives_a_full_batch_as_one_object(ctx: &Ctx) -> anyhow::Result<()> {
    let group = ctx.group(13);
    let produced = ctx.produce(&ctx.topic, &group.events(BATCH)).await?;
    ctx.expect_archived(&group.prefix(), &[&produced]).await
}

async fn segregates_events_by_org_and_project(ctx: &Ctx) -> anyhow::Result<()> {
    let shop = ctx.group(13);
    let blog = ctx.group_in(&shop.org_id, 13);
    let other_org = ctx.group(13);
    let groups = [&shop, &blog, &other_org];

    // Round-robin so consecutive offsets belong to different groups.
    let mut events = Vec::with_capacity(BATCH);
    let mut per_group: Vec<_> = groups
        .iter()
        .map(|group| group.events(BATCH).into_iter())
        .collect();
    for slot in (0..groups.len()).cycle().take(BATCH) {
        events.push(per_group[slot].next().expect("BATCH events per group"));
    }
    let produced = ctx.produce(&ctx.topic, &events).await?;

    for (slot, group) in groups.iter().enumerate() {
        let own: Vec<Produced> = produced
            .iter()
            .skip(slot)
            .step_by(groups.len())
            .cloned()
            .collect();
        ctx.expect_archived(&group.prefix(), &[&own])
            .await
            .with_context(|| format!("group {slot}"))?;
    }
    Ok(())
}

/// Hour 3 also pins the zero-padded `hour=03` key segment.
async fn segregates_events_by_arrival_hour(ctx: &Ctx) -> anyhow::Result<()> {
    let night = ctx.group(3);
    let afternoon = Group {
        org_id: night.org_id.clone(),
        project_id: night.project_id.clone(),
        hour: 14,
    };

    let mut events = Vec::with_capacity(BATCH);
    for (early, late) in night
        .events(BATCH / 2)
        .into_iter()
        .zip(afternoon.events(BATCH / 2))
    {
        events.push(early);
        events.push(late);
    }
    let produced = ctx.produce(&ctx.topic, &events).await?;

    let night_records: Vec<Produced> = produced.iter().step_by(2).cloned().collect();
    let afternoon_records: Vec<Produced> = produced.iter().skip(1).step_by(2).cloned().collect();
    ctx.expect_archived(&night.prefix(), &[&night_records])
        .await
        .context("hour 03")?;
    ctx.expect_archived(&afternoon.prefix(), &[&afternoon_records])
        .await
        .context("hour 14")
}

async fn keeps_consuming_after_a_flush(ctx: &Ctx) -> anyhow::Result<()> {
    let group = ctx.group(13);
    let first = ctx.produce(&ctx.topic, &group.events(BATCH)).await?;
    ctx.expect_archived(&group.prefix(), &[&first])
        .await
        .context("first batch")?;

    let second = ctx.produce(&ctx.topic, &group.events(BATCH)).await?;
    ctx.expect_archived(&group.prefix(), &[&first, &second])
        .await
        .context("second batch")
}

/// Unroutable payloads are archived one per object under `_quarantine/` and
/// still count toward the batch size.
async fn quarantines_unroutable_payloads(ctx: &Ctx) -> anyhow::Result<()> {
    let group = ctx.group(13);
    let mut events = group.events(BATCH - 2);
    let tail = events.split_off(BATCH / 2);

    let mut routed = ctx.produce(&ctx.topic, &events).await?;
    let not_json = ctx
        .produce_raw(
            &ctx.topic,
            "poison",
            format!("not json {}", uuid::Uuid::new_v4()),
        )
        .await?;
    let mut no_org = group.events(1).remove(0);
    no_org.org_id = String::new();
    let no_org = ctx.produce(&ctx.topic, &[no_org]).await?.remove(0);
    routed.extend(ctx.produce(&ctx.topic, &tail).await?);

    ctx.expect_archived(&group.prefix(), &[&routed])
        .await
        .context("routed events")?;
    ctx.expect_archived(
        "_quarantine/",
        &[
            std::slice::from_ref(&not_json),
            std::slice::from_ref(&no_org),
        ],
    )
    .await
    .context("quarantine")
}

/// Fewer than `n` events flush as one object when the window ends.
async fn flushes_a_partial_batch_when_the_window_ends(ctx: &Ctx) -> anyhow::Result<()> {
    let topic = ctx.case_topic().await?;
    let group = ctx.group(13);
    let produced = ctx.produce(&topic, &group.events(BATCH - 1)).await?;

    let archiver = ctx.archiver(&[&topic], SHORT_WINDOW_SECS).await?;
    with_log_on_error(
        &archiver,
        ctx.expect_archived(&group.prefix(), &[&produced]),
    )
    .await
}

/// `n + k` events: the first `n` flush on the count trigger, the surplus
/// `k` stay buffered until the next window ends.
async fn carries_the_surplus_into_the_next_window(ctx: &Ctx) -> anyhow::Result<()> {
    let topic = ctx.case_topic().await?;
    let group = ctx.group(13);
    let produced = ctx
        .produce(&topic, &group.events(BATCH + BATCH / 2))
        .await?;
    let (full, surplus) = produced.split_at(BATCH);

    let archiver = ctx.archiver(&[&topic], SHORT_WINDOW_SECS).await?;
    with_log_on_error(
        &archiver,
        ctx.expect_archived(&group.prefix(), &[full, surplus]),
    )
    .await
}

/// With several topics subscribed, one group's events from all of them land
/// in one object. Order across topics is not defined, so the object is
/// checked line-set-wise, plus per-topic order and one `ranges` entry per
/// topic.
async fn merges_every_subscribed_topic_into_one_object(ctx: &Ctx) -> anyhow::Result<()> {
    let topics = [ctx.case_topic().await?, ctx.case_topic().await?];
    let group = ctx.group(13);
    let mut produced = Vec::with_capacity(BATCH);
    for (i, event) in group.events(BATCH).iter().enumerate() {
        let topic = &topics[i % topics.len()];
        produced.extend(ctx.produce(topic, std::slice::from_ref(event)).await?);
    }

    // Produced up front, so the whole backlog hits the count trigger.
    let topic_names: Vec<&str> = topics.iter().map(String::as_str).collect();
    let archiver = ctx.archiver(&topic_names, SHARED_WINDOW_SECS).await?;
    with_log_on_error(&archiver, async {
        let prefix = group.prefix();
        ctx.s3.wait_for(&prefix, 1, ARCHIVE_TIMEOUT).await?;
        tokio::time::sleep(SETTLE).await;
        let keys = ctx.s3.list(&prefix).await?;
        ensure!(
            keys.len() == 1,
            "expected one object under {prefix}, found {keys:?}"
        );
        let key = &keys[0];
        let object = ctx.s3.get(key).await?;
        ensure!(
            *key == object_key(&prefix, &object.body),
            "{key} is not named by the hash of its body"
        );

        let lines = object.lines()?;
        let mut got: Vec<&str> = lines.clone();
        let mut want: Vec<&str> = produced
            .iter()
            .map(|record| std::str::from_utf8(&record.payload))
            .collect::<Result<_, _>>()?;
        got.sort_unstable();
        want.sort_unstable();
        ensure!(
            got == want,
            "{key} does not hold exactly the produced payloads"
        );

        for topic in &topics {
            let in_file: Vec<&str> = lines
                .iter()
                .copied()
                .filter(|line| {
                    produced
                        .iter()
                        .any(|record| &record.topic == topic && record.payload == line.as_bytes())
                })
                .collect();
            let in_topic: Vec<&str> = produced
                .iter()
                .filter(|record| &record.topic == topic)
                .map(|record| std::str::from_utf8(&record.payload))
                .collect::<Result<_, _>>()?;
            ensure!(
                in_file == in_topic,
                "{key} does not keep {topic}'s events in production order"
            );
        }
        expect_metadata(key, &object.metadata, &produced)
    })
    .await
}

/// The S3 semantics the archiver's replay safety relies on: a conditional
/// re-put of identical content under its content-hash key is rejected with
/// 412 (which the archiver treats as success) and leaves one intact object.
async fn identical_content_reputs_are_idempotent(ctx: &Ctx) -> anyhow::Result<()> {
    let group = ctx.group(13);
    let mut body = Vec::new();
    for event in group.events(2) {
        serde_json::to_writer(&mut body, &event)?;
        body.push(b'\n');
    }
    let key = object_key(&group.prefix(), &body);
    let compressed = zstd::encode_all(&body[..], 3)?;

    ensure!(
        ctx.s3.put_if_absent(&key, compressed.clone()).await?,
        "first conditional put of {key} was rejected"
    );
    ensure!(
        !ctx.s3.put_if_absent(&key, compressed).await?,
        "conditional re-put of identical content was not rejected with 412"
    );
    let keys = ctx.s3.list(&group.prefix()).await?;
    ensure!(keys == [key.clone()], "expected only {key}, found {keys:?}");
    ensure!(
        ctx.s3.get(&key).await?.body == body,
        "{key} was corrupted by the re-put"
    );
    Ok(())
}

// ============ Helpers ============

fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_owned()
}

/// The archiver's file body: each payload verbatim, newline-terminated.
fn jsonl(records: &[Produced]) -> Vec<u8> {
    let mut body = Vec::new();
    for record in records {
        body.extend_from_slice(&record.payload);
        body.push(b'\n');
    }
    body
}

/// `{prefix}{sha256(body) as hex, first 16 chars}.jsonl.zst`.
fn object_key(prefix: &str, body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    format!("{prefix}{}.jsonl.zst", hex::encode(&digest[..8]))
}

/// `source`, `event-count`, `ranges` (one `topic:partition:first:last`
/// entry per topic, in any order; every case produces to partition 0) and
/// `created-at`.
fn expect_metadata(
    key: &str,
    metadata: &std::collections::HashMap<String, String>,
    records: &[Produced],
) -> anyhow::Result<()> {
    let mut spans: BTreeMap<&str, (i64, i64)> = BTreeMap::new();
    for record in records {
        let span = spans
            .entry(&record.topic)
            .or_insert((record.offset, record.offset));
        span.0 = span.0.min(record.offset);
        span.1 = span.1.max(record.offset);
    }
    let want_ranges: Vec<String> = spans
        .iter()
        .map(|(topic, (first, last))| format!("{topic}:0:{first}:{last}"))
        .collect();
    let mut got_ranges: Vec<&str> = metadata
        .get("ranges")
        .map(|ranges| ranges.split(';').collect())
        .unwrap_or_default();
    got_ranges.sort_unstable();
    let field = |name: &str| metadata.get(name).map(String::as_str);

    ensure!(
        field("source") == Some(SOURCE_ID),
        "{key}: wrong source in {metadata:?}"
    );
    ensure!(
        field("event-count") == Some(records.len().to_string().as_str()),
        "{key}: wrong event-count in {metadata:?}"
    );
    ensure!(
        got_ranges == want_ranges,
        "{key}: wrong ranges in {metadata:?}"
    );
    ensure!(
        field("created-at").is_some_and(|at| chrono::DateTime::parse_from_rfc3339(at).is_ok()),
        "{key}: missing or invalid created-at in {metadata:?}"
    );
    Ok(())
}

/// Awaits a case step, printing `service`'s log tail if it fails.
async fn with_log_on_error(
    service: &Service,
    step: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let result = step.await;
    if result.is_err() {
        service.print_log_tail();
    }
    result
}
