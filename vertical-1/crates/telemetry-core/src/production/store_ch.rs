//! ClickHouse analytical event store.

use crate::error::{CoreError, CoreResult};
use crate::model::{
    ActorIdentity, AclSnapshot, CanonicalEventRecord, EventQuery, QueryContext,
};
use crate::store::EventStore;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use telemetry_proto::{EventCategory, SourceProvider};
use tracing::debug;

/// Row used for SELECT (native protocol).
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
struct ChEventRow {
    event_id: String,
    tenant_id: String,
    provider: String,
    category: String,
    event_type: String,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    event_timestamp: DateTime<Utc>,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    ingested_at: DateTime<Utc>,
    actor_global_user_id: String,
    actor_provider_user_id: String,
    actor_email: String,
    is_private: u8,
    allowed_group_ids: Vec<String>,
    acl_version: u64,
    resource_id: String,
    parent_resource_id: String,
    attributes_json: String,
    raw_payload_s3_uri: String,
    event_sequence_number: u64,
}

/// JSONEachRow insert payload (RFC3339 timestamps).
#[derive(Debug, Serialize)]
struct ChInsertJson {
    event_id: String,
    tenant_id: String,
    provider: String,
    category: String,
    event_type: String,
    event_timestamp: String,
    ingested_at: String,
    actor_global_user_id: String,
    actor_provider_user_id: String,
    actor_email: String,
    is_private: u8,
    allowed_group_ids: Vec<String>,
    acl_version: u64,
    resource_id: String,
    parent_resource_id: String,
    attributes_json: String,
    raw_payload_s3_uri: String,
    event_sequence_number: u64,
}

pub struct ClickHouseEventStore {
    client: clickhouse::Client,
    database: String,
}

impl ClickHouseEventStore {
    pub fn connect(
        url: &str,
        database: &str,
        user: &str,
        password: &str,
    ) -> CoreResult<Arc<Self>> {
        let client = clickhouse::Client::default()
            .with_url(url)
            .with_database(database)
            .with_user(user)
            .with_password(password);
        Ok(Arc::new(Self {
            client,
            database: database.to_string(),
        }))
    }

    pub async fn ping(&self) -> CoreResult<()> {
        self.client
            .query("SELECT 1")
            .fetch_one::<u8>()
            .await
            .map_err(|e| CoreError::Storage(format!("clickhouse ping: {e}")))?;
        Ok(())
    }

    fn table(&self) -> String {
        format!("{}.canonical_events_local", self.database)
    }

    fn fmt_ts(dt: DateTime<Utc>) -> String {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }

    fn to_insert_json(event: &CanonicalEventRecord) -> ChInsertJson {
        ChInsertJson {
            event_id: event.event_id.clone(),
            tenant_id: event.tenant_id.clone(),
            provider: event.provider.clickhouse_label().to_string(),
            category: event.category.clickhouse_label().to_string(),
            event_type: event.event_type.clone(),
            event_timestamp: Self::fmt_ts(event.event_timestamp),
            ingested_at: Self::fmt_ts(event.ingested_at),
            actor_global_user_id: event.actor.global_user_id.clone(),
            actor_provider_user_id: event.actor.provider_user_id.clone(),
            actor_email: event.actor.email.clone(),
            is_private: if event.acl.is_private { 1 } else { 0 },
            allowed_group_ids: event.acl.allowed_group_ids.clone(),
            acl_version: event.acl.acl_version,
            resource_id: event.resource_id.clone(),
            parent_resource_id: event.parent_resource_id.clone(),
            attributes_json: event.attributes.to_string(),
            raw_payload_s3_uri: event.raw_payload_s3_uri.clone(),
            event_sequence_number: event.event_sequence_number,
        }
    }

