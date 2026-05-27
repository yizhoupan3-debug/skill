//! Session-level tool call and token usage tracker.
//!
//! Stores an accumulator in `artifacts/current/SESSION_CALL_TRACKER.json`
//! to detect anomalous usage patterns, especially in Desktop MCP mode
//! where PreToolUse/Stop hooks are unavailable.

use crate::task_write_lock::apply_task_ledger_mutation;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const TRACKER_FILE: &str = "SESSION_CALL_TRACKER.json";
const SCHEMA_VERSION: &str = "fw-session-call-tracker-v1";

fn cap_per_tool_keys(per_tool: &mut serde_json::Map<String, Value>) {
    let max_keys = crate::router_env_flags::router_rs_session_call_tracker_tool_keys_max();
    if per_tool.len() <= max_keys {
        return;
    }
    let mut entries: Vec<(String, u64)> = per_tool
        .iter()
        .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    while per_tool.len() > max_keys {
        if let Some((key, _)) = entries.first() {
            per_tool.remove(key);
            entries.remove(0);
        } else {
            break;
        }
    }
}

/// Global write lock for atomic file operations.
static TRACKER_WRITE_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

fn get_tracker_lock() -> &'static std::sync::Mutex<()> {
    TRACKER_WRITE_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Initialize or reset the session tracker.
pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    let path = tracker_path(repo_root);
    let now = unix_timestamp();
    let payload = default_payload(now);
    write_tracker(&path, &payload)
}

/// Record a tool call in the session tracker.
pub fn record_tool_call(repo_root: &Path, tool_name: &str) -> Result<(), String> {
    apply_task_ledger_mutation(repo_root, || {
        let path = tracker_path(repo_root);
        let mut payload = load_or_init_tracker(&path)?;

        payload["total_calls"] = json!(payload["total_calls"].as_u64().unwrap_or(0) + 1);

        let per_tool = payload["per_tool"]
            .as_object_mut()
            .ok_or_else(|| "per_tool not an object".to_string())?;
        let count = per_tool.get(tool_name).and_then(Value::as_u64).unwrap_or(0);
        per_tool.insert(tool_name.to_string(), json!(count + 1));
        cap_per_tool_keys(per_tool);

        write_tracker(&path, &payload)
    })
}

/// Check for anomalies. Returns a list of human-readable warning strings.
pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String> {
    apply_task_ledger_mutation(repo_root, || {
        let path = tracker_path(repo_root);
        let mut payload = load_or_init_tracker(&path)?;
        let mut warnings: Vec<String> = vec![];

        let total = payload["total_calls"].as_u64().unwrap_or(0);

        // Rule 1: Skipped routing -- no skill_route after 10+ calls
        if total >= 10 {
            let has_routing = payload["per_tool"].get("skill_route").is_some();
            if !has_routing {
                warnings.push("Session has 10+ tool calls but never called skill_route -- routing may have been skipped.".to_string());
            }
        }

        // Rule 2: No goal state set after 20+ calls
        // Desktop sessions that run 20+ calls without setting a GOAL_STATE are
        // likely drifting. The 20-call threshold is tuned for Desktop MCP where
        // sessions are shorter than CLI hooks (which can hit 50+ calls easily).
        if total >= 20 {
            let has_goal = payload["per_tool"].get("goal_state_manage").is_some();
            if !has_goal {
                warnings.push("Session has 20+ tool calls but goal was never started (goal_state_manage not called) -- task may lack focus.".to_string());
            }
        }

        // Rule 3: No closeout after 15+ calls
        // Desktop MCP sessions typically end at 10-20 calls. The 15-call threshold
        // catches unverified Desktop sessions before they grow too long.
        if total >= 15 {
            let has_closeout = payload["per_tool"].get("closeout_gate").is_some();
            if !has_closeout {
                warnings.push("Session has 15+ tool calls but closeout_gate was never called -- session may end without verification.".to_string());
            }
        }

        // Rule 4: Bash dominates
        if let Some(bash_count) = payload["per_tool"].get("Bash").and_then(Value::as_u64) {
            if total > 0 && bash_count > total / 2 {
                warnings.push(format!("Bash calls ({bash_count}) exceed 50% of total ({total}) -- possible unsafe automation."));
            }
        }

        // Rule 5: Write dominates
        if let Some(write_count) = payload["per_tool"].get("Write").and_then(Value::as_u64) {
            if total > 0 && write_count > total * 3 / 10 {
                warnings.push(format!("Write calls ({write_count}) exceed 30% of total ({total}) -- possible blind overwriting."));
            }
        }

        // Update anomaly_flags in the tracker
        if !warnings.is_empty() {
            let flags: Vec<Value> = warnings.iter().map(|w| json!(w)).collect();
            payload["anomaly_flags"] = json!(flags);
            write_tracker(&path, &payload)?;
        }

        Ok(warnings)
    })
}

