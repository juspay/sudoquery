//! # kafka-to-s3-tests
//!
//! Black-box integration test harness for the `kafka_to_s3` archiver service.
//!
//! ## Purpose
//!
//! The service under test continuously consumes canonical events (JSON,
//! matching the `canonical-event` crate's [`canonical_event::CanonicalEvent`]
//! model) from a Kafka topic and archives them to S3/MinIO as
//! zstd-compressed JSONL. The harness produces events to Kafka, polls MinIO
//! for archived objects, and asserts on object paths, contents, batching,
//! provenance metadata, and org/proj/hour segregation.
//!
//! ## The service runs externally
//!
//! The service is **not** managed by this harness. The operator starts it
//! manually (bare binary or container) with configuration matching the
//! environment variables below, before running the tests. The harness's
//! [`launcher::StubLauncher`] is a deliberate no-op seam: every test calls it
//! at a single call-site, so a future implementation that spawns the
//! binary/container in-process stays mechanical.
//!
//! Contract under test (drives the assertions):
//!
//! - The service waits until at least `n` events are available (`n` = the
//!   service's configured batch size, mirrored by `K2S_TEST_BATCH_SIZE`) or
//!   1 minute has elapsed, whichever comes first, and then picks up `n`
//!   events to work with. Surplus events carry into the next cycle; a
//!   timeout cycle picks up the buffered remainder (fewer than `n`).
//! - Events picked in one cycle are segregated into separate files per
//!   org+proj(+hour) group, so a single file may contain fewer than
//!   `n` events.
//! - Object key format:
//!   `{org}/{proj}/dt=YYYY-MM-DD/hour=H/{sha256-16}.jsonl.zst`
//!   where `dt`/`hour` derive from the event's `arrived_at` field (UTC) and
//!   the file name is the sha256 of the file's uncompressed JSONL content,
//!   truncated to the first 16 hex characters. Identical content therefore
//!   always maps to the same key (idempotent overwrite); object names are
//!   never derived from Kafka offsets.
//! - Every archived object carries provenance S3 metadata: `source` (the
//!   service's `SOURCE_ID`, when configured), `event-count`, and `ranges`
//!   (`topic:partition:first:last` entries joined by `;`).
//! - Events without a `proj_id` are out of scope: every harness-produced
//!   event sets one.
//!
//! ## Environment variables
//!
//! All optional; defaults target the local docker compose stack (redpanda on
//! `localhost:19092`, MinIO on `localhost:9000` with `minioadmin`):
//!
//! | Variable | Default | Meaning |
//! |---|---|---|
//! | `K2S_TEST_KAFKA` | `localhost:19092` | Kafka broker list |
//! | `K2S_TEST_TOPIC` | `canonical-events` | Topic the service consumes |
//! | `K2S_TEST_BATCH_SIZE` | `100` | Must match the service's configured `n` |
//! | `K2S_TEST_S3_ENDPOINT` | `http://localhost:9000` | MinIO/S3 endpoint |
//! | `K2S_TEST_S3_BUCKET` | `canonical-events` | Must match the service's bucket |
//! | `K2S_TEST_S3_REGION` | `us-east-1` | S3 region |
//! | `K2S_TEST_S3_ACCESS_KEY` | `minioadmin` | S3 access key |
//! | `K2S_TEST_S3_SECRET_KEY` | `minioadmin` | S3 secret key |
//! | `K2S_TEST_SOURCE_ID` | — (unset) | Mirrors the service's `SOURCE_ID`; when set, the archived objects' `source` metadata is asserted against it |
//!
//! ## Running
//!
//! ```text
//! # start redpanda + MinIO (docker compose -f tests/docker-compose.yml up -d)
//! # and the kafka_to_s3 service, then:
//! cargo test -p kafka-to-s3-tests
//!
//! # the slow timer-dependent tests are ignored by default:
//! cargo test -p kafka-to-s3-tests -- --ignored
//! ```
//!
//! Tests share one topic and one service instance, so they serialize on a
//! global lock ([`serial`]) and each test uses a `unique_org` prefix to
//! isolate its S3 namespace. Kafka offsets are not assumed to start at 0.

pub mod config;
pub mod events;
pub mod launcher;
pub mod producer;
pub mod s3;

use std::sync::OnceLock;

use tokio::sync::{Mutex, MutexGuard};

/// Global serialization lock for the integration tests.
///
/// The tests share one Kafka topic and one service instance, so they must not
/// run concurrently.
static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquires the global test-serialization lock; every test awaits this first.
///
/// The guard is held for the whole test body, so tests in the same binary
/// execute strictly one after another.
pub async fn serial() -> MutexGuard<'static, ()> {
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}
