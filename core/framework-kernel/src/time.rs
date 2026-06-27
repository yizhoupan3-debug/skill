//! Time helpers — single source of truth for timestamp generation.

use chrono::Local;

/// Return the current UTC time as an RFC 3339 string with **seconds** precision.
///
/// This is the canonical `now_iso` for the entire framework.
/// All crates MUST use this instead of rolling their own.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Return the current **local** time as an RFC 3339 string with **seconds** precision.
///
/// This is the canonical `current_local_timestamp` for the entire framework.
/// Prefer this over local `chrono::Local` calls to keep time formatting consistent.
pub fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn now_iso_returns_valid_rfc3339() {
        let ts = now_iso();
        // Should contain 'T' separator and 'Z' UTC suffix.
        assert!(ts.contains('T'), "expected T separator in: {ts}");
        assert!(ts.ends_with('Z'), "expected Z UTC suffix in: {ts}");
        // Should parse back without error.
        let parsed = chrono::DateTime::parse_from_rfc3339(&ts);
        assert!(parsed.is_ok(), "failed to parse {ts}: {:?}", parsed.err());
    }

    #[test]
    fn current_local_timestamp_returns_valid_rfc3339() {
        let ts = current_local_timestamp();
        // Should contain 'T' separator.
        assert!(ts.contains('T'), "expected T separator in: {ts}");
        // Should NOT end with 'Z' (local time has offset).
        // Parse with chrono's flexible parser.
        let parsed = chrono::DateTime::parse_from_rfc3339(&ts);
        assert!(
            parsed.is_ok(),
            "failed to parse local timestamp {ts}: {:?}",
            parsed.err()
        );
    }

    #[test]
    fn now_iso_has_seconds_precision() {
        let ts = now_iso();
        // Seconds format: ...T12:34:56Z — no sub-second precision.
        let time_part = ts.split('T').nth(1).unwrap();
        // The time part before 'Z' should be HH:MM:SS
        let without_z = time_part.trim_end_matches('Z');
        let segments: Vec<&str> = without_z.split(':').collect();
        assert_eq!(segments.len(), 3, "expected HH:MM:SS in {without_z}");
        assert_eq!(
            segments[2].len(),
            2,
            "seconds should be 2 digits: {}",
            segments[2]
        );
    }

    #[test]
    fn now_iso_returns_consistent_format() {
        let ts1 = now_iso();
        let ts2 = now_iso();
        // Both should have the same length (RFC 3339 with seconds precision).
        assert_eq!(ts1.len(), ts2.len());
        // Same format pattern.
        assert!(ts1.ends_with('Z'));
        assert!(ts2.ends_with('Z'));
    }

    #[test]
    fn current_local_timestamp_returns_consistent_format() {
        let ts1 = current_local_timestamp();
        let ts2 = current_local_timestamp();
        assert_eq!(ts1.len(), ts2.len());
    }

    #[test]
    fn timestamps_are_recent() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let ts = now_iso();
        let parsed = chrono::DateTime::parse_from_rfc3339(&ts).unwrap();
        let ts_secs = parsed.timestamp() as u64;
        // The timestamp should be within 60 seconds of "now".
        let diff = now_secs.abs_diff(ts_secs);
        assert!(
            diff < 60,
            "timestamp diff too large: {diff}s (now={now_secs}, ts={ts_secs})"
        );
    }
}
