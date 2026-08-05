//! Live event observability: embedded ring buffer + optional external Postgres (Neon free tier).
//!
//! When `OBSERVE_DATABASE_URL` is set, every Approve / Don't send / compile / OAuth-ish event
//! is written so you can `SELECT * FROM twin_events ORDER BY at DESC` and watch the system.

use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use twin_core::store::InMemoryTwinStore;

const KV_KEY: &str = "event_log";
const MAX_EMBEDDED: usize = 500;

#[derive(Clone)]
pub struct EventObserver {
    embedded: Option<Arc<InMemoryTwinStore>>,
    pg: Option<sqlx::PgPool>,
}

impl EventObserver {
    pub async fn from_env(embedded: Option<Arc<InMemoryTwinStore>>) -> Self {
        let pg = match std::env::var("OBSERVE_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            Some(url) => match sqlx::postgres::PgPoolOptions::new()
                .max_connections(3)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(&url)
                .await
            {
                Ok(pool) => {
                    if let Err(e) = migrate(&pool).await {
                        tracing::warn!(error = %e, "observe migrate failed — embedded log only");
                        None
                    } else {
                        tracing::info!("observe: external Postgres connected (Neon/etc.)");
                        Some(pool)
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "observe: OBSERVE_DATABASE_URL connect failed");
                    None
                }
            },
            None => None,
        };
        Self { embedded, pg }
    }

    pub fn external_connected(&self) -> bool {
        self.pg.is_some()
    }

    pub async fn log(
        &self,
        tenant_id: &str,
        kind: &str,
        subject: &str,
        detail: Value,
    ) {
        let at = Utc::now().to_rfc3339();
        let entry = json!({
            "at": at,
            "tenant_id": tenant_id,
            "kind": kind,
            "subject": subject,
            "detail": detail,
        });

        if let Some(store) = &self.embedded {
            let mut arr = store
                .get_tenant_kv(tenant_id, KV_KEY)
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            arr.push(entry.clone());
            if arr.len() > MAX_EMBEDDED {
                let drop_n = arr.len() - MAX_EMBEDDED;
                arr.drain(0..drop_n);
            }
            store.put_tenant_kv(tenant_id, KV_KEY, Value::Array(arr));
        }

        if let Some(pool) = &self.pg {
            let detail_s = detail.to_string();
            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO twin_events (tenant_id, kind, subject, detail, at)
                VALUES ($1, $2, $3, $4::jsonb, $5::timestamptz)
                "#,
            )
            .bind(tenant_id)
            .bind(kind)
            .bind(subject)
            .bind(detail_s)
            .bind(&at)
            .execute(pool)
            .await
            {
                tracing::warn!(error = %e, %kind, "observe pg insert failed");
            }
        }
    }

    pub fn list_embedded(&self, tenant_id: &str, limit: usize) -> Vec<Value> {
        let Some(store) = &self.embedded else {
            return vec![];
        };
        let arr = store
            .get_tenant_kv(tenant_id, KV_KEY)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        arr.into_iter().rev().take(limit).collect()
    }

    pub async fn list_pg(&self, tenant_id: &str, limit: i64) -> Option<Vec<Value>> {
        let pool = self.pg.as_ref()?;
        let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
            r#"
            SELECT kind, subject, detail::text, at::text, id::text
            FROM twin_events
            WHERE tenant_id = $1
            ORDER BY at DESC
            LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .ok()?;
        Some(
            rows.into_iter()
                .map(|(kind, subject, detail, at, id)| {
                    let detail_v: Value =
                        serde_json::from_str(&detail).unwrap_or(json!({ "raw": detail }));
                    json!({
                        "id": id,
                        "at": at,
                        "kind": kind,
                        "subject": subject,
                        "detail": detail_v,
                    })
                })
                .collect(),
        )
    }
}

async fn migrate(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_events (
            id BIGSERIAL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            detail JSONB NOT NULL DEFAULT '{}'::jsonb,
            at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        CREATE INDEX IF NOT EXISTS twin_events_tenant_at
            ON twin_events (tenant_id, at DESC);
        CREATE INDEX IF NOT EXISTS twin_events_kind
            ON twin_events (kind);
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}
