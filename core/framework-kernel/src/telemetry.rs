//! MPSC telemetry pipeline: workers enqueue, Log Aggregator serializes disk writes.

use core_errors::FrameworkError;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

// Re-export from the single source of truth in telemetry-types.
pub use telemetry_types::{PredictionOutcomeCheck, TelemetryEvent};

pub trait TelemetryWriter: Send + Sync {
    fn write_event(&self, event: &TelemetryEvent) -> Result<(), FrameworkError>;
}

/// Worker-side writer: enqueue only; never touches the journal file directly.
pub struct MpscTelemetryWriter {
    sender: SyncSender<TelemetryEvent>,
}

impl MpscTelemetryWriter {
    pub fn new(sender: SyncSender<TelemetryEvent>) -> Self {
        Self { sender }
    }
}

impl TelemetryWriter for MpscTelemetryWriter {
    fn write_event(&self, event: &TelemetryEvent) -> Result<(), FrameworkError> {
        self.sender
            .send(event.clone())
            .map_err(|e| FrameworkError::validation(format!("telemetry channel closed: {e}")))
    }
}

pub struct LogAggregatorHandle {
    sender: SyncSender<TelemetryEvent>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl LogAggregatorHandle {
    pub fn writer(&self) -> MpscTelemetryWriter {
        MpscTelemetryWriter::new(self.sender.clone())
    }

    /// All [`MpscTelemetryWriter`] clones must be dropped before calling this.
    pub fn shutdown(self) {
        drop(self.sender);
        if let Ok(mut join) = self.join.lock()
            && let Some(handle) = join.take() {
                let _ = handle.join();
            }
    }
}

pub struct LogAggregator;

/// Cumulative count of telemetry journal flush failures since process start.
/// Observable via [`flush_failure_count`].
static FLUSH_FAILURE_COUNT: AtomicU64 = AtomicU64::new(0);

/// Returns the number of telemetry journal flush failures since process start.
pub fn flush_failure_count() -> u64 {
    FLUSH_FAILURE_COUNT.load(Ordering::Relaxed)
}

impl LogAggregator {
/// Start aggregator; returns handle with writer + bounded MPSC sender.
///
/// **Back-pressure note:** The channel is bounded (`sync_channel(1024)`).  When
/// journal IO is slow (e.g. disk contention, NFS stalls) the `SyncSender`
/// will block callers of [`MpscTelemetryWriter::write_event`] until space
/// frees up.  Monitor [`flush_failure_count`] to detect sustained back-pressure.
///
/// **Future improvement:** Replace `send` with `try_send` + drop-on-full to
/// make the telemetry path fully non-blocking at the cost of event loss under
/// extreme load.
    pub fn start(journal_path: impl AsRef<Path>) -> LogAggregatorHandle {
        let journal_path = journal_path.as_ref().to_path_buf();
        let (sender, receiver) = mpsc::sync_channel::<TelemetryEvent>(1024);
        let join = thread::spawn(move || {
            if let Some(parent) = journal_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut buffer: Vec<TelemetryEvent> = Vec::with_capacity(16);
            loop {
                match receiver.recv_timeout(Duration::from_secs(5)) {
                    Ok(event) => {
                        buffer.push(event);
                        if buffer.len() >= 10
                            && let Err(e) = flush_buffer(&journal_path, &mut buffer) {
                                FLUSH_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed);
                                tracing::warn!("[router-rs] telemetry flush error: {e}");
                            }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Err(e) = flush_buffer(&journal_path, &mut buffer) {
                            FLUSH_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!("[router-rs] telemetry flush error: {e}");
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        if let Err(e) = flush_buffer(&journal_path, &mut buffer) {
                            FLUSH_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!("[router-rs] telemetry final flush error: {e}");
                        }
                        break;
                    }
                }
            }
        });
        LogAggregatorHandle {
            sender,
            join: Mutex::new(Some(join)),
        }
    }
}

#[derive(Serialize)]
struct JournalLine<'a> {
    ts: String,
    #[serde(flatten)]
    event: &'a TelemetryEvent,
}

fn flush_buffer(journal_path: &Path, buffer: &mut Vec<TelemetryEvent>) -> Result<(), FrameworkError> {
    if buffer.is_empty() {
        return Ok(());
    }
    let mut lines = String::new();
    for event in buffer.iter() {
        let line = JournalLine {
            ts: crate::time::now_iso(),
            event,
        };
        let serialized = serde_json::to_string(&line).map_err(FrameworkError::from)?;
        lines.push_str(&serialized);
        lines.push('\n');
    }
    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(journal_path)
            .map_err(|e| FrameworkError::validation(format!("open journal {}: {e}", journal_path.display())))?;
        file.write_all(lines.as_bytes())
            .map_err(|e| FrameworkError::validation(format!("append journal: {e}")))?;
        file.sync_all().map_err(|e| FrameworkError::validation(format!("sync journal: {e}")))?;
    }
    write_atomic_snapshot(journal_path)?;
    buffer.clear();
    Ok(())
}

