# sink-opensearch

Consumes canonical events from Kafka and indexes them in OpenSearch through the `_bulk` API.

- **Delivery:** at-least-once. Offsets are committed only after OpenSearch or the dead letter queue (DLQ) has acknowledged every record.
- **No duplicates:** every document is a `create` with `_id` set to the event's `id`, so a replay, or a client resending an event, finds the document already there.
- **Nothing dropped silently:** every record ends up indexed, in the DLQ, or blocking its partition with an error in the logs.

Design and delivery semantics are in [SPEC.md](SPEC.md).

## Run locally

```bash
docker compose -f tests/docker-compose.yml up -d redpanda opensearch opensearch-init \
  opensearch-dashboards opensearch-dashboards-init console
docker exec redpanda rpk cluster health --watch --exit-when-healthy
docker exec redpanda rpk topic create events.generic events.generic.dlq -p 3

SINK_CONFIG=crates/sink-opensearch/cac.toml LOG_FORMAT=pretty \
  cargo run -p sink-opensearch
```

Web UIs:
- **OpenSearch Dashboards:** http://localhost:5601. Discover opens on the `events-*` index pattern, timed by `@timestamp` (`occured_at`), over the last 7 days. Widen the time range for older events.
- **Redpanda Console:** http://localhost:8081, for topics, the DLQ and consumer groups.

To see events flow end to end, run the collector alongside it (`KAFKA_BOOTSTRAP_SERVERS=localhost:19092 cargo run -p event-collector`), post the sample events from the root README, then:

```bash
curl 'localhost:9200/events-merchant-1/_search?pretty'
```

## Configuration

Settings live in a CAC (Superposition) file, read the same way the event collector reads its `cac.toml`. The path comes from `SINK_CONFIG` (default `cac.toml` in the working directory). [`cac.toml`](cac.toml) has every key, commented, pointing at the local stack.

The differences from the collector's file:

- **One key per setting,** named `section.name`: `"batch.max_docs"`, `"opensearch.index"`, and so on. That way an override can change one setting without repeating a whole section.
- **Unknown or misspelled keys** stop the sink at startup.
- **Only `opensearch.index` is resolved per org,** for each event's `org_id` and `proj_id`, so `[[overrides]]` on those dimensions change where an org's events go. Everything else is resolved once at startup with no org, and org overrides of it are ignored.
- **Org overrides apply live.** The sink re-reads the file every 30 seconds and re-resolves each org's index at most every 30 seconds, so an org override applies within about a minute without a restart. Other settings need a restart.

Every key can also be set from the environment, and **the environment wins over the file.** The variable is the key in upper case with `.` replaced by `_`. Empty values count as unset.

A `.env` file in the working directory, or the nearest parent directory that has one, is loaded at startup. Variables already set in the environment win over it, and a malformed `.env` stops the sink.

| Key | Variable | Default | Notes |
|---|---|---|---|
| `kafka.topics` | `KAFKA_TOPICS` | required | Topics to consume. In the variable, comma-separated: `events.a,events.b` |
| `kafka.group_id` | `KAFKA_GROUP_ID` | required | Consumer group |
| `kafka.client_config` | `KAFKA_CLIENT_CONFIG` | `{}` | Raw librdkafka properties; `bootstrap.servers` is required. The sink always sets `group.id`, `enable.auto.commit=false` and `enable.auto.offset.store=false`. The variable is a JSON object of strings, merged into the file's properties key by key |
| `opensearch.url` | `OPENSEARCH_URL` | required | `http` or `https`; a path prefix is kept |
| `opensearch.index` | `OPENSEARCH_INDEX` | required | Index, alias or data stream to write to. `{org_id}` is replaced with each event's org, e.g. `events-{org_id}`. Can be overridden per org; see [Per-org indexes](#per-org-indexes). The variable applies to every org, so per-org overrides are ignored while it is set |
| `opensearch.request_timeout_ms` | `OPENSEARCH_REQUEST_TIMEOUT_MS` | `30000` | Per bulk request |
| `batch.max_docs` | `BATCH_MAX_DOCS` | `1000` | Flush a partition's buffer at this many records |
| `batch.max_bytes` | `BATCH_MAX_BYTES` | `5242880` | Flush at this many bytes; a bigger single document goes to the DLQ |
| `batch.linger_ms` | `BATCH_LINGER_MS` | `1000` | Longest a record waits before its buffer is flushed |
| `batch.max_in_flight` | `BATCH_MAX_IN_FLIGHT` | `8` | Concurrent bulk requests across all partitions |
| `retry.initial_backoff_ms` | `RETRY_INITIAL_BACKOFF_MS` | `100` | Exponential backoff with full jitter |
| `retry.max_backoff_ms` | `RETRY_MAX_BACKOFF_MS` | `30000` | |
| `dlq.topic` | `DLQ_TOPIC` | required | Must exist, and must not be in `kafka.topics` |
| `commit.interval_ms` | `COMMIT_INTERVAL_MS` | `5000` | Periodic offset commit |
| `shutdown.grace_ms` | `SHUTDOWN_GRACE_MS` | `25000` | Time to finish writing after SIGTERM; keep it below the orchestrator's termination grace period |
| `server.addr` | `SERVER_ADDR` | `0.0.0.0:9464` | `/health` and `/metrics` |

