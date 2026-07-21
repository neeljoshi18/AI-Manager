//! End-to-end ingestion pipeline (edge path).
//!
//! Order of operations (Spec §3.1 + Invariants):
//! 1. Authenticate (HMAC)
//! 2. Rate-limit (per tenant)
//! 3. Dedup claim (delivery id)
//! 4. Vault raw payload → object store
//! 5. Resolve actor identity + ACL snapshot
//! 6. Normalize → CanonicalEvent
//! 7. Durable publish to bus  **before** returning success
//! 8. (Async) consumer writes ClickHouse / applies ACL revocations

use crate::acl::AclStore;
use crate::auth::{verify_webhook, WebhookHeaders};
use crate::bus::EventBus;
use crate::dedup::DedupStore;
use crate::error::{CoreError, CoreResult};
use crate::metrics::{IngestMetrics, Timer};
use crate::model::{
    AclRevocationRecord, BusMessage, BusPayload, BusTopic, CanonicalEventRecord, IngestOutcome,
    IngestStatus, TenantConfig, new_event_id,
};
use crate::normalize::{self, NormalizeContext};
use crate::object_store::ObjectStore;
use crate::rate_limit::RateLimiter;
use crate::time::now_utc;
use serde_json::Value;
use std::sync::Arc;
use telemetry_proto::{EventCategory, SourceProvider};
use tracing::{info, warn};

/// Shared dependencies for the ingestion edge.
pub struct IngestPipeline {
    pub tenants: Arc<dyn TenantRegistry>,
    pub auth_skip: bool,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub dedup: Arc<dyn DedupStore>,
    pub object_store: Arc<dyn ObjectStore>,
    pub acl: Arc<dyn AclStore>,
    pub bus: Arc<dyn EventBus>,
    pub metrics: Arc<IngestMetrics>,
    /// When true, also write directly to the analytical store (embedded single-process mode).
    pub inline_store: Option<Arc<dyn crate::store::EventStore>>,
}

#[async_trait::async_trait]
pub trait TenantRegistry: Send + Sync {
    async fn get(&self, tenant_id: &str) -> CoreResult<Option<TenantConfig>>;
    async fn upsert(&self, config: TenantConfig) -> CoreResult<()>;
}

/// Simple in-memory tenant registry.
pub struct InMemoryTenantRegistry {
    inner: dashmap::DashMap<String, TenantConfig>,
}

impl InMemoryTenantRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: dashmap::DashMap::new(),
        })
    }

    pub fn with_tenant(config: TenantConfig) -> Arc<Self> {
        let reg = Self::new();
        reg.inner.insert(config.tenant_id.clone(), config);
        reg
    }
}

#[async_trait::async_trait]
impl TenantRegistry for InMemoryTenantRegistry {
    async fn get(&self, tenant_id: &str) -> CoreResult<Option<TenantConfig>> {
        Ok(self.inner.get(tenant_id).map(|c| c.clone()))
    }

    async fn upsert(&self, config: TenantConfig) -> CoreResult<()> {
        self.inner.insert(config.tenant_id.clone(), config);
        Ok(())
    }
}

