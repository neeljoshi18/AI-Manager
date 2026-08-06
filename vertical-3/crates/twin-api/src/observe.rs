//! Live observability + twin state mirror to external Postgres (Neon free tier).
//!
//! When `OBSERVE_DATABASE_URL` (or `DATABASE_URL`) is set:
//! - Events land in `twin_events` continuously
//! - Twins / maps / drafts / kv dual-write on every product mutation + debounced full sync
//! so Neon stays current without manual "Mirror" clicks.

use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use twin_core::model::{DraftDelivery, SlackUserMap, Twin};
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

    /// Continuous dual-write: one twin row (upsert).
    pub async fn write_twin(&self, t: &Twin) {
        let Some(pool) = &self.pg else {
            return;
        };
        let cfg = t.config_json.to_string();
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO twin_twins (
              tenant_id, twin_id, twin_kind, subject_id, display_name,
              timezone, channel_id, enabled, high_auto_publish, config_json,
              shadow_until, created_at, updated_at
            ) VALUES (
              $1,$2,$3,$4,$5,$6,$7,$8,$9,$10::jsonb,$11,$12,$13
            )
            ON CONFLICT (tenant_id, twin_id) DO UPDATE SET
              twin_kind = EXCLUDED.twin_kind,
              subject_id = EXCLUDED.subject_id,
              display_name = EXCLUDED.display_name,
              timezone = EXCLUDED.timezone,
              channel_id = EXCLUDED.channel_id,
              enabled = EXCLUDED.enabled,
              high_auto_publish = EXCLUDED.high_auto_publish,
              config_json = EXCLUDED.config_json,
              shadow_until = EXCLUDED.shadow_until,
              updated_at = EXCLUDED.updated_at
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
        {
            tracing::warn!(error = %e, twin = %t.twin_id, "neon dual-write twin failed");
        }
    }

    pub async fn write_map(&self, m: &SlackUserMap) {
        let Some(pool) = &self.pg else {
            return;
        };
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO twin_slack_maps (tenant_id, global_user_id, slack_user_id, slack_team_id)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (tenant_id, global_user_id) DO UPDATE SET
              slack_user_id = EXCLUDED.slack_user_id,
              slack_team_id = EXCLUDED.slack_team_id
            "#,
        )
        .bind(&m.tenant_id)
        .bind(&m.global_user_id)
        .bind(&m.slack_user_id)
        .bind(&m.slack_team_id)
        .execute(pool)
        .await
        {
            tracing::warn!(error = %e, "neon dual-write map failed");
        }
    }

    pub async fn write_draft(&self, d: &DraftDelivery) {
        let Some(pool) = &self.pg else {
            return;
        };
        let edited = d.edited_text.clone().unwrap_or_default();
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO twin_drafts (
              tenant_id, draft_id, ledger_id, twin_id, status,
              draft_text, edited_text, slack_dm_channel, slack_dm_ts,
              veto_deadline, created_at, updated_at
            ) VALUES (
              $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12
            )
            ON CONFLICT (tenant_id, draft_id) DO UPDATE SET
              ledger_id = EXCLUDED.ledger_id,
              twin_id = EXCLUDED.twin_id,
              status = EXCLUDED.status,
              draft_text = EXCLUDED.draft_text,
              edited_text = EXCLUDED.edited_text,
              slack_dm_channel = EXCLUDED.slack_dm_channel,
              slack_dm_ts = EXCLUDED.slack_dm_ts,
              veto_deadline = EXCLUDED.veto_deadline,
              updated_at = EXCLUDED.updated_at
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
        {
            tracing::warn!(error = %e, draft = %d.draft_id, "neon dual-write draft failed");
        }
    }

    pub async fn write_kv(&self, tenant_id: &str, key: &str, value: &Value) {
        let Some(pool) = &self.pg else {
            return;
        };
        let vs = value.to_string();
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO twin_tenant_kv (tenant_id, key, value)
            VALUES ($1,$2,$3::jsonb)
            ON CONFLICT (tenant_id, key) DO UPDATE SET value = EXCLUDED.value
            "#,
        )
        .bind(tenant_id)
        .bind(key)
        .bind(&vs)
        .execute(pool)
        .await
        {
            tracing::warn!(error = %e, %key, "neon dual-write kv failed");
        }
    }

    /// Full mirror of embedded twin state → Neon (idempotent upserts; safe to re-run).
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

        // Dedupe by primary key (export can theoretically list dups after merges)
        let mut twin_map: HashMap<String, Twin> = HashMap::new();
        for t in snap.twins.into_iter().filter(|t| t.tenant_id == tenant_id) {
            twin_map.insert(t.twin_id.clone(), t);
        }
        let mut map_map: HashMap<String, SlackUserMap> = HashMap::new();
        for m in snap.slack_maps.into_iter().filter(|m| m.tenant_id == tenant_id) {
            map_map.insert(m.global_user_id.clone(), m);
        }
        let mut draft_map: HashMap<String, DraftDelivery> = HashMap::new();
        for d in snap.drafts.into_iter().filter(|d| d.tenant_id == tenant_id) {
            draft_map.insert(d.draft_id.clone(), d);
        }
        let mut kv_map: HashMap<String, Value> = HashMap::new();
        for k in snap.tenant_kv.into_iter().filter(|k| k.tenant_id == tenant_id) {
            kv_map.insert(k.key.clone(), k.value);
        }

        let n_twins = twin_map.len();
        for t in twin_map.values() {
            self.write_twin(t).await;
        }
        let n_maps = map_map.len();
        for m in map_map.values() {
            self.write_map(m).await;
        }
        let n_drafts = draft_map.len();
        for d in draft_map.values() {
            self.write_draft(d).await;
        }
        let n_kv = kv_map.len();
        for (key, value) in &kv_map {
            self.write_kv(tenant_id, key, value).await;
        }

        // One-shot backfill of embedded event log (append-only; may re-insert — ok for pilot)
        if let Some(arr) = store
            .get_tenant_kv(tenant_id, KV_KEY)
            .and_then(|v| v.as_array().cloned())
        {
            for e in arr.iter().rev().take(50) {
                let kind = e.get("kind").and_then(|x| x.as_str()).unwrap_or("legacy");
                if kind == "sync_to_db" || kind == "legacy" {
                    continue;
                }
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

        Ok(json!({
            "ok": true,
            "tenant_id": tenant_id,
            "synced_at": now,
            "twins": n_twins,
            "slack_maps": n_maps,
            "drafts": n_drafts,
            "tenant_kv": n_kv,
            "mode": "upsert",
            "continuous": true,
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
