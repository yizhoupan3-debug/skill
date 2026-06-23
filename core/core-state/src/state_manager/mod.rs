//! State manager module — split into sub-modules for maintainability.
//!
//! All pub APIs are re-exported here so downstream code using
//! `use core_state::state_manager::*` continues to work unchanged.

mod goal_ops;
mod pointer_ops;
mod quality_gate_ops;
mod scrub_ops;
mod validation;

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

// ── Constants ──
pub const GOAL_STATE_FILENAME: &str = "GOAL_STATE.json";
pub const GOAL_STATE_SCHEMA_VERSION: &str = "router-rs-goal-v1";
pub const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
pub const QUALITY_GATE_STATE_FILENAME: &str = "RFV_LOOP_STATE.json";
pub const REQUIRES_COMPLETION_EVIDENCE_KEY: &str = "requires_completion_evidence";
// LEGACY_GOAL_DRIVE_PARAGRAPH_PREFIX removed — no callers, legacy prefix from retired goal-drive paragraph format.
pub const CONTINUITY_SESSION_CHECKPOINT_TASK_ID: &str = "continuity-session";

// Re-export from validation
pub use validation::EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN;
pub use validation::{
    source_traceable_heuristic, validate_external_research_strict,
    validate_external_research_structured,
};

// Re-export from pointer_ops
pub use pointer_ops::{
    ensure_task_directory, neutralize_task_pointers_for_task, read_active_task_id,
    read_focus_task_id, read_primary_task_id, read_task_pointer_pair,
    sync_task_pointers_after_goal_drive, write_active_task_pointer,
};

// Re-export from quality_gate_ops (primary names)
pub use quality_gate_ops::{
    deactivate_goal_for_conflict_with_quality_gate, quality_gate_state_path, read_quality_gate_state,
};
// Re-export from goal_ops
pub use goal_ops::{
    framework_goal_drive, task_evidence_artifacts_summary_for_task,
    task_evidence_success_only_self_attested,
};

// Re-export from scrub_ops
pub use scrub_ops::{
    merge_hook_nudge_paragraph, scrub_followup_fields_in_hook_output,
    scrub_spoof_host_followup_lines, strip_followup_paragraphs_with_line_prefix,
};

// ── Goal state path ──
pub fn goal_state_path_for_task(
    repo_root: &Path,
    task_id: &str,
) -> Result<std::path::PathBuf, String> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join(GOAL_STATE_FILENAME))
}

fn goal_state_path_for_nested_under_current(
    repo_root: &Path,
    rel: &str,
) -> Option<std::path::PathBuf> {
    let rel = rel.trim().trim_matches('/');
    if rel.is_empty() {
        return None;
    }
    let mut dir = repo_root.join("artifacts/current");
    for seg in rel.split(['/', '\\']) {
        let seg = seg.trim();
        if seg.is_empty() || crate::utils::path_guard::safe_task_id_component(seg).is_none() {
            return None;
        }
        dir = dir.join(seg);
    }
    Some(dir.join(GOAL_STATE_FILENAME))
}

// ── Session ID / staleness ──
fn current_env_session_id() -> Option<String> {
    for (key, val) in std::env::vars() {
        if key.ends_with("_SESSION_ID") {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

/// Check if a GOAL_STATE's session_id matches the current session.
/// If the GOAL_STATE has a `session_id` field and it does not match the current
/// environment session (from env vars or absent), annotate with `stale=true`.
///
/// Goals without `session_id` (legacy) are treated as still valid (backward compat).
fn annotate_goal_staleness(goal: &mut Value) {
    let goal_session_id = match goal.get("session_id").and_then(Value::as_str) {
        Some(s) => s.trim(),
        None => {
            // Legacy goal without session_id — not stale (backward compat)
            return;
        }
    };
    if goal_session_id.is_empty() {
        return;
    }
    // Get current session_id from env (do NOT auto-generate; absence means we can't compare)
    let current_session_id = current_env_session_id();
    match current_session_id {
        Some(ref current) if current != goal_session_id => {
            if let Some(obj) = goal.as_object_mut() {
                obj.insert("stale".to_string(), serde_json::json!(true));
                obj.insert(
                    "stale_reason".to_string(),
                    serde_json::json!("session_id mismatch: goal belongs to a different session"),
                );
            }
        }
        _ => {
            // Same session or can't determine current session — not stale
        }
    }
}

// ── Goal state read ──
pub fn read_goal_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_goal_drive: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let (active, focus) = read_task_pointer_pair(repo_root);
        let Some(t) = active.or(focus) else {
            return Ok(None);
        };
        t
    };
    crate::utils::path_guard::validate_task_id_component(&task_id)
        .map_err(|e| format!("framework_goal_drive: invalid task_id for GOAL_STATE path: {e}"))?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read GOAL_STATE: {err}"))?;
    let mut value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse GOAL_STATE: {err}"))?;
    // session-scoped goal: check session_id staleness
    annotate_goal_staleness(&mut value);
    Ok(Some(value))
}

/// 能解析为 JSON 的 `GOAL_STATE` 才返回；读失败或非法 JSON 返回 `None`（便于换指针/扫描回退）。
pub fn read_goal_state_pair_if_valid(repo_root: &Path, task_id: &str) -> Option<(Value, String)> {
    if task_id.trim().is_empty() {
        return None;
    }
    let path = match goal_state_path_for_task(repo_root, task_id) {
        Ok(p) => p,
        Err(_) => goal_state_path_for_nested_under_current(repo_root, task_id)?,
    };
    if !path.is_file() {
        return None;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("read goal state ({:?}): {e}", path);
            return None;
        }
    };
    let mut value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("parse goal state ({:?}): {e}", path);
            return None;
        }
    };
    // session-scoped goal: annotate staleness
    annotate_goal_staleness(&mut value);
    let tid_out = task_id
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    Some((value, tid_out))
}

