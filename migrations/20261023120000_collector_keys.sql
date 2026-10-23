-- collector_keys: per-project API keys for the collector's authenticated
-- endpoint, sent as `Authorization: Bearer sk_<kid>_<secret>`.
--
--   kid            random and public; names the key in the bearer token
--   hashed_secret  sha256 of the secret's 32 random bytes; the secret itself
--                  is shown to its creator once and stored nowhere
--
-- dashboard-server creates, lists and revokes keys. The collector only reads
-- this table, and `projects` and `organizations` to resolve a key's org.
--
-- Replaces project_tokens, whose tokens were stored in plain text and never
-- checked by anything.

DROP TABLE public.project_tokens;

CREATE TABLE public.collector_keys (
    kid text PRIMARY KEY,
    project_id text NOT NULL REFERENCES public.projects(id),
    hashed_secret bytea NOT NULL CHECK (octet_length(hashed_secret) = 32),
    key_name text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    expires_at timestamp with time zone,
    revoked_at timestamp with time zone
);

-- Dashboard lookups: the keys of a project.
CREATE INDEX idx_collector_keys_project_id ON public.collector_keys USING btree (project_id);
