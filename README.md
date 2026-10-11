# xtask — dev task runner

One entry point for the common cargo, docker compose, and database workflows
in this workspace.

```bash
cargo xtask <TASK>
```

The `cargo xtask` alias (`.cargo/config.toml`) runs `cargo run -p xtask --release --`,
so tasks always execute from the workspace root, regardless of the current
directory. The first invocation compiles the binary; later runs are fast.

`cargo xtask ls` (or `cargo xtask` with no task) prints the task list.

## Tasks

| Task | What it does |
|---|---|
| `fmt [--check]` | Format the workspace (check only with `--check`) |
| `clippy` | Lint the workspace, warnings are errors |
| `test` | Run all workspace tests |
| `ci` | `fmt --check`, `clippy` and `test` — the CI gate |
| `ls` | List available tasks |
| `ps` | Show this repo's docker compose containers and status |
| `setup [svc...]` | Choose docker compose services to start |
| `db status` | Check the database is up and all migrations are applied |
| `db migration` | Apply pending migrations |
| `db migration add <name>` | Create a new migration file |
| `e2e dashboard [filter]` | End-to-end org/project tests against dashboard-server |
| `e2e collector [filter]` | End-to-end ingestion tests through events-collector into Kafka |
| `e2e kafka-to-s3 [filter]` | End-to-end archiving tests through kafka-to-s3 into S3 |

## Docker compose tasks

`ps` and `setup` run plain `docker`; they never call `sudo` themselves. If
docker needs root on your machine, run the task under sudo. Use the built
binary rather than `sudo cargo …`, so cargo doesn't run as root (root
usually has no toolchain on its `PATH`, and would leave root-owned files in
`target/`):

```sh
cargo build -p xtask --release          # once, or after changing xtask
sudo target/release/xtask setup postgres
sudo target/release/xtask ps
```

Adding yourself to the `docker` group avoids sudo altogether.

### `ps`

`docker compose ps -a` — the containers of this repo's compose project
(`name: sudo-query-dev-53111d` in `docker-compose.yml`), running and exited,
with state, health and mapped ports.

### `setup [services...]`

Starts a chosen subset of the compose stack via `docker compose up -d`.

- **Interactive** (`cargo xtask setup`): lists the services discovered live
  from `docker-compose.yml` and asks which to start. Answer with numbers
  (`1 3`), names (`postgres keycloak`), mixed or comma-separated tokens,
  `all`, or press Enter to start everything.
- **Non-interactive**: pass the services as arguments —
  `cargo xtask setup postgres redpanda`. Unknown names exit with an error
  listing what is available.

The dev stack: `postgres` (host port 5433, initialized from `pg_schema.sql`
on first boot), `redpanda` (19092), `minio` (9000, console 9001), `keycloak`
(8080), `clickhouse` (8123 HTTP, 19000 native TCP; starts empty, and
dashboard-server creates a database per project from `ckh/project.sql`) and
`chproxy` (9090).

Every project gets its own Kafka topic (`events.{org_id}.{project_id}`) and
ClickHouse database, both created by dashboard-server when the project is
created and dropped when it is deleted.

## Database tasks

Migration tooling for the Postgres dev database, built on the sqlx library —
no `sqlx-cli` install required.

Connection: `DATABASE_URL` env var if set, otherwise
`postgres://hyper_analytics_user:hyper_analytics@localhost:5433/hyper_analytics`
(the root compose postgres). The URL is password-masked in all output.

Migrations live in `migrations/` at the workspace root, one file per
migration, named `<version>_<description>.sql` (sqlx convention, versions are
`YYYYMMDDHHMMSS` timestamps). The first file, `20260930000000_baseline.sql`,
is the frozen base schema generated from `pg_schema.sql` — never edit it.

### `db status`

Waits up to 30s for the database to accept connections, then reports the
migration state:

```
database: up (postgres://hyper_analytics_user:***@localhost:5433/hyper_analytics)
migrations: 3 applied, 0 pending
```

Exit code 0 only when the database is up, no migration is pending and no
migration is dirty (a migration whose transaction failed and needs a manual
fix — reported as a warning). Read-only: never writes to the database.

### `db migration`

Applies pending migrations:

```
waiting for database (up to 30s): postgres://hyper_analytics_user:***@...
baselined 1 existing migration(s) against the existing schema (pg_schema.sql is the baseline; not re-run)
applying 1 migration(s):
  20261004120000_add_users_table add users table
migrations: 4 applied, 0 pending
```

