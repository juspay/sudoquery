-- ============================================================================
-- Complete ClickHouse schema for the canonical-event pipeline.
--
-- Source of truth: CanonicalEvent in crates/canonical-event (events-collector
-- workspace). Wire format is the serde JSON of that struct; the struct is
-- #[skip_serializing_none], so optional fields are simply absent from Kafka
-- messages and land on each column type's natural default ('' for String,
-- NULL for Nullable, zero UUID for UUID).
--
-- Contents:
--   1. events_v2                fact table, event-name-centric sort
--   2. user_events_v2           fact table, actor-centric sort
--   3. events_kafka             Kafka engine table (canonical wire columns)
--   4. events_v2_mv             events_kafka -> events_v2
--   5. user_events_v2_mv        events_kafka -> user_events_v2
--   6. event_schema_catalog     fact table (description dropped)
--   7. event_schema_catalog_kafka
--   8. event_schema_catalog_mv
--   9. Row-level security       project_user role + row policies
--
-- Column mapping (wire -> fact tables):
--   id -> event_id, name -> event_name, occured_at -> event_timestamp,
--   actor_id -> actor_id, org_id/project_id/session_id/anon_id/source/
--   correlation_id/trace_id/authenticated/properties/arrived_at -> same names,
--   country/timezone/ip_address <- system_properties.geo.country /
--   system_properties.timezone / system_properties.ip_address
--
-- Requires ClickHouse 24.8+ (JSON column type, .:Type path accessors).
-- ============================================================================


-- ============================================================================
-- 1. events_v2 — fact table, event-name-centric sort (successor of events_v1)
-- ============================================================================

CREATE TABLE IF NOT EXISTS default.events_v2
(
    `project_id`      LowCardinality(String),
    `org_id`          LowCardinality(String)     DEFAULT '',
    `event_id`        UUID,
    `event_name`      LowCardinality(String),
    `event_timestamp` DateTime64(3),
    `arrived_at`      Nullable(DateTime64(3))    DEFAULT NULL,
    `session_id`      String                     DEFAULT '',
    `anon_id`         String,
    `actor_id`        String                     DEFAULT '',
    `source`          LowCardinality(String)     DEFAULT '',
    `correlation_id`  String                     DEFAULT '',
    `trace_id`        String                     DEFAULT '',
    `authenticated`   Nullable(Bool)             DEFAULT NULL,
    `properties`      JSON,
    `country`         LowCardinality(Nullable(FixedString(2))) DEFAULT NULL,
    `timezone`        LowCardinality(Nullable(String)) DEFAULT NULL,
    `ip_address`      Nullable(String)           DEFAULT NULL,
    `inserted_at`     DateTime                   DEFAULT now(),
    `updated_at`      DateTime                   DEFAULT now(),
    `is_deleted`      UInt8                      DEFAULT 0
)
ENGINE = ReplacingMergeTree(updated_at, is_deleted)
PARTITION BY toYYYYMM(event_timestamp)
ORDER BY (project_id, event_name, event_timestamp, event_id)
SETTINGS index_granularity = 8192;


-- ============================================================================
-- 2. user_events_v2 — fact table, actor-centric sort (successor of
--    user_events_v1; same column set as events_v2, differs only in ORDER BY)
-- ============================================================================

