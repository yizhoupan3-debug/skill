//! Zero-match query collector.
//!
//! Records every `route_task()` call that produces a score ≤ 0.0 (no skill
//! matched) into `logs/routing/zero_matches.ndjson`.  The collected data
//! drives coverage-gap analysis and trigger-hint planning for new skills.
//!
//! Thread-safe via `Mutex<File>`, with 10 MB auto-rotation.

#![allow(dead_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

const LOG_DIR: &str = "logs/routing";
const LOG_FILE: &str = "zero_matches.ndjson";
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

static COLLECTOR: LazyLock<Mutex<Option<ZeroMatchCollector>>> =
    LazyLock::new(|| Mutex::new(None));

struct ZeroMatchCollector {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl ZeroMatchCollector {
    fn new(log_dir: &str) -> Result<Self, String> {
        fs::create_dir_all(log_dir)
            .map_err(|e| format!("create zero-match log dir {log_dir}: {e}"))?;
        let path = PathBuf::from(log_dir).join(LOG_FILE);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("open zero-match log {path:?}: {e}"))?;
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
                .map_err(|e| format!("rotate zero-match log {path:?}: {e}", path = self.path))?;
            self.writer = BufWriter::new(rotated);
        }
        Ok(())
    }

    fn write_entry(&mut self, query: &str) -> Result<(), String> {
        self.rotate_if_needed()?;
        let entry = serde_json::json!({
            "ts": super::routing_logger::iso_timestamp_now(),
            "query": query,
        });
        let line = entry.to_string();
        self.writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("write zero-match entry: {e}"))?;
        self.writer
            .write_all(b"\n")
            .map_err(|e| format!("write zero-match newline: {e}"))
    }
}

/// Initialize the zero-match collector.
/// Called once during routing engine startup.
pub fn init_collector() {
    let log_dir = std::env::var("FRAMEWORK_ROOT")
        .ok()
        .map(|root| format!("{root}/{LOG_DIR}"))
        .unwrap_or_else(|| LOG_DIR.to_string());
    if let Ok(collector) = ZeroMatchCollector::new(&log_dir) {
        if let Ok(mut guard) = COLLECTOR.lock() {
            *guard = Some(collector);
        }
    }
}

/// Record a zero-match query.
pub fn record_zero_match(query: &str) {
    if let Ok(mut guard) = COLLECTOR.lock() {
        if let Some(ref mut collector) = *guard {
            let _ = collector.write_entry(query);
        }
    }
}
