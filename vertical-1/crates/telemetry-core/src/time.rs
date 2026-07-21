use chrono::{DateTime, TimeZone, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_millis(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

pub fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Unix epoch milliseconds → UTC.
pub fn from_millis(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}

/// Unix epoch seconds → UTC.
pub fn from_secs(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(Utc::now)
}
