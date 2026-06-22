//! Unified **read model** for per-task continuity artifacts (L2) consumed by L3.
//!
//! Design: see `core/core-state/src/task_state.rs` (this file) and
//! `core/core-state/src/task_state_aggregate.rs` for the three-phase resolve model.
//! **Writes** serialize via `task_write_lock`
//! (phase 2); this module only aggregates read models (`ResolvedTaskView`, `ContinuityFrame`).

use crate::state_manager::{
    goal_state_requests_continuation, read_goal_state, task_evidence_artifacts_summary_for_task,
};
use crate::state_manager::{
    read_quality_gate_state, validate_external_research_strict, validate_external_research_structured,
};

use serde::Serialize;
#[cfg(not(test))]
use std::sync::OnceLock;
use serde_json::Value;
use std::path::Path;

// Cached env var lookups — avoid repeated OS syscalls in hot paths.
// Disabled in test builds so tests can set env vars per-test.
fn depth_score_mode_is_strict() -> bool {
    #[cfg(not(test))]
    {
        static CACHED: OnceLock<bool> = OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var("ROUTER_RS_DEPTH_SCORE_MODE")
                .map(|v| v.trim() == "strict")
                .unwrap_or(false)
        })
    }
    #[cfg(test)]
    {
        std::env::var("ROUTER_RS_DEPTH_SCORE_MODE")
            .map(|v| v.trim() == "strict")
            .unwrap_or(false)
    }
}

fn depth_compliance_hint_enabled() -> bool {
    #[cfg(not(test))]
    {
        static CACHED: OnceLock<bool> = OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT")
                .map(|v| v.trim() == "1")
                .unwrap_or(false)
        })
    }
    #[cfg(test)]
    {
        std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT")
            .map(|v| v.trim() == "1")
            .unwrap_or(false)
    }
}

pub const RESOLVED_TASK_VIEW_SCHEMA_VERSION: &str = "router-rs-resolved-task-view-v1";

/// When present in [`ResolvedTaskView::resolution_notes`], `active_task.json` resolves to a task
/// with no readable `GOAL_STATE.json`, while `focus_task.json` names another task that **does**
/// have a valid goal file. [`crate::state_manager::read_goal_state_for_hydration`] still does not
/// fall back to focus; callers may surface this for diagnostics only.
pub const RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL: &str =
    "continuity:active_goal_missing_focus_has_goal";

/// Active resolves with a readable GOAL that does not request continuation while `focus_task` does.
pub const RESOLUTION_NOTE_ACTIVE_GOAL_NOT_DRIVING_FOCUS_DRIVES: &str =
    "continuity:active_goal_not_driving_focus_drives";

// RESOLUTION_NOTE_POINTER_PROMOTED_FOCUS_TO_ACTIVE: removed — no callers, superseded by pointer auto-promotion logic.

pub const RESOLUTION_NOTE_DUAL_GOAL_POINTER_CONFLICT: &str =
    "continuity:dual_goal_pointer_conflict";

/// Suffix for [`depth_compliance_refresh_hint`] when `ROUTER_RS_DEPTH_SCORE_MODE` is not `strict`
/// yet structured external-research rounds are present advisory counters only for the third depth point.
pub const DEPTH_COMPLIANCE_LEGACY_EXTERNAL_DEPTH_NOTE_ZH: &str = " · legacy第三分不含「仅外研结构化轮次」；d3需GOAL checkpoint或RFV对抗轮，或设ROUTER_RS_DEPTH_SCORE_MODE=strict";

/// Zh line appended to Codex continuity digest and Cursor SessionStart when
/// [`task_view_has_active_goal_focus_mismatch_note`] is true (same bytes as legacy digest string).
pub const CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH: &str = concat!(
    "连续性提示: active 无 GOAL 而 focus 有 GOAL；下一帧 continuity 将尝试把 focus 提升为 active（见 ADR-001）。",
    "若仍未修复请核对 `artifacts/current/active_task.json`。",
);

// CONTINUITY_ACTIVE_NOT_DRIVING_FOCUS_DRIVES_HINT_ZH: removed — no callers, hint text retained for reference only in comments.

/// Pushes [`RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL`] when appropriate. Active may still
/// have RFV or other task-scoped state with no readable GOAL; the note is about pointers vs
/// hydration, not "goal_drive-only".
fn maybe_note_active_goal_missing_focus_has_goal(
    repo_root: &Path,
    pointers: &TaskPointers,
    resolved_task_id: &str,
    goal_state: Option<&Value>,
    notes: &mut Vec<String>,
) {
    let Some(active_id) = pointers.active_task_id.as_deref() else {
        return;
    };
    if resolved_task_id != active_id {
        return;
    }
    if goal_state.is_some() {
        return;
    }
    let Some(focus_id) = pointers.focus_task_id.as_deref() else {
        return;
    };
    if focus_id == active_id {
        return;
    }
    let Ok(Some(_)) = read_goal_state(repo_root, Some(focus_id)) else {
        return;
    };
    notes.push(format!(
        "{RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL} active={active_id} focus={focus_id}"
    ));
}

fn maybe_note_active_goal_not_driving_focus_drives(
    repo_root: &Path,
    pointers: &TaskPointers,
    resolved_task_id: &str,
    goal_state: Option<&Value>,
    notes: &mut Vec<String>,
) {
    let Some(active_id) = pointers.active_task_id.as_deref() else {
        return;
    };
    let Some(focus_id) = pointers.focus_task_id.as_deref() else {
        return;
    };
    if active_id == focus_id || resolved_task_id != active_id {
        return;
    }
    let Some(active_goal) = goal_state else {
        return;
    };
    if crate::state_manager::goal_state_requests_continuation(active_goal) {
        return;
    }
    let Ok(Some(focus_goal)) = crate::state_manager::read_goal_state(repo_root, Some(focus_id))
    else {
        return;
    };
    if !crate::state_manager::goal_state_requests_continuation(&focus_goal) {
        return;
    }
    notes.push(format!(
        "{RESOLUTION_NOTE_ACTIVE_GOAL_NOT_DRIVING_FOCUS_DRIVES} active={active_id} focus={focus_id}"
    ));
}