Other variables:

| Variable | Effect |
|---|---|
| `SINK_CONFIG` | Path of the CAC file (default `cac.toml`) |
| `KAFKA_BOOTSTRAP_SERVERS` | Overrides `bootstrap.servers`, winning over `KAFKA_CLIENT_CONFIG` too |
| `OPENSEARCH_USERNAME`, `OPENSEARCH_PASSWORD` | Basic auth; accepted only from the environment, and both or neither |
| `RUST_LOG` | Log filter (default `info`) |
| `LOG_FORMAT` | `pretty` for human-readable logs; JSON otherwise |

## Per-org indexes

With `"opensearch.index" = "events-{org_id}"`, each org's events go to their own index (`events-merchant-1`, `events-merchant-2`, …). Orgs can't collide on field types, and each org's data can be kept, sized or deleted on its own.

CAC overrides change the index for one org, or for one of its projects:

```toml
# A dedicated index for a big org.
[[overrides]]
_context_ = { org_id = "merchant-1" }
"opensearch.index" = "events-merchant-1-dedicated"

# Several small orgs in one shared index, to save shards. Queries on it
# must filter by org_id.
[[overrides]]
_context_ = { org_id = "merchant-2" }
"opensearch.index" = "events-shared"
```

- **Invalid overrides:** if an org's configured index isn't a valid name, that org's events go to the DLQ as `invalid_index` and other orgs are unaffected. Fix the override and replay from the DLQ.

