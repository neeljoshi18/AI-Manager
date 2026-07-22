//! Delivery: DM + veto state machine + channel publish via egress only.

mod mock_slack;
mod policy;
mod slack;
mod worker;

pub use mock_slack::{MockSlackClient, SlackCall};
pub use policy::DeliveryPolicy;
pub use slack::{EgressSlackClient, SlackClient, SlackPostResult};
pub use worker::{DeliveryService, StartDeliveryOpts};

use twin_core::model::*;
use twin_core::state_machine::{apply_delivery_event, DeliveryEvent};
use twin_core::store::TwinStore;
use twin_core::{TwinError, TwinResult};
use chrono::Utc;
use std::sync::Arc;

/// High-level helpers used by twin-api and twin-verify.
pub async fn start_delivery_for_ledger(
    store: Arc<dyn TwinStore>,
    slack: Arc<dyn SlackClient>,
    twin: &Twin,
    snap: &LedgerSnapshot,
    draft_text: &str,
    policy: &DeliveryPolicy,
    now: chrono::DateTime<Utc>,
) -> TwinResult<DraftDelivery> {
    DeliveryService::new(store, slack, policy.clone())
        .start_after_compile(twin, snap, draft_text, now)
        .await
}

pub async fn veto_draft(
    store: Arc<dyn TwinStore>,
    tenant_id: &str,
    draft_id: &str,
) -> TwinResult<DraftDelivery> {
    let mut draft = store
        .get_draft(tenant_id, draft_id)
        .await?
        .ok_or_else(|| TwinError::NotFound(format!("draft {draft_id}")))?;
    let snap = store
        .get_ledger(tenant_id, &draft.ledger_id)
        .await?
        .ok_or_else(|| TwinError::NotFound("ledger".into()))?;
    let next = apply_delivery_event(
        draft.status,
        &DeliveryEvent::Veto,
        snap.confidence_rollup,
        false,
    )
    .ok_or_else(|| TwinError::Conflict(format!("cannot veto from {:?}", draft.status)))?;
    draft.status = next;
    draft.updated_at = Utc::now();
    store.update_draft(draft.clone()).await?;
    Ok(draft)
}

pub async fn edit_draft(
    store: Arc<dyn TwinStore>,
    tenant_id: &str,
    draft_id: &str,
    edited_text: &str,
) -> TwinResult<DraftDelivery> {
    let mut draft = store
        .get_draft(tenant_id, draft_id)
        .await?
        .ok_or_else(|| TwinError::NotFound(format!("draft {draft_id}")))?;
    let snap = store
        .get_ledger(tenant_id, &draft.ledger_id)
        .await?
        .ok_or_else(|| TwinError::NotFound("ledger".into()))?;
    let next = apply_delivery_event(
        draft.status,
        &DeliveryEvent::Edit,
        snap.confidence_rollup,
        false,
    )
    .ok_or_else(|| TwinError::Conflict(format!("cannot edit from {:?}", draft.status)))?;
    draft.status = next;
    draft.edited_text = Some(edited_text.to_string());
    draft.updated_at = Utc::now();
    store.update_draft(draft.clone()).await?;
    Ok(draft)
}

pub async fn force_publish(
    store: Arc<dyn TwinStore>,
    slack: Arc<dyn SlackClient>,
    twin: &Twin,
    tenant_id: &str,
    draft_id: &str,
) -> TwinResult<(DraftDelivery, Option<PublishRecord>)> {
    let service = DeliveryService::new(store, slack, DeliveryPolicy::default());
    service.explicit_publish(twin, tenant_id, draft_id).await
}

pub async fn process_silence_timeout(
    store: Arc<dyn TwinStore>,
    slack: Arc<dyn SlackClient>,
    twin: &Twin,
    tenant_id: &str,
    draft_id: &str,
) -> TwinResult<(DraftDelivery, Option<PublishRecord>)> {
    let service = DeliveryService::new(store, slack, DeliveryPolicy::default());
    service.silence_timeout(twin, tenant_id, draft_id).await
}

pub use twin_core::ids::body_hash;
pub use twin_core::state_machine::initial_draft_status;