- **Auto-baseline**: a database that already has the `pg_schema.sql` schema
  (docker-initialized volume) but no migration history gets its known
  migrations recorded as applied without re-running them.
- A completely empty database is migrated from scratch, baseline included.
- Each migration runs in its own transaction; already-applied migrations are
  checksum-validated (editing an applied migration file fails loudly).

### `db migration add <name>`

Creates an empty `migrations/<timestamp>_<name>.sql` scaffold. The name must
match `^[a-z0-9_]+$`. Write your SQL into it, then apply with
`cargo xtask db migration`.

### Workflow

1. `cargo xtask setup postgres` (or the full stack)
2. `cargo xtask db migration add my_change`
3. edit `migrations/<timestamp>_my_change.sql`
4. `cargo xtask db migration`
5. `cargo xtask db status` — must report `0 pending`

Keep `pg_schema.sql` in sync: after applying migrations, re-dump the schema
so fresh docker volumes match — the auto-baseline assumes `pg_schema.sql`
covers every applied migration.

## End-to-end tests

`cargo xtask e2e <suite> [filter]` builds a service, starts it on a free
localhost port, and drives it over HTTP. Cases run sequentially; `[filter]`
runs only those whose name contains it (e.g. `cargo xtask e2e dashboard
project_`). Exit code 1 if any case fails, with the service log tail printed.

### `e2e dashboard`

Organization and project creation against `dashboard-server`. Needs the
compose postgres, redpanda and clickhouse (`cargo xtask setup postgres
redpanda clickhouse`):

- **Database:** a throwaway `dashboard_e2e_<random>` database is created on
  the dev Postgres (`DATABASE_URL` or the default above), migrated from
  scratch, and dropped afterwards. Dev data is never touched.
- **Auth:** an in-process dummy OIDC provider serves the realm JWKS and mints
  RS256 access tokens shaped like Keycloak's. Keys are generated per run.
- **Projects:** creation is checked against the real Kafka topic and
  ClickHouse database it provisions: the database's objects, the project
  user's grants, an event produced to the topic reaching the project's
  tables, and every query path (`/query`, `/events`, `/events/today/count`,
  `/event_props`) reading only the requesting project's database. Deletion
  must drop the topic, database and user. A second server with a wrong
  ClickHouse admin password produces real provisioning failures.
- **Cleanup:** every project database, ClickHouse user and topic the run
  created is dropped. `CLICKHOUSE_URL`, `CLICKHOUSE_USER`,
  `CLICKHOUSE_PASSWORD` and `CLICKHOUSE_KAFKA_BROKERS` override the compose
  defaults.
- **Server:** runs from a scratch directory (`DASHBOARD_BIND_ADDR` sets the
  port), so a repo `.env` is not loaded.

### `e2e collector`

Event ingestion through `events-collector` into Kafka. Needs the compose
redpanda and localstack (`cargo xtask setup redpanda localstack`), or any
brokers in `KAFKA_BOOTSTRAP_SERVERS` and any KMS at `LOCALSTACK_URL`:

- **Topics:** the collector runs with `KAFKA_TOPIC=e2e-collector-<run>.{org_id}.{project_id}`,
  so every test org/project publishes to its own topic. Cases create those
  topics up front and the run deletes them at the end.
- **Checks:** events are read back from Kafka and checked for ids, org and
  project, enrichment (`arrived_at`, client IP and country), batch system
  properties, the record key, per-project topic isolation, and that rejected
  requests (header/body mismatch, missing or invalid ids, no bearer token)
  publish nothing.
- **Authenticated endpoint:** its bearer token is a KMS ciphertext. Cases
  start their own collector with a token encrypted by a fresh localstack
  KMS key: the right token publishes and a wrong one gets 401; a ciphertext
  KMS cannot decrypt is reported by `/v1/health` and refused with 500.
- **Server:** reads the real `crates/events-collector/cac.toml` (via
  `COLLECTOR_CONFIG`).

### `e2e kafka-to-s3`

Archiving through `kafka-to-s3` into S3. Needs the compose redpanda and minio
(`cargo xtask setup redpanda minio`); `S3_ENDPOINT`, `S3_ACCESS_KEY` and
`S3_SECRET_KEY` override the MinIO defaults. Each run uses its own topics and
bucket, deleted afterwards, and checks object keys, contents, metadata,
batching, quarantine, multi-topic and `^`-pattern subscriptions.
