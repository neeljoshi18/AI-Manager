//! Tenant registry persisted in CockroachDB.

use crate::error::{CoreError, CoreResult};
use crate::model::TenantConfig;
use crate::pipeline::TenantRegistry;
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;

pub struct CockroachTenantRegistry {
    pool: PgPool,
}

impl CockroachTenantRegistry {
    pub async fn connect(database_url: &str) -> CoreResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| CoreError::Storage(format!("cockroach tenants connect: {e}")))?;
        Ok(Arc::new(Self { pool }))
    }
}

#[async_trait]
impl TenantRegistry for CockroachTenantRegistry {
    async fn get(&self, tenant_id: &str) -> CoreResult<Option<TenantConfig>> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT config_json FROM tenants WHERE tenant_id = $1")
                .bind(tenant_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| CoreError::Storage(format!("tenant get: {e}")))?;
        match row {
            Some((json,)) => {
                let mut cfg: TenantConfig = serde_json::from_value(json).map_err(|e| {
                    CoreError::Storage(format!("tenant config parse: {e}"))
                })?;
                cfg.tenant_id = tenant_id.to_string();
                Ok(Some(cfg))
            }
            None => Ok(None),
        }
    }

    async fn upsert(&self, config: TenantConfig) -> CoreResult<()> {
        let json = serde_json::to_value(&config)
            .map_err(|e| CoreError::Storage(format!("tenant serialize: {e}")))?;
        sqlx::query(
            r#"
            INSERT INTO tenants (tenant_id, config_json)
            VALUES ($1, $2)
            ON CONFLICT (tenant_id) DO UPDATE SET config_json = EXCLUDED.config_json
            "#,
        )
        .bind(&config.tenant_id)
        .bind(json)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Storage(format!("tenant upsert: {e}")))?;
        Ok(())
    }
}
