//! Provider → CanonicalEvent normalizers.
//!
//! Metadata-only: we never store proprietary source code. Raw JSON is vaulted
//! to object storage; structured attributes hold collaborative patterns only.

mod github;
mod gitlab;
mod jira;
mod linear;
mod slack;
mod teams;
mod zendesk;

use crate::error::{CoreError, CoreResult};
use crate::model::{
    ActorIdentity, AclSnapshot, CanonicalEventRecord, TenantConfig, new_event_id,
};
use crate::time::{from_millis, from_secs, now_utc, parse_rfc3339};
use chrono::{DateTime, Utc};
use serde_json::Value;
use telemetry_proto::{EventCategory, SourceProvider};

pub use github::normalize_github;
pub use gitlab::normalize_gitlab;
pub use jira::normalize_jira;
pub use linear::normalize_linear;
pub use slack::normalize_slack;
pub use teams::normalize_teams;
pub use zendesk::normalize_zendesk;

/// Context passed into every normalizer.
#[derive(Debug, Clone)]
pub struct NormalizeContext {
    pub tenant_id: String,
    pub provider: SourceProvider,
    pub delivery_id: Option<String>,
    pub event_name: Option<String>,
    pub raw_payload_s3_uri: String,
    pub default_group_ids: Vec<String>,
    pub actor_global_user_id: String,
    pub acl_version: u64,
    /// Explicit groups from ACL lookup / resource mapping.
    pub allowed_group_ids: Vec<String>,
    pub is_private: bool,
}

/// Normalize a raw webhook body into a canonical event.
pub fn normalize(
    provider: SourceProvider,
    body: &[u8],
    ctx: &NormalizeContext,
) -> CoreResult<CanonicalEventRecord> {
    let value: Value = serde_json::from_slice(body).map_err(|e| {
        CoreError::Normalization(format!("invalid JSON: {e}"))
    })?;

    match provider {
        SourceProvider::Github => normalize_github(&value, ctx),
        SourceProvider::Gitlab => normalize_gitlab(&value, ctx),
        SourceProvider::Jira => normalize_jira(&value, ctx),
        SourceProvider::Linear => normalize_linear(&value, ctx),
        SourceProvider::Slack => normalize_slack(&value, ctx),
        SourceProvider::Teams => normalize_teams(&value, ctx),
        SourceProvider::Zendesk => normalize_zendesk(&value, ctx),
        SourceProvider::Unspecified => Err(CoreError::Normalization(
            "unspecified provider".into(),
        )),
    }
}

/// Build a partially-filled canonical event; normalizers fill the rest.
pub(crate) fn base_event(
    ctx: &NormalizeContext,
    category: EventCategory,
    event_type: &str,
    timestamp: DateTime<Utc>,
    actor: ActorIdentity,
    resource_id: String,
    parent_resource_id: String,
    attributes: Value,
) -> CanonicalEventRecord {
    let event_id = ctx
        .delivery_id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(new_event_id);

    let groups = if ctx.allowed_group_ids.is_empty() {
        ctx.default_group_ids.clone()
    } else {
        ctx.allowed_group_ids.clone()
    };

    CanonicalEventRecord {
        event_id,
        tenant_id: ctx.tenant_id.clone(),
        provider: ctx.provider,
        category,
        event_type: event_type.to_string(),
        event_timestamp: timestamp,
        ingested_at: now_utc(),
        actor: ActorIdentity {
            global_user_id: if actor.global_user_id.is_empty() {
                ctx.actor_global_user_id.clone()
            } else {
                actor.global_user_id
            },
            ..actor
        },
        acl: AclSnapshot {
            tenant_id: ctx.tenant_id.clone(),
            allowed_group_ids: groups,
            is_private: ctx.is_private,
            acl_version: ctx.acl_version,
        },
        resource_id,
        parent_resource_id,
        attributes,
        raw_payload_s3_uri: ctx.raw_payload_s3_uri.clone(),
        event_sequence_number: timestamp.timestamp_millis().max(0) as u64,
    }
}

pub(crate) fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

pub(crate) fn i64_field(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| x.as_i64())
}

pub(crate) fn bool_field(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|x| x.as_bool())
}

pub(crate) fn nested<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(*k)?;
    }
    Some(cur)
}

pub(crate) fn nested_str<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    nested(v, keys).and_then(|x| x.as_str())
}

pub(crate) fn parse_timestamp(v: &Value) -> DateTime<Utc> {
    // Try common shapes: ISO string, unix seconds, unix millis.
    if let Some(s) = v.as_str() {
        if let Some(dt) = parse_rfc3339(s) {
            return dt;
        }
    }
    if let Some(n) = v.as_i64() {
        if n > 1_000_000_000_000 {
            return from_millis(n);
        }
        return from_secs(n);
    }
    if let Some(n) = v.as_f64() {
        return from_secs(n as i64);
    }
    now_utc()
}

pub(crate) fn actor_from_user(user: &Value, global_user_id: &str) -> ActorIdentity {
    ActorIdentity {
        global_user_id: global_user_id.to_string(),
        provider_user_id: str_field(user, "id")
            .map(|s| s.to_string())
            .or_else(|| i64_field(user, "id").map(|n| n.to_string()))
            .or_else(|| str_field(user, "login").map(|s| s.to_string()))
            .or_else(|| str_field(user, "accountId").map(|s| s.to_string()))
            .unwrap_or_default(),
        email: str_field(user, "email").unwrap_or("").to_string(),
        display_name: str_field(user, "name")
            .or_else(|| str_field(user, "login"))
            .or_else(|| str_field(user, "displayName"))
            .or_else(|| str_field(user, "real_name"))
            .unwrap_or("")
            .to_string(),
    }
}

/// Detect whether JSON looks like an identity / membership change.
pub fn is_identity_event(event_type: &str) -> bool {
    let t = event_type.to_ascii_lowercase();
    t.contains("membership")
        || t.contains("member")
        || t.contains("team_add")
        || t.contains("team_remove")
        || t.contains("removed_from")
        || t.contains("added_to")
        || t.contains("permission")
}

/// Build NormalizeContext from tenant config + delivery headers.
pub fn context_from_tenant(
    tenant: &TenantConfig,
    provider: SourceProvider,
    delivery_id: Option<String>,
    event_name: Option<String>,
    raw_uri: String,
    actor_global_user_id: String,
    acl_version: u64,
    allowed_group_ids: Vec<String>,
    is_private: bool,
) -> NormalizeContext {
    NormalizeContext {
        tenant_id: tenant.tenant_id.clone(),
        provider,
        delivery_id,
        event_name,
        raw_payload_s3_uri: raw_uri,
        default_group_ids: tenant.default_group_ids.clone(),
        actor_global_user_id,
        acl_version,
        allowed_group_ids,
        is_private,
    }
}
