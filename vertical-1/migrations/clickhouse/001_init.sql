-- Vertical 1 analytical storage schema (Spec §4.4)
-- Applied on ClickHouse bootstrap (docker-compose / ops).

CREATE DATABASE IF NOT EXISTS enterprise_telemetry;

-- Single-node friendly local table (cluster DDL uses ON CLUSTER in multi-node).
CREATE TABLE IF NOT EXISTS enterprise_telemetry.canonical_events_local
(
    event_id String,
    tenant_id String,
    -- LowCardinality(String) instead of Enum8 for simpler client inserts
    provider LowCardinality(String),
    category LowCardinality(String),
    event_type LowCardinality(String),
    event_timestamp DateTime64(3, 'UTC'),
    ingested_at DateTime64(3, 'UTC') DEFAULT now64(3),

    -- Actor Details
    actor_global_user_id String,
    actor_provider_user_id String,
    actor_email String,

    -- Access Control List Data
    is_private UInt8,
    allowed_group_ids Array(String),
    acl_version UInt64,

    -- Metadata & Payloads
    resource_id String,
    parent_resource_id String,
    attributes_json String,
    raw_payload_s3_uri String,

    event_sequence_number UInt64 DEFAULT 0
)
ENGINE = ReplacingMergeTree(acl_version)
PARTITION BY toYYYYMM(event_timestamp)
PRIMARY KEY (tenant_id, category, provider)
ORDER BY (tenant_id, category, provider, event_type, resource_id, event_id)
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;

-- Query-facing view (single node). Multi-node: use Distributed engine over shard locals.
CREATE VIEW IF NOT EXISTS enterprise_telemetry.canonical_events AS
SELECT * FROM enterprise_telemetry.canonical_events_local;

-- Example ACL-filtered query (mandatory pattern for all Vertical 2+ reads):
--
-- SELECT event_id, event_type, attributes_json, actor_global_user_id
-- FROM enterprise_telemetry.canonical_events_local
-- WHERE tenant_id = {tenant:String}
--   AND (
--       is_private = 0
--       OR hasAny(allowed_group_ids, {user_groups:Array(String)})
--   )
--   AND event_timestamp >= now() - INTERVAL 7 DAY;
