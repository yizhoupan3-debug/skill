//! Session-level tool call and token usage tracker.
//!
//! Stores an accumulator in `artifacts/current/SESSION_CALL_TRACKER.json`
//! to detect anomalous usage patterns, especially in Desktop MCP mode
//! where PreToolUse/Stop hooks are unavailable.
//!
//! **Performance optimization**: `record_tool_call` uses in-memory `AtomicU64`
//! counters and only persists to disk periodically (every `FLUSH_INTERVAL_SECS`
//! seconds), avoiding the previous 5-step sync I/O chain on every tool call.

use core_state::utils::task_write_lock::apply_task_ledger_mutation;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CacheStats {
    /// Anthropic API 返回的 cache 命中 token 数（cache_read_input_tokens）。
    pub cache_read_input_tokens: u64,
    /// 本次调用写入缓存的 token 数（cache_creation_input_tokens）。
    pub cache_creation_input_tokens: u64,
    /// 非缓存输入 token 数（cache_read 是 input 的子集，由 API 分别报告）。
    pub input_tokens: u64,
    /// 模型输出 token 数（output_tokens）。
    pub output_tokens: u64,
}
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TRACKER_FILE: &str = "SESSION_CALL_TRACKER.json";
const SCHEMA_VERSION: &str = "fw-session-call-tracker-v1";

/// Minimum interval between disk flushes (seconds). Tool calls within this
/// window are accumulated in memory only.
const FLUSH_INTERVAL_SECS: u64 = 5;

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
static TRACKER_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_tracker_lock() -> &'static Mutex<()> {
    TRACKER_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

// ── In-memory accumulator ────────────────────────────────────────────
static TOTAL_CALLS: AtomicU64 = AtomicU64::new(0);
static FLUSH_NEEDED: AtomicBool = AtomicBool::new(false);

/// Per-tool call counts accumulated in memory.
static PER_TOOL: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
static PER_TOOL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Accumulated token usage (input, output, cache_read, cache_creation).
static TOKEN_INPUT: AtomicU64 = AtomicU64::new(0);
static TOKEN_OUTPUT: AtomicU64 = AtomicU64::new(0);
static TOKEN_CACHE_READ: AtomicU64 = AtomicU64::new(0);
static TOKEN_CACHE_CREATION: AtomicU64 = AtomicU64::new(0);

/// Last time `flush_to_disk` actually wrote.
static LAST_FLUSH: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

fn last_flush() -> &'static Mutex<Option<Instant>> {
    LAST_FLUSH.get_or_init(|| Mutex::new(None))
}

fn should_flush() -> bool {
    let guard = last_flush().lock().expect("last_flush lock");
    match *guard {
        Some(t) => t.elapsed() >= Duration::from_secs(FLUSH_INTERVAL_SECS),
        None => true,
    }
}

