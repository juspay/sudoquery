# kafka-to-s3

Archives canonical events from Kafka to S3 as zstd-compressed JSONL, segregated
per tenant + workspace and partitioned by arrival date/hour. The downstream
counterpart of the HTTP `event-collector` service: it consumes what the collector
publishes and lands it in object storage.

## Configuration

Environment only (no `cac.toml`). Invalid values fail fast at boot with a
message naming the variable.

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `KAFKA_BOOTSTRAP_SERVERS` | yes | — | librdkafka `bootstrap.servers` |
| `KAFKA_TOPICS` | yes | — | Comma-separated, trimmed, non-empty; the service subscribes to all of them |
| `KAFKA_GROUP_ID` | yes | — | Consumer group |
| `S3_BUCKET` | yes | — | Target bucket |
| `S3_PREFIX` | no | — | Key prefix, no leading/trailing `/` |
| `S3_ENDPOINT` | no | — | Endpoint override (MinIO/local) |
| `S3_FORCE_PATH_STYLE` | no | `false` | `true` for MinIO |
| `BATCH_MAX_EVENTS` | no | `10000` | Batch size `n`; must be > 0 |
| `FLUSH_INTERVAL_SECS` | no | `60` | Window length; must be > 0 |
| `SOURCE_ID` | no | — | Provenance metadata only (e.g. `aws-prod-a`) |

AWS credentials/region come from the standard SDK chain. `RUST_LOG` controls
log verbosity (defaults to `info`).

## S3 key layout

```
{S3_PREFIX?}/{tenant_id}/{workspace_id}/dt=YYYY-MM-DD/hour=HH/{hash}.jsonl.zst
{S3_PREFIX?}/_quarantine/{hash}.jsonl.zst
```

- `dt`/`hour` come from the event's `arrived_at` (UTC); if `arrived_at` is
  absent or unparseable the service falls back to `occured_at` and logs a
  warning. The hour is zero-padded (`hour=03`).
- `workspace_id` absent routes the event to the literal `default` directory
  (with a warning).
- `hash` is the first 16 lowercase hex chars of the sha256 of the uncompressed
  JSONL body. Identical content produces an identical key; different content
  produces a different key.
- File content is the raw Kafka payload bytes, one event per line, in
  consumption order — events are never re-serialized.
- One file per `(tenant, workspace, dt, hour)` per flush cycle, merged across
  all topics and partitions. Per-partition consumption order is preserved
  within a file; cross-partition ordering is not guaranteed — downstream
  consumers sort by `arrived_at` as needed.
- Objects are written with `If-None-Match: *`. A `412 PreconditionFailed`
  (same key already exists) is treated as success: same hash means same
  content.
- Each object carries metadata: `source` (from `SOURCE_ID`, omitted when
  unset), `event-count`, `ranges` (`topic:partition:first:last` joined by `;`),
  and `created-at` (RFC3339).

### Quarantine

Events whose payload is not valid JSON, is missing `tenant_id`, or has no
parseable `arrived_at`/`occured_at` are written (raw bytes verbatim, one object
per event) to `_quarantine/`, logged as errors, and otherwise treated like any
other event: they count toward the batch size and their offsets are committed
past. A poison pill never blocks the pipeline.

## Batch-cycle semantics

The service waits until at least `BATCH_MAX_EVENTS` (`n`) events are buffered —
counted across all topics and partitions, quarantined events included — or
until `FLUSH_INTERVAL_SECS` have elapsed since the window started, whichever
comes first:

- **n-trigger**: exactly `n` events are picked up, FIFO. Surplus events stay
  buffered; if the surplus is itself ≥ `n`, the next cycle fires immediately
  with no wait. Backlogs drain in `n`-sized cycles, so no file ever exceeds `n`
  events.
- **timeout**: everything buffered (fewer than `n`) is picked up.
- The window is tumbling: the timer resets after every cycle completes,
  including empty windows (which are no-ops — no file, no commit).

Each cycle uploads all of its files, and only after every upload succeeds does
it commit the consumed Kafka offsets (next offset = max offset + 1 per topic
partition, computed from the picked-up batch only).

## Delivery semantics: at-least-once

Offsets are committed only after all uploads in a cycle succeed. A crash or
rebalance before a commit means events are consumed again and re-uploaded.
Because files are content-hash-named, redelivery usually overwrites the same
key — but different batch boundaries produce different files that contain
overlapping events. **Downstream consumers MUST deduplicate by event `id`.**

Upload failures retry with exponential backoff (5 attempts, 1s base, jitter);
if retries are exhausted the process exits 1 so the container restarts and
replays from the last committed offset.

## Local development

Start the local infrastructure (redpanda on `localhost:19092`, plus an
S3-compatible endpoint such as MinIO):

```bash
docker compose -f tests/docker-compose.yml up -d
```

Run the archiver against it:

```bash
KAFKA_BOOTSTRAP_SERVERS=localhost:19092 \
KAFKA_TOPICS=events \
KAFKA_GROUP_ID=k2s-local \
S3_BUCKET=events-archive \
S3_ENDPOINT=http://localhost:9000 \
S3_FORCE_PATH_STYLE=true \
BATCH_MAX_EVENTS=100 \
FLUSH_INTERVAL_SECS=10 \
RUST_LOG=debug \
cargo run -p kafka-to-s3
```

Unit tests (no infrastructure needed):

```bash
cargo test -p kafka-to-s3
cargo clippy -p kafka-to-s3 --all-targets
cargo fmt -p kafka-to-s3 --check
```