CREATE TABLE IF NOT EXISTS default.user_events_v2
(
    `project_id`      LowCardinality(String),
    `org_id`          LowCardinality(String)     DEFAULT '',
    `actor_id`        String                     DEFAULT '',
    `anon_id`         String,
    `session_id`      String                     DEFAULT '',
    `event_id`        UUID,
    `event_name`      LowCardinality(String),
    `event_timestamp` DateTime64(3),
    `arrived_at`      Nullable(DateTime64(3))    DEFAULT NULL,
    `source`          LowCardinality(String)     DEFAULT '',
    `correlation_id`  String                     DEFAULT '',
    `trace_id`        String                     DEFAULT '',
    `authenticated`   Nullable(Bool)             DEFAULT NULL,
    `properties`      JSON,
    `country`         LowCardinality(Nullable(FixedString(2))) DEFAULT NULL,
    `timezone`        LowCardinality(Nullable(String)) DEFAULT NULL,
    `ip_address`      Nullable(String)           DEFAULT NULL,
    `inserted_at`     DateTime                   DEFAULT now(),
    `updated_at`      DateTime                   DEFAULT now(),
    `is_deleted`      UInt8                      DEFAULT 0
)
ENGINE = ReplacingMergeTree(updated_at, is_deleted)
PARTITION BY toYYYYMM(event_timestamp)
ORDER BY (project_id, actor_id, event_timestamp, event_id)
SETTINGS index_granularity = 8192;


-- ============================================================================
-- 3. events_kafka — Kafka engine table (no DEFAULT expressions allowed).
--    Column names MUST mirror the serialized CanonicalEvent JSON keys
--    exactly (JSONEachRow maps by name). envelop_version is captured on the
--    wire but not persisted to the fact tables.
--    TODO: reconcile kafka_topic_list with the collector's publish topic
--    (cac.toml: events.generic, overridable via KAFKA_TOPIC). The legacy
--    dashboard-server producer (old-shape EventRow JSON) was removed from the
--    crate; the collector is the sole producer for this topic.
-- ============================================================================

CREATE TABLE IF NOT EXISTS default.events_kafka
(
    `envelop_version`   LowCardinality(String),
    `id`                UUID,
    `name`              LowCardinality(String),
    `occured_at`        DateTime64(3),
    `arrived_at`        Nullable(DateTime64(3)),
    `org_id`            LowCardinality(String),
    `project_id`        LowCardinality(String),
    `session_id`        String,
    `anon_id`           String,
    `actor_id`          String,
    `source`            LowCardinality(String),
    `correlation_id`    String,
    `trace_id`          String,
    `authenticated`     Nullable(Bool),
    `properties`        JSON,
    `system_properties` JSON
)
ENGINE = Kafka
SETTINGS
    -- local compose network (redpanda internal listener); prod (ECS) uses
    -- redpanda.hyper-analytics.internal:19092
    kafka_broker_list          = 'redpanda:9092',
    kafka_topic_list           = 'events',
    kafka_group_name           = 'clickhouse-events-consumer',
    kafka_format               = 'JSONEachRow',
    kafka_skip_broken_messages = 100,
    -- chrono serializes DateTime<Utc> as RFC3339 ("...T..Z"); best_effort
    -- parses it reliably where the basic parser may reject it
    date_time_input_format     = 'best_effort',
    -- future CanonicalEvent fields must not brick ingestion
    input_format_skip_unknown_fields = 1;


-- ============================================================================
-- 4. events_v2_mv — events_kafka -> events_v2
--    Optional wire fields absent from a message land on the Kafka-engine
--    column defaults ('', NULL, zero UUID) and flow through unchanged.
--    country is guarded to ISO-2 length so unvalidated client-supplied
--    values degrade to NULL instead of failing the FixedString(2) insert.
-- ============================================================================

CREATE MATERIALIZED VIEW IF NOT EXISTS default.events_v2_mv
TO default.events_v2
AS
SELECT
    id                    AS event_id,
    name                  AS event_name,
    occured_at            AS event_timestamp,
    arrived_at,
    org_id,
    project_id,
    session_id,
    anon_id,
    actor_id,
    source,
    correlation_id,
    trace_id,
    authenticated,
    properties,
    if(length(system_properties.geo.country.:String) = 2,
       substring(system_properties.geo.country.:String, 1, 2),
       NULL)               AS country,
    system_properties.timezone.:String      AS timezone,
    system_properties.ip_address.:String    AS ip_address,
    now()                 AS inserted_at
