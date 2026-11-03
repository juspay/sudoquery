-- collector_keys: the credentials of events-collector's authenticated
-- endpoint (`POST /v1/events/authenticated`).
--
-- The collector only reads this table; whatever creates keys (cdp's or
-- sudoquery's dashboard) owns it and applies this DDL in its database,
-- adding a foreign key to its own workspaces table if it likes. The
-- collector connects with a role that may only SELECT from it, e.g.:
--
--   CREATE ROLE collector LOGIN PASSWORD '…';
--   GRANT SELECT ON collector_keys TO collector;
--
-- A key is `sk_<kid>_<secret>`, sent as `Authorization: Bearer <key>`:
--   kid     16 random lowercase hex characters (8 bytes); public, the row's id
--   secret  32 random bytes as 64 lowercase hex characters
-- Only sha256 of the secret's 32 bytes (not of its hex text) is stored. The
-- full key is shown to the client once, at creation, and kept nowhere else.
-- The hash is not encrypted: 32 random bytes can't be recovered from it.
--
-- A request is scoped by its key: the collector takes org_id and
-- workspace_id from the row (workspace ids are unique only within an org in
-- cdp, so both are needed). Revoking or expiring a key takes effect within
-- the collector's cache lifetime (COLLECTOR_KEYS_CACHE_SECS, default 30).
CREATE TABLE IF NOT EXISTS collector_keys (
  kid           text PRIMARY KEY CHECK (kid ~ '^[0-9a-f]{16}$'),
  org_id        text NOT NULL,
  workspace_id  text NOT NULL,
  hashed_secret bytea NOT NULL CHECK (length(hashed_secret) = 32),  -- sha256(secret)
  key_name      text NOT NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  created_by    text,
  expires_at    timestamptz,
  revoked_at    timestamptz
);

-- Dashboard lookups: "list keys for workspace".
CREATE INDEX IF NOT EXISTS collector_keys_workspace ON collector_keys (org_id, workspace_id);
