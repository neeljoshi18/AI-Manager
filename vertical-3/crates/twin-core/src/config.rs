use crate::model::{
    DEFAULT_BLOCKER_VETO_WINDOW_SECS, DEFAULT_MEDIUM_VETO_WINDOW_SECS, DEFAULT_SHADOW_MODE_DAYS,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwinConfig {
    pub runtime_mode: String,
    pub http_bind: String,
    pub cockroach_url: Option<String>,
    /// Vertical 1 ingestion base URL (health + last-event probes).
    pub v1_base_url: String,
    pub v2_base_url: String,
    pub egress_proxy_url: Option<String>,
    pub egress_enforce: bool,
    pub redis_url: Option<String>,
    pub skip_auth: bool,
    pub shadow_mode_days: i64,
    pub medium_veto_window_secs: i64,
    pub blocker_veto_window_secs: i64,
    pub high_auto_publish_default: bool,
    /// Length of status ledger period (aligned wall clock). Default 1h.
    pub status_window_secs: i64,
    /// Min seconds between Slack DMs per twin. Default 30m. Ingest is continuous; notify is batched.
    pub notify_interval_secs: i64,
    /// Background compile tick. 0 = disabled. Default 30m.
    pub compile_interval_secs: i64,
    /// When true, every compile may DM (demo). When false, only scheduler / force_notify.
    pub notify_on_compile_default: bool,
}

impl Default for TwinConfig {
    fn default() -> Self {
        Self {
            runtime_mode: "embedded".into(),
            http_bind: "0.0.0.0:18083".into(),
            cockroach_url: Some(
                "postgresql://root@127.0.0.1:26257/status_twins?sslmode=disable".into(),
            ),
            v1_base_url: "http://127.0.0.1:18080".into(),
            v2_base_url: "http://127.0.0.1:18082".into(),
            egress_proxy_url: Some("http://127.0.0.1:18090".into()),
            egress_enforce: true,
            redis_url: Some("redis://127.0.0.1:6379".into()),
            skip_auth: true,
            shadow_mode_days: DEFAULT_SHADOW_MODE_DAYS,
            medium_veto_window_secs: DEFAULT_MEDIUM_VETO_WINDOW_SECS,
            blocker_veto_window_secs: DEFAULT_BLOCKER_VETO_WINDOW_SECS,
            high_auto_publish_default: false,
            status_window_secs: 3600,
            notify_interval_secs: 1800,
            compile_interval_secs: 1800,
            notify_on_compile_default: false,
        }
    }
}

impl TwinConfig {
    pub fn from_env() -> Self {
        let mut c = Self::default();
        if let Ok(v) = std::env::var("RUNTIME_MODE") {
            c.runtime_mode = v;
        }
        if let Ok(v) = std::env::var("BIND_ADDR")
            .or_else(|_| std::env::var("HTTP_BIND"))
            .or_else(|_| std::env::var("TWIN_HTTP_BIND"))
        {
            c.http_bind = v;
        }
        if let Ok(v) = std::env::var("COCKROACH_URL").or_else(|_| std::env::var("DATABASE_URL")) {
            c.cockroach_url = Some(v.replace("/defaultdb", "/status_twins")
                .replace("/context_graph", "/status_twins"));
        }
        if let Ok(v) = std::env::var("V1_BASE_URL") {
            c.v1_base_url = v;
        }
        if let Ok(v) = std::env::var("V2_BASE_URL") {
            c.v2_base_url = v;
        }
        if let Ok(v) = std::env::var("EGRESS_PROXY_URL") {
            c.egress_proxy_url = Some(v).filter(|s| !s.is_empty());
        }
        if let Ok(v) = std::env::var("EGRESS_ENFORCE") {
            c.egress_enforce = v == "1" || v.eq_ignore_ascii_case("true");
        }
        if let Ok(v) = std::env::var("REDIS_URL") {
            c.redis_url = Some(v);
        }
        if let Ok(v) = std::env::var("SKIP_AUTH") {
            c.skip_auth = v == "1" || v.eq_ignore_ascii_case("true");
        }
        if let Ok(v) = std::env::var("SHADOW_MODE_DAYS") {
            if let Ok(n) = v.parse() {
                c.shadow_mode_days = n;
            }
        }
        if let Ok(v) = std::env::var("MEDIUM_VETO_WINDOW_SECS") {
            if let Ok(n) = v.parse() {
                c.medium_veto_window_secs = n;
            }
        }
        if let Ok(v) = std::env::var("HIGH_AUTO_PUBLISH_DEFAULT") {
            c.high_auto_publish_default = v == "1" || v.eq_ignore_ascii_case("true");
        }
        if let Ok(v) = std::env::var("STATUS_WINDOW_SECS") {
            if let Ok(n) = v.parse::<i64>() {
                c.status_window_secs = n.max(60);
            }
        }
        if let Ok(v) = std::env::var("NOTIFY_INTERVAL_SECS") {
            if let Ok(n) = v.parse::<i64>() {
                c.notify_interval_secs = n.max(0);
            }
        }
        if let Ok(v) = std::env::var("COMPILE_INTERVAL_SECS") {
            if let Ok(n) = v.parse::<i64>() {
                c.compile_interval_secs = n.max(0);
            }
        }
        if let Ok(v) = std::env::var("NOTIFY_ON_COMPILE") {
            c.notify_on_compile_default = v == "1" || v.eq_ignore_ascii_case("true");
        }
        c
    }

    /// Floor `now` to status window start (UTC).
    pub fn aligned_period(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        use chrono::{Duration, TimeZone, Utc};
        let w = self.status_window_secs.max(60);
        let ts = now.timestamp();
        let start_ts = ts - (ts.rem_euclid(w));
        let start = Utc
            .timestamp_opt(start_ts, 0)
            .single()
            .unwrap_or(now - Duration::seconds(w));
        let end = start + Duration::seconds(w);
        (start, end)
    }

    pub fn is_embedded(&self) -> bool {
        self.runtime_mode.eq_ignore_ascii_case("embedded")
    }
}
