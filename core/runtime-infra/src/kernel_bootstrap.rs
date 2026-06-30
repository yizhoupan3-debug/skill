//! B0 kernel DI: TokenizerProvider (B1→B0), review context probes, and route cache invalidator.
//!
//! Telemetry bootstrap (LogAggregator, TelemetryObserver) removed per v10 Wave 2d.
//!
//! # Cross-session cleanup
//! On first bootstrap, stale task pointers in `artifacts/current/` are cleared
//! to prevent cross-session leakage (a 3-day-old `active_task_id` would otherwise
//! be read as current by `load_framework_runtime_view`). Task subdirectories with
//! evidence artifacts are preserved — only index and pointer files are reset.

use routing_engine::routing_runtime_watch;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

struct RouteTokenizerProvider;

impl framework_core::TokenizerProvider for RouteTokenizerProvider {
    fn tokenize_query(&self, text: &str) -> Vec<String> {
        routing_engine::route::tokenize_query(text)
    }
}

static BOOTSTRAP_SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Request the bootstrap background thread to shut down gracefully.
/// The thread will exit on its next loop iteration.
pub fn request_bootstrap_shutdown() {
    BOOTSTRAP_SHUTDOWN.store(true, Ordering::Relaxed);
}

static BOOTSTRAP_ONCE: Once = Once::new();

/// Idempotent B0 wiring (tokenizer DI + probes + route cache invalidator).
pub fn ensure_kernel_bootstrap() {
    BOOTSTRAP_ONCE.call_once(|| {
        bootstrap_core(); // tokenizer + probes (all modes need these)
        clear_stale_artifact_pointers(); // P0: prevent cross-session pointer leakage
        spawn_routing_runtime_cache_invalidator();
    });
}

#[cfg(target_os = "linux")]
fn current_exe_name() -> Option<String> {
    std::env::current_exe()
        .ok()?
        .file_name()?
        .to_str()
        .map(|s| s.to_string())
}

#[cfg(not(target_os = "linux"))]
fn current_exe_name() -> Option<String> {
    std::env::current_exe()
        .ok()?
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Clear stale task pointers from `artifacts/current/` at bootstrap,
/// preventing cross-session leakage (P0). Also cleans up stale task
/// subdirectories (P1) that are no longer referenced by any active pointer.
/// Only runs in the main CLI process (not in subagent binaries) to avoid
/// corrupting the parent session's state.
///
/// # Safety
/// - Only clears pointer/index files (active_task.json, focus_task.json,
///   task_registry.json, and the `active_task_id`/`focus_task_id` fields
///   of TASK_POINTERS.json).
/// - Task subdirectories with evidence artifacts are only removed when
///   their task_id does not match any entry in TASK_POINTERS.json (i.e.,
///   they are orphaned subdirectories from prior sessions).
/// - Subagent binaries (containing "subagent", "agent-daemon", or
///   "subagent-permit" in their path) skip this step.
fn clear_stale_artifact_pointers() {
    // Only run in the main process — skip if we're a subagent
    if let Some(exe) = current_exe_name() {
        let exe_lower = exe.to_ascii_lowercase();
        if exe_lower.contains("subagent")
            || exe_lower.contains("agent-daemon")
            || exe_lower.contains("subagent-permit")
        {
            return;
        }
    }

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("clear_stale_artifact_pointers: no cwd ({e})");
            return;
        }
    };
    let artifact_dir = cwd.join("artifacts/current");
    if !artifact_dir.is_dir() {
        return; // nothing to clean
    }

    // 1. Build a set of known task IDs from TASK_POINTERS.json, then clear pointers
    let mut known_task_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let pointers_path = artifact_dir.join("TASK_POINTERS.json");
    if pointers_path.is_file() {
        let raw = match std::fs::read_to_string(&pointers_path) {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("clear_stale_artifact_pointers: read TASK_POINTERS failed ({e})");
                return;
            }
        };
        // Read known task IDs before clearing
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(tasks) = data.get("tasks").and_then(|v| v.as_array()) {
                for entry in tasks {
                    if let Some(tid) = entry.get("task_id").and_then(|v| v.as_str()) {
                        known_task_ids.insert(tid.to_string());
                    }
                }
            }
        }
        // Check if there's actually stale data before writing
        let should_clear = raw.contains(r#""active_task_id""#)
            || raw.contains(r#""focus_task_id""#);
        if should_clear {
            let mut data: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(_) => serde_json::json!({}),
            };
            if let Some(obj) = data.as_object_mut() {
                obj.remove("active_task_id");
                obj.remove("focus_task_id");
                if let Some(tasks) = obj.get_mut("tasks").and_then(|v| v.as_array_mut()) {
                    tasks.clear();
                }
            }
            if let Err(e) =
                core_state_utils::atomic_write::write_atomic_json(&pointers_path, &data)
            {
                tracing::warn!(
                    "clear_stale_artifact_pointers: write TASK_POINTERS.json failed ({e})"
                );
            }
        }
    }

    // 2. Remove orphaned task subdirectories (those whose task_id is NOT in
    //    TASK_POINTERS.json. After the pointer clear above, known_task_ids is
    //    empty, so ALL task subdirectories from prior sessions are removed.)
    if let Ok(entries) = std::fs::read_dir(&artifact_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Only clean subdirectories that look like task dirs (contain GOAL_STATE.json)
            if !path.join("GOAL_STATE.json").is_file() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Skip directories that match known task IDs (still referenced)
            if !known_task_ids.is_empty() && known_task_ids.contains(&dir_name) {
                continue;
            }
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::debug!(
                    "clear_stale_artifact_pointers: remove task dir {} failed ({e})",
                    path.display()
                );
            }
        }
    }

    // 3. Remove legacy pointer files
    for name in ["active_task.json", "focus_task.json", "task_registry.json"] {
        let path = artifact_dir.join(name);
        if path.is_file() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::debug!(
                    "clear_stale_artifact_pointers: remove {} failed ({e})",
                    path.display()
                );
            }
        }
    }
}

/// Core DI: tokenizer provider + review context probes.
/// Light enough for any subprocess — no threads, no file handles.
fn bootstrap_core() {
    framework_core::install_tokenizer_provider(Box::new(RouteTokenizerProvider));
    framework_core::review_context_signals::install_review_context_probes(
        routing_engine::route::has_paper_context,
        routing_engine::route::has_github_pr_context,
    );
}

/// Invalidate route record cache when `SKILL_ROUTING_RUNTIME.json` changes on disk (P1-1).
/// Polls every 1s (config file changes don't need sub-second detection).
fn spawn_routing_runtime_cache_invalidator() {
    thread::spawn(move || {
        let watch = routing_runtime_watch();
        let mut rx = watch.receiver();
        #[allow(clippy::let_unit_value)]
        let _ = rx.borrow_and_update();
        loop {
            if BOOTSTRAP_SHUTDOWN.load(Ordering::Relaxed) {
                tracing::debug!("kernel_bootstrap: shutdown requested, exiting cache invalidator");
                return;
            }
            thread::sleep(Duration::from_secs(1));
            if !matches!(rx.has_changed(), Ok(true)) {
                continue;
            }
            #[allow(clippy::let_unit_value)]
            let _ = rx.borrow_and_update();
            if let Err(e) = routing_engine::route::invalidate_records_cache() {
                tracing::warn!("route cache invalidation failed: {e}");
            }
        }
    });
}