/// Read the current tracker state as JSON (for MCP resource).
#[allow(dead_code)]
pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String> {
    let path = tracker_path(repo_root);
    load_or_init_tracker(&path)
}

// --- Internal helpers ---

fn tracker_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("current")
        .join(TRACKER_FILE)
}

fn default_payload(started_at: u64) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "started_at": started_at,
        "total_calls": 0,
        "per_tool": {},
        "token_usage": {
            "input": 0,
            "output": 0,
            "total": 0
        },
        "anomaly_flags": []
    })
}

/// Load tracker from disk, or initialize and persist a new one if missing.
fn load_or_init_tracker(path: &Path) -> Result<Value, String> {
    if path.exists() {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read tracker: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse tracker: {e}"))
    } else {
        let now = unix_timestamp();
        let payload = default_payload(now);
        write_tracker(path, &payload)?;
        Ok(payload)
    }
}

fn write_tracker(path: &Path, payload: &Value) -> Result<(), String> {
    // Acquire global lock for thread-safe writes
    let lock = get_tracker_lock();
    let _guard = lock
        .lock()
        .map_err(|e| format!("tracker lock poisoned: {e}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create tracker dir: {e}"))?;
    }

    // Use temp file + rename for atomic write
    let temp_path = path.with_extension("tmp");
    let content = serde_json::to_string_pretty(payload)
        .map_err(|e| format!("Failed to serialize tracker: {e}"))?;

    let file = File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("Failed to flush: {e}"))?;
    drop(writer);

    std::fs::rename(&temp_path, path).map_err(|e| format!("Failed to atomically rename: {e}"))?;

    Ok(())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) fn test_lock_roundtrip() -> bool {
    let lock = get_tracker_lock();
    let guard = lock.lock();
    drop(guard);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "router-rs-session-tracker-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("artifacts").join("current")).unwrap();
        path
    }

    #[test]
    fn init_creates_tracker() {
        let repo = test_repo("init");
        init_tracker(&repo).unwrap();
        assert!(tracker_path(&repo).is_file());
    }

    #[test]
    fn record_tool_increments() {
        let repo = test_repo("record");
        init_tracker(&repo).unwrap();
        record_tool_call(&repo, "Read").unwrap();
        record_tool_call(&repo, "Read").unwrap();
        record_tool_call(&repo, "Bash").unwrap();
        let state = read_tracker_state(&repo).unwrap();
        assert_eq!(state["total_calls"], 3);
        assert_eq!(state["per_tool"]["Read"], 2);
        assert_eq!(state["per_tool"]["Bash"], 1);
    }

    #[test]
    fn auto_init_on_missing() {
        let repo = test_repo("autoinit");
        let state = read_tracker_state(&repo).unwrap();
        assert_eq!(state["total_calls"], 0);
        // Verify it was persisted to disk
        assert!(tracker_path(&repo).is_file());
    }

    #[test]
    fn no_anomalies_below_threshold() {
        let repo = test_repo("no-anom");
        init_tracker(&repo).unwrap();
        for _ in 0..5 {
            record_tool_call(&repo, "Read").unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn anomaly_skipped_routing() {
        let repo = test_repo("anom-routing");
        init_tracker(&repo).unwrap();
        for _ in 0..12 {
            record_tool_call(&repo, "Read").unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.iter().any(|w| w.contains("routing")));
    }

    #[test]
    fn anomaly_no_closeout() {
        let repo = test_repo("anom-closeout");
        init_tracker(&repo).unwrap();
        for _ in 0..55 {
            record_tool_call(&repo, "Read").unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.iter().any(|w| w.contains("closeout_gate")));
    }

    #[test]
    fn per_tool_cap_evicts_lowest_count() {
        let repo = test_repo("cap");
        init_tracker(&repo).unwrap();
        let max = crate::router_env_flags::router_rs_session_call_tracker_tool_keys_max();
        for i in 0..=max {
            record_tool_call(&repo, &format!("tool_{i}")).unwrap();
        }
        record_tool_call(&repo, "tool_new").unwrap();
        let state = read_tracker_state(&repo).unwrap();
        let per_tool = state["per_tool"].as_object().expect("object");
        assert!(per_tool.len() <= max);
        assert!(per_tool.get("tool_new").is_some());
        assert!(per_tool.get("tool_0").is_none());
    }

    #[test]
    fn anomaly_bash_dominates() {
        let repo = test_repo("anom-bash");
        init_tracker(&repo).unwrap();
        for _ in 0..6 {
            record_tool_call(&repo, "Bash").unwrap();
        }
        for _ in 0..2 {
            record_tool_call(&repo, "Read").unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.iter().any(|w| w.contains("Bash")));
    }
}
