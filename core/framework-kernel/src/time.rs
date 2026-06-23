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