    async fn insert_json(&self, event: &CanonicalEventRecord) -> CoreResult<()> {
        let row = Self::to_insert_json(event);
        let _ = row; // constructed for potential future JSONEachRow path
        let groups = format_ch_string_array(&event.acl.allowed_group_ids);
        let sql = format!(
            r#"INSERT INTO {table} (
                event_id, tenant_id, provider, category, event_type,
                event_timestamp, ingested_at,
                actor_global_user_id, actor_provider_user_id, actor_email,
                is_private, allowed_group_ids, acl_version,
                resource_id, parent_resource_id, attributes_json, raw_payload_s3_uri,
                event_sequence_number
            ) VALUES (
                '{event_id}', '{tenant_id}', '{provider}', '{category}', '{event_type}',
                parseDateTime64BestEffort('{event_ts}', 3), parseDateTime64BestEffort('{ingested}', 3),
                '{actor_g}', '{actor_p}', '{actor_e}',
                {is_private}, {groups}, {acl_version},
                '{resource_id}', '{parent}', '{attrs}', '{raw}',
                {seq}
            )"#,
            table = self.table(),
            event_id = esc(&event.event_id),
            tenant_id = esc(&event.tenant_id),
            provider = esc(event.provider.clickhouse_label()),
            category = esc(event.category.clickhouse_label()),
            event_type = esc(&event.event_type),
            event_ts = esc(&Self::fmt_ts(event.event_timestamp)),
            ingested = esc(&Self::fmt_ts(event.ingested_at)),
            actor_g = esc(&event.actor.global_user_id),
            actor_p = esc(&event.actor.provider_user_id),
            actor_e = esc(&event.actor.email),
            is_private = if event.acl.is_private { 1 } else { 0 },
            groups = groups,
            acl_version = event.acl.acl_version,
            resource_id = esc(&event.resource_id),
            parent = esc(&event.parent_resource_id),
            attrs = esc(&event.attributes.to_string()),
            raw = esc(&event.raw_payload_s3_uri),
            seq = event.event_sequence_number,
        );
        self.client
            .query(&sql)
            .execute()
            .await
            .map_err(|e| CoreError::Storage(format!("ch insert: {e}")))?;
        Ok(())
    }

    fn from_row(row: ChEventRow) -> CanonicalEventRecord {
        let provider = match row.provider.as_str() {
            "GITHUB" => SourceProvider::Github,
            "GITLAB" => SourceProvider::Gitlab,
            "JIRA" => SourceProvider::Jira,
            "LINEAR" => SourceProvider::Linear,
            "SLACK" => SourceProvider::Slack,
            "TEAMS" => SourceProvider::Teams,
            "ZENDESK" => SourceProvider::Zendesk,
            _ => SourceProvider::Unspecified,
        };
        let category = match row.category.as_str() {
            "CODE" => EventCategory::Code,
            "WORK_ITEM" => EventCategory::WorkItem,
            "COMMUNICATION" => EventCategory::Communication,
            "IDENTITY" => EventCategory::Identity,
            _ => EventCategory::Unspecified,
        };
        let attributes =
            serde_json::from_str(&row.attributes_json).unwrap_or(serde_json::json!({}));
        CanonicalEventRecord {
            event_id: row.event_id,
            tenant_id: row.tenant_id.clone(),
            provider,
            category,
            event_type: row.event_type,
            event_timestamp: row.event_timestamp,
            ingested_at: row.ingested_at,
            actor: ActorIdentity {
                global_user_id: row.actor_global_user_id,
                provider_user_id: row.actor_provider_user_id,
                email: row.actor_email,
                display_name: String::new(),
            },
            acl: AclSnapshot {
                tenant_id: row.tenant_id,
                allowed_group_ids: row.allowed_group_ids,
                is_private: row.is_private == 1,
                acl_version: row.acl_version,
            },
            resource_id: row.resource_id,
            parent_resource_id: row.parent_resource_id,
            attributes,
            raw_payload_s3_uri: row.raw_payload_s3_uri,
            event_sequence_number: row.event_sequence_number,
        }
    }
}

#[async_trait]
impl EventStore for ClickHouseEventStore {
    async fn upsert(&self, event: CanonicalEventRecord) -> CoreResult<()> {
        self.insert_json(&event).await?;
        debug!(event_id = %event.event_id, "clickhouse upsert ok");
        Ok(())
    }

