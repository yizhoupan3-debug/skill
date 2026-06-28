//! Structured tool routing audit logging.
//!
//! Logs every `route_tool_from_records()` decision as a JSON line to
//! `logs/tool-routing/tool_routing_audit.ndjson`.  Thread-safe via
//! `Mutex<File>`, with 10 MB auto-rotation.
//!
//! Parallel to `routing-engine/src/route/routing_logger.rs`.

#![allow(dead_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use crate::types::McpToolDecision;

const LOG_DIR: &str = "logs/tool-routing";
const LOG_FILE: &str = "tool_routing_audit.ndjson";
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

static LOGGER: LazyLock<Mutex<Option<ToolRoutingLogger>>> = LazyLock::new(|| Mutex::new(None));

struct ToolRoutingLogger {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl ToolRoutingLogger {
    fn new(log_dir: &str) -> Result<Self, String> {
        fs::create_dir_all(log_dir)
            .map_err(|e| format!("create tool routing log dir {log_dir}: {e}"))?;
        let path = PathBuf::from(log_dir).join(LOG_FILE);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("open tool routing log {path:?}: {e}"))?;
        Ok(Self {
            writer: BufWriter::new(file),
            path,
        })
    }

    fn rotate_if_needed(&mut self) -> Result<(), String> {
        if self.path.metadata().map(|m| m.len()).unwrap_or(0) >= MAX_LOG_BYTES {
            let rotated = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&self.path)
                .map_err(|e| format!("rotate tool routing log {path:?}: {e}", path = self.path))?;
            self.writer = BufWriter::new(rotated);
        }
        Ok(())
    }

    fn write_entry(&mut self, entry: &str) -> Result<(), String> {
        self.rotate_if_needed()?;
        self.writer
            .write_all(entry.as_bytes())
            .map_err(|e| format!("write tool routing log: {e}"))?;
        self.writer
            .write_all(b"\n")
            .map_err(|e| format!("write tool routing log newline: {e}"))
    }
}

/// Initialize the tool routing logger. Creates the log directory and opens
/// the audit log file for appending. Idempotent — safe to call multiple times;
/// subsequent calls are silently ignored once the logger is active.
///
/// Should be called during runtime-core bootstrap with the repo root's log path.
pub fn init_tool_routing_logger(log_dir: &str) -> Result<(), String> {
    let mut guard = LOGGER
        .lock()
        .map_err(|_| "routing logger mutex poisoned".to_string())?;
    if guard.is_some() {
        return Ok(()); // Already initialized — idempotent
    }
    *guard = Some(ToolRoutingLogger::new(log_dir)?);
    Ok(())
}

/// Check whether the tool routing logger is active (has been initialized).
pub fn is_tool_routing_logger_active() -> bool {
    LOGGER.lock().ok().map(|g| g.is_some()).unwrap_or(false)
}

/// Write a tool routing decision to the structured audit log.
///
/// Auto-initializes with default log directory on first call if
/// `init_tool_routing_logger` was not called explicitly.
pub fn log_tool_decision(decision: &McpToolDecision, query: &str) {
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
    if let Ok(mut guard) = LOGGER.lock() {
        if guard.is_none() {
            // Auto-init with default path on first use
            match ToolRoutingLogger::new(LOG_DIR) {
                Ok(logger) => {
                    *guard = Some(logger);
                }
                Err(e) => {
                    tracing::warn!("failed to auto-init tool routing logger: {e}");
                    return;
                }
            }
        }
        if let Some(ref mut logger) = *guard {
            let _ = logger.write_entry(&entry.to_string());
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
}
