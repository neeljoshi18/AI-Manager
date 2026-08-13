//! Vertical 3 twin domain: ledgers, confidence tiers, draft/veto state, twin store.

pub mod config;
pub mod confidence;
pub mod error;
pub mod egress;
pub mod ids;
pub mod ledger_text;
pub mod model;
pub mod notify_policy;
pub mod state_machine;
pub mod store;
pub mod time_ist;

#[cfg(feature = "production")]
pub mod store_crdb;

pub use config::TwinConfig;
pub use confidence::{roll_up_confidence, score_item_confidence};
pub use error::{TwinError, TwinResult};
pub use egress::{EgressClient, EgressConfig, SLACK_TOOL, TEAMS_TOOL, TOOL_HEADER};
pub use ids::{ledger_id_for, person_twin_id, team_twin_id};
pub use model::*;
pub use notify_policy::{
    decide_notify, ledger_fingerprint, load_notify_state, record_dm_sent, write_notify_state,
    NotifyDecision, NotifyState, SuppressReason,
};
pub use state_machine::{apply_delivery_event, DeliveryEvent, DeliveryTransition};
pub use store::{InMemoryTwinStore, TwinStore};
pub use time_ist::{
    format_ist_compact, format_ist_day, format_ist_list, format_ist_rfc3339, format_lookback_ist,
    ist_hour, ist_weekday_mon0, parse_as_utc, reformat_stored_to_ist_list,
    reformat_stored_to_ist_rfc3339, to_ist, DISPLAY_TIMEZONE,
};
