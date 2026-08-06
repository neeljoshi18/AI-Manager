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

    /// Replace V2 graph snapshot nodes+edges in Neon for a tenant (bulk, one txn).
    ///
    /// Strategy: DELETE tenant rows → bulk INSERT via UNNEST (few round-trips).
    /// Row-by-row upserts were too slow for Neon free-tier RTT (~500+ nodes).
    ///
    /// Nodes use `id`. Edges use stable `id` when present, else
    /// `{type}:{from}->{to}:{valid_from}` (empty segments allowed).
    pub async fn sync_graph_snapshot(
        &self,
        tenant_id: &str,
        nodes: &[Value],
        edges: &[Value],
    ) -> Result<Value, String> {
        let pool = self
            .pg
            .as_ref()
            .ok_or_else(|| "OBSERVE_DATABASE_URL not connected".to_string())?;

        let now = Utc::now().to_rfc3339();

        // Collect rows in memory first (dedupe by id).
        let mut node_map: HashMap<String, (String, String, String, String)> = HashMap::new();
        for n in nodes {
            let node_id = n
                .get("id")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("")
                .to_string();
            if node_id.is_empty() {
                continue;
            }
            let node_type = n
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let label = n
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let resource_id = n
                .get("resource_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let props_s = node_props_json(n).to_string();
            node_map.insert(node_id, (node_type, label, resource_id, props_s));
        }

        let mut edge_map: HashMap<String, (String, String, String, Option<String>, String)> =
            HashMap::new();
        for e in edges {
            let edge_type = e
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let from_id = e
                .get("from")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let to_id = e
                .get("to")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let valid_from = e
                .get("valid_from")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let edge_id = e
                .get("id")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    synthesize_edge_id(
                        &edge_type,
                        &from_id,
                        &to_id,
                        valid_from.as_deref().unwrap_or(""),
                    )
                });
            if edge_id.is_empty() {
                continue;
            }
            let props_s = edge_props_json(e).to_string();
            edge_map.insert(edge_id, (edge_type, from_id, to_id, valid_from, props_s));
        }

        let mut node_ids: Vec<String> = Vec::with_capacity(node_map.len());
        let mut node_types: Vec<String> = Vec::with_capacity(node_map.len());
        let mut node_labels: Vec<String> = Vec::with_capacity(node_map.len());
        let mut node_resources: Vec<String> = Vec::with_capacity(node_map.len());
        let mut node_props: Vec<String> = Vec::with_capacity(node_map.len());
        for (id, (ty, lab, res, props)) in node_map {
            node_ids.push(id);
            node_types.push(ty);
            node_labels.push(lab);
            node_resources.push(res);
            node_props.push(props);
        }

        let mut edge_ids: Vec<String> = Vec::with_capacity(edge_map.len());
        let mut edge_types: Vec<String> = Vec::with_capacity(edge_map.len());
        let mut edge_froms: Vec<String> = Vec::with_capacity(edge_map.len());
        let mut edge_tos: Vec<String> = Vec::with_capacity(edge_map.len());
        let mut edge_vfs: Vec<Option<String>> = Vec::with_capacity(edge_map.len());
        let mut edge_props: Vec<String> = Vec::with_capacity(edge_map.len());
        for (id, (ty, from, to, vf, props)) in edge_map {
            edge_ids.push(id);
            edge_types.push(ty);
            edge_froms.push(from);
            edge_tos.push(to);
            edge_vfs.push(vf);
            edge_props.push(props);
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("graph export begin: {e}"))?;

        sqlx::query("DELETE FROM graph_nodes WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("graph_nodes clear: {e}"))?;
        sqlx::query("DELETE FROM graph_edges WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("graph_edges clear: {e}"))?;

        // Chunk bulk inserts so payloads stay reasonable (~200 rows / round-trip).
        const CHUNK: usize = 200;
        for start in (0..node_ids.len()).step_by(CHUNK) {
            let end = (start + CHUNK).min(node_ids.len());
            sqlx::query(
                r#"
                INSERT INTO graph_nodes (
                  tenant_id, node_id, node_type, label, resource_id, props, synced_at
                )
                SELECT $1, n, t, l, r, p::jsonb, $2::timestamptz
                FROM UNNEST(
                  $3::text[], $4::text[], $5::text[], $6::text[], $7::text[]
                ) AS u(n, t, l, r, p)
                "#,
            )
            .bind(tenant_id)
            .bind(&now)
            .bind(&node_ids[start..end])
            .bind(&node_types[start..end])
            .bind(&node_labels[start..end])
            .bind(&node_resources[start..end])
            .bind(&node_props[start..end])
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("graph_nodes bulk insert: {e}"))?;
        }

        for start in (0..edge_ids.len()).step_by(CHUNK) {
            let end = (start + CHUNK).min(edge_ids.len());
            sqlx::query(
                r#"
                INSERT INTO graph_edges (
                  tenant_id, edge_id, edge_type, from_id, to_id, valid_from, props, synced_at
                )
                SELECT $1, eid, et, fr, tto, vf, p::jsonb, $2::timestamptz
                FROM UNNEST(
                  $3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[]
                ) AS u(eid, et, fr, tto, vf, p)
                "#,
            )
            .bind(tenant_id)
            .bind(&now)
            .bind(&edge_ids[start..end])
            .bind(&edge_types[start..end])
            .bind(&edge_froms[start..end])
            .bind(&edge_tos[start..end])
            .bind(
                &edge_vfs[start..end]
                    .iter()
                    .map(|v| v.clone().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .bind(&edge_props[start..end])
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("graph_edges bulk insert: {e}"))?;
        }

        let n_nodes = node_ids.len() as i32;
        let n_edges = edge_ids.len() as i32;
        sqlx::query(
            r#"
            INSERT INTO graph_export_meta (tenant_id, node_count, edge_count, synced_at, source)
            VALUES ($1, $2, $3, $4::timestamptz, 'v2_snapshot')
            ON CONFLICT (tenant_id) DO UPDATE SET
              node_count = EXCLUDED.node_count,
              edge_count = EXCLUDED.edge_count,
              synced_at = EXCLUDED.synced_at,
              source = EXCLUDED.source
            "#,
        )
        .bind(tenant_id)
        .bind(n_nodes)
        .bind(n_edges)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("graph_export_meta: {e}"))?;

        tx.commit()
            .await
            .map_err(|e| format!("graph export commit: {e}"))?;

        Ok(json!({
            "ok": true,
            "tenant_id": tenant_id,
            "synced_at": now,
            "nodes": n_nodes,
            "edges": n_edges,
            "mode": "replace_bulk",
            "source": "v2_snapshot",
            "tables": ["graph_nodes", "graph_edges", "graph_export_meta"],
        }))
    }
}

