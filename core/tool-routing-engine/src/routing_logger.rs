//! Structured tool routing audit logging.
//!
//! Logs every `route_tool_from_records()` decision as a JSON line to
//! `logs/tool-routing/tool_routing_audit.ndjson`.  Auto-initializes on
//! first use (creates directory + file).  Thread-safe via `Mutex<File>`.
//!
//! Parallel to `routing-engine/src/route/routing.rs::log_decision`.

use crate::types::McpToolDecision;
use std::io::Write;

/// Write a tool routing decision to the structured audit log.
/// Auto-initializes on first use — creates `logs/tool-routing/tool_routing_audit.ndjson`
/// relative to `FRAMEWORK_ROOT` or `CARGO_MANIFEST_DIR`.
pub fn log_tool_decision(decision: &McpToolDecision, query: &str) {
    static LOG_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    static LOG_FILE: std::sync::Mutex<Option<std::io::BufWriter<std::fs::File>>> =
        std::sync::Mutex::new(None);

    let path = LOG_PATH.get_or_init(|| {
        let root = std::env::var("FRAMEWORK_ROOT")
            .or_else(|_| std::env::var("CARGO_MANIFEST_DIR"))
            .unwrap_or_else(|_| ".".to_string());
        let p = std::path::Path::new(&root).join("logs/tool-routing/tool_routing_audit.ndjson");
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        p
    });

    let entry = serde_json::json!({
        "ts": iso_timestamp_now(),
        "query": query,
        "selected_tool": decision.selected_tool,
        "score": decision.score,
        "fuzzy_match": decision.fuzzy_match,
        "matched_token_count": decision.matched_token_count,
        "dispatch_domain": decision.dispatch_domain,
        "mcp_server": decision.mcp_server,
        "top_3_reasons": &decision.reasons.iter().take(3).cloned().collect::<Vec<_>>(),
    });

    if let Ok(mut guard) = LOG_FILE.lock() {
        if guard.is_none() {
            *guard = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open(path)
                .ok()
                .map(|f| {
                    // Rotate if ≥ 10 MB (truncate and restart)
                    if f.metadata().map(|m| m.len()).unwrap_or(0) >= 10 * 1024 * 1024 {
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

/// Returns an ISO-8601 timestamp string.
pub(crate) fn iso_timestamp_now() -> String {
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

/// Gregorian date from days since Unix epoch (Howard Hinnant algorithm).
fn days_to_date(days: i64) -> (i64, i64, i64) {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn days_to_date_epoch() {
        assert_eq!(days_to_date(0), (1970, 1, 1));
    }

    #[test]
    fn iso_timestamp_now_returns_non_empty_string() {
        let ts = iso_timestamp_now();
        assert!(!ts.is_empty());
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
    }

    #[test]
    fn fuzzy_match_handles_typo() {
        use crate::fuzzy::best_fuzzy_score;
        let hints = vec!["screenshot".to_string(), "浏览器截图".to_string()];
        let score = best_fuzzy_score("screeenshot", &hints);
        assert!(score.is_some(), "typo should fuzzy-match via n-gram");
        assert!(score.unwrap() > 50.0);
    }
}
