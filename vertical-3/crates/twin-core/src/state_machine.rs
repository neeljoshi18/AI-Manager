//! Pure delivery state machine (TAS §4.4).

use crate::model::{ConfidenceTier, DraftStatus};

/// Events that drive draft delivery transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryEvent {
    /// Shadow compile only (within shadow_until).
    EnterShadow,
    /// Left shadow; ready for DM path.
    LeaveShadow,
    /// DM sent to developer → PENDING.
    DmSent,
    /// User pressed Edit.
    Edit,
    /// User pressed Veto.
    Veto,
    /// Silence timeout for Medium (consent by silence).
    MediumSilenceTimeout,
    /// Explicit Publish as-is / confirm after edit.
    ExplicitPublish,
    /// High tier with high_auto_publish=true.
    HighAutoPublish,
    /// Blocker requires human — no auto channel post.
    ForceHuman,
    /// Channel post succeeded.
    PublishSucceeded,
    /// Channel post failed.
    PublishFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryTransition {
    pub from: DraftStatus,
    pub to: DraftStatus,
    pub event: DeliveryEvent,
}

/// Apply a delivery event. Returns `None` if the transition is illegal.
pub fn apply_delivery_event(
    current: DraftStatus,
    event: &DeliveryEvent,
    confidence: ConfidenceTier,
    high_auto_publish: bool,
) -> Option<DraftStatus> {
    match (current, event) {
        (DraftStatus::Shadow, DeliveryEvent::LeaveShadow) => Some(DraftStatus::Pending),
        (DraftStatus::Shadow, DeliveryEvent::EnterShadow) => Some(DraftStatus::Shadow),

        (DraftStatus::Pending, DeliveryEvent::Edit) => Some(DraftStatus::Edited),
        (DraftStatus::Pending, DeliveryEvent::Veto) => Some(DraftStatus::Vetoed),
        (DraftStatus::Pending, DeliveryEvent::MediumSilenceTimeout)
            if confidence == ConfidenceTier::Medium =>
        {
            Some(DraftStatus::PublishQueued)
        }
        (DraftStatus::Pending, DeliveryEvent::ExplicitPublish) => Some(DraftStatus::PublishQueued),
        (DraftStatus::Pending, DeliveryEvent::HighAutoPublish)
            if confidence == ConfidenceTier::High && high_auto_publish =>
        {
            Some(DraftStatus::PublishQueued)
        }
        (DraftStatus::Pending, DeliveryEvent::ForceHuman)
            if confidence == ConfidenceTier::Blocker =>
        {
            Some(DraftStatus::ForceHuman)
        }
        (DraftStatus::Pending, DeliveryEvent::PublishFailed) => Some(DraftStatus::PublishFailed),

        (DraftStatus::Edited, DeliveryEvent::ExplicitPublish) => Some(DraftStatus::PublishQueued),
        (DraftStatus::Edited, DeliveryEvent::MediumSilenceTimeout)
            if confidence == ConfidenceTier::Medium =>
        {
            Some(DraftStatus::PublishQueued)
        }
        (DraftStatus::Edited, DeliveryEvent::Veto) => Some(DraftStatus::Vetoed),

        (DraftStatus::ForceHuman, DeliveryEvent::ExplicitPublish) => {
            Some(DraftStatus::PublishQueued)
        }
        (DraftStatus::ForceHuman, DeliveryEvent::Veto) => Some(DraftStatus::Vetoed),
        (DraftStatus::ForceHuman, DeliveryEvent::Edit) => Some(DraftStatus::Edited),

        (DraftStatus::PublishQueued, DeliveryEvent::PublishSucceeded) => {
            Some(DraftStatus::Published)
        }
        (DraftStatus::PublishQueued, DeliveryEvent::PublishFailed) => {
            Some(DraftStatus::PublishFailed)
        }

        (DraftStatus::Vetoed, _) => None,
        (DraftStatus::Published, _) => None,

        (DraftStatus::PublishFailed, DeliveryEvent::ExplicitPublish) => {
            Some(DraftStatus::PublishQueued)
        }
        (DraftStatus::PublishFailed, DeliveryEvent::HighAutoPublish)
            if confidence == ConfidenceTier::High && high_auto_publish =>
        {
            Some(DraftStatus::PublishQueued)
        }

        (DraftStatus::Pending, DeliveryEvent::DmSent) => Some(DraftStatus::Pending),
        (DraftStatus::Shadow, DeliveryEvent::DmSent) => None,

        _ => None,
    }
}

/// Choose initial draft status after compile (policy, TAS §5).
pub fn initial_draft_status(
    in_shadow: bool,
    confidence: ConfidenceTier,
    high_auto_publish: bool,
) -> DraftStatus {
    if in_shadow {
        return DraftStatus::Shadow;
    }
    match confidence {
        ConfidenceTier::Blocker => DraftStatus::ForceHuman,
        ConfidenceTier::High if high_auto_publish => DraftStatus::PublishQueued,
        ConfidenceTier::High | ConfidenceTier::Medium => DraftStatus::Pending,
    }
}

/// Whether silence timeout may auto-queue publish.
pub fn silence_may_publish(confidence: ConfidenceTier) -> bool {
    confidence == ConfidenceTier::Medium
}

/// Whether channel publish is allowed from this status (after human or silence policy).
pub fn may_channel_publish(status: DraftStatus) -> bool {
    matches!(status, DraftStatus::PublishQueued)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veto_terminal() {
        let next = apply_delivery_event(
            DraftStatus::Pending,
            &DeliveryEvent::Veto,
            ConfidenceTier::Medium,
            false,
        );
        assert_eq!(next, Some(DraftStatus::Vetoed));
        assert!(
            apply_delivery_event(
                DraftStatus::Vetoed,
                &DeliveryEvent::ExplicitPublish,
                ConfidenceTier::Medium,
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn medium_silence_publishes() {
        let next = apply_delivery_event(
            DraftStatus::Pending,
            &DeliveryEvent::MediumSilenceTimeout,
            ConfidenceTier::Medium,
            false,
        );
        assert_eq!(next, Some(DraftStatus::PublishQueued));
    }

    #[test]
    fn blocker_silence_does_not_auto() {
        let next = apply_delivery_event(
            DraftStatus::Pending,
            &DeliveryEvent::MediumSilenceTimeout,
            ConfidenceTier::Blocker,
            false,
        );
        assert!(next.is_none());
        assert_eq!(
            initial_draft_status(false, ConfidenceTier::Blocker, true),
            DraftStatus::ForceHuman
        );
    }

    #[test]
    fn high_auto_publish() {
        assert_eq!(
            initial_draft_status(false, ConfidenceTier::High, true),
            DraftStatus::PublishQueued
        );
        assert_eq!(
            initial_draft_status(false, ConfidenceTier::High, false),
            DraftStatus::Pending
        );
    }

    #[test]
    fn shadow_no_dm_transition() {
        assert_eq!(
            initial_draft_status(true, ConfidenceTier::Medium, false),
            DraftStatus::Shadow
        );
        assert!(
            apply_delivery_event(
                DraftStatus::Shadow,
                &DeliveryEvent::DmSent,
                ConfidenceTier::Medium,
                false,
            )
            .is_none()
        );
    }
}