/// Drain in-memory accumulators and persist to disk. Called periodically
/// from `record_tool_call` and on-demand from `check_anomalies` / `read_tracker_state`.
fn flush_to_disk(repo_root: &Path) -> Result<(), String> {
    if !FLUSH_NEEDED.load(Ordering::Relaxed) && !should_flush() {
        return Ok(());
    }

    let path = tracker_path(repo_root);

    // Drain per-tool counts
    let tool_drain = {
        let lock = PER_TOOL_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("per_tool lock");
        let map = PER_TOOL.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = map.lock().expect("per_tool map lock");
        std::mem::take(&mut *map)
    };

    // Drain atomic counters
    let total = TOTAL_CALLS.swap(0, Ordering::Relaxed);
    let in_tok = TOKEN_INPUT.swap(0, Ordering::Relaxed);
    let out_tok = TOKEN_OUTPUT.swap(0, Ordering::Relaxed);
    let cr_tok = TOKEN_CACHE_READ.swap(0, Ordering::Relaxed);
    let cc_tok = TOKEN_CACHE_CREATION.swap(0, Ordering::Relaxed);
    FLUSH_NEEDED.store(false, Ordering::Relaxed);

    if total == 0 && tool_drain.is_empty() && in_tok == 0 && out_tok == 0 {
        return Ok(());
    }

    apply_task_ledger_mutation(repo_root, || {
        let mut payload = load_or_init_tracker(&path)?;

        payload["total_calls"] = json!(
            payload["total_calls"].as_u64().unwrap_or(0) + total
        );

        let per_tool = payload["per_tool"]
            .as_object_mut()
            .ok_or_else(|| "per_tool not an object".to_string())?;
        for (tool, count) in &tool_drain {
            let cur = per_tool.get(tool).and_then(Value::as_u64).unwrap_or(0);
            per_tool.insert(tool.clone(), json!(cur + count));
        }
        cap_per_tool_keys(per_tool);

        // §CodeGraph usage breakdown: collect from per_tool
        let total_codegraph_calls: u64 = per_tool
            .iter()
            .filter(|(k, _)| k.starts_with("codegraph_"))
            .map(|(_, v)| v.as_u64().unwrap_or(0))
            .sum();
        let codegraph_per_tool: Value = per_tool
            .iter()
            .filter(|(k, _)| k.starts_with("codegraph_"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if total_codegraph_calls > 0 {
            payload["codegraph_usage"] = json!({
                "total_calls": total_codegraph_calls,
                "per_tool": codegraph_per_tool,
            });
        }

        // Token usage accumulation
        let tu = payload["token_usage"]
            .as_object_mut()
            .ok_or_else(|| "token_usage not an object".to_string())?;
        let cur_in = tu.get("input").and_then(Value::as_u64).unwrap_or(0);
        let cur_out = tu.get("output").and_then(Value::as_u64).unwrap_or(0);
        let cur_cr = tu.get("cache_read").and_then(Value::as_u64).unwrap_or(0);
        let cur_cc = tu.get("cache_creation").and_then(Value::as_u64).unwrap_or(0);
        tu.insert("input".to_string(), json!(cur_in + in_tok));
        tu.insert("output".to_string(), json!(cur_out + out_tok));
        tu.insert("cache_read".to_string(), json!(cur_cr + cr_tok));
        tu.insert("cache_creation".to_string(), json!(cur_cc + cc_tok));
        tu.insert(
            "total".to_string(),
            json!(cur_in + in_tok + cur_out + out_tok),
        );

        write_tracker(&path, &payload)?;

        let mut guard = last_flush().lock().expect("last_flush lock");
        *guard = Some(Instant::now());

        Ok(())
    })
}

// ── Public API ───────────────────────────────────────────────────────

/// Initialize or reset the session tracker.
pub fn init_tracker(repo_root: &Path) -> Result<(), String> {
    let path = tracker_path(repo_root);
    let now = unix_timestamp();
    let payload = default_payload(now);
    write_tracker(&path, &payload)
}

/// Record a tool call in the session tracker.
///
/// Hot path: increments in-memory atomics only. Disk persistence happens
/// periodically (every `FLUSH_INTERVAL_SECS` seconds) or on demand.
pub fn record_tool_call(
    repo_root: &Path,
    tool_name: &str,
    cache_stats: Option<CacheStats>,
) -> Result<(), String> {
    TOTAL_CALLS.fetch_add(1, Ordering::Relaxed);
    FLUSH_NEEDED.store(true, Ordering::Relaxed);

    {
        let lock = PER_TOOL_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("per_tool lock");
        let map = PER_TOOL.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = map.lock().expect("per_tool map lock");
        *map.entry(tool_name.to_string()).or_insert(0) += 1;
    }

    if let Some(stats) = cache_stats {
        TOKEN_INPUT.fetch_add(stats.input_tokens, Ordering::Relaxed);
        TOKEN_OUTPUT.fetch_add(stats.output_tokens, Ordering::Relaxed);
        TOKEN_CACHE_READ
            .fetch_add(stats.cache_read_input_tokens, Ordering::Relaxed);
        TOKEN_CACHE_CREATION
            .fetch_add(stats.cache_creation_input_tokens, Ordering::Relaxed);
    }

    // Periodic flush
    if let Err(e) = flush_to_disk(repo_root) {
        tracing::warn!("failed to flush session tracker to disk: {e}");
    }

    Ok(())
}

/// Check for anomalies. Returns a list of human-readable warning strings.
pub fn check_anomalies(repo_root: &Path) -> Result<Vec<String>, String> {
    // Flush in-memory accumulators so anomaly check sees up-to-date data.
    flush_to_disk(repo_root)?;

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
            let has_closeout = payload["per_tool"].as_object().is_some_and(|obj| {
                obj.keys().any(|k| k.eq_ignore_ascii_case("closeout_gate"))
            });
            if !has_closeout {
                warnings.push("Session has 15+ tool calls but closeout_gate was never called -- session may end without verification.".to_string());
            }
        }

        // Rule 4: Bash dominates
        let bash_count = payload["per_tool"].as_object()
            .and_then(|obj| obj.iter().find(|(k, _)| k.eq_ignore_ascii_case("Bash")))
            .and_then(|(_, v)| v.as_u64());
        if let Some(bash_count) = bash_count
            && total > 0 && bash_count > total / 2 {
                warnings.push(format!("Bash calls ({bash_count}) exceed 50% of total ({total}) -- possible unsafe automation."));
            }

        // Rule 5: Write dominates
        let write_count = payload["per_tool"].as_object()
            .and_then(|obj| obj.iter().find(|(k, _)| k.eq_ignore_ascii_case("Write")))
            .and_then(|(_, v)| v.as_u64());
        if let Some(write_count) = write_count
            && total > 0 && write_count > total * 3 / 10 {
                warnings.push(format!("Write calls ({write_count}) exceed 30% of total ({total}) -- possible blind overwriting."));
            }

        // Rule 6: CodeGraph index exists but no codegraph tools used (after 20+ calls)
        if total >= 20 {
            let codegraph_total = payload["codegraph_usage"]
                .as_object()
                .and_then(|cg| cg.get("total_calls"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if codegraph_total == 0 {
                // Check if the index file exists (don't warn if codegraph isn't set up)
                let index_path = repo_root.join("artifacts/codegraph/index.sqlite");
                if index_path.exists() {
                    warnings.push("CodeGraph index exists but no codegraph tools have been called in this session. Consider using codegraph_search/codegraph_impact for code-aware work.".to_string());
                }
            }
        }

        // Update anomaly_flags in the tracker (always write, clearing stale flags)
        payload["anomaly_flags"] = json!(warnings);
        write_tracker(&path, &payload)?;

        Ok(warnings)
    })
}

/// Read the current tracker state as JSON (for MCP resource).
pub fn read_tracker_state(repo_root: &Path) -> Result<Value, String> {
    // Flush in-memory accumulators so readers see up-to-date data.
    flush_to_disk(repo_root)?;

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
            "total": 0,
            "cache_read": 0,
            "cache_creation": 0
        },
        "anomaly_flags": [],
        "host_id": null
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
    let _guard = lock.lock().map_err(|e| {
        eprintln!("[router-rs] tracker lock poisoned: {e}");
        format!("tracker lock poisoned: {e}")
    })?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create tracker dir: {e}"))?;
    }

    // Use temp file + rename for atomic write (PID suffix avoids cross-process collision)
    let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
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