/// Incoming webhook request after HTTP extraction.
#[derive(Debug, Clone)]
pub struct IngestRequest {
    pub tenant_id: String,
    pub provider: SourceProvider,
    pub body: Vec<u8>,
    pub headers: IngestHeaders,
    /// When true, route to events.backfill instead of realtime.
    pub is_backfill: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IngestHeaders {
    pub signature_256: Option<String>,
    pub signature: Option<String>,
    pub gitlab_token: Option<String>,
    pub shared_secret: Option<String>,
    pub linear_signature: Option<String>,
    pub slack_signature: Option<String>,
    pub slack_timestamp: Option<String>,
    pub delivery_id: Option<String>,
    pub event_name: Option<String>,
}

impl IngestPipeline {
    pub async fn ingest(&self, req: IngestRequest) -> CoreResult<IngestOutcome> {
        let timer = Timer::start();
        let result = self.ingest_inner(req).await;
        let latency_ms = timer.elapsed_ms();
        self.metrics.record_latency(latency_ms);

        match result {
            Ok(outcome) => {
                match outcome.status {
                    IngestStatus::Accepted => {
                        self.metrics.accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    IngestStatus::Duplicate => {
                        self.metrics.duplicates.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        self.metrics.record_dedup_hit();
                    }
                    IngestStatus::DeadLettered => {
                        self.metrics
                            .dead_lettered
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                Ok(IngestOutcome {
                    latency_ms,
                    ..outcome
                })
            }
            Err(CoreError::Auth(msg)) => {
                self.metrics
                    .auth_failures
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err(CoreError::Auth(msg))
            }
            Err(CoreError::RateLimited { retry_after_secs }) => {
                self.metrics
                    .rate_limited
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err(CoreError::RateLimited { retry_after_secs })
            }
            Err(CoreError::Duplicate { event_id }) => {
                self.metrics.duplicates.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.metrics.record_dedup_hit();
                Ok(IngestOutcome {
                    event_id,
                    status: IngestStatus::Duplicate,
                    latency_ms,
                })
            }
            Err(e) => {
                self.metrics
                    .errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err(e)
            }
        }
    }

    async fn ingest_inner(&self, req: IngestRequest) -> CoreResult<IngestOutcome> {
        self.metrics.record_tenant(&req.tenant_id);

        // Slack URL verification short-circuit (must respond with challenge).
        if req.provider == SourceProvider::Slack {
            if let Ok(v) = serde_json::from_slice::<Value>(&req.body) {
                if v.get("type").and_then(|t| t.as_str()) == Some("url_verification") {
                    // Not a telemetry event; caller handles challenge at HTTP layer.
                    return Err(CoreError::Validation("slack_url_verification".into()));
                }
            }
        }

        let tenant = self
            .tenants
            .get(&req.tenant_id)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("tenant {}", req.tenant_id)))?;

        // 1. Auth
        if !self.auth_skip {
            let secret = tenant.secret_for(req.provider).ok_or_else(|| {
                CoreError::Auth(format!(
                    "no webhook secret configured for {:?}",
                    req.provider
                ))
            })?;
            let headers = WebhookHeaders {
                signature_256: req.headers.signature_256.as_deref(),
                signature: req.headers.signature.as_deref(),
                gitlab_token: req.headers.gitlab_token.as_deref(),
                shared_secret: req.headers.shared_secret.as_deref(),
                linear_signature: req.headers.linear_signature.as_deref(),
                slack_signature: req.headers.slack_signature.as_deref(),
                slack_timestamp: req.headers.slack_timestamp.as_deref(),
                delivery_id: req.headers.delivery_id.as_deref(),
                event_name: req.headers.event_name.as_deref(),
            };
            verify_webhook(req.provider, secret, &req.body, &headers)?;
        }

        // 2. Rate limit
        self.rate_limiter.check(&req.tenant_id).await?;

        // 3. Dedup
        let delivery_id = req
            .headers
            .delivery_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(new_event_id);

        if !self.dedup.try_claim(&delivery_id).await? {
            self.metrics.record_dedup_hit();
            return Ok(IngestOutcome {
                event_id: delivery_id,
                status: IngestStatus::Duplicate,
                latency_ms: 0,
            });
        }
        self.metrics.record_dedup_miss();

        // Process with release-on-hard-failure so retries can re-claim.
        match self
            .process_claimed(&tenant, &req, &delivery_id)
            .await
        {
            Ok(outcome) => Ok(outcome),
            Err(e) => {
                // Only release on infrastructure failures — dead-letter is a successful claim.
                if !matches!(e, CoreError::DeadLetter(_)) {
                    let _ = self.dedup.release(&delivery_id).await;
                }
                Err(e)
            }
        }
    }

    async fn process_claimed(
        &self,
        tenant: &crate::model::TenantConfig,
        req: &IngestRequest,
        delivery_id: &str,
    ) -> CoreResult<IngestOutcome> {
        // 4. Vault raw payload
        let raw_uri = match self
            .object_store
            .put_raw_payload(
                &req.tenant_id,
                req.provider.as_str_name_lower(),
                delivery_id,
                &req.body,
            )
            .await
        {
            Ok(uri) => uri,
            Err(e) => {
                warn!(error = %e, "object store put failed; continuing with empty uri");
                String::new()
            }
        };

        // 5–6. Normalize (+ actor / ACL resolution)
        let event = match self
            .normalize_and_enrich(tenant, req, delivery_id, raw_uri)
            .await
        {
            Ok(evt) => evt,
            Err(e) => {
                // Challenge 3: schema mutation → DLQ, do not halt.
                warn!(error = %e, "normalization failed; dead-lettering");
                let dlq_uri = self
                    .object_store
                    .put_dlq(&req.tenant_id, "normalize_error", &req.body)
                    .await
                    .unwrap_or_default();
                info!(dlq_uri = %dlq_uri, "payload written to DLQ");
                return Ok(IngestOutcome {
                    event_id: delivery_id.to_string(),
                    status: IngestStatus::DeadLettered,
                    latency_ms: 0,
                });
            }
        };

        // Identity events may also produce ACL revocations.
        if event.category == EventCategory::Identity {
            if let Some(rev) = extract_acl_revocation(&event) {
                let _ = self.acl.apply_revocation(&rev).await;
                self.bus
                    .publish(BusMessage {
                        topic: BusTopic::EventsAcl,
                        partition_key: rev.tenant_id.clone(),
                        payload: BusPayload::Acl(rev),
                    })
                    .await?;
            }
        }

        // 7. Durable publish
        let topic = if req.is_backfill {
            BusTopic::EventsBackfill
        } else {
            BusTopic::EventsRealtime
        };
        self.bus
            .publish(BusMessage {
                topic: BusTopic::EventsRaw,
                partition_key: event.tenant_id.clone(),
                payload: BusPayload::Event(event.clone()),
            })
            .await?;
        self.bus
            .publish(BusMessage {
                topic,
                partition_key: event.tenant_id.clone(),
                payload: BusPayload::Event(event.clone()),
            })
            .await?;

        // Inline analytical write (embedded, or production with INLINE_CONSUMER=true).
        if let Some(store) = &self.inline_store {
            store.upsert(event.clone()).await?;
        }

        Ok(IngestOutcome {
            event_id: event.event_id,
            status: IngestStatus::Accepted,
            latency_ms: 0,
        })
    }

    async fn normalize_and_enrich(
        &self,
        tenant: &TenantConfig,
        req: &IngestRequest,
        delivery_id: &str,
        raw_uri: String,
    ) -> CoreResult<CanonicalEventRecord> {
        // Pre-parse for actor hints so we can resolve global_user_id.
        let value: Value = serde_json::from_slice(&req.body)
            .map_err(|e| CoreError::Normalization(format!("invalid JSON: {e}")))?;

        let provider_user_id = extract_provider_user_id(req.provider, &value);
        let email = extract_email(&value).unwrap_or_default();
        let display_name = extract_display_name(&value).unwrap_or_default();

        let global_user_id = if provider_user_id.is_empty() {
            String::new()
        } else {
            self.acl
                .ensure_user(
                    &req.tenant_id,
                    &provider_user_id,
                    &email,
                    &display_name,
                )
                .await?
        };

        let is_private = detect_private(req.provider, &value);
        let allowed = if is_private {
            // For private resources, use tenant defaults as the allow-list unless
            // we later map repo→team. Vertical 2 will refine resource ACLs.
            tenant.default_group_ids.clone()
        } else {
            tenant.default_group_ids.clone()
        };

        let acl_version = self.acl.current_acl_version(&req.tenant_id).await.max(1);

        let ctx = NormalizeContext {
            tenant_id: tenant.tenant_id.clone(),
            provider: req.provider,
            delivery_id: Some(delivery_id.to_string()),
            event_name: req.headers.event_name.clone(),
            raw_payload_s3_uri: raw_uri,
            default_group_ids: tenant.default_group_ids.clone(),
            actor_global_user_id: global_user_id,
            acl_version,
            allowed_group_ids: allowed,
            is_private,
        };

        normalize::normalize(req.provider, &req.body, &ctx)
    }
}

fn extract_provider_user_id(provider: SourceProvider, v: &Value) -> String {
    match provider {
        SourceProvider::Github => v
            .get("sender")
            .and_then(|s| s.get("id"))
            .and_then(|id| id.as_i64().map(|n| n.to_string()).or_else(|| id.as_str().map(|s| s.to_string())))
            .or_else(|| {
                v.get("sender")
                    .and_then(|s| s.get("login"))
                    .and_then(|l| l.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default(),
        SourceProvider::Gitlab => v
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(|id| id.as_i64().map(|n| n.to_string()))
            .or_else(|| v.get("user_id").and_then(|id| id.as_i64().map(|n| n.to_string())))
            .unwrap_or_default(),
        SourceProvider::Jira => v
            .get("user")
            .and_then(|u| u.get("accountId").or_else(|| u.get("key")))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        SourceProvider::Slack => v
            .pointer("/event/user")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string(),
        SourceProvider::Linear => v
            .pointer("/data/creator/id")
            .or_else(|| v.pointer("/actor/id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        SourceProvider::Teams => v
            .pointer("/from/id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        SourceProvider::Zendesk => v
            .pointer("/detail/assignee_id")
            .and_then(|x| x.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default(),
        SourceProvider::Unspecified => String::new(),
    }
}

fn extract_email(v: &Value) -> Option<String> {
    v.pointer("/sender/email")
        .or_else(|| v.pointer("/user/email"))
        .or_else(|| v.pointer("/event/user/profile/email"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

fn extract_display_name(v: &Value) -> Option<String> {
    v.pointer("/sender/login")
        .or_else(|| v.pointer("/sender/name"))
        .or_else(|| v.pointer("/user/name"))
        .or_else(|| v.pointer("/from/name"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

fn detect_private(provider: SourceProvider, v: &Value) -> bool {
    match provider {
        SourceProvider::Github => v
            .pointer("/repository/private")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        SourceProvider::Gitlab => v
            .pointer("/project/visibility_level")
            .and_then(|x| x.as_i64())
            .map(|l| l == 0)
            .unwrap_or(false),
        _ => false,
    }
}

/// Best-effort ACL revocation extraction from identity events.
fn extract_acl_revocation(event: &CanonicalEventRecord) -> Option<AclRevocationRecord> {
    let et = event.event_type.to_ascii_lowercase();
    let change_type = if et.contains("remove") || et.contains("left") || et.contains("deleted") {
        "removed_from_group"
    } else if et.contains("add") || et.contains("joined") || et.contains("created") {
        "added_to_group"
    } else {
        return None;
    };

    let group_id = event
        .attributes
        .get("team")
        .or_else(|| event.attributes.get("channel"))
        .or_else(|| event.attributes.get("group_name"))
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();

    let provider_user_id = event
        .attributes
        .get("member")
        .or_else(|| event.attributes.get("user"))
        .and_then(|x| x.as_str())
        .unwrap_or(&event.actor.provider_user_id)
        .to_string();

    Some(AclRevocationRecord {
        event_id: event.event_id.clone(),
        tenant_id: event.tenant_id.clone(),
        global_user_id: event.actor.global_user_id.clone(),
        provider_user_id,
        provider: event.provider,
        group_id,
        change_type: change_type.to_string(),
        acl_version: event.acl.acl_version,
        timestamp: now_utc(),
    })
}

/// Background consumer loop: bus → analytical store + ACL application.
pub async fn run_consumer_loop(
    bus: Arc<dyn EventBus>,
    store: Arc<dyn crate::store::EventStore>,
    acl: Arc<dyn AclStore>,
    consumer_group: &str,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> CoreResult<()> {
    let mut raw_sub = bus.subscribe(BusTopic::EventsRaw, consumer_group).await?;
    let mut acl_sub = bus
        .subscribe(BusTopic::EventsAcl, &format!("{consumer_group}-acl"))
        .await?;

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("consumer shutdown requested");
                    break;
                }
            }
            msg = raw_sub.recv() => {
                match msg {
                    Ok(m) => {
                        if let BusPayload::Event(e) = m.payload {
                            if let Err(err) = store.upsert(e).await {
                                warn!(error = %err, "store upsert failed");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "raw bus recv error");
                    }
                }
            }
            msg = acl_sub.recv() => {
                match msg {
                    Ok(m) => {
                        if let BusPayload::Acl(rev) = m.payload {
                            if let Err(err) = acl.apply_revocation(&rev).await {
                                warn!(error = %err, "acl apply failed");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "acl bus recv error");
                    }
                }
            }
        }
    }
    Ok(())
}
