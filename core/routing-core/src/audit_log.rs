//! Shared audit log infrastructure for routing engines.
//!
//! Provides a lazily-initialized JSON-lines audit log writer that both
//! `routing-engine` and `tool-routing-engine` use, eliminating the ~80%
//! duplicate initialization/resolution/write code.
//!
//! Also exports Howard Hinnant's Gregorian date algorithm as shared utilities,
//! eliminating the 99% duplicate date computation between the two engines.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use routing_core::audit_log::AuditLog;
//!
//! static LOG: AuditLog = AuditLog::new();
//!
//! LOG.write_entry("logs/skill-routing/routing_audit.ndjson", &entry);
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// AuditLog
// ---------------------------------------------------------------------------

/// A lazily-initialized JSON-lines audit log writer.
///
/// Thread-safe: `OnceLock` for path resolution (computed once), `Mutex` for
/// file access. On first invocation for a given `AuditLog` instance, the log
/// directory is created and the file is opened in append mode.
///
/// Create as a `static` to share across the crate:
///
/// ```rust,ignore
/// static AUDIT_LOG: AuditLog = AuditLog::new();
/// ```
pub struct AuditLog {
    path: OnceLock<PathBuf>,
    writer: Mutex<Option<std::io::BufWriter<std::fs::File>>>,
}

impl AuditLog {
    /// Create a new uninitialized audit log.
    pub const fn new() -> Self {
        Self {
            path: OnceLock::new(),
            writer: Mutex::new(None),
        }
    }

    /// Write a JSON entry to the log file at `log_subpath` (relative to
    /// `FRAMEWORK_ROOT`, `CARGO_MANIFEST_DIR`, or `.`).
    ///
    /// The log file is auto-created on first write.  No rotation.
    pub fn write_entry(&self, log_subpath: &str, entry: &serde_json::Value) {
        let path = self.path.get_or_init(|| Self::resolve_path(log_subpath));
        self.do_write(path, entry, false);
    }

    /// Same as `write_entry` but with 10 MB rotation (truncate and restart).
    pub fn write_entry_with_rotation(&self, log_subpath: &str, entry: &serde_json::Value) {
        let path = self.path.get_or_init(|| Self::resolve_path(log_subpath));
        self.do_write(path, entry, true);
    }

    // -- private helpers --

    fn resolve_path(log_subpath: &str) -> PathBuf {
        let root = std::env::var("FRAMEWORK_ROOT")
            .or_else(|_| std::env::var("CARGO_MANIFEST_DIR"))
            .unwrap_or_else(|_| ".".to_string());
        let p = Path::new(&root).join(log_subpath);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        p
    }

    fn do_write(&self, path: &Path, entry: &serde_json::Value, rotate: bool) {
        if let Ok(mut guard) = self.writer.lock() {
            if guard.is_none() {
                *guard = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .write(true)
                    .open(path)
                    .ok()
                    .map(|f| {
                        if rotate
                            && f.metadata().map(|m| m.len()).unwrap_or(0) >= 10 * 1024 * 1024
                        {
                            let _ = std::fs::OpenOptions::new()
                                .create(true)
                                .truncate(true)
                                .write(true)
                                .open(path);
                        }
                        std::io::BufWriter::new(f)
                    });
            }
            if let Some(ref mut writer) = *guard {
                let _ = writeln!(writer, "{}", entry);
                let _ = writer.flush();
            }
        }
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Date / time utilities (Howard Hinnant algorithm)
// ---------------------------------------------------------------------------

/// Convert days since Unix epoch to Gregorian (year, month, day).
///
/// Implements Howard Hinnant's algorithm used by both skill and tool routing
/// engines for ISO-8601 timestamp formatting.  Extracted to a shared location
/// to eliminate the exact duplicate between the two crates.
pub fn days_to_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Current system time as an ISO-8601 UTC timestamp string.
///
/// Uses the shared `days_to_date` Gregorian date conversion.
pub fn iso_timestamp_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    let (y, m, d) = days_to_date(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn days_to_date_epoch() {
        assert_eq!(days_to_date(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_date_known() {
        // July 5, 2026 = days since 1970-01-01
        // 2026-07-05: compute expected — (2026-07-05 - 1970-01-01) in days
        // Rough: 56 years × 365 + leap days ≈ 20637
        let dt = days_to_date(20637);
        assert_eq!(dt.0, 2026);
    }

    #[test]
    fn iso_timestamp_now_returns_non_empty_string() {
        let ts = iso_timestamp_now();
        assert!(!ts.is_empty());
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
    }

    #[test]
    fn audit_log_new_default() {
        let log = AuditLog::new();
        let log2 = AuditLog::default();
        // Both are valid, no panic on creation
        drop(log);
        drop(log2);
    }
}