/// Point-in-time journal metadata for B11 readers (§4.2 atomic rename).
fn write_atomic_snapshot(journal_path: &Path) -> Result<(), FrameworkError> {
    let dir = journal_path
        .parent()
        .ok_or_else(|| FrameworkError::validation(format!("journal path {} has no parent", journal_path.display())))?;
    let snapshot_path = dir.join("snapshot.json");
    let tmp_path = dir.join("snapshot.json.tmp");
    let bytes = fs::metadata(journal_path).map(|m| m.len()).unwrap_or(0);
    let snapshot = serde_json::json!({
        "schema_version": "telemetry-snapshot-v1",
        "journal_path": journal_path.display().to_string(),
        "bytes": bytes,
        "updated_unix_secs": SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    let payload = serde_json::to_string(&snapshot).map_err(FrameworkError::from)? + "\n";
    fs::write(&tmp_path, payload.as_bytes())
        .map_err(|e| FrameworkError::validation(format!("write snapshot tmp {}: {e}", tmp_path.display())))?;
    fs::rename(&tmp_path, &snapshot_path)
        .map_err(|e| FrameworkError::validation(format!("rename snapshot {}: {e}", snapshot_path.display())))?;
    Ok(())
}

static GLOBAL_WRITER: std::sync::OnceLock<Arc<dyn TelemetryWriter>> = std::sync::OnceLock::new();

pub fn install_global_telemetry_writer(writer: Arc<dyn TelemetryWriter>) {
    let _ = GLOBAL_WRITER.set(writer);
}

pub fn global_telemetry_writer() -> Option<Arc<dyn TelemetryWriter>> {
    GLOBAL_WRITER.get().cloned()
}

pub fn emit_telemetry(event: &TelemetryEvent) {
    if let Some(writer) = global_telemetry_writer()
        && let Err(e) = writer.write_event(event) {
            FLUSH_FAILURE_COUNT.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("[router-rs] telemetry emit error: {e}");
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn flush_writes_atomic_snapshot() {
        let suffix = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fwk-snapshot-{suffix}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let handle = LogAggregator::start(&journal);
        {
            let writer = handle.writer();
            writer
                .write_event(&TelemetryEvent::RouteDecision {
                    task: "t".into(),
                    skill: "pdf".into(),
                    confidence: 0.8,
                    reroute: false,
                    latency_ms: 0,
                    reasons: vec![],
                    matched_tokens: 0,
                    parity_gate: "".into(),
                    candidates: vec![],
                })
                .unwrap();
        }
        handle.shutdown();
        let snapshot = dir.join("snapshot.json");
        assert!(
            snapshot.is_file(),
            "expected atomic snapshot at {}",
            snapshot.display()
        );
        let raw = fs::read_to_string(&snapshot).unwrap();
        assert!(raw.contains("telemetry-snapshot-v1"));
        assert!(raw.contains("events.jsonl"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_writers_enqueue_without_tearing() {
        let suffix = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fwk-telemetry-conc-{suffix}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let handle = LogAggregator::start(&journal);
        let mut joins = Vec::new();
        for idx in 0..4 {
            let writer = handle.writer();
            joins.push(thread::spawn(move || {
                for n in 0..8 {
                    let _ = writer.write_event(&TelemetryEvent::ToolCall {
                        tool: format!("tool-{idx}-{n}"),
                        duration_ms: n as u64,
                        success: n % 2 == 0,
                    });
                }
            }));
        }
        for join in joins {
            join.join().unwrap();
        }
        handle.shutdown();
        let raw = fs::read_to_string(&journal).unwrap();
        let line_count = raw.lines().filter(|line| !line.is_empty()).count();
        assert_eq!(line_count, 32, "expected one JSONL line per enqueued event");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn aggregator_writes_jsonl_lines() {
        let suffix = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fwk-telemetry-{suffix}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("events.jsonl");
        let handle = LogAggregator::start(&journal);
        {
            let writer = handle.writer();
            writer
                .write_event(&TelemetryEvent::HookFired {
                    hook_name: "pre_tool".into(),
                    action: "allow".into(),
                })
                .unwrap();
        }
        handle.shutdown();
        let raw = fs::read_to_string(&journal).unwrap();
        assert!(raw.contains("\"hook_fired\""));
        assert!(raw.contains("pre_tool"));
        assert!(
            raw.lines()
                .any(|line| line.contains("\"ts\"") && line.contains("T")),
            "expected RFC3339 ts on flushed journal lines"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