/// `GOAL_STATE` 是否处于「宏控制应续跑」态（`drive_until_done` + `status=running`）。
/// Stale goals (session_id mismatch) do NOT request continuation.
pub fn goal_state_requests_continuation(state: &Value) -> bool {
    // Stale goals from a different session should not drive continuation
    if state.get("stale").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    let drive = state
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    drive && status == "running"
}

// ── Hydration ──

/// Cursor Stop/drive 门控回补：只依次尝试 `active_task.json`、`focus_task.json`。
/// 历史 orphan goal 不能被当作当前任务续跑真源。
pub fn read_goal_state_for_hydration(repo_root: &Path) -> Result<Option<(Value, String)>, String> {
    let (active_task_id, focus_task_id) = read_task_pointer_pair(repo_root);
    read_goal_state_for_hydration_from_pointer_ids(repo_root, &active_task_id, &focus_task_id)
}

/// Same semantics as [`read_goal_state_for_hydration`], but uses pointer ids from a single
/// snapshot (e.g. paired with [`crate::task_state::resolve_task_view_with_pointers`]).
pub fn read_goal_state_for_hydration_from_pointer_ids(
    repo_root: &Path,
    active_task_id: &Option<String>,
    focus_task_id: &Option<String>,
) -> Result<Option<(Value, String)>, String> {
    select_goal_state_from_pointer_ids(repo_root, active_task_id, focus_task_id)
}

/// Single continuation truth for hydration, Stop checkpoint, and hook drive followups.
///
/// Priority: active GOAL when it requests continuation; else focus when it requests continuation;
/// else active GOAL if readable; else focus GOAL. Never scans orphan goals by mtime.
pub fn select_goal_state_from_pointer_ids(
    repo_root: &Path,
    active_task_id: &Option<String>,
    focus_task_id: &Option<String>,
) -> Result<Option<(Value, String)>, String> {
    let active_pair = active_task_id
        .as_ref()
        .and_then(|id| read_goal_state_pair_if_valid(repo_root, id));
    let focus_pair = focus_task_id
        .as_ref()
        .and_then(|id| read_goal_state_pair_if_valid(repo_root, id));

    if let Some((goal, tid)) = active_pair {
        if goal_state_requests_continuation(&goal) {
            return Ok(Some((goal, tid)));
        }
        if let Some((fgoal, ftid)) = focus_pair {
            if goal_state_requests_continuation(&fgoal) {
                return Ok(Some((fgoal, ftid)));
            }
            // Active readable but not driving: prefer focus GOAL when present (completed active + running focus).
            return Ok(Some((fgoal, ftid)));
        }
        return Ok(Some((goal, tid)));
    }
    // Active pointer set but GOAL unreadable: fall back to focus when readable (P1-11).
    if active_task_id
        .as_ref()
        .is_some_and(|id| !id.trim().is_empty())
    {
        if let Some(pair) = focus_pair {
            return Ok(Some(pair));
        }
        return Ok(None);
    }
    if let Some((goal, tid)) = focus_pair {
        if goal_state_requests_continuation(&goal) {
            return Ok(Some((goal, tid)));
        }
        return Ok(Some((goal, tid)));
    }
    Ok(None)
}

// ── Diagnostics scan ──

const GOAL_DISCOVER_MAX_DEPTH: usize = 8;

