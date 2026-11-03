-- clickhouse-proxy's own tables. They live in its own schema
-- (DATABASE_SCHEMA, default clickhouse_proxy), next to its own
-- _sqlx_migrations, so the proxy can share a Postgres database with other
-- services without touching their tables or migration history.

-- Saved SQL consoles. `owner` is whatever the caller names its users by: a
-- dashboard-server user id, a CDP dashboard email.
CREATE TABLE IF NOT EXISTS consoles (
    id           uuid PRIMARY KEY,
    workspace_id text NOT NULL,
    owner        text NOT NULL,
    name         text,
    console      text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS consoles_by_owner ON consoles (workspace_id, owner, created_at DESC);

-- What each event in a workspace means.
CREATE TABLE IF NOT EXISTS event_descriptions (
    id           uuid PRIMARY KEY,
    workspace_id text NOT NULL,
    event_name   text NOT NULL,
    description  text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, event_name)
);

-- What each property of an event means.
CREATE TABLE IF NOT EXISTS property_descriptions (
    id            uuid PRIMARY KEY,
    workspace_id  text NOT NULL,
    event_name    text NOT NULL,
    property_name text NOT NULL,
    property_type text NOT NULL,
    description   text NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, event_name, property_name)
);
