-- project_status: a project's infrastructure (Kafka topic, ClickHouse database
-- and user, OpenSearch data stream) is provisioned asynchronously.
--
--   PENDING   created from the dashboard, nothing provisioned yet
--   CREATING  the admin API started provisioning it
--   ERRORED   provisioning stopped at a step; provisioning_error says which
--   ACTIVE    everything is provisioned
--
-- Existing projects were provisioned when they were created, so they are
-- ACTIVE; new projects start PENDING.

CREATE TYPE public.project_status AS ENUM (
    'PENDING',
    'CREATING',
    'ERRORED',
    'ACTIVE'
);

ALTER TABLE public.projects
    ADD COLUMN status public.project_status DEFAULT 'ACTIVE' NOT NULL,
    ADD COLUMN provisioning_error jsonb;

ALTER TABLE public.projects ALTER COLUMN status SET DEFAULT 'PENDING';