fn discover_goal_state_task_ids_under_current(
    repo_root: &Path,
) -> Result<Vec<(String, SystemTime)>, String> {
    let current = repo_root.join("artifacts/current");
    if !current.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    visit_goal_state_dirs(&current, &current, GOAL_DISCOVER_MAX_DEPTH, &mut out)?;
    Ok(out)
}

fn visit_goal_state_dirs(
    dir: &Path,
    current_root: &Path,
    depth: usize,
    out: &mut Vec<(String, SystemTime)>,
) -> Result<(), String> {
    if depth == 0 {
        return Ok(());
    }
    let goal_path = dir.join(GOAL_STATE_FILENAME);
    if goal_path.is_file()
        && let Ok(rel) = dir.strip_prefix(current_root) {
            let tid_norm = rel
                .to_str()
                .map(|s| s.trim().replace('\\', "/"))
                .filter(|s| !s.is_empty());
            if let Some(tid_norm) = tid_norm {
                let mtime = fs::metadata(&goal_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                out.push((tid_norm, mtime));
            }
        }
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let p = entry.path();
        if p.is_dir() {
            visit_goal_state_dirs(&p, current_root, depth - 1, out)?;
        }
    }
    Ok(())
}

/// 诊断 / 测试专用 mtime 扫描：picks the **newest** `GOAL_STATE.json` under `artifacts/current/**`.
///
/// 整个扫描链（包括下方 `discover_*` / `visit_*` / `GOAL_DISCOVER_MAX_DEPTH`）只在
/// `hydration_ignores_orphan_goal_when_active_task_missing` 等单测里复活 orphan goal 用于负面断言。
/// **绝不能**从 Cursor / Codex / Claude hook 的续跑路径调用：continuity 真源是
/// [`read_goal_state_for_hydration`]（active → focus，不做 orphan mtime sweep）。
pub fn read_goal_state_for_diagnostics_scan(
    repo_root: &Path,
) -> Result<Option<(Value, String)>, String> {
    let mut candidates = discover_goal_state_task_ids_under_current(repo_root)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    candidates.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    for (tid, _) in candidates {
        if let Some(pair) = read_goal_state_pair_if_valid(repo_root, &tid) {
            return Ok(Some(pair));
        }
    }
    Ok(None)
}

// ── Evidence helpers ──

