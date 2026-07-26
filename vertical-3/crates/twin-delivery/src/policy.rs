use twin_core::model::{
    DEFAULT_BLOCKER_VETO_WINDOW_SECS, DEFAULT_MEDIUM_VETO_WINDOW_SECS,
};

#[derive(Debug, Clone)]
pub struct DeliveryPolicy {
    pub medium_veto_window_secs: i64,
    pub blocker_veto_window_secs: i64,
    /// Notify Policy v1: max status DMs per person per UTC day (0 = unlimited).
    pub max_dms_per_day: u32,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            medium_veto_window_secs: DEFAULT_MEDIUM_VETO_WINDOW_SECS,
            blocker_veto_window_secs: DEFAULT_BLOCKER_VETO_WINDOW_SECS,
            // Developer-first: at most one status DM per day unless story changes + cap allows blocker break
            max_dms_per_day: std::env::var("MAX_STATUS_DMS_PER_DAY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
        }
    }
}
