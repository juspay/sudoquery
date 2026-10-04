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

## Docker compose tasks

Docker requires `sudo` on this machine, so `ps` and `setup` invoke docker
through it (the password prompt appears normally).

### `ps`

`sudo docker compose ps -a` — the containers of this repo's compose project
(`name: sudo-query-dev-53111d` in `docker-compose.yml`), running and exited,
with state, health and mapped ports.

### `setup [services...]`

Starts a chosen subset of the compose stack via `sudo docker compose up -d`.

- **Interactive** (`cargo xtask setup`): lists the services discovered live
  from `docker-compose.yml` and asks which to start. Answer with numbers
  (`1 3`), names (`postgres keycloak`), mixed or comma-separated tokens,
  `all`, or press Enter to start everything.
- **Non-interactive**: pass the services as arguments —
  `cargo xtask setup postgres redpanda`. Unknown names exit with an error
  listing what is available.

The dev stack: `postgres` (host port 5433, initialized from `pg_schema.sql`
on first boot), `redpanda` (19092), `minio` (9000, console 9001) and
`keycloak` (8080).

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