FROM default.events_kafka;


-- ============================================================================
-- 5. user_events_v2_mv — events_kafka -> user_events_v2 (same mapping)
-- ============================================================================

CREATE MATERIALIZED VIEW IF NOT EXISTS default.user_events_v2_mv
TO default.user_events_v2
AS
SELECT
    id                    AS event_id,
    name                  AS event_name,
    occured_at            AS event_timestamp,
    arrived_at,
    org_id,
    project_id,
    session_id,
    anon_id,
    actor_id,
    source,
    correlation_id,
    trace_id,
    authenticated,
    properties,
    if(length(system_properties.geo.country.:String) = 2,
       substring(system_properties.geo.country.:String, 1, 2),
       NULL)               AS country,
    system_properties.timezone.:String      AS timezone,
    system_properties.ip_address.:String    AS ip_address,
    now()                 AS inserted_at
FROM default.events_kafka;


-- ============================================================================
-- 6. event_schema_catalog — property catalog.
--    `description` is dropped: the producer (dashboard-server/src/ingest.rs,
--    EventSchemaEntry) never emits it — it was an always-empty default.
--    On the existing deployment, drop it once:
--      ALTER TABLE default.event_schema_catalog DROP COLUMN IF EXISTS description;
--    and update its consumer (llm_chatv1/execute_tool.rs selects description).
-- ============================================================================

CREATE TABLE IF NOT EXISTS default.event_schema_catalog
(
    `project_id` LowCardinality(String),
    `event_name` String,
    `property`   String,
    `type`       String
)
ENGINE = ReplacingMergeTree()
ORDER BY (project_id, event_name, property, type);


-- ============================================================================
-- 7. event_schema_catalog_kafka — Kafka engine table for catalog ingestion
--    (no DEFAULT expressions allowed)
-- ============================================================================

CREATE TABLE IF NOT EXISTS default.event_schema_catalog_kafka
(
    `project_id`  LowCardinality(String),
    `event_name`  String,
    `property`    String,
    `type`        String
)
ENGINE = Kafka
SETTINGS
    kafka_broker_list          = 'redpanda:9092',
    kafka_topic_list           = 'event_schema_catalog',
    kafka_group_name           = 'clickhouse-event-schema-catalog-consumer',
    kafka_format               = 'JSONEachRow',
    kafka_skip_broken_messages = 100;


-- ============================================================================
-- 8. event_schema_catalog_mv — event_schema_catalog_kafka -> event_schema_catalog
-- ============================================================================

CREATE MATERIALIZED VIEW IF NOT EXISTS default.event_schema_catalog_mv
TO default.event_schema_catalog
AS
SELECT
    project_id,
    event_name,
    property,
    type
FROM default.event_schema_catalog_kafka;


-- ============================================================================
-- 9. Row-Level Security.
--    Every ClickHouse user whose name equals their project slug can only see
--    rows where project_id matches their username (currentUser()).
--    The 'default' user is NOT assigned this role and retains full access.
-- ============================================================================

CREATE ROLE IF NOT EXISTS project_user;


GRANT SELECT ON default.events_v2 TO project_user;
GRANT SELECT ON default.user_events_v2 TO project_user;
GRANT SELECT ON default.event_schema_catalog TO project_user;


CREATE ROW POLICY IF NOT EXISTS proj_filter_events_v2
    ON default.events_v2
    AS PERMISSIVE FOR SELECT
    USING project_id = currentUser()
    TO project_user;


CREATE ROW POLICY IF NOT EXISTS proj_filter_user_events_v2
    ON default.user_events_v2
    AS PERMISSIVE FOR SELECT
    USING project_id = currentUser()
    TO project_user;


CREATE ROW POLICY IF NOT EXISTS proj_filter_schema_catalog
    ON default.event_schema_catalog
    AS PERMISSIVE FOR SELECT
    USING project_id = currentUser()
    TO project_user;
