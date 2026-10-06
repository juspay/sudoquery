# sink-opensearch: Spec

Revised 2026-09-25 to match what v1 implements. The original, broader spec was trimmed; what was cut and why is under [Deferred](#deferred).

## Overview

`sink-opensearch` consumes canonical events from Kafka and writes them to OpenSearch with the `_bulk` API. It is at-least-once, writes idempotently, and never drops data silently: every record ends up indexed, in the dead letter queue (DLQ), or blocking its partition with an error in the logs.

### Goals

- At-least-once delivery: a partition's offsets are committed only after OpenSearch or the DLQ has acknowledged every record up to that offset.
- Idempotent writes: `create` with `_id` = the event's `id`, so replays and client resends can't create duplicates.
- Correct handling of partial bulk failures, retries with backoff, backpressure, rebalances and graceful shutdown.
- Bounded memory, a single binary, and enough observability to operate it.

### Non-goals (v1)

- Exactly-once delivery end to end.
- Installing index templates or ISM policies at runtime. The repo ships `index-template.json` for `events-*` (typed envelope fields, `properties` as `flat_object`); the local stack installs it, and other environments install it at deploy time.
- Payloads other than JSON canonical events.
- Any destination other than OpenSearch.

## Architecture

```
Kafka ──▶ event loop ──batch──▶ write task (≤1 per partition) ──▶ OpenSearch _bulk
            ▲   │                     │
            │   │                     └──rejects──▶ DLQ topic
            │   └── pause / resume
            │
      offset tracker ◀── result (epoch-checked)
            │
            └──commit──▶ Kafka
```

The crate has two halves:

- **`runtime/`** is sink-agnostic. It consumes, buffers, batches, retries, dead-letters and commits. It knows only the `Writer` trait, and moves to `sq-sink-runtime` when a second sink needs it.
- **`opensearch/`** implements `Writer`: build the `create` operation, send `_bulk`, and classify the response.

```rust
pub trait Writer: Send + Sync + 'static {
    type Doc: Clone + Send + Sync + 'static;
    fn prepare(&self, payload: Option<&[u8]>) -> Result<Self::Doc, Rejection>;
    fn doc_size(doc: &Self::Doc) -> usize;
    fn write(&self, docs: Vec<Self::Doc>)
        -> impl Future<Output = Result<Vec<ItemOutcome>, WriteError>> + Send;
}
```

### Event loop

One task owns all per-partition state. It selects over four sources: the shutdown token, write results, a tick (every `linger` clamped to 10–100 ms), and `StreamConsumer::recv()`, which is cancellation-safe.

- **On a record:** `prepare` it. A failure becomes a batch entry marked for the DLQ, so every record resolves through one path. Append it to the partition's buffer. If the partition is idle and the buffer is ready, start a write task. If the buffer is full while a batch is in flight, pause the partition.
- **On a result:** ignore it if its epoch is stale. Otherwise record the batch's last offset as complete, start the next batch if one is ready, and resume the partition if its buffer has room.
- **On a tick:** drop state for revoked partitions, flush buffers older than `linger`, and every `commit.interval` start an async commit.

A buffer is ready when it holds `max_docs` records or `max_bytes` bytes, or its oldest record has waited `linger`. A global semaphore caps concurrent bulk requests at `max_in_flight`.

### Write task

A write task owns one batch until every record in it is resolved:

1. Send every prepared document.
2. Retry the whole request on a request-level failure; resend only the failed items on item-level failures. Both use backoff.
3. Send rejected records to the DLQ, retrying until Kafka confirms each one.
4. Report the batch as complete.

It can be cancelled on revoke or when the shutdown grace period runs out; a cancelled batch is never committed.

## Delivery semantics

### Offsets

- Each partition has at most one batch in flight, and a batch completes only when all its records are resolved. Offsets therefore always complete in order, and a partition's commit position is the last completed offset + 1.
- The offset tracker is the only source of committed offsets. It is shared with the consumer callbacks behind a mutex that is never held across an `.await`.
- Commits are async every `commit.interval` and confirmed through the commit callback. A failed commit stays pending and is retried on the next tick.
- They are synchronous on revoke (in `pre_rebalance`) and at shutdown.

### Rebalance

- The protocol is librdkafka's default (eager). Each assignment of a partition gets a new epoch.
- **Revoke:** `pre_rebalance` synchronously commits the partition's completed offset and forgets the partition. It does not wait for in-flight batches: the new owner replays them, and document IDs make the replay harmless.
- **After revoke:** the event loop notices the epoch change, drops the partition's buffer and cancels its in-flight batch. Late results carry the old epoch and are ignored.

### Ordering

A partition's batches are written strictly in order, and a batch's retries finish before the next batch starts, so Kafka's per-partition order is preserved in OpenSearch.

### Document IDs and target index

`_id` is the canonical event's `id` (UUID). The action is always `create`, so a replay gets a 409, which counts as success. `create` also works with data streams.

The target comes from the CAC key `opensearch.index`, resolved for each event's `org_id` and `proj_id`, so CAC overrides can move an org, or one of its projects, to another index. In the resolved value, `{org_id}` is replaced with the event's org: `events-{org_id}` gives each org its own index or data stream.

- **Checks:** the default's fixed parts are checked against OpenSearch's naming rules at startup. Each org's resolved value is checked when the org is first seen, and every full name is checked per event.
- **Invalid org ID:** an org ID that can't form a valid name is dead-lettered as `invalid_org`. Org IDs are never lowercased or cleaned up, because that could put two orgs in one index.
- **Invalid override:** an org whose override isn't a valid template is dead-lettered as `invalid_index`, without affecting other orgs.
- **Caching:** resolved values are cached per org for 30 seconds, and the CAC file is re-read every 30 seconds, so a change applies within about a minute without a restart.

### Event time

The event's time in OpenSearch is `occured_at`, set by the client. `index-template.json` maps `@timestamp` as an alias for it, so tools that default to `@timestamp` sort and filter by when things happened. `arrived_at`, the collector's clock, is kept for freshness and ingestion lag. `occured_at` is always present, so it's also the field to use if indexes later become data streams.

## Errors

The classifier is a pure function with table tests.

| Condition | Handling |
|---|---|
| Transport error or timeout | Retry the request |
| HTTP 429, 502, 503, 504 | Retry the request |
| Any other non-200: 400, 401, 403, 404, 413, 500, … | Retry the request and log at error. It's a credential, config or client bug, and blocking beats dead-lettering good data |
| Unparseable response, or item count differs from the request | Retry the request |
| Item 200, 201 | Done |
| Item 409 | Done: already written |
| Item 429, 5xx | Retry the item |
| Item 400 (mapping or parse errors) | DLQ |
| Any other item 4xx: 404 index missing, 403 disk-full or read-only block, 401 | Retry the item and log at error. These are about the cluster, not the document; dead-lettering would send every event to the DLQ |
| Unknown item status | Retry the item |
| Payload empty or not a canonical event | DLQ, without a write |
| `org_id` can't form a valid index name | DLQ (`invalid_org`), without a write |
| The org's configured `opensearch.index` isn't a valid template | DLQ (`invalid_index`), without a write |
| Document larger than `batch.max_bytes` | DLQ, without a write |

Retries never give up: capped exponential backoff with full jitter, `random(0, min(max, initial × 2^(attempt−1)))`. A partition whose writes keep failing stays blocked, and its paused consumption bounds memory.

Payloads are sent as-is. A payload containing a newline is re-encoded on one line first, since NDJSON can't hold line breaks.

### Backpressure

A partition is paused when its buffer is full while its batch is in flight, and resumed when the buffer has room again. The consumer keeps being polled while partitions are paused, so group membership stays healthy. Memory is bounded by roughly assigned partitions × 2 × `max_bytes`, plus librdkafka's prefetch queue.

### DLQ

- The producer is idempotent, which implies `acks=all`.
- The key and payload are the original bytes.
- Headers: `dlq.source.topic`, `dlq.source.partition`, `dlq.source.offset`, `dlq.error.class` (`decode`, `invalid_org`, `invalid_index`, `too_large`, `rejected`), `dlq.error.reason`, `dlq.error.status` when there is an HTTP status, `dlq.attempts`, `dlq.failed_at`.
- A record is resolved only after Kafka confirms the dead letter. If the DLQ is unavailable, the partition stays blocked.
- The DLQ is at-least-once too. When a partition moves or a sink crashes after a record was dead-lettered but before its offset was committed, the next owner dead-letters it again. Replaying is still safe, because document IDs deduplicate.
- To replay after a fix, run a sink with the DLQ as its topic.

## Shutdown

1. SIGTERM or SIGINT cancels the shutdown token, and the loop stops receiving.
2. Every buffer is flushed without waiting for `linger`. The loop waits for write tasks up to `shutdown.grace`, then cancels what's left.
3. Completed offsets are committed synchronously.
4. The consumer is dropped, which leaves the group, and the DLQ producer is flushed.
5. The process exits 0 if everything drained and 1 otherwise. Undrained records are safe because their offsets were never committed.

A fatal consumer error skips the drain: in-flight batches are cancelled, completed offsets are committed, and the process exits 1.

## Configuration

See [README.md](README.md#configuration). Settings come from a CAC (Superposition) file with one `section.name` key per setting and `org_id` and `proj_id` dimensions, loaded the same way as the collector's `cac.toml`. Every setting can also be set from the environment, with the key in upper case and `.` replaced by `_` (`BATCH_MAX_DOCS` for `batch.max_docs`), and the environment wins over the file. Unknown keys are rejected, and values are validated at startup: index naming rules, URL scheme, DLQ topic not consumed, and both or neither credential. Process-wide settings are resolved once with no org; `opensearch.index` is resolved per org.

## Observability

- JSON logs via `tracing`; `LOG_FORMAT=pretty` for local use.
- Rebalances, commits on revoke and shutdown, and DLQ writes log at info. Retries log at warn, or at error when an operator must act.
- `/health` and `/metrics` on `server.addr`.
- Six metrics, listed in the README.
- Consumer lag comes from Kafka's side (MSK publishes it to CloudWatch).

## Testing

- **Unit tests:** config and validation, document building, response parsing, the classifier table, bulk client behaviour against wiremock (throttling, auth failures, timeouts, mixed items), backoff bounds, batching limits, the offset tracker including stale epochs, and the write task's retry and dead-letter decisions with a fake writer.
- **Integration tests** (`tests/pipeline.rs`, `#[ignore]`, run against `tests/docker-compose.yml`):

| Scenario | Pass condition |
|---|---|
| 1,000 events | Index count is 1,000; a document's source equals its payload; committed offsets equal end offsets |
| Crash mid-stream, then restart | Count equals produced (no loss, no duplicates); committed offsets equal end offsets |
| Mapping conflicts and an undecodable record | Good documents indexed; bad ones in the DLQ with correct headers and original bytes; offsets advance |
| OpenSearch paused, more events produced, then unpaused | Every event indexed; committed offsets equal end offsets |
| `properties` changing shape (number, string, object, new keys) under `index-template.json` | Every event indexed, none rejected; `properties` is a `flat_object`; exact and nested property searches work |
| Events sent in a different order from when they happened | Sorting and range queries on `@timestamp` follow `occured_at` |
| Events from three orgs with `opensearch.index = "…-{org_id}"`, and a CAC override giving one org a dedicated index | Each valid org's index holds exactly its events; the overridden org is only in its dedicated index; the org with uppercase letters goes to the DLQ as `invalid_org` |

## Deferred

Cut from the original spec for v1, with the reason:

| Item | Why it was cut | Revisit when |
|---|---|---|
| Public library API: `Transform` and `BulkClient` traits, builder, `examples/` | One internal caller, and the input is always our own JSON. wiremock covers what `BulkClient` existed for | Another team needs a custom transform |
| Routing mini-language beyond `{org_id}`: field or date placeholders, ID strategies, `index`/`update`/`delete` actions, tombstones, external versioning | Events are immutable, keyed by UUID and never tombstoned; per-org indexes are the only routing needed | Session documents need upserts, or indexes need another dimension |
| One record → many documents, with an out-of-order offset tracker | One record is one document; in-order completion makes the tracker trivial | A topic needs fan-out |
| `max_attempts`, `on_exhausted`, `dlq.enabled` | One behaviour: retryable errors block, permanent errors go to the DLQ | Blocking proves too costly in practice |
| 413 batch halving | Batches are capped at `max_bytes` when built, and oversized documents are dead-lettered up front | The cluster's request limit is below `max_bytes` |
| Global backpressure controller with a resume ratio | Per-partition pause bounds memory already | A single instance owns very many partitions |
| Cooperative-sticky rebalancing; draining in-flight work on revoke | Replays are idempotent, and draining inside the callback risks deadlock | Rebalance pauses become a problem at scale |
| Pluggable decoders: Avro, Protobuf, Schema Registry | We own the producer, and it writes JSON | A producer we don't control is added |
| gzip, custom TLS CA, SigV4, MSK IAM | Not needed for the current clusters | The target cluster requires them |
| 16 metrics, in-app consumer lag | Six cover operation; lag is available from Kafka | A dashboard needs more |
| Chaos suite: 1→3→1 scaling, toxiproxy, criterion benchmarks, memory acceptance test | Four integration scenarios cover the failure paths that matter for v1 | Throughput targets are set |
