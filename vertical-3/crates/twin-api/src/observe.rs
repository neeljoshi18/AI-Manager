//! Live observability + twin state mirror to external Postgres (Neon free tier).
//!
//! When `OBSERVE_DATABASE_URL` (or `DATABASE_URL`) is set:
//! - Events (approve / don't-send / …) land in `twin_events`
//! - Full twin snapshot mirrors into relational tables + `twin_snapshot_json`
//! so you can SQL everything that used to live only in the Docker volume JSON.

use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use twin_core::store::InMemoryTwinStore;
use twin_core::model::{DraftDelivery, SlackUserMap, Twin};

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
                .max_connections(5)
                .acquire_timeout(std::time::Duration::from_secs(8))
                .connect(&url)
                .await
            {
                Ok(pool) => {
                    if let Err(e) = migrate(&pool).await {
                        tracing::warn!(error = %e, "observe migrate failed — embedded log only");
                        None
                    } else {
                        tracing::info!("observe: external Postgres connected (Neon)");
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

    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.pg.as_ref()
    }

    pub async fn log(&self, tenant_id: &str, kind: &str, subject: &str, detail: Value) {
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

    /// Full mirror of embedded twin state → Neon (twins, maps, drafts, kv, snapshot blob).
    pub async fn sync_store(
        &self,
        tenant_id: &str,
        store: &InMemoryTwinStore,
    ) -> Result<Value, String> {
        let pool = self
            .pg
            .as_ref()
            .ok_or_else(|| "OBSERVE_DATABASE_URL not connected".to_string())?;

        let snap = store.export_snapshot();
        let now = Utc::now().to_rfc3339();

        // Full JSON backup (source of truth for round-trip)
        let snap_json = serde_json::to_string(&snap).map_err(|e| e.to_string())?;
        sqlx::query(
            r#"
            INSERT INTO twin_snapshot_json (tenant_id, snapshot, synced_at)
            VALUES ($1, $2::jsonb, $3::timestamptz)
            ON CONFLICT (tenant_id) DO UPDATE
              SET snapshot = EXCLUDED.snapshot, synced_at = EXCLUDED.synced_at
            "#,
        )
        .bind(tenant_id)
        .bind(&snap_json)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Relational projections (filter to this tenant for multi-tenant safety)
        let twins: Vec<Twin> = snap
            .twins
            .into_iter()
            .filter(|t| t.tenant_id == tenant_id)
            .collect();
        let maps: Vec<SlackUserMap> = snap
            .slack_maps
            .into_iter()
            .filter(|m| m.tenant_id == tenant_id)
            .collect();
        let drafts: Vec<DraftDelivery> = snap
            .drafts
            .into_iter()
            .filter(|d| d.tenant_id == tenant_id)
            .collect();
        let kv = snap
            .tenant_kv
            .into_iter()
            .filter(|k| k.tenant_id == tenant_id)
            .collect::<Vec<_>>();

        // Clear + reinsert tenant rows (simple, correct for pilot)
        sqlx::query("DELETE FROM twin_twins WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM twin_slack_maps WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM twin_drafts WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM twin_tenant_kv WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut n_twins = 0usize;
        for t in &twins {
            let cfg = t.config_json.to_string();
            sqlx::query(
                r#"
                INSERT INTO twin_twins (
                  tenant_id, twin_id, twin_kind, subject_id, display_name,
                  timezone, channel_id, enabled, high_auto_publish, config_json,
                  shadow_until, created_at, updated_at
                ) VALUES (
                  $1,$2,$3,$4,$5,$6,$7,$8,$9,$10::jsonb,$11,$12,$13
                )
                "#,
            )
            .bind(&t.tenant_id)
            .bind(&t.twin_id)
            .bind(t.twin_kind.as_str())
            .bind(&t.subject_id)
            .bind(&t.display_name)
            .bind(&t.timezone)
            .bind(&t.channel_id)
            .bind(t.enabled)
            .bind(t.high_auto_publish)
            .bind(&cfg)
            .bind(t.shadow_until.map(|d| d.to_rfc3339()))
            .bind(t.created_at.to_rfc3339())
            .bind(t.updated_at.to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            n_twins += 1;
        }

        let mut n_maps = 0usize;
        for m in &maps {
            sqlx::query(
                r#"
                INSERT INTO twin_slack_maps (tenant_id, global_user_id, slack_user_id, slack_team_id)
                VALUES ($1,$2,$3,$4)
                "#,
            )
            .bind(&m.tenant_id)
            .bind(&m.global_user_id)
            .bind(&m.slack_user_id)
            .bind(&m.slack_team_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            n_maps += 1;
        }

        let mut n_drafts = 0usize;
        for d in &drafts {
            let edited = d.edited_text.clone().unwrap_or_default();
            sqlx::query(
                r#"
                INSERT INTO twin_drafts (
                  tenant_id, draft_id, ledger_id, twin_id, status,
                  draft_text, edited_text, slack_dm_channel, slack_dm_ts,
                  veto_deadline, created_at, updated_at
                ) VALUES (
                  $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12
                )
                "#,
            )
            .bind(&d.tenant_id)
            .bind(&d.draft_id)
            .bind(&d.ledger_id)
            .bind(&d.twin_id)
            .bind(d.status.as_str())
            .bind(&d.draft_text)
            .bind(&edited)
            .bind(&d.slack_dm_channel)
            .bind(&d.slack_dm_ts)
            .bind(d.veto_deadline.map(|x| x.to_rfc3339()))
            .bind(d.created_at.to_rfc3339())
            .bind(d.updated_at.to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            n_drafts += 1;
        }

        let mut n_kv = 0usize;
        for k in &kv {
            let vs = k.value.to_string();
            sqlx::query(
                r#"
                INSERT INTO twin_tenant_kv (tenant_id, key, value)
                VALUES ($1,$2,$3::jsonb)
                "#,
            )
            .bind(&k.tenant_id)
            .bind(&k.key)
            .bind(&vs)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            n_kv += 1;
        }

        // Copy embedded event log into twin_events if empty-ish (idempotent-ish by detail)
        if let Some(arr) = store
            .get_tenant_kv(tenant_id, KV_KEY)
            .and_then(|v| v.as_array().cloned())
        {
            for e in arr.iter().rev().take(200) {
                let kind = e.get("kind").and_then(|x| x.as_str()).unwrap_or("legacy");
                let subject = e.get("subject").and_then(|x| x.as_str()).unwrap_or("");
                let detail = e.get("detail").cloned().unwrap_or(json!({}));
                let at = e
                    .get("at")
                    .and_then(|x| x.as_str())
                    .unwrap_or(now.as_str());
                let _ = sqlx::query(
                    r#"
                    INSERT INTO twin_events (tenant_id, kind, subject, detail, at)
                    VALUES ($1,$2,$3,$4::jsonb,$5::timestamptz)
                    "#,
                )
                .bind(tenant_id)
                .bind(kind)
                .bind(subject)
                .bind(detail.to_string())
                .bind(at)
                .execute(pool)
                .await;
            }
        }

        self.log(
            tenant_id,
            "sync_to_db",
            "embedded",
            json!({
                "twins": n_twins,
                "slack_maps": n_maps,
                "drafts": n_drafts,
                "tenant_kv": n_kv,
                "synced_at": now,
            }),
        )
        .await;

        Ok(json!({
            "ok": true,
            "tenant_id": tenant_id,
            "synced_at": now,
            "twins": n_twins,
            "slack_maps": n_maps,
            "drafts": n_drafts,
            "tenant_kv": n_kv,
            "tables": [
                "twin_events",
                "twin_snapshot_json",
                "twin_twins",
                "twin_slack_maps",
                "twin_drafts",
                "twin_tenant_kv"
            ],
        }))
    }
}

async fn migrate(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // sqlx may not run multi-statement in one query on all drivers — split.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_events (
            id BIGSERIAL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            detail JSONB NOT NULL DEFAULT '{}'::jsonb,
            at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS twin_events_tenant_at ON twin_events (tenant_id, at DESC)",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS twin_events_kind ON twin_events (kind)")
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_snapshot_json (
            tenant_id TEXT PRIMARY KEY,
            snapshot JSONB NOT NULL,
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_twins (
            tenant_id TEXT NOT NULL,
            twin_id TEXT NOT NULL,
            twin_kind TEXT NOT NULL,
            subject_id TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            timezone TEXT NOT NULL DEFAULT 'UTC',
            channel_id TEXT NOT NULL DEFAULT '',
            enabled BOOLEAN NOT NULL DEFAULT true,
            high_auto_publish BOOLEAN NOT NULL DEFAULT false,
            config_json JSONB NOT NULL DEFAULT '{}'::jsonb,
            shadow_until TEXT,
            created_at TEXT,
            updated_at TEXT,
            PRIMARY KEY (tenant_id, twin_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_slack_maps (
            tenant_id TEXT NOT NULL,
            global_user_id TEXT NOT NULL,
            slack_user_id TEXT NOT NULL,
            slack_team_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (tenant_id, global_user_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_drafts (
            tenant_id TEXT NOT NULL,
            draft_id TEXT NOT NULL,
            ledger_id TEXT NOT NULL,
            twin_id TEXT NOT NULL,
            status TEXT NOT NULL,
            draft_text TEXT NOT NULL DEFAULT '',
            edited_text TEXT NOT NULL DEFAULT '',
            slack_dm_channel TEXT NOT NULL DEFAULT '',
            slack_dm_ts TEXT NOT NULL DEFAULT '',
            veto_deadline TEXT,
            created_at TEXT,
            updated_at TEXT,
            PRIMARY KEY (tenant_id, draft_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS twin_tenant_kv (
            tenant_id TEXT NOT NULL,
            key TEXT NOT NULL,
            value JSONB NOT NULL DEFAULT '{}'::jsonb,
            PRIMARY KEY (tenant_id, key)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