#[cfg(any(test, feature = "test-support"))]
pub fn test_lock_roundtrip() -> bool {
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
        record_tool_call(&repo, "Read", None).unwrap();
        record_tool_call(&repo, "Read", None).unwrap();
        record_tool_call(&repo, "Bash", None).unwrap();
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
            record_tool_call(&repo, "Read", None).unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn anomaly_skipped_routing() {
        let repo = test_repo("anom-routing");
        init_tracker(&repo).unwrap();
        for _ in 0..12 {
            record_tool_call(&repo, "Read", None).unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.iter().any(|w| w.contains("routing")));
    }

    #[test]
    fn anomaly_no_closeout() {
        let repo = test_repo("anom-closeout");
        init_tracker(&repo).unwrap();
        for _ in 0..55 {
            record_tool_call(&repo, "Read", None).unwrap();
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
            record_tool_call(&repo, &format!("tool_{i}"), None).unwrap();
        }
        record_tool_call(&repo, "tool_new", None).unwrap();
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
            record_tool_call(&repo, "Bash", None).unwrap();
        }
        for _ in 0..2 {
            record_tool_call(&repo, "Read", None).unwrap();
        }
        let warnings = check_anomalies(&repo).unwrap();
        assert!(warnings.iter().any(|w| w.contains("Bash")));
    }

    #[test]
    fn record_tool_with_cache_stats() {
        let repo = test_repo("cache-stats");
        init_tracker(&repo).unwrap();
        let stats = CacheStats {
            cache_read_input_tokens: 100,
            cache_creation_input_tokens: 50,
            input_tokens: 200,
            output_tokens: 100,
        };
        record_tool_call(&repo, "Read", Some(stats)).unwrap();
        let state = read_tracker_state(&repo).unwrap();
        assert_eq!(state["token_usage"]["input"], 200);
        assert_eq!(state["token_usage"]["output"], 100);
        assert_eq!(state["token_usage"]["total"], 300);
        assert_eq!(state["token_usage"]["cache_read"], 100);
        assert_eq!(state["token_usage"]["cache_creation"], 50);
    }

    #[test]
    fn record_tool_without_cache_stats_preserves_usage() {
        let repo = test_repo("no-cache-stats");
        init_tracker(&repo).unwrap();
        record_tool_call(&repo, "Read", None).unwrap();
        let state = read_tracker_state(&repo).unwrap();
        assert_eq!(state["token_usage"]["input"], 0);
        assert_eq!(state["token_usage"]["output"], 0);
        assert_eq!(state["token_usage"]["cache_read"], 0);
    }

    #[test]
    fn debug_default_payload() {
        // Use a fixed timestamp so the snapshot is deterministic.
        insta::assert_debug_snapshot!(super::default_payload(1234567890));
    }

    #[test]
    fn debug_cache_stats_debug() {
        let stats = CacheStats {
            cache_read_input_tokens: 500,
            cache_creation_input_tokens: 200,
            input_tokens: 1000,
            output_tokens: 300,
        };
        insta::assert_debug_snapshot!(stats);
    }

    #[test]
    fn debug_schema_constants() {
        insta::assert_debug_snapshot!((super::SCHEMA_VERSION, super::TRACKER_FILE));
    }
}
