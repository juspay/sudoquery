-- ============================================================================
-- ClickHouse schema of one workspace, created by dashboard-server when the
-- workspace is created (src/provisioning.rs) and dropped with the workspace.
-- Each workspace gets its own database, named after the workspace slug, fed from
-- the workspace's own Kafka topic. There are no shared tables.
--
-- Source of truth for the wire format: CanonicalEvent in
-- crates/canonical-event. The struct is #[skip_serializing_none], so optional
-- fields are absent from Kafka messages and land on each column type's
-- natural default ('' for String, NULL for Nullable, zero UUID for UUID).
--
-- Column mapping (wire -> fact tables):
--   id -> event_id, name -> event_name, occured_at -> event_timestamp,
--   actor_id -> actor_id, org_id/workspace_id/session_id/anon_id/source/
--   correlation_id/trace_id/authenticated/properties/arrived_at -> same names,
--   country/timezone/ip_address <- system_properties.geo.country /
--   system_properties.timezone / system_properties.ip_address
--
-- Placeholders, filled in by dashboard-server:
--   {database}        workspace slug, also the database name
--   {workspace_id}      workspace slug, as stored in the workspace_id column
--   {topic}           the workspace's Kafka topic
--   {kafka_brokers}   broker list as seen from ClickHouse
--   {consumer_group}  ClickHouse's consumer group for the topic
--
-- Every statement must stay idempotent (IF NOT EXISTS): a failed workspace
-- creation is cleaned up and a retried one runs the whole file again.
-- Statements are separated by semicolons, so comments must not contain one.
--
-- Requires ClickHouse 24.8+ (JSON column type, .:Type path accessors).
-- ============================================================================

CREATE DATABASE IF NOT EXISTS `{database}`;

-- Fact table, event-name-centric sort.
CREATE TABLE IF NOT EXISTS `{database}`.events_v2
(
    `workspace_id`      LowCardinality(String),
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
ORDER BY (workspace_id, event_name, event_timestamp, event_id)
SETTINGS index_granularity = 8192;

-- Fact table, actor-centric sort (same columns, differs only in ORDER BY).
CREATE TABLE IF NOT EXISTS `{database}`.user_events_v2
(
    `workspace_id`      LowCardinality(String),
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
ORDER BY (workspace_id, actor_id, event_timestamp, event_id)
SETTINGS index_granularity = 8192;

-- Property catalog: every (event, property, type) the workspace has sent,
-- derived from events_v2 by event_schema_catalog_from_events_mv. `type` is
-- the ClickHouse type of the stored JSON path (String, Int64, Float64, ...),
-- the type `properties.<path>.:<Type>` reads, so a path stored with several
-- types has one row per type.
CREATE TABLE IF NOT EXISTS `{database}`.event_schema_catalog
(
    `workspace_id` LowCardinality(String),
    `event_name` String,
    `property`   String,
    `type`       String
)
ENGINE = ReplacingMergeTree()
ORDER BY (workspace_id, event_name, property, type);

-- Kafka engine table for the workspace topic (no DEFAULT expressions allowed).
-- Column names mirror the serialized CanonicalEvent JSON keys exactly
-- (JSONEachRow maps by name). envelop_version is read but not stored.
-- chrono serializes DateTime<Utc> as RFC3339, which best_effort parses, and
-- unknown fields are skipped so new CanonicalEvent fields never break ingestion.
CREATE TABLE IF NOT EXISTS `{database}`.events_kafka
(
    `envelop_version`   LowCardinality(String),
    `id`                UUID,
    `name`              LowCardinality(String),
    `occured_at`        DateTime64(3),
    `arrived_at`        Nullable(DateTime64(3)),
    `org_id`            LowCardinality(String),
    `workspace_id`        LowCardinality(String),
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
    kafka_broker_list          = '{kafka_brokers}',
    kafka_topic_list           = '{topic}',
    kafka_group_name           = '{consumer_group}',
    kafka_format               = 'JSONEachRow',
    kafka_skip_broken_messages = 100,
    date_time_input_format     = 'best_effort',
    input_format_skip_unknown_fields = 1;

-- The WHERE clauses keep another workspace's events out even if one reaches
-- this topic.
CREATE MATERIALIZED VIEW IF NOT EXISTS `{database}`.events_v2_mv
TO `{database}`.events_v2
AS
SELECT
    id                    AS event_id,
    name                  AS event_name,
    occured_at            AS event_timestamp,
    arrived_at,
    org_id,
    workspace_id,
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
FROM `{database}`.events_kafka
WHERE workspace_id = '{workspace_id}';

CREATE MATERIALIZED VIEW IF NOT EXISTS `{database}`.user_events_v2_mv
TO `{database}`.user_events_v2
AS
SELECT
    id                    AS event_id,
    name                  AS event_name,
    occured_at            AS event_timestamp,
    arrived_at,
    org_id,
    workspace_id,
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
FROM `{database}`.events_kafka
WHERE workspace_id = '{workspace_id}';

CREATE MATERIALIZED VIEW IF NOT EXISTS `{database}`.event_schema_catalog_from_events_mv
TO `{database}`.event_schema_catalog
AS
SELECT
    workspace_id,
    event_name,
    property,
    type
FROM `{database}`.events_v2
ARRAY JOIN
    mapKeys(JSONAllPathsWithTypes(properties))   AS property,
    mapValues(JSONAllPathsWithTypes(properties)) AS type
GROUP BY workspace_id, event_name, property, type;
