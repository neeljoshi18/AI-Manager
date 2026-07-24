//! CockroachDB `status_twins` store (production mode).

use crate::error::{TwinError, TwinResult};
use crate::model::*;
use crate::store::TwinStore;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::sync::Arc;

pub struct CrdbTwinStore {
    pool: PgPool,
}

impl CrdbTwinStore {
    pub async fn connect(url: &str) -> TwinResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await
            .map_err(|e| TwinError::Storage(format!("connect: {e}")))?;
        Ok(Arc::new(Self { pool }))
    }

    fn parse_kind(s: &str) -> TwinResult<TwinKind> {
        TwinKind::parse(s).ok_or_else(|| TwinError::Storage(format!("bad twin_kind: {s}")))
    }

    fn parse_conf(s: &str) -> TwinResult<ConfidenceTier> {
        ConfidenceTier::parse(s)
            .ok_or_else(|| TwinError::Storage(format!("bad confidence: {s}")))
    }

}

#[async_trait]
impl TwinStore for CrdbTwinStore {
    async fn upsert_twin(&self, twin: Twin) -> TwinResult<()> {
        sqlx::query(
            r#"
            UPSERT INTO twin (
                tenant_id, twin_id, twin_kind, subject_id, display_name, timezone,
                channel_id, shadow_until, high_auto_publish, enabled, config_json,
                created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
        )
        .bind(&twin.tenant_id)
        .bind(&twin.twin_id)
        .bind(twin.twin_kind.as_str())
        .bind(&twin.subject_id)
        .bind(&twin.display_name)
        .bind(&twin.timezone)
        .bind(&twin.channel_id)
        .bind(twin.shadow_until)
        .bind(twin.high_auto_publish)
        .bind(twin.enabled)
        .bind(&twin.config_json)
        .bind(twin.created_at)
        .bind(twin.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_twin(&self, tenant_id: &str, twin_id: &str) -> TwinResult<Option<Twin>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, twin_id, twin_kind, subject_id, display_name, timezone,
                      channel_id, shadow_until, high_auto_publish, enabled, config_json,
                      created_at, updated_at
               FROM twin WHERE tenant_id = $1 AND twin_id = $2"#,
        )
        .bind(tenant_id)
        .bind(twin_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => Ok(Some(Twin {
                tenant_id: r.get("tenant_id"),
                twin_id: r.get("twin_id"),
                twin_kind: Self::parse_kind(r.get::<String, _>("twin_kind").as_str())?,
                subject_id: r.get("subject_id"),
                display_name: r.get("display_name"),
                timezone: r.get("timezone"),
                channel_id: r.get("channel_id"),
                shadow_until: r.get("shadow_until"),
                high_auto_publish: r.get("high_auto_publish"),
                enabled: r.get("enabled"),
                config_json: r.get("config_json"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })),
        }
    }

    async fn list_twins(&self, tenant_id: &str) -> TwinResult<Vec<Twin>> {
        let rows = sqlx::query(
            r#"SELECT tenant_id, twin_id, twin_kind, subject_id, display_name, timezone,
                      channel_id, shadow_until, high_auto_publish, enabled, config_json,
                      created_at, updated_at
               FROM twin WHERE tenant_id = $1"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(Twin {
                tenant_id: r.get("tenant_id"),
                twin_id: r.get("twin_id"),
                twin_kind: Self::parse_kind(r.get::<String, _>("twin_kind").as_str())?,
                subject_id: r.get("subject_id"),
                display_name: r.get("display_name"),
                timezone: r.get("timezone"),
                channel_id: r.get("channel_id"),
                shadow_until: r.get("shadow_until"),
                high_auto_publish: r.get("high_auto_publish"),
                enabled: r.get("enabled"),
                config_json: r.get("config_json"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        }
        Ok(out)
    }

    async fn put_slack_map(&self, map: SlackUserMap) -> TwinResult<()> {
        sqlx::query(
            r#"UPSERT INTO slack_user_map (tenant_id, global_user_id, slack_user_id, slack_team_id, updated_at)
               VALUES ($1,$2,$3,$4, now())"#,
        )
        .bind(&map.tenant_id)
        .bind(&map.global_user_id)
        .bind(&map.slack_user_id)
        .bind(&map.slack_team_id)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_slack_map(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> TwinResult<Option<SlackUserMap>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, global_user_id, slack_user_id, slack_team_id
               FROM slack_user_map WHERE tenant_id = $1 AND global_user_id = $2"#,
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(row.map(|r| SlackUserMap {
            tenant_id: r.get("tenant_id"),
            global_user_id: r.get("global_user_id"),
            slack_user_id: r.get("slack_user_id"),
            slack_team_id: r.get("slack_team_id"),
        }))
    }

    async fn list_slack_maps(&self, tenant_id: &str) -> TwinResult<Vec<SlackUserMap>> {
        let rows = sqlx::query(
            r#"SELECT tenant_id, global_user_id, slack_user_id, slack_team_id
               FROM slack_user_map WHERE tenant_id = $1
               ORDER BY global_user_id"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| SlackUserMap {
                tenant_id: r.get("tenant_id"),
                global_user_id: r.get("global_user_id"),
                slack_user_id: r.get("slack_user_id"),
                slack_team_id: r.get("slack_team_id"),
            })
            .collect())
    }

    async fn put_ledger(&self, snap: LedgerSnapshot) -> TwinResult<()> {
        let ledger_json = serde_json::to_value(&snap.ledger)
            .map_err(|e| TwinError::Internal(e.to_string()))?;
        // Block replace if published
        let published: Option<(String,)> = sqlx::query_as(
            r#"SELECT publish_id FROM publish_record WHERE tenant_id = $1 AND ledger_id = $2"#,
        )
        .bind(&snap.tenant_id)
        .bind(&snap.ledger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        if published.is_some() {
            return Err(TwinError::Conflict(format!(
                "ledger {} already published",
                snap.ledger_id
            )));
        }

        sqlx::query(
            r#"
            UPSERT INTO ledger_snapshot (
                tenant_id, ledger_id, twin_id, period_start, period_end,
                confidence_rollup, ledger_json, graph_as_of, compiled_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(&snap.tenant_id)
        .bind(&snap.ledger_id)
        .bind(&snap.twin_id)
        .bind(snap.period_start)
        .bind(snap.period_end)
        .bind(snap.confidence_rollup.as_str())
        .bind(ledger_json)
        .bind(snap.graph_as_of)
        .bind(snap.compiled_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<LedgerSnapshot>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, ledger_id, twin_id, period_start, period_end,
                      confidence_rollup, ledger_json, graph_as_of, compiled_at
               FROM ledger_snapshot WHERE tenant_id = $1 AND ledger_id = $2"#,
        )
        .bind(tenant_id)
        .bind(ledger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => {
                let ledger: StatusLedger = serde_json::from_value(r.get("ledger_json"))
                    .map_err(|e| TwinError::Storage(e.to_string()))?;
                Ok(Some(LedgerSnapshot {
                    tenant_id: r.get("tenant_id"),
                    ledger_id: r.get("ledger_id"),
                    twin_id: r.get("twin_id"),
                    period_start: r.get("period_start"),
                    period_end: r.get("period_end"),
                    confidence_rollup: Self::parse_conf(
                        r.get::<String, _>("confidence_rollup").as_str(),
                    )?,
                    ledger,
                    graph_as_of: r.get("graph_as_of"),
                    compiled_at: r.get("compiled_at"),
                }))
            }
        }
    }

    async fn get_ledger_by_period(
        &self,
        tenant_id: &str,
        twin_id: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> TwinResult<Option<LedgerSnapshot>> {
        let row = sqlx::query(
            r#"SELECT ledger_id FROM ledger_snapshot
               WHERE tenant_id = $1 AND twin_id = $2
                 AND period_start = $3 AND period_end = $4"#,
        )
        .bind(tenant_id)
        .bind(twin_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => {
                let id: String = r.get("ledger_id");
                self.get_ledger(tenant_id, &id).await
            }
        }
    }

    async fn put_draft(&self, draft: DraftDelivery) -> TwinResult<()> {
        sqlx::query(
            r#"
            INSERT INTO draft_delivery (
                tenant_id, draft_id, ledger_id, twin_id, status,
                slack_dm_channel, slack_dm_ts, draft_text, edited_text,
                veto_deadline, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            ON CONFLICT (tenant_id, ledger_id) DO UPDATE SET
                status = EXCLUDED.status,
                slack_dm_channel = EXCLUDED.slack_dm_channel,
                slack_dm_ts = EXCLUDED.slack_dm_ts,
                draft_text = EXCLUDED.draft_text,
                edited_text = EXCLUDED.edited_text,
                veto_deadline = EXCLUDED.veto_deadline,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&draft.tenant_id)
        .bind(&draft.draft_id)
        .bind(&draft.ledger_id)
        .bind(&draft.twin_id)
        .bind(draft.status.as_str())
        .bind(&draft.slack_dm_channel)
        .bind(&draft.slack_dm_ts)
        .bind(&draft.draft_text)
        .bind(&draft.edited_text)
        .bind(draft.veto_deadline)
        .bind(draft.created_at)
        .bind(draft.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_draft(
        &self,
        tenant_id: &str,
        draft_id: &str,
    ) -> TwinResult<Option<DraftDelivery>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, draft_id, ledger_id, twin_id, status,
                      slack_dm_channel, slack_dm_ts, draft_text, edited_text,
                      veto_deadline, created_at, updated_at
               FROM draft_delivery WHERE tenant_id = $1 AND draft_id = $2"#,
        )
        .bind(tenant_id)
        .bind(draft_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => Ok(Some(row_to_draft(r)?)),
        }
    }

    async fn get_draft_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<DraftDelivery>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, draft_id, ledger_id, twin_id, status,
                      slack_dm_channel, slack_dm_ts, draft_text, edited_text,
                      veto_deadline, created_at, updated_at
               FROM draft_delivery WHERE tenant_id = $1 AND ledger_id = $2"#,
        )
        .bind(tenant_id)
        .bind(ledger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        match row {
            None => Ok(None),
            Some(r) => Ok(Some(row_to_draft(r)?)),
        }
    }

    async fn update_draft(&self, draft: DraftDelivery) -> TwinResult<()> {
        let res = sqlx::query(
            r#"
            UPDATE draft_delivery SET
                status = $3,
                slack_dm_channel = $4,
                slack_dm_ts = $5,
                draft_text = $6,
                edited_text = $7,
                veto_deadline = $8,
                updated_at = $9
            WHERE tenant_id = $1 AND draft_id = $2
            "#,
        )
        .bind(&draft.tenant_id)
        .bind(&draft.draft_id)
        .bind(draft.status.as_str())
        .bind(&draft.slack_dm_channel)
        .bind(&draft.slack_dm_ts)
        .bind(&draft.draft_text)
        .bind(&draft.edited_text)
        .bind(draft.veto_deadline)
        .bind(draft.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        if res.rows_affected() == 0 {
            return Err(TwinError::NotFound(format!("draft {}", draft.draft_id)));
        }
        Ok(())
    }

    async fn put_publish_if_absent(&self, rec: PublishRecord) -> TwinResult<bool> {
        let res = sqlx::query(
            r#"
            INSERT INTO publish_record (
                tenant_id, publish_id, ledger_id, draft_id, channel_id, slack_ts, body_hash, published_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT (tenant_id, ledger_id) DO NOTHING
            "#,
        )
        .bind(&rec.tenant_id)
        .bind(&rec.publish_id)
        .bind(&rec.ledger_id)
        .bind(&rec.draft_id)
        .bind(&rec.channel_id)
        .bind(&rec.slack_ts)
        .bind(&rec.body_hash)
        .bind(rec.published_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(res.rows_affected() > 0)
    }

    async fn get_publish_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<PublishRecord>> {
        let row = sqlx::query(
            r#"SELECT tenant_id, publish_id, ledger_id, draft_id, channel_id, slack_ts, body_hash, published_at
               FROM publish_record WHERE tenant_id = $1 AND ledger_id = $2"#,
        )
        .bind(tenant_id)
        .bind(ledger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(row.map(|r| PublishRecord {
            tenant_id: r.get("tenant_id"),
            publish_id: r.get("publish_id"),
            ledger_id: r.get("ledger_id"),
            draft_id: r.get("draft_id"),
            channel_id: r.get("channel_id"),
            slack_ts: r.get("slack_ts"),
            body_hash: r.get("body_hash"),
            published_at: r.get("published_at"),
        }))
    }

    async fn put_compile_run(&self, run: CompileRun) -> TwinResult<()> {
        sqlx::query(
            r#"
            UPSERT INTO compile_run (
                tenant_id, run_id, twin_id, status, error_text, started_at, finished_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(&run.tenant_id)
        .bind(&run.run_id)
        .bind(&run.twin_id)
        .bind(&run.status)
        .bind(&run.error_text)
        .bind(run.started_at)
        .bind(run.finished_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TwinError::Storage(e.to_string()))?;
        Ok(())
    }
}

fn row_to_draft(r: sqlx::postgres::PgRow) -> TwinResult<DraftDelivery> {
    use sqlx::Row;
    Ok(DraftDelivery {
        tenant_id: r.get("tenant_id"),
        draft_id: r.get("draft_id"),
        ledger_id: r.get("ledger_id"),
        twin_id: r.get("twin_id"),
        status: DraftStatus::parse(r.get::<String, _>("status").as_str())
            .ok_or_else(|| TwinError::Storage("bad status".into()))?,
        slack_dm_channel: r.get("slack_dm_channel"),
        slack_dm_ts: r.get("slack_dm_ts"),
        draft_text: r.get("draft_text"),
        edited_text: r.get("edited_text"),
        veto_deadline: r.get("veto_deadline"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}