/// Stable edge id when V2 omits `id`: `{type}:{from}->{to}:{valid_from}`.
fn synthesize_edge_id(edge_type: &str, from: &str, to: &str, valid_from: &str) -> String {
    format!("{edge_type}:{from}->{to}:{valid_from}")
}

/// Pack non-column node fields into props JSON (message/title/properties + rest).
fn node_props_json(n: &Value) -> Value {
    const SKIP: &[&str] = &["id", "type", "label", "resource_id"];
    let mut props = serde_json::Map::new();
    if let Some(obj) = n.as_object() {
        for (k, v) in obj {
            if SKIP.contains(&k.as_str()) {
                continue;
            }
            props.insert(k.clone(), v.clone());
        }
    }
    Value::Object(props)
}

fn edge_props_json(e: &Value) -> Value {
    const SKIP: &[&str] = &["id", "type", "from", "to", "valid_from"];
    let mut props = serde_json::Map::new();
    if let Some(obj) = e.as_object() {
        for (k, v) in obj {
            if SKIP.contains(&k.as_str()) {
                continue;
            }
            props.insert(k.clone(), v.clone());
        }
    }
    Value::Object(props)
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

    // V2 graph snapshot mirror (SQL insights; Graph UI remains primary)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS graph_nodes (
            tenant_id TEXT NOT NULL,
            node_id TEXT NOT NULL,
            node_type TEXT NOT NULL DEFAULT '',
            label TEXT NOT NULL DEFAULT '',
            resource_id TEXT NOT NULL DEFAULT '',
            props JSONB NOT NULL DEFAULT '{}'::jsonb,
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (tenant_id, node_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS graph_nodes_tenant_type ON graph_nodes (tenant_id, node_type)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS graph_edges (
            tenant_id TEXT NOT NULL,
            edge_id TEXT NOT NULL,
            edge_type TEXT NOT NULL DEFAULT '',
            from_id TEXT NOT NULL DEFAULT '',
            to_id TEXT NOT NULL DEFAULT '',
            valid_from TEXT,
            props JSONB NOT NULL DEFAULT '{}'::jsonb,
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (tenant_id, edge_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS graph_edges_tenant_type ON graph_edges (tenant_id, edge_type)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS graph_export_meta (
            tenant_id TEXT PRIMARY KEY,
            node_count INT NOT NULL DEFAULT 0,
            edge_count INT NOT NULL DEFAULT 0,
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            source TEXT NOT NULL DEFAULT 'v2_snapshot'
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
