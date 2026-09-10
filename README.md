# Event Collector

HTTP event ingestion service. Accepts events over HTTP, enriches them (arrival timestamp, client IP, country), converts them to a canonical format, and publishes them to Kafka keyed by `anon_id`.

## Image

Built and pushed to ECR by Jenkins on every commit:

```
223655089699.dkr.ecr.ap-south-1.amazonaws.com/cdp:event-collector-<short-commit-sha>
```

The tag suffix is the short commit SHA of the build (see the Jenkins build's `Commit id` stage).

### Pull

```bash
aws ecr get-login-password --region ap-south-1 | \
  docker login --username AWS --password-stdin 223655089699.dkr.ecr.ap-south-1.amazonaws.com

docker pull 223655089699.dkr.ecr.ap-south-1.amazonaws.com/cdp:event-collector-<short-commit-sha>
```

### Versioning

Releases are derived from [conventional commit](https://www.conventionalcommits.org) messages. Every commit publishes an `event-collector-<sha>` image; commits containing at least one releasable change additionally publish `event-collector-<version>` and create the git tag `event-collector-v<version>`:

| Commit message | Bump |
|---|---|
| `feat!:` / any `type!:` / `BREAKING CHANGE:` footer | major |
| `feat:` | minor |
| `fix:` | patch |
| `chore:`, `docs:`, anything else | no release |

Rules:

- Only commits since the previous `event-collector-v*` tag are considered
- With no previous tag the base is `0.0.0`, so the first `feat:` ships `0.1.0`
- The version git tag is only created after the image push succeeds, so a tag always corresponds to a published image
- Re-running the pipeline on an already-tagged commit produces no new version

## Run

```bash
docker run -d --name event-collector \
  -p 3000:3000 \
  -e KAFKA_BOOTSTRAP_SERVERS=broker1:9092,broker2:9092 \
  -e KAFKA_TOPIC=events \
  223655089699.dkr.ecr.ap-south-1.amazonaws.com/cdp:event-collector-<short-commit-sha>
```

The container listens on `0.0.0.0:3000`.

## Configuration

### Environment variables

Server settings come from the environment (never from `cac.toml`):

| Variable | Default | Effect |
|---|---|---|
| `SERVER_ADDR` | `0.0.0.0:3000` | Socket address to listen on |
| `SERVER_ACCEPT_CORS` | `false` | `true`/`false` (case-insensitive); enables permissive CORS |

Kafka settings from the environment take priority over the bundled `cac.toml`:

| Variable | Effect |
|---|---|
| `KAFKA_TOPIC` | Overrides `kafka_connector.topic` |
| `KAFKA_BOOTSTRAP_SERVERS` | Overrides `client_config["bootstrap.servers"]` |
| `KAFKA_CLIENT_CONFIG` | JSON object merged into `client_config`, e.g. `'{"message.timeout.ms":"10000"}'` |

Precedence for `bootstrap.servers`: `KAFKA_BOOTSTRAP_SERVERS` > `KAFKA_CLIENT_CONFIG` > `cac.toml`. Empty or whitespace-only values are treated as unset. Invalid JSON in `KAFKA_CLIENT_CONFIG` fails requests with a 500 naming the variable.

Only server and Kafka settings are env-driven. To change anything else (enrichment, allowed events), mount a custom config:

```bash
docker run -d -p 3000:3000 \
  -e KAFKA_BOOTSTRAP_SERVERS=broker:9092 \
  -v $(pwd)/cac.toml:/app/cac.toml:ro \
  .../cdp:event-collector-<short-commit-sha>
```

`cac.toml` sections:

- `kafka_connector` — `topic` and `client_config` (librdkafka properties: `bootstrap.servers`, `message.timeout.ms`, `socket.timeout.ms`, ...)
- `enrichment` — `arrived_at` (`enabled` default true, `override_existing`), `ip_address` (`override_existing`), `country` (`override_existing`, resolved from IP)
- `allowed_events` — allowlist of event names; absent means all events are accepted

Config is resolved per request. `x-tenant-id` / `x-workspace-id` are used as CAC dimension context, so per-tenant overrides in `cac.toml` apply automatically.

### Authentication

The authenticated events endpoint (`POST /cdp/collect/events/authenticated`) validates a bearer token against a secret decrypted from a cloud KMS. The token is decrypted once and cached for the lifetime of the process; rotating it requires a restart.

| Variable | Required | Purpose |
|---|---|---|
| `TOKEN_PROVIDER` | yes | `aws` or `gcp` (case-insensitive) |

#### AWS

| Variable | Required | Purpose |
|---|---|---|
| `KMS_CIPHERTEXT` | yes | Base64 KMS ciphertext of the token |
| `KMS_ENCRYPTION_CONTEXT` | no | JSON object; must match the context used at encrypt time, e.g. `'{"app":"my-app"}'` |

Standard AWS SDK credentials are used (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, or an instance role). Generate the ciphertext with:

```bash
aws kms encrypt \
  --key-id alias/my-key \
  --plaintext "my-secret-token" \
  --encryption-context '{"app":"my-app"}' \
  --query CiphertextBlob --output text
```

#### GCP

| Variable | Required | Purpose |
|---|---|---|
| `GCP_KMS_KEY` | yes | Full CryptoKeyVersion name: `projects/P/locations/L/keyRings/R/cryptoKeys/K/cryptoKeyVersions/V` |
| `GCP_KMS_CIPHERTEXT` | yes | Base64 KMS ciphertext of the token |
| `GCP_KMS_AAD` | no | Additional authenticated data; must match the AAD used at encrypt time |

Credentials are resolved via Application Default Credentials (`GOOGLE_APPLICATION_CREDENTIALS` or the metadata server).

## API

### Headers

| Header | Required | Purpose |
|---|---|---|
| `x-tenant-id` | yes | Tenant context for config resolution |
| `x-workspace-id` | no | Workspace context for config resolution |
| `x-forwarded-for` / `x-real-ip` | no | Client IP for enrichment; falls back to peer address |
| `authorization` | only for `/events/authenticated` | `Bearer <token>`; see [Authentication](#authentication) |

### `POST /cdp/collect/events`

Body is newline-delimited JSON (one event object per line):

```bash
curl -X POST http://localhost:3000/cdp/collect/events \
  -H "x-tenant-id: merchant-1" \
  -H "content-type: application/json" \
  --data-binary '{"envelop_version":"1.0","id":"0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c","name":"payment_initiated","tenant_id":"merchant-1","anon_id":"anon-42","occured_at":"2026-09-02T10:30:00Z","properties":{"amount":100,"currency":"INR"}}
{"envelop_version":"1.0","id":"3d1c2b9a-52e7-4f8a-9a1b-6c7d8e9f0a1b","name":"checkout_viewed","tenant_id":"merchant-1","anon_id":"anon-42","occured_at":"2026-09-02T10:29:00Z"}'
```

Event fields:

| Field | Required | Notes |
|---|---|---|
| `envelop_version` | yes | `"1.0"` |
| `id` | yes | UUID |
| `name` | yes | Event name, checked against `allowed_events` |
| `tenant_id` | yes | |
| `anon_id` | yes | Used as the Kafka message key |
| `occured_at` | yes | RFC 3339 timestamp |
| `workspace_id`, `session_id`, `actor_id`, `source`, `correlation_id`, `trace_id` | no | |
| `properties` | no | Arbitrary JSON |
| `system_properties` | no | `{ geo: { country }, timezone, ip_address }` |

### `POST /cdp/collect/events/authenticated`

Same body format as `POST /cdp/collect/events`, but requires a bearer token (see [Authentication](#authentication)). Events collected here carry `"authenticated": true` in the canonical event; events from the other endpoints carry `"authenticated": false`:

```bash
curl -X POST http://localhost:3000/cdp/collect/events/authenticated \
  -H "x-tenant-id: merchant-1" \
  -H "authorization: Bearer my-secret-token" \
  -H "content-type: application/json" \
  --data-binary '{"envelop_version":"1.0","id":"0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c","name":"payment_initiated","tenant_id":"merchant-1","anon_id":"anon-42","occured_at":"2026-09-02T10:30:00Z","properties":{"amount":100,"currency":"INR"}}'
```

Requests with a missing or invalid token return `401` with a `WWW-Authenticate: Bearer` header.

### `POST /cdp/collect/batch`

Body is a single JSON object with an events array and optional batch-level system properties:

```bash
curl -X POST http://localhost:3000/cdp/collect/batch \
  -H "x-tenant-id: merchant-1" \
  -H "content-type: application/json" \
  -d '{"events":[{"envelop_version":"1.0","id":"0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c","name":"payment_initiated","tenant_id":"merchant-1","anon_id":"anon-42","occured_at":"2026-09-02T10:30:00Z"}],"system_properties":{"timezone":"Asia/Kolkata"}}'
```

### Response

All ingest endpoints return the collection status:

```json
{"filtered": 0, "collected": 2, "total": 2}
```

`filtered` counts events rejected by `allowed_events`; `collected` counts events published to Kafka.

### `GET /cdp/collect/health`

Returns `200 {"status":"ok"}` when Kafka metadata is reachable, `503` otherwise. Suitable for load balancer health checks.

### Status codes

| Code | Meaning |
|---|---|
| 400 | Invalid JSON, missing `x-tenant-id`, or non-UTF-8 body |
| 401 | Missing or invalid bearer token on `/events/authenticated` |
| 502 | Kafka delivery failure |
| 500 | Other server-side errors |
| 503 | Health check could not reach Kafka |

## Local development

```bash
# build the image locally
./scripts/build-image.sh event-collector:local

# start redpanda (Kafka on localhost:19092, console on localhost:8081)
docker compose -f tests/docker-compose.yml up -d
```

The bundled `cac.toml` defaults point at `host.docker.internal:19092`, so the container can talk to the local redpanda directly.
