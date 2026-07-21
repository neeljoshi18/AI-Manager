//! CockroachDB ACL store (Postgres wire protocol).

use crate::acl::{AclInvalidation, AclStore};
use crate::error::{CoreError, CoreResult};
use crate::model::AclRevocationRecord;
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

pub struct CockroachAclStore {
    pool: PgPool,
    invalidation_tx: broadcast::Sender<AclInvalidation>,
}

impl CockroachAclStore {
    pub async fn connect(database_url: &str) -> CoreResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| CoreError::Storage(format!("cockroach connect: {e}")))?;
        let (tx, _) = broadcast::channel(4096);
        Ok(Arc::new(Self {
            pool,
            invalidation_tx: tx,
        }))
    }

    async fn bump_version(&self, tenant_id: &str) -> CoreResult<u64> {
        let row = sqlx::query(
            r#"
            INSERT INTO tenant_acl_version (tenant_id, acl_version, updated_at)
            VALUES ($1, 1, now())
            ON CONFLICT (tenant_id) DO UPDATE
              SET acl_version = tenant_acl_version.acl_version + 1,
                  updated_at = now()
            RETURNING acl_version
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl bump: {e}")))?;
        Ok(row.get::<i64, _>("acl_version") as u64)
    }

    fn publish(&self, tenant_id: &str, global_user_id: &str, version: u64) {
        let _ = self.invalidation_tx.send(AclInvalidation {
            tenant_id: tenant_id.to_string(),
            global_user_id: global_user_id.to_string(),
            acl_version: version,
        });
    }
}

#[async_trait]
impl AclStore for CockroachAclStore {
    async fn ensure_user(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
        email: &str,
        display_name: &str,
    ) -> CoreResult<String> {
        // Provider column is generic for cross-system map; store as "any" for edge path
        // (provider-specific mapping can refine later).
        let existing: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT global_user_id FROM user_identity_map
            WHERE tenant_id = $1 AND provider = 'multi' AND provider_user_id = $2
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl resolve: {e}")))?;

        if let Some((gid,)) = existing {
            return Ok(gid);
        }

        let global_user_id = format!("gu_{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO user_identity_map
              (tenant_id, provider, provider_user_id, global_user_id, email, display_name)
            VALUES ($1, 'multi', $2, $3, $4, $5)
            ON CONFLICT (tenant_id, provider, provider_user_id) DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(provider_user_id)
        .bind(&global_user_id)
        .bind(email)
        .bind(display_name)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl insert user: {e}")))?;

        // Re-read in case of conflict race
        let gid: (String,) = sqlx::query_as(
            r#"
            SELECT global_user_id FROM user_identity_map
            WHERE tenant_id = $1 AND provider = 'multi' AND provider_user_id = $2
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(provider_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl reread user: {e}")))?;
        Ok(gid.0)
    }

    async fn resolve_global_user_id(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
    ) -> CoreResult<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT global_user_id FROM user_identity_map
            WHERE tenant_id = $1 AND provider = 'multi' AND provider_user_id = $2
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl resolve: {e}")))?;
        Ok(row.map(|r| r.0))
    }

    async fn set_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_ids: &[String],
    ) -> CoreResult<u64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Storage(format!("acl tx: {e}")))?;
        sqlx::query(
            "DELETE FROM user_group_membership WHERE tenant_id = $1 AND global_user_id = $2",
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Storage(format!("acl clear groups: {e}")))?;

        for g in group_ids {
            sqlx::query(
                r#"
                INSERT INTO user_group_membership (tenant_id, global_user_id, group_id, acl_version)
                VALUES ($1, $2, $3, 1)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(tenant_id)
            .bind(global_user_id)
            .bind(g)
            .execute(&mut *tx)
            .await
            .map_err(|e| CoreError::Storage(format!("acl add group: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| CoreError::Storage(format!("acl commit: {e}")))?;

        let version = self.bump_version(tenant_id).await?;
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn add_user_to_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64> {
        sqlx::query(
            r#"
            INSERT INTO user_group_membership (tenant_id, global_user_id, group_id, acl_version)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl add: {e}")))?;
        let version = self.bump_version(tenant_id).await?;
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn remove_user_from_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64> {
        sqlx::query(
            r#"
            DELETE FROM user_group_membership
            WHERE tenant_id = $1 AND global_user_id = $2 AND group_id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl remove: {e}")))?;
        let version = self.bump_version(tenant_id).await?;
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn get_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> CoreResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT group_id FROM user_group_membership
            WHERE tenant_id = $1 AND global_user_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(global_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("acl groups: {e}")))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn apply_revocation(&self, rev: &AclRevocationRecord) -> CoreResult<u64> {
        match rev.change_type.as_str() {
            "removed_from_group" => {
                self.remove_user_from_group(&rev.tenant_id, &rev.global_user_id, &rev.group_id)
                    .await
            }
            "added_to_group" => {
                self.add_user_to_group(&rev.tenant_id, &rev.global_user_id, &rev.group_id)
                    .await
            }
            other => Err(CoreError::Validation(format!(
                "unknown ACL change_type: {other}"
            ))),
        }
    }

    fn subscribe_invalidations(&self) -> broadcast::Receiver<AclInvalidation> {
        self.invalidation_tx.subscribe()
    }

    async fn current_acl_version(&self, tenant_id: &str) -> u64 {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT acl_version FROM tenant_acl_version WHERE tenant_id = $1")
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        row.map(|r| r.0 as u64).unwrap_or(0)
    }
}
