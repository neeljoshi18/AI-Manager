//! India Standard Time (IST) display helpers.
//!
//! **Storage stays UTC.** All conversion is exact FixedOffset +05:30 — never
//! invents or rounds wall times beyond chrono's official offset math.
//!
//! IST has no DST; offset is always UTC+05:30.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, Timelike, Utc, Weekday};

/// IST = UTC+05:30 (India Standard Time / Asia/Kolkata).
pub fn ist_offset() -> FixedOffset {
    // 5h30m = 19800 seconds east of UTC
    FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("IST offset")
}

/// Convert a UTC instant to IST wall time (precise offset, no invention).
pub fn to_ist(dt: DateTime<Utc>) -> DateTime<FixedOffset> {
    dt.with_timezone(&ist_offset())
}

/// Parse flexible RFC3339 / ISO-ish timestamps (with or without Z/offset) as UTC instant.
pub fn parse_as_utc(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // "2026-08-12T12:00:00" / "2026-08-12 12:00:00" → assume UTC if no zone
    let cleaned = s.replace(' ', "T");
    if let Ok(naive) = NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%dT%H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}

/// RFC3339 with explicit `+05:30` (still the same instant as the UTC input).
pub fn format_ist_rfc3339(dt: DateTime<Utc>) -> String {
    to_ist(dt).to_rfc3339()
}

/// Human list form: `2026-08-13 17:30 IST` (from a real UTC instant).
pub fn format_ist_list(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%Y-%m-%d %H:%M IST").to_string()
}

/// Compact list: `08-13 17:30 IST`.
pub fn format_ist_compact(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%m-%d %H:%M IST").to_string()
}

/// Calendar day key in IST: `YYYY-MM-DD`.
pub fn format_ist_day(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%Y-%m-%d").to_string()
}

/// Hour of day 0–23 in IST (for heat maps).
pub fn ist_hour(dt: DateTime<Utc>) -> u32 {
    to_ist(dt).hour()
}

/// Weekday in IST (Mon=0 … Sun=6).
pub fn ist_weekday_mon0(dt: DateTime<Utc>) -> u32 {
    match to_ist(dt).weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    }
}

/// Date (naive, IST calendar) for digest headers.
pub fn ist_date_label(dt: DateTime<Utc>) -> String {
    to_ist(dt).date_naive().to_string()
}

/// Lookback window for digests: duration label + IST bounds (exact conversion).
pub fn format_lookback_ist(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let secs = (end - start).num_seconds().max(0);
    let label = if secs >= 86_400 {
        format!("{}d", (secs + 43_200) / 86_400)
    } else if secs >= 3600 {
        format!("{}h", (secs + 1800) / 3600)
    } else {
        format!("{}m", (secs + 30) / 60)
    };
    format!(
        "{label} ({} → {})",
        to_ist(start).format("%m-%d %H:%M IST"),
        to_ist(end).format("%m-%d %H:%M IST")
    )
}

/// Re-format a stored timestamp string (assumed UTC if Z/offset missing) → IST list form.
/// Returns original string if unparseable (never invents a time).
pub fn reformat_stored_to_ist_list(s: &str) -> String {
    match parse_as_utc(s) {
        Some(dt) => format_ist_list(dt),
        None => s.to_string(),
    }
}

/// Re-format stored → RFC3339 with +05:30; unchanged if unparseable.
pub fn reformat_stored_to_ist_rfc3339(s: &str) -> String {
    match parse_as_utc(s) {
        Some(dt) => format_ist_rfc3339(dt),
        None => s.to_string(),
    }
}

/// IANA-style zone id we set on person twins for display defaults.
pub const DISPLAY_TIMEZONE: &str = "Asia/Kolkata";

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn utc_noon_is_1730_ist() {
        let utc = Utc.with_ymd_and_hms(2026, 8, 12, 12, 0, 0).unwrap();
        let ist = to_ist(utc);
        assert_eq!(ist.hour(), 17);
        assert_eq!(ist.minute(), 30);
        assert_eq!(format_ist_list(utc), "2026-08-12 17:30 IST");
        assert!(format_ist_rfc3339(utc).ends_with("+05:30"));
        assert!(format_ist_rfc3339(utc).starts_with("2026-08-12T17:30:00"));
    }

    #[test]
    fn day_boundary_crosses_to_next_ist_day() {
        // 2026-08-12 20:00 UTC = 2026-08-13 01:30 IST
        let utc = Utc.with_ymd_and_hms(2026, 8, 12, 20, 0, 0).unwrap();
        assert_eq!(format_ist_day(utc), "2026-08-13");
        assert_eq!(ist_hour(utc), 1);
    }

    #[test]
    fn parse_z_and_roundtrip() {
        let s = "2026-08-12T00:00:00Z";
        let dt = parse_as_utc(s).expect("parse");
        assert_eq!(format_ist_list(dt), "2026-08-12 05:30 IST");
    }

    #[test]
    fn unparseable_not_invented() {
        assert_eq!(reformat_stored_to_ist_list("not-a-date"), "not-a-date");
        assert_eq!(reformat_stored_to_ist_rfc3339("not-a-date"), "not-a-date");
    }

    #[test]
    fn stored_z_relists_plus_0530() {
        let listed = reformat_stored_to_ist_rfc3339("2026-08-12T12:00:00Z");
        assert!(listed.ends_with("+05:30"));
        assert!(listed.starts_with("2026-08-12T17:30:00"));
    }

    #[test]
    fn lookback_uses_ist_labels() {
        let start = Utc.with_ymd_and_hms(2026, 8, 5, 12, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 8, 12, 12, 0, 0).unwrap();
        let w = format_lookback_ist(start, end);
        assert!(w.contains("IST"));
        assert!(w.contains("08-05 17:30 IST"));
        assert!(w.contains("08-12 17:30 IST"));
        assert!(!w.contains("UTC"));
    }
}