- **Org IDs are used as-is.** An ID that can't be part of an index name (uppercase letters, spaces, `\ / * ? " < > | , # :`, or a name over 255 bytes) sends the event to the DLQ as `invalid_org`. IDs are never lowercased or cleaned up, because two orgs could then end up in the same index.
- **Keep every index name under `events-*`,** overrides included, so the [index template](#index-template) applies to it.
- **Watch the shard count.** Every org costs at least one shard per index, plus replicas. Use one primary shard per org index, and roll over by size rather than by day, so small orgs don't pile up near-empty indexes.
- **Query by exact name, not a wildcard.** `events-merchant-1*` also matches org `merchant-1-eu`. Query the org's own index, alias or data stream name.

## Index template

[`index-template.json`](index-template.json) sets up every `events-*` index: one primary shard, explicit types for the event's fixed fields, and `properties` as a `flat_object`.

**Why `flat_object` for `properties`:** properties differ between events and change over time, and `flat_object` keeps them generic.
- **Nothing to maintain:** no schema per property; any keys, any nesting.
- **Type changes are fine:** `amount` can be `100` in one event and `"100 INR"` in the next, and neither is rejected.
- **The field count doesn't grow:** `properties` is one field however many keys appear.
- **What works:** exact match on a property, including nested ones (`properties.plan: premium`, `properties.cart.coupon: SAVE10`).
- **What doesn't:** values compare as text, so numeric ranges (`amount >= 50`), sums, averages and grouping by a property's value don't work. Do those analyses from S3, which gets every event. If one property really needs numeric queries in OpenSearch, copy just that property into a typed field.

**Event time is `occured_at`:** when the user did something, as reported by the client. `@timestamp` is an alias for it, so OpenSearch Dashboards and other tools that look for `@timestamp` sort and filter by when things happened, not when they arrived. For example, a phone that was offline for an hour still has its events placed where they happened. Choose `@timestamp` as the time field when creating an index pattern. `arrived_at`, the collector's clock, stays available for operational questions: what arrived recently, and ingestion lag (`arrived_at − occured_at`).

Other field choices:
- **Exact-match keywords:** `org_id`, `name`, `session_id` and the other IDs.
- **Dates:** `occured_at` and `arrived_at`.
- **`system_properties.ip_address`:** an `ip` field that ignores malformed values, so a junk `x-forwarded-for` header can't get an event rejected.

The sink doesn't install the template itself, since that needs cluster-level permissions. The local stack's `opensearch-init` service installs it on startup; other environments install it at deploy time, before the sink starts:

```bash
curl -X PUT "$OPENSEARCH_URL/_index_template/events" \
  -H 'Content-Type: application/json' \
  --data-binary @crates/sink-opensearch/index-template.json
```

A template only applies to indexes created after it's installed. Existing indexes keep their mapping until they're reindexed.

## Operating it

- **Exit codes:** 0 when everything consumed was written and committed before exiting; 1 on bad config, a fatal consumer error, or when the shutdown grace period ran out. Unwritten records are safe either way: their offsets were never committed.
- **A partition stops moving:** look for `write failed; retrying` at error level. A missing index, a disk-full or read-only cluster block, bad credentials and rejected requests block the partition on purpose, so good data never lands in the DLQ; fix the cause and the sink resumes by itself.
- **The DLQ can hold repeats:** a sink that crashes, or loses a partition, after dead-lettering a record but before committing its offset leaves that record to be dead-lettered again. Count unique `dlq.source.partition` + `dlq.source.offset` pairs, not messages.
- **The DLQ topic:** each dead letter keeps the original key and payload bytes, with these headers: `dlq.source.topic`, `dlq.source.partition`, `dlq.source.offset`, `dlq.error.class` (`decode`, `invalid_org`, `invalid_index`, `too_large` or `rejected`), `dlq.error.reason`, `dlq.error.status` (HTTP status, when there is one), `dlq.attempts`, `dlq.failed_at`. To replay after a fix, run a sink with the DLQ as its topic.
- **Consumer lag:** comes from Kafka (for MSK, the consumer-group lag metrics in CloudWatch), not from this service.

### Endpoints

- `GET /health`: 200 while the event loop is running; 503 if it hasn't ticked for 30 seconds.
- `GET /metrics`: Prometheus text format.

| Metric | Type | Labels |
|---|---|---|
| `sink_records_consumed_total` | counter | `topic` |
| `sink_docs_written_total` | counter | `result`: `created`, or `already_written` for a replay or resent event OpenSearch already had (409) |
| `sink_dlq_records_total` | counter | `class` |
| `sink_retries_total` | counter | `class`: `request` or `item` |
| `sink_bulk_duration_seconds` | histogram | |
| `sink_partitions_paused` | gauge | Partitions paused because their buffer is full |

## Tests

```bash
cargo test -p sink-opensearch                 # unit tests, no services needed

docker compose -f tests/docker-compose.yml up -d redpanda opensearch
cargo test -p sink-opensearch -- --ignored    # end-to-end against the local stack
```

The end-to-end tests cover:
- the happy path
- a crash mid-stream
- mapping conflicts going to the DLQ
- an OpenSearch outage (the test pauses the `opensearch` container)
- per-org indexes with a CAC override
- `properties` changing shape under the index template
- events sent out of order being sorted by `occured_at`

Each test makes its own topics, index and consumer group; the template test installs its own copy of the template.

### Load test

[`scripts/loadtest-opensearch-sink.py`](../../scripts/loadtest-opensearch-sink.py) runs real sink processes against the local stack while they rebalance:

1. It sends about 100,000 events over about 40 seconds. 5% of them are sent twice, like client retries, and 0.1% have an invalid org.
2. Meanwhile four sink instances join, one is killed with `kill -9`, and one is stopped gracefully.

It then checks:
- every valid event is in OpenSearch exactly once
- each org has its own index
- every bad event is in the DLQ
- every offset is committed
- graceful stops exit 0

It prints a rebalance timeline and counts re-reads and writes OpenSearch already had (409):

```bash
cargo build --release -p sink-opensearch
python3 scripts/loadtest-opensearch-sink.py                   # --events 300000 --partitions 12 --pace 10
```

It uses its own topics, consumer group and `events-lt-<run>-*` indexes, and keeps them so you can explore the data in Dashboards and the Console. It prints the commands to delete them, or pass `--clean-up` to delete them at the end. It never touches anything it didn't create. Sink logs are kept in `target/loadtest/<run>/`. The consumer session timeout is lowered to 10 seconds so a crashed instance leaves the group quickly.