fn maybe_note_dual_goal_pointer_conflict(
    repo_root: &Path,
    pointers: &TaskPointers,
    notes: &mut Vec<String>,
) {
    let Some(active_id) = pointers.active_task_id.as_deref().filter(|s| !s.is_empty()) else {
        return;
    };
    let Some(focus_id) = pointers.focus_task_id.as_deref().filter(|s| !s.is_empty()) else {
        return;
    };
    if active_id == focus_id {
        return;
    }
    let active_has = read_goal_state(repo_root, Some(active_id))
        .ok()
        .flatten()
        .is_some();
    let focus_has = read_goal_state(repo_root, Some(focus_id))
        .ok()
        .flatten()
        .is_some();
    if active_has && focus_has {
        notes.push(format!(
            "{RESOLUTION_NOTE_DUAL_GOAL_POINTER_CONFLICT} active={active_id} focus={focus_id}"
        ));
    }
}

/// When active has no readable GOAL but focus does, rewrite `active_task.json` to focus (ADR-001).
pub fn maybe_promote_focus_to_active_pointer(repo_root: &Path) -> bool {
    let pointers = read_task_pointers(repo_root);
    let Some(active_id) = pointers.active_task_id.as_deref().filter(|s| !s.is_empty()) else {
        return false;
    };
    if read_goal_state(repo_root, Some(active_id))
        .ok()
        .flatten()
        .is_some()
    {
        return false;
    }
    let Some(focus_id) = pointers.focus_task_id.as_deref().filter(|s| !s.is_empty()) else {
        return false;
    };
    if active_id == focus_id {
        return false;
    }
    if read_goal_state(repo_root, Some(focus_id))
        .ok()
        .flatten()
        .is_none()
    {
        return false;
    }
    let path = repo_root.join("artifacts/current/active_task.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = serde_json::json!({"task_id": focus_id});
    if crate::utils::atomic_write::write_atomic_json(&path, &body).is_err() {
        return false;
    }
    tracing::warn!(focus_id = %focus_id, active_id = %active_id, "promoted focus_task to active_task (active had no GOAL)");
    true
}

/// Single disk snapshot for Cursor **beforeSubmit** / **stop** continuity + gate hydrate.
///
/// - `pointer_view`：`resolve_task_view_with_pointers`（override 无 → active → focus），供续跑合并时与 `active_task` 对齐缓存。
/// - `hydration_goal`：与 `read_goal_state_for_hydration_from_pointer_ids` 一致（active → focus；不扫 orphan），供 `AG_FOLLOWUP` hydrate；与 `pointer_view` 共用同一 [`read_task_pointers`] 快照。
#[derive(Debug, Clone)]
pub struct ContinuityFrame {
    pub pointer_view: ResolvedTaskView,
    pub hydration_goal: Option<(serde_json::Value, String)>,
}

/// beforeSubmit / Stop 入口：一次构建指针视图 + hydration 目标对。
/// 会话级作用域：默认仅 active/focus 指针 hydration；不扫 orphan。
/// 遗留 diagnostics scan 仅当 `ROUTER_RS_GOAL_DIAGNOSTICS_SCAN_HYDRATE=1` 显式 opt-in。
pub fn resolve_continuity_frame(repo_root: &Path) -> ContinuityFrame {
    let pointers = read_task_pointers(repo_root);
    let pointer_view = resolve_task_view_with_pointers(repo_root, None, pointers.clone());
    let hydration_goal = crate::state_manager::read_goal_state_for_hydration_from_pointer_ids(
        repo_root,
        &pointers.active_task_id,
        &pointers.focus_task_id,
    )
    .ok()
    .flatten()
    .or_else(|| goal_hydration_diagnostics_scan_fallback(repo_root));
    ContinuityFrame {
        pointer_view,
        hydration_goal,
    }
}

fn goal_hydration_diagnostics_scan_fallback(repo_root: &Path) -> Option<(Value, String)> {
    if !goal_diagnostics_scan_hydrate_enabled() {
        return None;
    }
    crate::state_manager::read_goal_state_for_diagnostics_scan(repo_root)
        .ok()
        .flatten()
}

fn goal_diagnostics_scan_hydrate_enabled() -> bool {
    #[cfg(not(test))]
    {
        static CACHED: OnceLock<bool> = OnceLock::new();
        *CACHED.get_or_init(|| {
            match std::env::var("ROUTER_RS_GOAL_DIAGNOSTICS_SCAN_HYDRATE") {
                Ok(v) => {
                    let t = v.trim().to_lowercase();
                    matches!(t.as_str(), "1" | "true" | "yes" | "on")
                }
                Err(_) => false,
            }
        })
    }
    #[cfg(test)]
    {
        match std::env::var("ROUTER_RS_GOAL_DIAGNOSTICS_SCAN_HYDRATE") {
            Ok(v) => {
                let t = v.trim().to_lowercase();
                matches!(t.as_str(), "1" | "true" | "yes" | "on")
            }
            Err(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TaskPointers {
    pub active_task_id: Option<String>,
    pub focus_task_id: Option<String>,
}

/// `active_task.json` / `focus_task.json` 一次成对读取（比两次独立 open 的半态窗口更小）。
pub fn read_task_pointers(repo_root: &Path) -> TaskPointers {
    TaskPointers {
        active_task_id: crate::state_manager::read_active_task_id(repo_root),
        focus_task_id: crate::state_manager::read_focus_task_id(repo_root),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EvidenceRollup {
    pub task_id: String,
    pub evidence_rows_non_empty: bool,
    pub has_successful_verification: bool,
}

/// Aggregate "depth compliance" view (P1-A): cross-cuts RFV rounds, EVIDENCE_INDEX, and
/// goal checkpoints into a single read-only score. Consumers (closeout enforcement,
/// SessionStart digest, statusline) can inspect `depth_score` instead of re-deriving the
/// same booleans separately.
///
/// `depth_score` ∈ {0, 1, 2, 3}:
/// - 1 point: at least one RFV round with `verify_result=PASS`.
/// - 1 point: at least one successful EVIDENCE_INDEX row (`success==true` or `exit_code==0`).
/// - 1 point: at least one goal checkpoint recorded (model wrote progress at least once).
///
/// Notes:
/// - Rollup counters and `depth_score` are **advisory** for hooks/digest unless a ledger enables
///   **hard gates** (`GOAL_STATE.completion_gates` on `complete`, `RFV_LOOP_STATE.close_gates` on
///   RFV **显式 close** 与 **`max_rounds` 耗尽** 自动 closed 的 `append_round` 收口预览) — those paths read the same aggregate via [`resolve_task_view`].
/// - `qg_unknown_round_count` and `qg_pass_without_evidence_count` are explicitly broken out
///   so dashboards can flag "RFV says PASS but EVIDENCE shows no successful row in the same
///   window" — the cross-check label written by `rfv_loop::cross_link_evidence`.
/// - `qg_external_strict_ok_round_count` counts rounds whose `external_research` object passes
///   `validate_external_research_strict` while the RFV state has `external_research_strict=true`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct DepthCompliance {
    pub qg_pass_round_count: u64,
    pub qg_fail_round_count: u64,
    pub qg_skipped_round_count: u64,
    pub qg_unknown_round_count: u64,
    pub qg_pass_without_evidence_count: u64,
    pub qg_adversarial_round_count: u64,
    pub qg_falsification_test_count: u64,
    /// Quality Gate rounds with non-null **`external_research`** object (`append_round` 结构化块).
    pub qg_external_deep_structured_round_count: u64,
    /// Quality Gate rounds where `external_research` was an object, task **`external_research_strict`** was
    /// true at rollup time, and the blob passes [`validate_external_research_strict`].
    pub qg_external_strict_ok_round_count: u64,
    pub goal_checkpoint_count: u64,
    pub depth_score: u8,
}

/// Roll up RFV rounds + optional GOAL checkpoints + evidence_ok into [`DepthCompliance`].
///
/// Used by [`resolve_task_view`] and by RFV **`close_gates`** pre-write preview（显式 close 与
/// `max_rounds` 轮次上限收口）so rollup stays single-sourced. `rfv` is typically `RFV_LOOP_STATE` root; `goal` is optional `GOAL_STATE`.
pub fn depth_compliance_aggregate(
    goal: Option<&Value>,
    qg: Option<&Value>,
    evidence_ok: bool,
) -> DepthCompliance {
    let mut c = DepthCompliance::default();

    if let Some(g) = goal
        && let Some(arr) = g.get("checkpoints").and_then(Value::as_array) {
            c.goal_checkpoint_count = arr.len() as u64;
        }

    let mut strict_task = false;
    if let Some(r) = qg {
        strict_task = r
            .get("external_research_strict")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(rounds) = r.get("rounds").and_then(Value::as_array) {
            for round in rounds {
                if round
                    .get("adversarial_findings")
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty())
                {
                    c.qg_adversarial_round_count += 1;
                }
                if let Some(arr) = round.get("falsification_tests").and_then(Value::as_array) {
                    c.qg_falsification_test_count += arr.len() as u64;
                }
                let vr = round
                    .get("verify_result")
                    .and_then(Value::as_str)
                    .unwrap_or("UNKNOWN")
                    .to_ascii_uppercase();
                match vr.as_str() {
                    "PASS" => c.qg_pass_round_count += 1,
                    "FAIL" => c.qg_fail_round_count += 1,
                    "SKIPPED" => c.qg_skipped_round_count += 1,
                    _ => c.qg_unknown_round_count += 1,
                }
                if vr == "PASS"
                    && round
                        .get("cross_check")
                        .and_then(Value::as_str)
                        .map(|s| s == "no_evidence_window")
                        .unwrap_or(false)
                {
                    c.qg_pass_without_evidence_count += 1;
                }
                if round
                    .get("external_research")
                    .is_some_and(|v| !v.is_null() && v.is_object())
                {
                    c.qg_external_deep_structured_round_count += 1;
                    if strict_task
                        && let Some(er) = round.get("external_research")
                            && validate_external_research_structured(er).is_ok()
                                && validate_external_research_strict(er).is_ok()
                            {
                                c.qg_external_strict_ok_round_count += 1;
                            }
                }
            }
        }
    }

    let mut score: u8 = 0;
    if c.qg_pass_round_count > 0 {
        score += 1;
    }
    if evidence_ok {
        score += 1;
    }
    // Third point: checkpoints OR adversarial round OR strict external research. Legacy mode now counts
    // structured external research (strict_task + strict-pass rounds) to ensure pure external research
    // tasks can achieve full score. strict mode additionally counts falsification tests.
    let third_legacy = c.goal_checkpoint_count > 0
        || c.qg_adversarial_round_count > 0
        || (strict_task && c.qg_external_strict_ok_round_count > 0);
    let third = if depth_score_mode_is_strict() {
        third_legacy || c.qg_falsification_test_count > 0
    } else {
        third_legacy
    };
    if third {
        score += 1;
    }
    c.depth_score = score;
    c
}

/// High-level macro-controller mode for the resolved task id.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskControlMode {
    Idle,
    GoalDrive,
    QualityGate,    Conflict { reason: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResolvedTaskView {
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub pointers: TaskPointers,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rfv_loop_state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceRollup>,
    /// Aggregate depth-compliance view (P1-A); always present alongside `evidence` for tasks
    /// with a resolved id. `None` when no task id resolves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_compliance: Option<DepthCompliance>,
    pub control_mode: TaskControlMode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolution_notes: Vec<String>,
}

/// True when [`ResolvedTaskView::resolution_notes`] carries the active/focus GOAL observability short code.
pub fn task_view_has_active_goal_focus_mismatch_note(view: &ResolvedTaskView) -> bool {
    view.resolution_notes
        .iter()
        .any(|n| n.starts_with(RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL))
}

fn qg_active(state: &Value) -> bool {
    state
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"))
}

fn classify_control_mode(
    goal: Option<&Value>,
    qg: Option<&Value>,
    notes: &mut Vec<String>,
) -> TaskControlMode {
    let g_on = goal.is_some_and(goal_state_requests_continuation);
    let r_on = qg.is_some_and(qg_active);
    match (g_on, r_on) {
        (true, true) => {
            notes.push(
                "goal macro (drive_until_done+running) and RFV loop_status=active both true; invariant violation expected to be prevented by writers"
                    .to_string(),
            );
            TaskControlMode::Conflict {
                reason: "goal_drive_and_rfv_loop_both_active".to_string(),
            }
        }
        (true, false) => TaskControlMode::GoalDrive,
        (false, true) => TaskControlMode::QualityGate,
        (false, false) => TaskControlMode::Idle,
    }
}

fn push_resolution_read_err(notes: &mut Vec<String>, prefix: &'static str, err: String) {
    let mut line = format!("{prefix}: {err}");
    const MAX: usize = 220;
    if line.len() > MAX {
        let mut idx = MAX.min(line.len());
        while idx > 0 && !line.is_char_boundary(idx) {
            idx -= 1;
        }
        line.truncate(idx);
        line.push_str("...");
    }
    notes.push(line);
}

/// Resolve a single view for continuity debugging and future hook consumption.
///
/// `task_id` resolution: `task_id_override` (non-empty) > `active_task.json` > `focus_task.json`.
/// Does **not** scan `**/GOAL_STATE.json` by mtime (see design doc).
pub fn resolve_task_view(repo_root: &Path, task_id_override: Option<&str>) -> ResolvedTaskView {
    let pointers = read_task_pointers(repo_root);
    resolve_task_view_with_pointers(repo_root, task_id_override, pointers)
}

pub fn read_task_ledger_transactions(
    repo_root: &Path,
    task_id: &str,
) -> Vec<crate::task_ledger::LedgerTransaction> {
    let path = match crate::task_ledger::task_ledger_path(repo_root, task_id) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    if !path.is_file() {
        return Vec::new();
    }
    // Repair any trailing corrupt lines before reading (e.g. truncated crash writes).
    if let Err(e) = crate::utils::jsonl_maintenance::truncate_corrupt_tail(&path) {
        tracing::warn!(task_id = %task_id, error = %e, "truncate_corrupt_tail failed for TASK_LEDGER.jsonl");
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "failed to read TASK_LEDGER.jsonl");
            return Vec::new();
        }
    };
    let mut txs = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line_trim = line.trim();
        if line_trim.is_empty() {
            continue;
        }
        match serde_json::from_str::<crate::task_ledger::LedgerTransaction>(line_trim) {
            Ok(tx) => txs.push(tx),
            Err(e) => {
                tracing::warn!(
                    line = i + 1,
                    task_id = %task_id,
                    error = %e,
                    "skipping corrupt line in TASK_LEDGER.jsonl"
                );
            }
        }
    }
    txs
}

pub fn hydrate_task_state_hybrid(
    repo_root: &Path,
    task_id: &str,
) -> (
    Option<Value>,
    Option<Value>,
    Option<EvidenceRollup>,
    Vec<String>,
) {
    let mut resolution_notes = Vec::new();
    let mut goal_state: Option<Value> = None;
    let mut rfv_loop_state: Option<Value> = None;
    let mut evidence_rows_non_empty = false;
    let mut has_successful_verification = false;
    let mut base_loaded = false;
    let mut last_seq: Option<u64> = None;

    // 1. Try to load from TASK_STATE.json
    let agg_path = crate::task_state_aggregate::task_state_aggregate_path(repo_root, task_id);
    if agg_path.is_file()
        && let Ok(raw) = std::fs::read_to_string(&agg_path)
            && let Ok(agg) = serde_json::from_str::<Value>(&raw) {
                goal_state = agg.get("goal_state").cloned().filter(|v| !v.is_null());
                rfv_loop_state = agg.get("rfv_loop_state").cloned().filter(|v| !v.is_null());
                if let Some(ev) = agg.get("evidence") {
                    evidence_rows_non_empty = ev
                        .get("evidence_rows_non_empty")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    has_successful_verification = ev
                        .get("has_successful_verification")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                }
                last_seq = agg.get("last_seq").and_then(Value::as_u64);
                base_loaded = true;
            }

    // 2. If TASK_STATE.json not found or failed, fall back to raw physical files
    if !base_loaded {
        match read_goal_state(repo_root, Some(task_id)) {
            Ok(g) => goal_state = g,
            Err(e) => push_resolution_read_err(&mut resolution_notes, "goal_state_read_failed", e),
        }
        match read_quality_gate_state(repo_root, Some(task_id)) {
            Ok(v) => rfv_loop_state = v,
            Err(e) => {
                push_resolution_read_err(&mut resolution_notes, "rfv_loop_state_read_failed", e)
            }
        }
        let (rows, ok) = task_evidence_artifacts_summary_for_task(repo_root, task_id);
        evidence_rows_non_empty = rows;
        has_successful_verification = ok;
    }

    // 3. Read TASK_LEDGER.jsonl and replay transactions beyond last_seq
    let txs = read_task_ledger_transactions(repo_root, task_id);
    for tx in txs {
        if let Some(seq) = tx.seq
            && let Some(l_seq) = last_seq
                && seq <= l_seq {
                    continue;
                }
        // Replay
        match tx.tx_type.as_str() {
            "goal_state" => {
                goal_state = Some(tx.payload).filter(|v| !v.is_null());
            }
            "rfv_loop_state" => {
                rfv_loop_state = Some(tx.payload).filter(|v| !v.is_null());
            }
            "evidence" => {
                evidence_rows_non_empty = true;
                if crate::state_manager::evidence_index_entry_implies_success(&tx.payload) {
                    has_successful_verification = true;
                }
            }
            _ => {}
        }
    }

    let evidence = Some(EvidenceRollup {
        task_id: task_id.to_string(),
        evidence_rows_non_empty,
        has_successful_verification,
    });

    (goal_state, rfv_loop_state, evidence, resolution_notes)
}

/// Like [`resolve_task_view`], but uses a caller-supplied [`TaskPointers`] snapshot (e.g. paired
/// with [`read_goal_state_for_hydration_from_pointer_ids`] in [`resolve_continuity_frame`]).
pub fn resolve_task_view_with_pointers(
    repo_root: &Path,
    task_id_override: Option<&str>,
    pointers: TaskPointers,
) -> ResolvedTaskView {
    let tid = task_id_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| pointers.active_task_id.clone())
        .or_else(|| pointers.focus_task_id.clone());

    let mut resolution_notes: Vec<String> = Vec::new();

    let Some(ref task_id) = tid else {
        return ResolvedTaskView {
            schema_version: RESOLVED_TASK_VIEW_SCHEMA_VERSION.to_string(),
            task_id: None,
            pointers,
            goal_state: None,
            rfv_loop_state: None,
            evidence: None,
            depth_compliance: None,
            control_mode: TaskControlMode::Idle,
            resolution_notes: vec![
                "no_task_id: override empty and no active/focus pointer".to_string(),
            ],
        };
    };

    let (goal_state, rfv_loop_state, evidence, mut read_notes) =
        hydrate_task_state_hybrid(repo_root, task_id.as_str());
    resolution_notes.append(&mut read_notes);

    let evidence_ok = evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);

    let depth_compliance = Some(depth_compliance_aggregate(
        goal_state.as_ref(),
        rfv_loop_state.as_ref(),
        evidence_ok,
    ));

    let control_mode = classify_control_mode(
        goal_state.as_ref(),
        rfv_loop_state.as_ref(),
        &mut resolution_notes,
    );

    maybe_note_active_goal_missing_focus_has_goal(
        repo_root,
        &pointers,
        task_id.as_str(),
        goal_state.as_ref(),
        &mut resolution_notes,
    );
    maybe_note_active_goal_not_driving_focus_drives(
        repo_root,
        &pointers,
        task_id.as_str(),
        goal_state.as_ref(),
        &mut resolution_notes,
    );
    maybe_note_dual_goal_pointer_conflict(repo_root, &pointers, &mut resolution_notes);

    ResolvedTaskView {
        schema_version: RESOLVED_TASK_VIEW_SCHEMA_VERSION.to_string(),
        task_id: Some(task_id.clone()),
        pointers,
        goal_state,
        rfv_loop_state,
        evidence,
        depth_compliance,
        control_mode,
        resolution_notes,
    }
}

/// One-line hint for continuity digest / Codex SessionStart (`Continuity digest` prompt).
/// Omitted when no resolved `task_id` (idle). Keeps copy short for ~640-char caps.
pub fn depth_compliance_refresh_hint(view: &ResolvedTaskView) -> Option<String> {
    if !depth_compliance_hint_enabled()
    {
        return None;
    }
    let tid = view.task_id.as_deref()?.trim();
    if tid.is_empty() {
        return None;
    }
    let dc = view.depth_compliance.as_ref()?;
    let mut out = format!("深度信号: d{}/3", dc.depth_score);
    if dc.qg_pass_without_evidence_count > 0 {
        out.push_str(&format!(
            " · PASS无对照证据={}",
            dc.qg_pass_without_evidence_count
        ));
    }
    if dc.qg_external_deep_structured_round_count > 0 {
        out.push_str(&format!(
            " · 结构化外研轮次={}",
            dc.qg_external_deep_structured_round_count
        ));
    }
    if dc.qg_external_strict_ok_round_count > 0 {
        out.push_str(&format!(
            " · 外研strict通过轮次={}",
            dc.qg_external_strict_ok_round_count
        ));
    }
    if dc.qg_external_deep_structured_round_count > 0 && !depth_score_mode_is_strict() {
        out.push_str(DEPTH_COMPLIANCE_LEGACY_EXTERNAL_DEPTH_NOTE_ZH);
    }
    Some(out)
}

/// Optional hard gates for `framework_goal_drive` **`operation=complete`** (stored on `GOAL_STATE`).
#[derive(Debug, Clone)]
pub struct GoalCompletionGates {
    pub enabled: bool,
    pub min_depth_score: Option<u8>,
    pub require_successful_evidence_row: bool,
    pub min_goal_checkpoints: Option<u64>,
    pub block_on_rfv_pass_without_evidence: bool,
}

/// Parse `GOAL_STATE.completion_gates`. Missing / null → **off** (no gate). Object with
/// `"enabled": false` → parsed but [`validate_goal_completion_gates`] is a no-op.
pub fn parse_goal_completion_gates(goal: &Value) -> Option<GoalCompletionGates> {
    let raw = goal.get("completion_gates")?;
    if raw.is_null() {
        return None;
    }
    let o = raw.as_object()?;
    let enabled = o.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let min_depth_score = o
        .get("min_depth_score")
        .and_then(Value::as_u64)
        .map(|u| u.min(3) as u8);
    let require_successful_evidence_row = o
        .get("require_successful_evidence_row")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let min_goal_checkpoints = o.get("min_goal_checkpoints").and_then(Value::as_u64);
    let block_on_rfv_pass_without_evidence = o
        .get("block_on_rfv_pass_without_evidence")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(GoalCompletionGates {
        enabled,
        min_depth_score,
        require_successful_evidence_row,
        min_goal_checkpoints,
        block_on_rfv_pass_without_evidence,
    })
}

/// Enforce `completion_gates` against [`resolve_task_view`] output (same rollup as L3 digest).
pub fn validate_goal_completion_gates(
    view: &ResolvedTaskView,
    gates: &GoalCompletionGates,
) -> Result<(), String> {
    if !gates.enabled {
        return Ok(());
    }
    let Some(dc) = view.depth_compliance.as_ref() else {
        return Err(
            "GOAL completion_gates: missing depth rollup (no resolved task_id / idle view)"
                .to_string(),
        );
    };
    if let Some(min) = gates.min_depth_score
        && dc.depth_score < min {
            return Err(format!(
                "GOAL completion_gates: depth_score={} < min_depth_score={} (fix RFV/EVIDENCE/checkpoints or lower the gate; rollup from resolve_task_view)",
                dc.depth_score, min
            ));
        }
    if gates.require_successful_evidence_row {
        let ok = view
            .evidence
            .as_ref()
            .is_some_and(|e| e.has_successful_verification);
        if !ok {
            return Err(
                "GOAL completion_gates: require_successful_evidence_row but EVIDENCE_INDEX has no successful row"
                    .to_string(),
            );
        }
    }
    if let Some(min_ck) = gates.min_goal_checkpoints {
        let n = view
            .goal_state
            .as_ref()
            .and_then(|g| g.get("checkpoints"))
            .and_then(Value::as_array)
            .map(|a| a.len() as u64)
            .unwrap_or(0);
        if n < min_ck {
            return Err(format!(
                "GOAL completion_gates: checkpoints.len()={n} < min_goal_checkpoints={min_ck}"
            ));
        }
    }
    if gates.block_on_rfv_pass_without_evidence && dc.qg_pass_without_evidence_count > 0 {
        return Err(format!(
            "GOAL completion_gates: block_on_rfv_pass_without_evidence but qg_pass_without_evidence_count={}",
            dc.qg_pass_without_evidence_count
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static DEPTH_SCORE_MODE_ENV_TEST_MUTEX: Mutex<()> = Mutex::new(());

    use crate::utils::test_helpers::unique_repo;

    fn write_active(tmp: &Path, id: &str) {
        let p = tmp.join("artifacts/current/active_task.json");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!(r#"{{"task_id":"{id}"}}"#)).unwrap();
    }

    fn write_focus(tmp: &Path, id: &str) {
        let p = tmp.join("artifacts/current/focus_task.json");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!(r#"{{"task_id":"{id}"}}"#)).unwrap();
    }

    #[test]
    fn no_pointer_idle() {
        let tmp = unique_repo("no-ptr");
        fs::create_dir_all(&tmp).unwrap();
        let v = resolve_task_view(&tmp, None);
        assert_eq!(v.task_id, None);
        assert!(matches!(v.control_mode, TaskControlMode::Idle));
        assert!(!v.resolution_notes.is_empty());
        assert!(depth_compliance_refresh_hint(&v).is_none());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn active_only_goal_drive() {
        let _env_guard = DEPTH_SCORE_MODE_ENV_TEST_MUTEX
            .lock()
            .expect("depth score mode env mutex poisoned");
        let prior_hint = std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT").ok();
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", "1") };
        let tmp = unique_repo("goal");
        let tid = "t1";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "ship feature",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[]}"#,
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        assert_eq!(v.task_id.as_deref(), Some(tid));
        assert!(matches!(v.control_mode, TaskControlMode::GoalDrive));
        let ev = v.evidence.as_ref().expect("evidence");
        assert!(!ev.evidence_rows_non_empty);
        assert!(!ev.has_successful_verification);
        let hint = depth_compliance_refresh_hint(&v).expect("hint");
        assert!(hint.contains("深度信号") && hint.contains("d0/3"));
        assert!(
            !v.resolution_notes.iter().any(|n| {
                n.starts_with(super::RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL)
            }),
            "unexpected focus-has-goal note when active has GOAL: {:?}",
            v.resolution_notes
        );
        match prior_hint {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn promote_focus_to_active_when_active_lacks_goal() {
        // When active has no GOAL_STATE but focus does, promotion rewrites active_task.json.
        let tmp = unique_repo("promote-focus");
        let active_tid = "t-no-goal";
        let focus_tid = "t-has-goal";
        write_active(&tmp, active_tid);
        write_focus(&tmp, focus_tid);
        fs::create_dir_all(tmp.join("artifacts/current").join(focus_tid)).unwrap();
        fs::write(
            tmp.join("artifacts/current")
                .join(focus_tid)
                .join("GOAL_STATE.json"),
            r#"{"goal":"g","status":"planned","drive_until_done":false,"non_goals":["n"],"done_when":["a","b"],"validation_commands":["c"]}"#,
        )
        .unwrap();
        assert!(maybe_promote_focus_to_active_pointer(&tmp));
        // After promotion, resolve_task_view loads the focus task's goal via the rewritten pointer.
        let v = resolve_task_view(&tmp, None);
        assert_eq!(v.task_id.as_deref(), Some(focus_tid));
        assert!(v.goal_state.is_some());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn active_missing_goal_focus_has_goal_emits_continuity_note() {
        // With explicit override to the focus task, the pointer-based mismatch note
        // is not emitted (no active/focus pointer comparison occurs).
        let tmp = unique_repo("af-focus");
        let active_tid = "t-no-goal";
        let focus_tid = "t-has-goal";
        write_active(&tmp, active_tid);
        write_focus(&tmp, focus_tid);
        fs::create_dir_all(tmp.join("artifacts/current").join(active_tid)).unwrap();
        let focus_dir = tmp.join("artifacts/current").join(focus_tid);
        fs::create_dir_all(&focus_dir).unwrap();
        fs::write(
            focus_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "from focus",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();

        // Explicit override to the focus task loads its goal directly.
        let v = resolve_task_view(&tmp, Some(focus_tid));
        assert_eq!(v.task_id.as_deref(), Some(focus_tid));
        assert!(v.goal_state.is_some());
        // Explicit override bypasses pointer-based mismatch detection.
        assert!(
            !v.resolution_notes.iter().any(|n| {
                n.starts_with(super::RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL)
            }),
            "pointer-based mismatch note must not be emitted with explicit override: {:?}",
            v.resolution_notes
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn override_to_focus_skips_active_focus_goal_note_even_when_active_empty() {
        let tmp = unique_repo("af-override");
        let active_tid = "t-no-goal";
        let focus_tid = "t-has-goal";
        write_active(&tmp, active_tid);
        write_focus(&tmp, focus_tid);
        fs::create_dir_all(tmp.join("artifacts/current").join(active_tid)).unwrap();
        let focus_dir = tmp.join("artifacts/current").join(focus_tid);
        fs::create_dir_all(&focus_dir).unwrap();
        fs::write(
            focus_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "drive_until_done": false,
                "status": "running",
                "goal": "g"
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(focus_tid));
        assert_eq!(v.task_id.as_deref(), Some(focus_tid));
        assert!(
            !v.resolution_notes.iter().any(|n| {
                n.starts_with(super::RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL)
            }),
            "override resolves focus id — must not emit active/focus hydration mismatch note: {:?}",
            v.resolution_notes
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn unreadable_goal_state_adds_resolution_note() {
        let tmp = unique_repo("bad-goal");
        let tid = "tbad";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(task_dir.join("GOAL_STATE.json"), "{\"not\": valid json").unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        assert_eq!(v.task_id.as_deref(), Some(tid));
        assert!(v.goal_state.is_none());
        assert!(
            v.resolution_notes
                .iter()
                .any(|n| n.contains("goal_state_read_failed")),
            "notes={:?}",
            v.resolution_notes
        );
        assert!(
            v.resolution_notes
                .iter()
                .any(|n| n.contains("parse") || n.contains("GOAL")),
            "notes={:?}",
            v.resolution_notes
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn conflict_when_goal_and_rfv_active() {
        let tmp = unique_repo("conflict");
        let tid = "t2";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "drive_until_done": true,
                "status": "running",
                "goal": "g"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 0,
                "rounds": []
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        assert!(matches!(v.control_mode, TaskControlMode::Conflict { .. }));
        assert!(!v.resolution_notes.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_score_rewards_adversarial_rfv_without_goal_checkpoints() {
        let tmp = unique_repo("adv");
        let tid = "t-adv";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 1,
                "rounds": [{
                    "round": 1,
                    "verify_result": "PASS",
                    "adversarial_findings": [{"id":"A1"}],
                    "falsification_tests": [{"id":"T1"},{"id":"T2"}]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[]}"#,
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.expect("dc");
        assert_eq!(dc.qg_pass_round_count, 1);
        assert_eq!(dc.qg_adversarial_round_count, 1);
        assert_eq!(dc.qg_falsification_test_count, 2);
        assert_eq!(dc.goal_checkpoint_count, 0);
        assert_eq!(
            dc.depth_score, 2,
            "PASS (1) + adversarial (1) = 2, no evidence"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_score_strict_mode_counts_falsification_for_third_point() {
        let _env_guard = DEPTH_SCORE_MODE_ENV_TEST_MUTEX
            .lock()
            .expect("depth score mode env mutex poisoned");
        let prior = std::env::var("ROUTER_RS_DEPTH_SCORE_MODE").ok();
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", "strict") };
        let tmp = unique_repo("strict-fals");
        let tid = "t-strict-fals";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 1,
                "rounds": [{
                    "round": 1,
                    "verify_result": "PASS",
                    "falsification_tests": [{"id":"T1"}]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"x","exit_code":0}]}"#,
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.expect("dc");
        assert_eq!(dc.depth_score, 3, "strict third point via falsification");
        match prior {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_compliance_counts_structured_external_research_rounds() {
        let _env_guard = DEPTH_SCORE_MODE_ENV_TEST_MUTEX
            .lock()
            .expect("depth score mode env mutex poisoned");
        let prior_depth = std::env::var("ROUTER_RS_DEPTH_SCORE_MODE").ok();
        let prior_hint = std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT").ok();
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") };
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", "1") };
        let tmp = unique_repo("ext-deep");
        let tid = "t-ext";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 2,
                "rounds": [
                    {"round": 1, "verify_result": "PASS", "external_research": {"claims": [{"x": 1}]}},
                    {"round": 2, "verify_result": "FAIL"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.as_ref().expect("dc");
        assert_eq!(dc.qg_external_deep_structured_round_count, 1);

        let hint = depth_compliance_refresh_hint(&v).expect("hint");
        assert!(hint.contains("结构化外研轮次=1"));
        assert!(hint.contains(super::DEPTH_COMPLIANCE_LEGACY_EXTERNAL_DEPTH_NOTE_ZH));

        match prior_depth {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") },
        }
        match prior_hint {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_refresh_hint_legacy_external_note_suppressed_when_strict_score_mode_env() {
        let _env_guard = DEPTH_SCORE_MODE_ENV_TEST_MUTEX
            .lock()
            .expect("depth score mode env mutex poisoned");
        let prior = std::env::var("ROUTER_RS_DEPTH_SCORE_MODE").ok();
        let prior_hint = std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT").ok();
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", "strict") };
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", "1") };
        let tmp = unique_repo("ext-deep-strict-note");
        let tid = "t-ext-note";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 2,
                "rounds": [
                    {"round": 1, "verify_result": "PASS", "external_research": {"claims": [{"x": 1}]}},
                    {"round": 2, "verify_result": "FAIL"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let hint = depth_compliance_refresh_hint(&v).expect("hint");
        assert!(!hint.contains(super::DEPTH_COMPLIANCE_LEGACY_EXTERNAL_DEPTH_NOTE_ZH));

        match prior {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") },
        }
        match prior_hint {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_compliance_counts_external_strict_ok_when_flag_true() {
        let _env_guard = DEPTH_SCORE_MODE_ENV_TEST_MUTEX
            .lock()
            .expect("depth score mode env mutex poisoned");
        let prior_depth = std::env::var("ROUTER_RS_DEPTH_SCORE_MODE").ok();
        let prior_hint = std::env::var("ROUTER_RS_DEPTH_COMPLIANCE_HINT").ok();
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") };
        // SAFETY: test-only, env mutex held by caller or thread-local test serialization
        unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", "1") };
        let tmp = unique_repo("ext-strict");
        let tid = "t-ext-st";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        let t40 = "0123456789012345678901234567890123456789";
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "external_research_strict": true,
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 1,
                "rounds": [{
                    "round": 1,
                    "verify_result": "PASS",
                    "external_research": {
                        "claims": [{"claim": "c1", "sources": ["https://a.example/foo", "doi:10.1000/182"]}],
                        "contradiction_sweep": [
                            {"related_claim_or_topic": "t1", "contradicting_or_limiting_evidence": "e1", "sources": ["https://c.example/x"]},
                            {"related_claim_or_topic": "t2", "contradicting_or_limiting_evidence": "e2", "sources": ["https://d.example/y"]}
                        ],
                        "unknowns": [],
                        "retrieval_trace": {
                            "queries_used": ["q1 one two", "q2 three four", "q3 five six"],
                            "inclusion_rules": t40,
                            "exclusions": t40,
                            "exclusion_rationale": t40
                        }
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.as_ref().expect("dc");
        assert_eq!(dc.qg_external_deep_structured_round_count, 1);
        assert_eq!(dc.qg_external_strict_ok_round_count, 1);

        let hint = depth_compliance_refresh_hint(&v).expect("hint");
        assert!(hint.contains("外研strict通过轮次=1"));
        assert!(hint.contains(super::DEPTH_COMPLIANCE_LEGACY_EXTERNAL_DEPTH_NOTE_ZH));

        match prior_depth {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_SCORE_MODE", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_SCORE_MODE") },
        }
        match prior_hint {
            Some(p) => unsafe { std::env::set_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT", p) },
            None => unsafe { std::env::remove_var("ROUTER_RS_DEPTH_COMPLIANCE_HINT") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn depth_compliance_strict_ok_zero_when_strict_flag_false() {
        let tmp = unique_repo("ext-strict-off");
        let tid = "t-off";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        let t40 = "0123456789012345678901234567890123456789";
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "external_research_strict": false,
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 3,
                "current_round": 1,
                "rounds": [{
                    "round": 1,
                    "verify_result": "PASS",
                    "external_research": {
                        "claims": [{"claim": "c1", "sources": ["https://a.example/foo", "doi:10.1000/182"]}],
                        "contradiction_sweep": [
                            {"related_claim_or_topic": "t1", "contradicting_or_limiting_evidence": "e1", "sources": ["https://c.example/x"]},
                            {"related_claim_or_topic": "t2", "contradicting_or_limiting_evidence": "e2", "sources": ["https://d.example/y"]}
                        ],
                        "unknowns": [],
                        "retrieval_trace": {
                            "queries_used": ["q1 one two", "q2 three four", "q3 five six"],
                            "inclusion_rules": t40,
                            "exclusions": t40,
                            "exclusion_rationale": t40
                        }
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.as_ref().expect("dc");
        assert_eq!(dc.qg_external_deep_structured_round_count, 1);
        assert_eq!(dc.qg_external_strict_ok_round_count, 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn override_wins_over_active() {
        let tmp = unique_repo("override");
        write_active(&tmp, "active-id");
        let other = "other-id";
        let task_dir = tmp.join("artifacts/current").join(other);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "drive_until_done": false,
                "status": "completed",
                "goal": "done"
            }))
            .unwrap(),
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(other));
        assert_eq!(v.task_id.as_deref(), Some(other));
        assert!(matches!(v.control_mode, TaskControlMode::Idle));
        let _ = fs::remove_dir_all(&tmp);
    }

    /// P1-A: depth_compliance aggregates RFV PASS rounds + EVIDENCE successful rows + checkpoints.
    #[test]
    fn depth_compliance_rolls_up_pass_evidence_and_checkpoints() {
        let tmp = unique_repo("depth-compl");
        let tid = "t-depth";
        write_active(&tmp, tid);
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        // GOAL with one checkpoint.
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "ship",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [{"note": "step 1"}],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
        // RFV with PASS round + UNKNOWN round + a no_evidence_window flag.
        fs::write(
            task_dir.join("RFV_LOOP_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-rfv-loop-v1",
                "loop_status": "active",
                "goal": "g",
                "max_rounds": 5,
                "current_round": 2,
                "rounds": [
                    {"round": 1, "verify_result": "PASS", "cross_check": "no_evidence_window"},
                    {"round": 2, "verify_result": "UNKNOWN"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        // EVIDENCE with one successful row.
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
        )
        .unwrap();

        let v = resolve_task_view(&tmp, Some(tid));
        let dc = v.depth_compliance.expect("depth_compliance present");
        assert_eq!(dc.qg_pass_round_count, 1);
        assert_eq!(dc.qg_unknown_round_count, 1);
        assert_eq!(dc.qg_pass_without_evidence_count, 1);
        assert_eq!(dc.goal_checkpoint_count, 1);
        // Score = 3 (pass + evidence_ok + checkpoint).
        assert_eq!(dc.depth_score, 3);
        let _ = fs::remove_dir_all(&tmp);
    }

    /// P1-A: empty state → score 0.
    #[test]
    fn depth_compliance_zero_when_nothing_recorded() {
        let tmp = unique_repo("depth-zero");
        write_active(&tmp, "t-zero");
        let task_dir = tmp.join("artifacts/current/t-zero");
        fs::create_dir_all(&task_dir).unwrap();
        let v = resolve_task_view(&tmp, Some("t-zero"));
        let dc = v.depth_compliance.expect("depth_compliance present");
        assert_eq!(dc.depth_score, 0);
        assert_eq!(dc.qg_pass_round_count, 0);
        assert_eq!(dc.goal_checkpoint_count, 0);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn continuity_frame_skips_orphan_goal_without_pointer_or_opt_in() {
        let tmp = unique_repo("orphan-hydr");
        let tid = "t-orph";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"goal":"orphan","status":"running","drive_until_done":true}"#,
        )
        .unwrap();
        let frame = resolve_continuity_frame(&tmp);
        assert!(
            frame.pointer_view.task_id.is_none(),
            "stub pointers → no task_id in pointer_view"
        );
        assert!(
            frame.hydration_goal.is_none(),
            "new session without pointer must not auto-hydrate orphan goal"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hydrate_replays_ledger_tail_after_projection() {
        let tmp = unique_repo("hydrate-replay");
        let tid = "t-replay";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("TASK_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-task-state-aggregate-v1",
                "task_id": tid,
                "last_seq": 0,
                "goal_state": {"status": "planned", "goal": "old"},
                "rfv_loop_state": null,
                "evidence": {"evidence_rows_non_empty": false, "has_successful_verification": false}
            }))
            .unwrap(),
        )
        .unwrap();
        let ledger_line = serde_json::json!({
            "ts": "2026-01-02T00:00:00Z",
            "tx_type": "goal_state",
            "payload": {"status": "running", "goal": "from-ledger"},
            "seq": 1,
            "schema_version": 1
        });
        fs::write(
            task_dir.join("TASK_LEDGER.jsonl"),
            format!("{}\n", serde_json::to_string(&ledger_line).unwrap()),
        )
        .unwrap();
        let (goal, _, _, _) = hydrate_task_state_hybrid(&tmp, tid);
        let goal = goal.expect("goal");
        assert_eq!(goal["goal"], json!("from-ledger"));
        assert_eq!(goal["status"], json!("running"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hydrate_falls_back_to_physical_files() {
        let tmp = unique_repo("hydrate-fallback");
        let tid = "t-fallback";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"status":"running","goal":"physical-file"}"#,
        )
        .unwrap();
        let (goal, _, _, _) = hydrate_task_state_hybrid(&tmp, tid);
        let goal = goal.expect("goal");
        assert_eq!(goal["goal"], json!("physical-file"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hydrate_skips_corrupt_ledger_lines() {
        let tmp = unique_repo("hydrate-corrupt");
        let tid = "t-corrupt";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        let good = serde_json::json!({
            "ts": "2026-01-02T00:00:00Z",
            "tx_type": "goal_state",
            "payload": {"status": "running", "goal": "good-line"},
            "seq": 0,
            "schema_version": 1
        });
        fs::write(
            task_dir.join("TASK_LEDGER.jsonl"),
            format!("not-json\n{}\n", serde_json::to_string(&good).unwrap()),
        )
        .unwrap();
        let txs = read_task_ledger_transactions(&tmp, tid);
        assert_eq!(txs.len(), 1);
        let (goal, _, _, _) = hydrate_task_state_hybrid(&tmp, tid);
        assert_eq!(goal.expect("goal")["goal"], json!("good-line"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn read_task_pointers_reads_legacy_pointer_files() {
        let tmp = unique_repo("pointers-legacy");
        let active_dir = tmp.join("artifacts/current");
        fs::create_dir_all(&active_dir).unwrap();
        fs::write(
            active_dir.join("active_task.json"),
            r#"{"task_id":"active-from-disk"}"#,
        )
        .unwrap();
        fs::write(
            active_dir.join("focus_task.json"),
            r#"{"task_id":"focus-from-disk"}"#,
        )
        .unwrap();
        let pointers = read_task_pointers(&tmp);
        assert_eq!(pointers.active_task_id.as_deref(), Some("active-from-disk"));
        assert_eq!(pointers.focus_task_id.as_deref(), Some("focus-from-disk"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_task_view_idle_when_no_override_and_stub_pointers() {
        let tmp = unique_repo("resolve-idle");
        let active_dir = tmp.join("artifacts/current");
        fs::create_dir_all(&active_dir).unwrap();
        // With task_id_override = None and stub pointers, should return idle view.
        let view = resolve_task_view(&tmp, None);
        assert!(
            view.task_id.is_none(),
            "idle view: task_id should be None when no override and stub pointers"
        );
        assert!(
            view.goal_state.is_none(),
            "idle view: goal_state should be None"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_task_view_uses_explicit_override_even_with_stub() {
        let tmp = unique_repo("resolve-override");
        let tid = "explicit-task";
        let task_dir = tmp.join("artifacts/current").join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            r#"{"status":"running","goal":"test-goal"}"#,
        )
        .unwrap();
        // With explicit task_id_override, should resolve even though pointers are stubbed.
        let view = resolve_task_view(&tmp, Some(tid));
        assert_eq!(view.task_id.as_deref(), Some(tid));
        assert!(
            view.goal_state.is_some(),
            "goal_state should resolve with explicit override"
        );
        let _ = fs::remove_dir_all(&tmp);
    }
}