    async fn query(
        &self,
        ctx: &QueryContext,
        filter: &EventQuery,
    ) -> CoreResult<Vec<CanonicalEventRecord>> {
        if ctx.tenant_id != filter.tenant_id {
            return Err(CoreError::AclDenied(
                "query tenant_id does not match context".into(),
            ));
        }

        let groups_sql = format_ch_string_array(&ctx.group_ids);
        let mut sql = format!(
            r#"
            SELECT
              event_id, tenant_id, provider, category,
              event_type, event_timestamp, ingested_at,
              actor_global_user_id, actor_provider_user_id, actor_email,
              is_private, allowed_group_ids, acl_version,
              resource_id, parent_resource_id, attributes_json, raw_payload_s3_uri,
              event_sequence_number
            FROM {table}
            WHERE tenant_id = ?
              AND (is_private = 0 OR hasAny(allowed_group_ids, {groups}))
            "#,
            table = self.table(),
            groups = groups_sql
        );

        if filter.resource_id.is_some() {
            sql.push_str(" AND resource_id = ? ");
        }
        if filter.event_type.is_some() {
            sql.push_str(" AND event_type = ? ");
        }
        sql.push_str(" ORDER BY event_timestamp DESC LIMIT ?");

        let mut q = self.client.query(&sql).bind(&filter.tenant_id);
        if let Some(ref rid) = filter.resource_id {
            q = q.bind(rid);
        }
        if let Some(ref et) = filter.event_type {
            q = q.bind(et);
        }
        q = q.bind(filter.limit.max(1) as u64);

        let rows = q
            .fetch_all::<ChEventRow>()
            .await
            .map_err(|e| CoreError::Storage(format!("ch query: {e}")))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    async fn count_unique(&self, tenant_id: &str) -> CoreResult<u64> {
        let sql = format!(
            "SELECT uniqExact(event_id) FROM {table} FINAL WHERE tenant_id = ?",
            table = self.table()
        );
        let n = self
            .client
            .query(&sql)
            .bind(tenant_id)
            .fetch_one::<u64>()
            .await
            .map_err(|e| CoreError::Storage(format!("ch count: {e}")))?;
        Ok(n)
    }

    async fn get_raw(
        &self,
        tenant_id: &str,
        event_id: &str,
    ) -> CoreResult<Option<CanonicalEventRecord>> {
        let sql = format!(
            r#"
            SELECT
              event_id, tenant_id, provider, category,
              event_type, event_timestamp, ingested_at,
              actor_global_user_id, actor_provider_user_id, actor_email,
              is_private, allowed_group_ids, acl_version,
              resource_id, parent_resource_id, attributes_json, raw_payload_s3_uri,
              event_sequence_number
            FROM {table} FINAL
            WHERE tenant_id = ? AND event_id = ?
            LIMIT 1
            "#,
            table = self.table()
        );
        let mut cursor = self
            .client
            .query(&sql)
            .bind(tenant_id)
            .bind(event_id)
            .fetch::<ChEventRow>()
            .map_err(|e| CoreError::Storage(format!("ch get: {e}")))?;
        match cursor.next().await {
            Ok(Some(row)) => Ok(Some(Self::from_row(row))),
            Ok(None) => Ok(None),
            Err(e) => Err(CoreError::Storage(format!("ch get next: {e}"))),
        }
    }

    async fn latest_state_for_resource(
        &self,
        ctx: &QueryContext,
        resource_id: &str,
    ) -> CoreResult<Option<CanonicalEventRecord>> {
        let groups_sql = format_ch_string_array(&ctx.group_ids);
        let sql = format!(
            r#"
            SELECT
              event_id, tenant_id, provider, category,
              event_type, event_timestamp, ingested_at,
              actor_global_user_id, actor_provider_user_id, actor_email,
              is_private, allowed_group_ids, acl_version,
              resource_id, parent_resource_id, attributes_json, raw_payload_s3_uri,
              event_sequence_number
            FROM {table} FINAL
            WHERE tenant_id = ?
              AND resource_id = ?
              AND (is_private = 0 OR hasAny(allowed_group_ids, {groups}))
            ORDER BY event_timestamp DESC, event_sequence_number DESC
            LIMIT 1
            "#,
            table = self.table(),
            groups = groups_sql
        );
        let mut cursor = self
            .client
            .query(&sql)
            .bind(&ctx.tenant_id)
            .bind(resource_id)
            .fetch::<ChEventRow>()
            .map_err(|e| CoreError::Storage(format!("ch latest: {e}")))?;
        match cursor.next().await {
            Ok(Some(row)) => Ok(Some(Self::from_row(row))),
            Ok(None) => Ok(None),
            Err(e) => Err(CoreError::Storage(format!("ch latest next: {e}"))),
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn format_ch_string_array(items: &[String]) -> String {
    if items.is_empty() {
        return "emptyArrayString()".to_string();
    }
    let inner = items
        .iter()
        .map(|s| format!("'{}'", esc(s)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}
