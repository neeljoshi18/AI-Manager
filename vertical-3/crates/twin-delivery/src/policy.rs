use twin_core::model::{
    DEFAULT_BLOCKER_VETO_WINDOW_SECS, DEFAULT_MEDIUM_VETO_WINDOW_SECS,
};

#[derive(Debug, Clone)]
pub struct DeliveryPolicy {
    pub medium_veto_window_secs: i64,
    pub blocker_veto_window_secs: i64,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            medium_veto_window_secs: DEFAULT_MEDIUM_VETO_WINDOW_SECS,
            blocker_veto_window_secs: DEFAULT_BLOCKER_VETO_WINDOW_SECS,
        }
    }
}
