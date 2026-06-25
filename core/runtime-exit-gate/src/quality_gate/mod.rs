//! Quality Gate 多轮闭环：Rust 真源 `RFV_LOOP_STATE.json` + stdio，支撑长任务轮次账本与宿主并行 lane 之后的 supervisor 合并落盘。

pub use core_state::state_manager::read_quality_gate_state;
// QUALITY_GATE_STATE_FILENAME is imported via core_state::state_manager::quality_gate_state_path.
// quality_gate_state_path is re-exported below.

use core_policy::error::FrameworkError;
use core_state_utils::atomic_write::write_atomic_json;
#[allow(unused_imports)] // consumed by tests via `use super::*`
use core_state::state_manager::{
    source_traceable_heuristic, validate_external_research_strict,
    validate_external_research_structured,
};
use framework_kernel::repo_roots::resolve_repo_root_arg;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, FrameworkError>;

// ---- Constants ----
pub const QUALITY_GATE_LOOP_SCHEMA_VERSION: &str = "router-rs-quality-gate-v1";
/// Repo-relative path; keep in sync with `cursor_hooks` merge logic that surfaces this substring.
pub const QG_EXTERNAL_RESEARCH_SCHEMA_REL_PATH: &str =
    "configs/framework/QUALITY_GATE_EXTERNAL_RESEARCH.schema.json";
/// `retrieval_trace` prose fields must be at least this many **trimmed** chars under strict mode.
pub const EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN: usize = 40;
/// Allowed `verify_result` enum (uppercase).
/// `append_round` rejects values outside this set so PASS/FAIL is auditable, not free-form.
pub const ALLOWED_VERIFY_RESULTS: &[&str] = &["PASS", "FAIL", "SKIPPED", "UNKNOWN"];

// ---- Shared helper functions ----

/// Check external_research_strict flag from loaded RFV state object.
/// Used by both close_gates enforcement and append_round validation.
fn external_research_strict_from_loaded_state(obj: &Map<String, Value>) -> bool {
    match obj.get("external_research_strict") {
        Some(Value::Bool(b)) => *b,
        _ => false,
    }
}

fn normalize_verify_result(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("UNKNOWN".to_string());
    }
    let upper = trimmed.to_ascii_uppercase();
    if ALLOWED_VERIFY_RESULTS.iter().any(|s| *s == upper) {
        return Ok(upper);
    }
    Err(FrameworkError::validation(format!(
        "verify_result must be one of {ALLOWED_VERIFY_RESULTS:?} (case-insensitive), got {raw:?}"
    )))
}

/// EVIDENCE_INDEX 行视为「成功验证」：`success==true` 或 `exit_code==0`。
/// 实际规则下沉到 [`core_policy::hook_common::evidence_index_entry_implies_success`]，与 `goal_drive`
/// 共用一份口径（避免历史上的两套独立判定函数）。
fn evidence_row_is_success(row: &Value) -> bool {
    core_state::state_manager::evidence_index_entry_implies_success(row)
}

/// 取上一轮 `at`；若无上一轮则取 RFV state 的 `updated_at`；都无则返回 None。
fn previous_round_window_start(state_obj: &Map<String, Value>) -> Option<String> {
    let rounds = state_obj.get("rounds").and_then(Value::as_array)?;
    if let Some(last) = rounds.last()
        && let Some(at) = last.get("at").and_then(Value::as_str) {
            return Some(at.to_string());
        }
    state_obj
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// 解析 RFC 3339 时间字符串为 DateTime<Utc>。
/// 只接受标准 RFC 3339 格式，拒绝非标准格式以避免歧义。
fn parse_iso_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
}

/// 判断一个 evidence 行的 recorded_at 是否在 window 内。
/// 保守处理：解析失败视为不在窗口内（避免 false positive）。
fn is_timestamp_in_window(row_at: Option<&str>, window_start: Option<&str>) -> bool {
    match (window_start, row_at) {
        (Some(start), Some(at)) => {
            let start_dt = match parse_iso_datetime(start) {
                Some(dt) => dt,
                None => return false,
            };
            let at_dt = match parse_iso_datetime(at) {
                Some(dt) => dt,
                None => return false,
            };
            at_dt > start_dt
        }
        (Some(_), None) => false,
        (None, Some(_)) => true,
        (None, None) => false,
    }
}

/// Check whether any finding in the list has severity P0, A, or B.
/// Used by the convergence floor logic to decide if this round is "stable".
fn has_ab_level_findings(findings: &[Value]) -> bool {
    findings.iter().any(|f| {
        let severity = f
            .get("severity")
            .or_else(|| f.get("level"))
            .and_then(Value::as_str)
            .unwrap_or("");
        matches!(severity, "P0" | "p0" | "A" | "a" | "B" | "b")
    })
}

/// Re-export canonical path from core-state (single source of truth).
pub use core_state::state_manager::quality_gate_state_path;

fn value_string_list(payload: &Value, key: &str) -> Vec<Value> {
    payload
        .get(key)
        .and_then(|v| {
            if let Some(arr) = v.as_array() {
                Some(
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(|s| json!(s))
                        .collect(),
                )
            } else { v.as_str().map(|s| vec![json!(s)]) }
        })
        .unwrap_or_default()
}

fn value_array_or_empty(payload: &Value, key: &str) -> Result<Vec<Value>> {
    let Some(v) = payload.get(key) else {
        return Ok(Vec::new());
    };
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(arr) = v.as_array() else {
        return Err(FrameworkError::validation(format!("{key} must be array (or null), got {v:?}")));
    };
    Ok(arr.clone())
}

fn clamp_max_rounds(raw: u64) -> (u64, bool) {
    let cap = fr_exec::router_env_flags::router_rs_qg_max_rounds_cap();
    if raw > cap { (cap, true) } else { (raw, false) }
}

fn resolve_framework_quality_gate_repo(payload: &Value) -> Result<PathBuf> {
    let repo_root = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "framework_quality_gate requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(FrameworkError::validation(format!(
            "framework_quality_gate: repo_root is not a directory: {}",
            repo_root.display()
        )));
    }
    Ok(resolve_repo_root_arg(Some(repo_root.as_path()))?)
}

/// Attach JSON-backed operator nudge reference lines for stdio callers (non-hook path).
fn merge_operator_nudge_refs(resp: &mut Value, repo_root: &Path, state: Option<&Value>) {
    let nudges = crate::harness_ops::resolve_harness_operator_nudges(repo_root);
    let mut refs = Map::new();
    if !nudges.qg_loop_continue_reasoning_depth.is_empty() {
        refs.insert(
            "qg_loop_continue_reasoning_depth".to_string(),
            json!(nudges.qg_loop_continue_reasoning_depth),
        );
    }
    if state.is_some_and(rt_core_contracts::harness_context_signals::quality_gate_state_signals_math)
        && !nudges.math_reasoning_harness_line.is_empty()
    {
        refs.insert(
            "math_reasoning_harness_line".to_string(),
            json!(nudges.math_reasoning_harness_line),
        );
    }
    if !refs.is_empty() && let Some(obj) = resp.as_object_mut() {
        obj.insert("operator_nudge_refs".to_string(), Value::Object(refs));
    }
}

// ---- Submodules ----

pub mod close_gates;
pub mod evidence;
pub mod flow;
#[cfg(test)]
pub mod tests;

// Re-export all pub items from submodules so internal sibling calls work
// and external paths remain unchanged.
pub use close_gates::{parse_close_gates, enforce_rfv_close_gates};
pub use evidence::{cross_link_evidence, EvidenceReadError};
pub use flow::framework_quality_gate;