/// 单一来源：`EVIDENCE_INDEX.json` 单条 artifact 是否计作「成功验证」。
/// 规则：`success == true` **或** `exit_code` 取 0（i64 或 u64 皆可）。
/// `rfv_loop` 与 `goal_drive` 都走这里，防止两路证据口径分叉。
pub fn evidence_index_entry_implies_success(entry: &Value) -> bool {
    if entry.get("success").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    match entry.get("exit_code") {
        Some(v) => v.as_i64() == Some(0) || v.as_u64() == Some(0),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Lock for tests that mutate env vars (*_SESSION_ID etc.) to avoid race conditions.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn goal_start_writes_and_status_reads() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

        let rr = repo.display().to_string();
        let out = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "my-task",
            "goal": "ship feature X",
            "non_goals": ["rewrite unrelated modules"],
            "done_when": ["tests green", "review checklist cleared"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["operation"], json!("start"));

        let st = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "status",
            "task_id": "my-task",
        }))
        .expect("status");
        assert!(st["goal_state"].is_object());
        assert_eq!(
            st["goal_state"][REQUIRES_COMPLETION_EVIDENCE_KEY],
            json!(true)
        );

        fs::write(
            repo.join("artifacts/current/my-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0}]}"#,
        )
        .expect("evidence");

        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "my-task",
        }))
        .expect("complete");
        assert!(!repo.join("artifacts/current/active_task.json").is_file());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_start_persists_lifecycle_profile() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-lifecycle-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");
        let rr = repo.display().to_string();
        let out = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "t-lite",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": false,
            "lifecycle_profile": "interactive",
        }))
        .expect("start");
        assert_eq!(out["ok"], json!(true));
        // Verify lifecycle_profile persisted via status read
        let st = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "status",
            "task_id": "t-lite",
        }))
        .expect("status");
        assert_eq!(st["goal_state"]["lifecycle_profile"], json!("interactive"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_start_rejects_incomplete_drive_contract() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-start-bad-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");
        let rr = repo.display().to_string();

        let missing_non_goals = framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "bad-start",
            "goal": "g",
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect_err("non_goals required");
        assert!(missing_non_goals.contains("non_goals"));

        let single_done_when = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "bad-start",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect_err("two done_when items required");
        assert!(single_done_when.contains("done_when"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_clear_removes_state_file() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-clear-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cl-task")).expect("mkdir");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cl-task",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        let path = goal_state_path_for_task(&repo, "cl-task").expect("goal path");
        assert!(path.is_file());
        let out = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "clear",
            "task_id": "cl-task",
        }))
        .expect("clear");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["removed"], json!(true));
        assert!(!path.is_file());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn resume_restores_drive_until_done_by_default_after_pause() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-resume-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rs-task")).expect("mkdir");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "rs-task",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "pause",
            "task_id": "rs-task",
        }))
        .expect("pause");
        let paused = read_goal_state(&repo, Some("rs-task"))
            .expect("read")
            .expect("some");
        assert_eq!(paused["drive_until_done"], json!(false));
        assert_eq!(paused[REQUIRES_COMPLETION_EVIDENCE_KEY], json!(true));
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "resume",
            "task_id": "rs-task",
        }))
        .expect("resume");
        let running = read_goal_state(&repo, Some("rs-task"))
            .expect("read2")
            .expect("some2");
        assert_eq!(running["status"], json!("running"));
        assert_eq!(
            running["drive_until_done"],
            json!(true),
            "explicit resume should restore drive continuation by default"
        );
        assert_eq!(running[REQUIRES_COMPLETION_EVIDENCE_KEY], json!(true));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn resume_can_leave_drive_until_done_disabled_when_requested() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-resume-off-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rs-off")).expect("mkdir");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "rs-off",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "pause",
            "task_id": "rs-off",
        }))
        .expect("pause");
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "resume",
            "task_id": "rs-off",
            "drive_until_done": false,
        }))
        .expect("resume");
        let running = read_goal_state(&repo, Some("rs-off"))
            .expect("read")
            .expect("some");
        assert_eq!(running["status"], json!("running"));
        assert_eq!(running["drive_until_done"], json!(false));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_evidence_summary_detects_success_row() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-evidence-sum-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/te")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"te"}"#,
        )
        .expect("active");
        fs::write(
            repo.join("artifacts/current/te/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test","exit_code":0}]}"#,
        )
        .expect("evidence");
        assert_eq!(
            task_evidence_artifacts_summary_for_task(&repo, "te"),
            (true, true)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_rejected_without_successful_evidence_for_drive_goal() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-complete-noev-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/noev")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"noev"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "noev",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        let err = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "noev",
        }))
        .expect_err("complete should require evidence");
        assert!(err.contains("EVIDENCE_INDEX"), "err={err}");
        let st = read_goal_state(&repo, Some("noev"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("running"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_after_pause_still_requires_successful_evidence() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-complete-paused-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/paused")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"paused"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "paused",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "pause",
            "task_id": "paused",
        }))
        .expect("pause");
        let err = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "paused",
        }))
        .expect_err("paused drive goal still requires evidence");
        assert!(err.contains("EVIDENCE_INDEX"), "err={err}");
        let st = read_goal_state(&repo, Some("paused"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("paused"));
        assert_eq!(st[REQUIRES_COMPLETION_EVIDENCE_KEY], json!(true));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn legacy_goal_complete_requires_evidence_when_validation_contract_exists() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-complete-legacy-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/legacy")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"legacy"}"#,
        )
        .expect("ptr");
        fs::write(
            repo.join("artifacts/current/legacy/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","goal":"legacy","status":"running","drive_until_done":false,"done_when":["d1"],"validation_commands":["cargo test -q"],"checkpoints":[]}"#,
        )
        .expect("legacy goal");
        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "legacy",
        }))
        .expect_err("legacy validation contract requires evidence");
        assert!(err.contains("EVIDENCE_INDEX"), "err={err}");
        let st = read_goal_state(&repo, Some("legacy"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("running"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn explicit_non_drive_goal_can_complete_without_evidence_when_allowed() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-complete-no-gate-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/nogate")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"nogate"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "nogate",
            "goal": "g",
            "drive_until_done": false,
            "requires_completion_evidence": false,
        }))
        .expect("start");
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "nogate",
        }))
        .expect("complete without evidence");
        // complete archives GOAL_STATE.json (does not delete)
        let goal_path = goal_state_path_for_task(&repo, "nogate").expect("goal path");
        assert!(
            goal_path.is_file(),
            "GOAL_STATE should persist (archived) after complete"
        );
        let archived = read_goal_state(&repo, Some("nogate"))
            .expect("read")
            .expect("state");
        assert_eq!(archived["archived"], json!(true));
        let _ = fs::remove_dir_all(&repo);
    }

    /// GOAL 与 RFV 同 task 互斥：goal_drive start 应将活跃 RFV 标为 superseded。
    #[test]
    fn goal_drive_start_supersedes_active_rfv_same_task() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-rfv-mutex-ag-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/mx-task")).expect("mkdir");
        let rr = repo.display().to_string();

        // Write RFV state file directly (quality_gate module is being deleted;
        // this test only needs a valid RFV state file on disk).
        let rfv_path = quality_gate_state_path(&repo, "mx-task").expect("rfv path");
        if let Some(parent) = rfv_path.parent() {
            fs::create_dir_all(parent).expect("mkdir rfv dir");
        }
        crate::utils::atomic_write::write_atomic_json(
            &rfv_path,
            &json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "goal": "rfv phase",
                "loop_status": "active",
                "max_rounds": 3u64,
            }),
        )
        .expect("write rfv state");

        let ag = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "mx-task",
            "goal": "goal_drive phase",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        assert_eq!(ag["quality_gate_superseded"], json!(true));

        let rfv_path = quality_gate_state_path(&repo, "mx-task").expect("rfv path");
        let raw = fs::read_to_string(&rfv_path).expect("read rfv");
        let v: Value = serde_json::from_str(&raw).expect("parse rfv");
        assert_eq!(v["loop_status"], json!("superseded"));
        assert_eq!(v["superseded_by"], json!("goal_drive"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_rejected_when_completion_gates_depth_not_met() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-gate-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/ggate")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"ggate"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "ggate",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
            "completion_gates": {
                "enabled": true,
                "min_depth_score": 2
            }
        }))
        .expect("start");
        fs::write(
            repo.join("artifacts/current/ggate/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"t","exit_code":0}]}"#,
        )
        .expect("evidence");
        let err = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "ggate",
        }))
        .expect_err("gate should reject");
        assert!(
            err.contains("completion_gates") && err.contains("depth_score"),
            "err={err}"
        );
        let st = read_goal_state(&repo, Some("ggate"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("running"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_allowed_when_completion_gates_satisfied() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-gate-ok-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/gok")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"gok"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "gok",
            "goal": "g",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
            "completion_gates": { "enabled": true, "min_depth_score": 1 }
        }))
        .expect("start");
        fs::write(
            repo.join("artifacts/current/gok/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"t","exit_code":0}]}"#,
        )
        .expect("evidence");
        fs::write(
            repo.join("artifacts/current/gok").join(QUALITY_GATE_STATE_FILENAME),
            r#"{"schema_version":"router-rs-rfv-loop-v1","loop_status":"active","goal":"g","max_rounds":3,"current_round":1,"rounds":[{"round":1,"verify_result":"PASS"}]}"#,
        )
        .expect("rfv");
        crate::task_state_aggregate::sync_task_state_aggregate(&repo, "gok").expect("sync agg");
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "gok",
        }))
        .expect("complete ok");
        // complete archives GOAL_STATE.json (does not delete)
        let goal_path = goal_state_path_for_task(&repo, "gok").expect("goal path");
        assert!(
            goal_path.is_file(),
            "GOAL_STATE should persist (archived) after complete"
        );
        let archived = read_goal_state(&repo, Some("gok"))
            .expect("read")
            .expect("state");
        assert_eq!(archived["archived"], json!(true));
        assert!(archived.get("completed_at").and_then(Value::as_str).is_some());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn read_rfv_loop_state_honors_override_and_active_pointer() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("core-state-rfv-read-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");

        let path = quality_gate_state_path(&repo, "rfv-task").expect("path");
        let state = json!({
            "schema_version": "router-rs-rfv-loop-v1",
            "loop_status": "active",
            "goal": "g",
        });
        crate::utils::atomic_write::write_atomic_json(&path, &state).expect("write rfv");

        let read = read_quality_gate_state(&repo, Some("rfv-task"))
            .expect("read")
            .expect("some");
        assert_eq!(read["loop_status"], json!("active"));

        let via_active = read_quality_gate_state(&repo, Some("rfv-task"))
            .expect("read active")
            .expect("some");
        assert_eq!(via_active["goal"], json!("g"));

        assert!(
            read_quality_gate_state(&repo, Some("missing-task"))
                .expect("read missing")
                .is_none()
        );

        let err = read_quality_gate_state(&repo, Some("   ")).expect_err("empty override");
        assert!(err.contains("empty"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn deactivate_goal_for_conflict_with_rfv_marks_superseded() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("core-state-goal-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/g-rfv")).expect("mkdir");
        let goal_path = repo.join("artifacts/current/g-rfv/GOAL_STATE.json");
        crate::utils::atomic_write::write_atomic_json(
            &goal_path,
            &json!({
                "schema_version": GOAL_STATE_SCHEMA_VERSION,
                "status": "running",
                "goal": "ship",
            }),
        )
        .expect("goal");

        let changed = deactivate_goal_for_conflict_with_quality_gate(&repo, "g-rfv").expect("deactivate");
        assert!(changed);
        let st = read_goal_state(&repo, Some("g-rfv"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("superseded"));
        assert_eq!(st["metadata"]["superseded_by"], json!("quality_gate"));

        let _ = fs::remove_dir_all(&repo);
    }

    /// session-scoped: start writes session_id, complete deletes GOAL_STATE.json
    #[test]
    fn goal_session_scoped_start_writes_session_id_and_complete_deletes() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-session-goal-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");
        let rr = repo.display().to_string();

        let out = framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "sess-task",
            "goal": "test session binding",
            "session_id": "test-session-abc",
            "non_goals": ["unrelated"],
            "done_when": ["done1", "done2"],
            "validation_commands": ["echo ok"],
            "drive_until_done": true,
        }))
        .expect("start");
        assert_eq!(out["ok"], json!(true));

        // Verify session_id is written
        let st = read_goal_state(&repo, Some("sess-task"))
            .expect("read")
            .expect("state");
        assert_eq!(st["session_id"], json!("test-session-abc"));

        // Write evidence for completion gate
        fs::write(
            repo.join("artifacts/current/sess-task/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"echo ok","exit_code":0}]}"#,
        )
        .expect("evidence");

        // Complete and verify GOAL_STATE.json is archived (not deleted)
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "complete",
            "task_id": "sess-task",
        }))
        .expect("complete");
        let goal_path = goal_state_path_for_task(&repo, "sess-task").expect("goal path");
        assert!(
            goal_path.is_file(),
            "GOAL_STATE.json should persist (archived) after complete"
        );
        // Verify archive markers
        let archived = read_goal_state(&repo, Some("sess-task"))
            .expect("read")
            .expect("state");
        assert_eq!(archived["archived"], json!(true));
        assert!(archived.get("completed_at").and_then(Value::as_str).is_some());

        let _ = fs::remove_dir_all(&repo);
    }

    /// session-scoped: stale detection when session_id mismatches
    #[test]
    fn goal_read_annotates_stale_when_session_id_mismatches() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-stale-goal-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/stale-task")).expect("mkdir");

        // Write a goal with a different session_id than the current env
        let goal_path = goal_state_path_for_task(&repo, "stale-task").expect("path");
        let goal_json = json!({
            "schema_version": GOAL_STATE_SCHEMA_VERSION,
            "status": "running",
            "goal": "old session goal",
            "session_id": "old-session-xyz",
            "drive_until_done": true,
            "non_goals": [],
            "done_when": [],
            "validation_commands": [],
            "checkpoints": [],
            "blocker": null,
            "updated_at": framework_kernel::time::now_iso(),
        });
        crate::utils::atomic_write::write_atomic_json(&goal_path, &goal_json).expect("write goal");

        // Set a different current session via env var
        unsafe { std::env::set_var("CLAUDE_SESSION_ID", "new-session-456") };

        let st = read_goal_state(&repo, Some("stale-task"))
            .expect("read")
            .expect("state");
        assert_eq!(st["stale"], json!(true));
        assert!(
            st["stale_reason"]
                .as_str()
                .unwrap()
                .contains("session_id mismatch")
        );

        // Clean up env var
        unsafe { std::env::remove_var("CLAUDE_SESSION_ID") };
        let _ = fs::remove_dir_all(&repo);
    }

    /// session-scoped: same session_id is NOT stale
    #[test]
    fn goal_read_not_stale_when_session_id_matches() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Clear all *_SESSION_ID env vars that could trigger staleness
        let session_vars: Vec<String> = std::env::vars()
            .filter(|(k, _)| k.ends_with("_SESSION_ID"))
            .map(|(k, _)| k)
            .collect();
        for k in &session_vars {
            unsafe { std::env::remove_var(k) };
        }
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-not-stale-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/match-task")).expect("mkdir");

        // Pass session_id in payload to avoid env var race with parallel tests.
        let rr = repo.display().to_string();
        framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "match-task",
            "goal": "current session goal",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
            "drive_until_done": true,
            "session_id": "my-session-789",
        }))
        .expect("start");

        let st = read_goal_state(&repo, Some("match-task"))
            .expect("read")
            .expect("state");
        assert_eq!(st["session_id"], json!("my-session-789"));
        // Should NOT be stale since session matches
        assert!(st.get("stale").is_none());

        let _ = fs::remove_dir_all(&repo);
    }

    /// session-scoped: legacy goals without session_id are NOT stale (backward compat)
    #[test]
    fn goal_read_legacy_without_session_id_not_stale() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-legacy-goal-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/legacy-task")).expect("mkdir");

        let goal_path = goal_state_path_for_task(&repo, "legacy-task").expect("path");
        // Write a legacy goal without session_id
        let goal_json = json!({
            "schema_version": GOAL_STATE_SCHEMA_VERSION,
            "status": "running",
            "goal": "legacy goal",
            "drive_until_done": true,
            "non_goals": [],
            "done_when": [],
            "validation_commands": [],
            "checkpoints": [],
            "blocker": null,
            "updated_at": framework_kernel::time::now_iso(),
        });
        crate::utils::atomic_write::write_atomic_json(&goal_path, &goal_json).expect("write goal");

        // Legacy goal without session_id should NOT be marked stale regardless of current env.
        // No need to set CLAUDE_SESSION_ID — annotate_goal_staleness returns early
        // when goal has no session_id field.
        let st = read_goal_state(&repo, Some("legacy-task"))
            .expect("read")
            .expect("state");
        assert!(st.get("stale").is_none());

        let _ = fs::remove_dir_all(&repo);
    }

    /// stale goals do NOT request continuation
    #[test]
    fn stale_goal_does_not_request_continuation() {
        let mut goal = json!({
            "drive_until_done": true,
            "status": "running",
            "stale": true,
            "stale_reason": "session_id mismatch",
        });
        assert!(
            !goal_state_requests_continuation(&goal),
            "stale goal should not request continuation"
        );
        // Without stale flag, should request continuation
        goal.as_object_mut().unwrap().remove("stale");
        goal.as_object_mut().unwrap().remove("stale_reason");
        assert!(goal_state_requests_continuation(&goal));
    }

    #[test]
    fn amend_updates_fields_and_preserves_checkpoints() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-amend-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/amend-task")).expect("mkdir");
        let rr = repo.display().to_string();

        // Start a goal
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "amend-task",
            "goal": "original goal",
            "non_goals": ["original non-goal"],
            "done_when": ["original done1", "original done2"],
            "validation_commands": ["cargo check"],
            "drive_until_done": false,
        }))
        .expect("start");

        // Add a checkpoint
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "checkpoint",
            "task_id": "amend-task",
            "note": "first milestone",
        }))
        .expect("checkpoint");

        // Amend: update goal and done_when, keep progress
        let amend_result = framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "amend",
            "task_id": "amend-task",
            "goal": "updated goal",
            "done_when": ["new done1", "new done2", "new done3"],
        }))
        .expect("amend");
        assert_eq!(amend_result["ok"], json!(true));

        // Read back and verify
        let st = read_goal_state(&repo, Some("amend-task"))
            .expect("read")
            .expect("state");
        assert_eq!(st["goal"], json!("updated goal"));
        assert_eq!(st["non_goals"][0], json!("original non-goal"), "non_goals should be preserved");
        assert_eq!(st["done_when"].as_array().map(|a| a.len()), Some(3), "done_when should be replaced");
        // Checkpoints should be preserved
        assert_eq!(st["checkpoints"].as_array().map(|a| a.len()), Some(1), "checkpoints preserved");
        assert_eq!(st["checkpoints"][0]["note"], json!("first milestone"));
        // Status should remain running
        assert_eq!(st["status"], json!("running"));
        // Amend marker should be set
        assert!(st.get("amended_at").and_then(Value::as_str).is_some());

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn amend_rejected_for_completed_goal() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-amend-fail-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/amend-done")).expect("mkdir");
        let rr = repo.display().to_string();

        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "amend-done",
            "goal": "finish fast",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
            "drive_until_done": false,
        }))
        .expect("start");

        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "complete",
            "task_id": "amend-done",
        }))
        .expect("complete");

        // Amend should fail on completed goal
        let err = framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "amend",
            "task_id": "amend-done",
            "goal": "new goal",
        }))
        .expect_err("amend should fail on completed");
        assert!(err.contains("cannot amend a completed"), "err: {err}");

        let _ = fs::remove_dir_all(&repo);
    }

    /// Full lifecycle: start → checkpoint → amend (keep_progress default) →
    /// checkpoint → complete → verify archived: true, GOAL_STATE.json still on disk.
    #[test]
    fn goal_full_lifecycle_start_checkpoint_amend_complete_archived() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-lifecycle-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/lifecycle-task")).expect("mkdir");
        let rr = repo.display().to_string();

        // 1. Start a goal with drive_until_done: false
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "lifecycle-task",
            "goal": "original goal",
            "non_goals": ["unrelated"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
            "drive_until_done": false,
        }))
        .expect("start");

        // 2. checkpoint("milestone 1")
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "checkpoint",
            "task_id": "lifecycle-task",
            "note": "milestone 1",
        }))
        .expect("checkpoint 1");

        // 3. amend: update goal + done_when, keep_progress defaults to true
        let amend_result = framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "amend",
            "task_id": "lifecycle-task",
            "goal": "updated goal",
            "done_when": ["new d1", "new d2", "new d3"],
        }))
        .expect("amend");
        assert_eq!(amend_result["ok"], json!(true));

        // 4. checkpoint("milestone 2") after amend
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "checkpoint",
            "task_id": "lifecycle-task",
            "note": "milestone 2",
        }))
        .expect("checkpoint 2");

        // 5. complete
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "complete",
            "task_id": "lifecycle-task",
        }))
        .expect("complete");

        // 6. Verify
        let goal_path = goal_state_path_for_task(&repo, "lifecycle-task").expect("goal path");
        assert!(goal_path.is_file(), "GOAL_STATE.json must still exist after complete");

        let state = read_goal_state(&repo, Some("lifecycle-task"))
            .expect("read goal state")
            .expect("state exists");
        assert_eq!(state["archived"], json!(true), "archived must be true");
        assert!(
            state.get("completed_at").and_then(Value::as_str).is_some(),
            "completed_at must be present"
        );
        assert_eq!(state["status"], json!("completed"));
        assert_eq!(state["goal"], json!("updated goal"), "amend goal must persist");
        assert_eq!(
            state["checkpoints"].as_array().map(|a| a.len()),
            Some(2),
            "two checkpoints expected"
        );
        assert_eq!(state["drive_until_done"], json!(false));

        let _ = fs::remove_dir_all(&repo);
    }

    /// complete does not delete GOAL_STATE.json — it is archived in place.
    #[test]
    fn complete_preserves_goal_state_file() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-complete-preserves-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cp-task")).expect("mkdir");
        let rr = repo.display().to_string();

        // Start with requires_completion_evidence: false so we can complete without evidence
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cp-task",
            "goal": "preserve file test",
            "drive_until_done": false,
            "requires_completion_evidence": false,
        }))
        .expect("start");

        let goal_path = goal_state_path_for_task(&repo, "cp-task").expect("path");
        assert!(goal_path.is_file(), "GOAL_STATE.json must exist before complete");

        // Capture raw content hash (length as proxy) before complete
        let raw_before = fs::read_to_string(&goal_path).expect("read before");

        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "complete",
            "task_id": "cp-task",
        }))
        .expect("complete");

        // File must still physically exist
        assert!(
            goal_path.is_file(),
            "GOAL_STATE.json must be physically present after complete"
        );

        // Verify archived: true and structural fields
        let state = read_goal_state(&repo, Some("cp-task"))
            .expect("read after complete")
            .expect("state exists");
        assert_eq!(state["archived"], json!(true));
        assert!(state.get("completed_at").and_then(Value::as_str).is_some());
        assert_eq!(state["status"], json!("completed"));

        // File content changed (archived status injected)
        let raw_after = fs::read_to_string(&goal_path).expect("read after");
        assert_ne!(raw_before, raw_after, "file content must change after archiving");

        let _ = fs::remove_dir_all(&repo);
    }

    /// amend with keep_progress: false clears existing checkpoints.
    #[test]
    fn amend_clears_checkpoints_when_keep_progress_false() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-amend-clear-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/ac-task")).expect("mkdir");
        let rr = repo.display().to_string();

        // Start
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "ac-task",
            "goal": "clear test",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
            "drive_until_done": false,
        }))
        .expect("start");

        // Add two checkpoints
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "checkpoint",
            "task_id": "ac-task",
            "note": "cp 1",
        }))
        .expect("cp1");
        framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "checkpoint",
            "task_id": "ac-task",
            "note": "cp 2",
        }))
        .expect("cp2");

        // Verify checkpoints exist before amend
        let before = read_goal_state(&repo, Some("ac-task"))
            .expect("read before")
            .expect("state");
        assert_eq!(
            before["checkpoints"].as_array().map(|a| a.len()),
            Some(2),
            "should have 2 checkpoints before amend"
        );

        // Amend with keep_progress: false
        let amend_result = framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "amend",
            "task_id": "ac-task",
            "goal": "cleared goal",
            "keep_progress": false,
        }))
        .expect("amend");
        assert_eq!(amend_result["ok"], json!(true));

        // Verify checkpoints cleared
        let after = read_goal_state(&repo, Some("ac-task"))
            .expect("read after")
            .expect("state");
        assert_eq!(
            after["checkpoints"].as_array().map(|a| a.len()),
            Some(0),
            "checkpoints must be cleared when keep_progress is false"
        );
        assert_eq!(after["goal"], json!("cleared goal"));

        let _ = fs::remove_dir_all(&repo);
    }
}
