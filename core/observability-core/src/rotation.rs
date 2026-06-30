use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ── Worker log lifecycle ──────────────────────────────────────────────

/// Manages lifecycle of sub‑process worker log files.
///
/// Extracted from `session-supervisor/src/runtime.rs` where it was inline.
/// Provides path resolution and TTL-based cleanup so worker logs don't
/// accumulate indefinitely on disk.
#[derive(Clone, Debug)]
pub struct WorkerLogManager {
    log_dir: PathBuf,
    retention_secs: i64,
}

impl WorkerLogManager {
    /// Create a new manager that stores logs under `log_dir`.
    ///
    /// `retention_secs` controls how long a worker log file survives
    /// after its last modification time once the worker is no longer
    /// tracked in the supervisor store (default: 86 400 = 24 hours).
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            retention_secs: 86_400,
        }
    }

    /// Create a manager with a custom retention period.
    pub fn with_retention(log_dir: PathBuf, retention_secs: i64) -> Self {
        Self {
            log_dir,
            retention_secs,
        }
    }

    /// Build a `WorkerLogManager` from a session-supervisor-style `state_path`.
    ///
    /// Derives the log directory as `<state_path_parent>/logs/`, matching the
    /// convention used in `session-supervisor/src/runtime.rs`.
    pub fn for_state_path(state_path: &Path) -> Self {
        let log_dir = log_dir_from_state_path(state_path);
        Self::new(log_dir)
    }

    /// Build the log file path for a given `worker_id`.
    ///
    /// The ID is sanitised so that filesystem traversal is impossible.
    pub fn worker_log_path(&self, worker_id: &str) -> PathBuf {
        self.log_dir.join(format!("{}.log", sanitize(worker_id)))
    }

    /// Remove log files for workers that are no longer active **and**
    /// whose files are older than the retention period.
    ///
    /// Best-effort — logs warnings on I/O errors but never panics.
    pub fn cleanup_stale_logs(&self, active_worker_ids: &[String]) {
        if !self.log_dir.is_dir() {
            return;
        }

        let active: HashSet<String> = active_worker_ids
            .iter()
            .map(|id| sanitize(id))
            .collect();

        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(self.retention_secs);

        if let Ok(entries) = std::fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("log") {
                    continue;
                }
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                // Skip if the worker is still tracked
                if active.contains(&stem) {
                    continue;
                }
                // Check file age
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let dur = modified
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default();
                        if let Some(mtime) =
                            chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                        {
                            if mtime > cutoff {
                                continue; // too recent
                            }
                        }
                    }
                }
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::debug!(
                        "cleanup_stale_logs: remove {} failed ({e})",
                        path.display()
                    );
                }
            }
        }
    }
}

/// Sanitise an arbitrary string into a filesystem-safe slug.
/// Replaces non-alphanumeric characters with dashes and collapses runs.
pub fn sanitize(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "worker".to_string()
    } else {
        slug
    }
}

// ── Backward-compatible free functions ────────────────────────────────
// These match the original signatures in session-supervisor/src/runtime.rs
// so callers only need to change the import path, not the call syntax.

/// Build a worker log path from a session-supervisor `state_path`.
///
/// Returns `<state_parent>/logs/<sanitized_worker_id>.log`.
pub fn worker_log_path(state_path: &Path, worker_id: &str) -> PathBuf {
    WorkerLogManager::for_state_path(state_path).worker_log_path(worker_id)
}

/// Remove stale worker log files from disk.
///
/// `state_path` is the session-supervisor state file path (its parent's
/// `logs/` subdirectory is scanned). `active_worker_ids` are the workers
/// still tracked in the supervisor store and exempt from cleanup.
pub fn cleanup_stale_logs(state_path: &Path, active_worker_ids: &[String]) {
    WorkerLogManager::for_state_path(state_path).cleanup_stale_logs(active_worker_ids);
}

fn log_dir_from_state_path(state_path: &Path) -> PathBuf {
    state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("logs")
}
