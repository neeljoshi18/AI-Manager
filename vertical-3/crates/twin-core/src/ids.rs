use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

/// Person twin id: `twin:person:{global_user_id}`
pub fn person_twin_id(global_user_id: &str) -> String {
    format!("twin:person:{global_user_id}")
}

/// Team twin id: `twin:team:{team_node_id}`
pub fn team_twin_id(team_node_id: &str) -> String {
    format!("twin:team:{team_node_id}")
}

/// Deterministic ledger id from tenant + twin + period.
pub fn ledger_id_for(
    tenant_id: &str,
    twin_id: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> String {
    let mut h = Sha256::new();
    h.update(tenant_id.as_bytes());
    h.update(b"|");
    h.update(twin_id.as_bytes());
    h.update(b"|");
    h.update(period_start.to_rfc3339().as_bytes());
    h.update(b"|");
    h.update(period_end.to_rfc3339().as_bytes());
    let dig = h.finalize();
    format!("led_{}", hex::encode(&dig[..12]))
}

/// SHA-256 hex of body text for publish_record.body_hash.
pub fn body_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn person_twin_id_stable() {
        assert_eq!(person_twin_id("gu_alice"), "twin:person:gu_alice");
    }

    #[test]
    fn ledger_id_deterministic() {
        let s = Utc.with_ymd_and_hms(2026, 7, 21, 0, 0, 0).unwrap();
        let e = Utc.with_ymd_and_hms(2026, 7, 22, 0, 0, 0).unwrap();
        let a = ledger_id_for("ten_acme", "twin:person:gu_alice", s, e);
        let b = ledger_id_for("ten_acme", "twin:person:gu_alice", s, e);
        assert_eq!(a, b);
        assert!(a.starts_with("led_"));
    }
}
