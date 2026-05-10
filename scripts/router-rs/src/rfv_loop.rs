//! Review-Fix-Verify 多轮闭环：Rust 真源 `RFV_LOOP_STATE.json` + stdio，支撑长任务轮次账本与宿主并行 lane 之后的 supervisor 合并落盘。

use crate::autopilot_goal::read_active_task_id;
use crate::framework_runtime::resolve_repo_root_arg;
use crate::router_env_flags::{
    router_rs_env_enabled_default_true, router_rs_goal_prompt_verbose,
    router_rs_operator_inject_globally_enabled,
};
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const RFV_LOOP_STATE_FILENAME: &str = "RFV_LOOP_STATE.json";
pub const RFV_LOOP_SCHEMA_VERSION: &str = "router-rs-rfv-loop-v1";
const MAX_ROUNDS_HARD_CAP: u64 = 1000;
/// Cursor hook：`RFV_LOOP_CONTINUE` 跟进；设为 `0`/`false`/`off`/`no` 关闭。
const RFV_LOOP_HOOK_ENV: &str = "ROUTER_RS_RFV_LOOP_HOOK";

/// Allowed `verify_result` enum (uppercase); see `reasoning-depth-contract.md`.
/// `append_round` rejects values outside this set so PASS/FAIL is auditable, not free-form.
pub const ALLOWED_VERIFY_RESULTS: &[&str] = &["PASS", "FAIL", "SKIPPED", "UNKNOWN"];

fn normalize_verify_result(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("UNKNOWN".to_string());
    }
    let upper = trimmed.to_ascii_uppercase();
    if ALLOWED_VERIFY_RESULTS.iter().any(|s| *s == upper) {
        return Ok(upper);
    }
    Err(format!(
        "verify_result must be one of {ALLOWED_VERIFY_RESULTS:?} (case-insensitive), got {raw:?}"
    ))
}

/// EVIDENCE_INDEX 行视为「成功验证」：`success==true` 或 `exit_code==0`。
fn evidence_row_is_success(row: &Value) -> bool {
    if row.get("success").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    matches!(row.get("exit_code").and_then(|v| v.as_i64()), Some(0))
        || matches!(row.get("exit_code").and_then(|v| v.as_u64()), Some(0))
}

/// 读取同任务目录下的 `EVIDENCE_INDEX.json`；非法 / 缺失视为空。
fn read_evidence_index_artifacts(repo_root: &Path, task_id: &str) -> Vec<Value> {
    let path = repo_root
        .join("artifacts/current")
        .join(task_id)
        .join("EVIDENCE_INDEX.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    val.get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 取上一轮 `at`；若无上一轮则取 RFV state 的 `updated_at`；都无则返回 None。
fn previous_round_window_start(state_obj: &Map<String, Value>) -> Option<String> {
    let rounds = state_obj.get("rounds").and_then(Value::as_array)?;
    if let Some(last) = rounds.last() {
        if let Some(at) = last.get("at").and_then(Value::as_str) {
            return Some(at.to_string());
        }
    }
    state_obj
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Cross-link 本轮 verify 与 EVIDENCE_INDEX 成功行：返回 `(refs, cross_check_label)`。
/// `refs` 为 EVIDENCE artifacts 数组中的索引（u64）；`cross_check_label` 为可选标签：
/// - `"no_evidence_window"`：claimed PASS 但窗口内无成功 evidence（**审计警告**，不阻断写入）
/// - `"evidence_after_fail"`：claimed FAIL 但仍有成功 evidence（信息性，便于人工核对）
/// - `None`：未声明 PASS/FAIL，或一致。
fn cross_link_evidence(
    repo_root: &Path,
    task_id: &str,
    state_obj: &Map<String, Value>,
    verify_result: &str,
) -> (Vec<Value>, Option<String>) {
    let artifacts = read_evidence_index_artifacts(repo_root, task_id);
    if artifacts.is_empty() {
        let label = if verify_result == "PASS" {
            Some("no_evidence_window".to_string())
        } else {
            None
        };
        return (Vec::new(), label);
    }
    let window_start = previous_round_window_start(state_obj);
    let mut refs: Vec<Value> = Vec::new();
    for (idx, row) in artifacts.iter().enumerate() {
        if !evidence_row_is_success(row) {
            continue;
        }
        let row_at = row
            .get("recorded_at")
            .or_else(|| row.get("at"))
            .and_then(Value::as_str);
        let in_window = match (&window_start, row_at) {
            (Some(start), Some(at)) => at > start.as_str(),
            (None, _) => true,
            (Some(_), None) => true,
        };
        if in_window {
            refs.push(json!(idx as u64));
        }
    }
    let label = match verify_result {
        "PASS" if refs.is_empty() => Some("no_evidence_window".to_string()),
        "FAIL" if !refs.is_empty() => Some("evidence_after_fail".to_string()),
        _ => None,
    };
    (refs, label)
}

fn rfv_loop_hook_enabled() -> bool {
    // P1-E: aggregate kill-switch first.
    router_rs_operator_inject_globally_enabled()
        && router_rs_env_enabled_default_true(RFV_LOOP_HOOK_ENV)
}

fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("serialize RFV_LOOP_STATE failed: {err}"))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, text)
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn rfv_loop_state_path(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id)
        .join(RFV_LOOP_STATE_FILENAME)
}

/// Autopilot 在同 task 上 `start`/`upsert`/`resume` 时结束 RFV 的 `loop_status=active`（与 GOAL 互斥；保留文件并标记 `superseded`）。
pub(crate) fn deactivate_rfv_for_conflict_with_autopilot(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    let path = rfv_loop_state_path(repo_root, task_id);
    if !path.is_file() {
        return Ok(false);
    }
    let mut state = read_rfv_loop_state(repo_root, Some(task_id))?
        .ok_or_else(|| format!("RFV_LOOP_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "RFV_LOOP_STATE root must be object".to_string())?;
    let active = obj
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"));
    if !active {
        return Ok(false);
    }
    obj.insert("loop_status".to_string(), json!("superseded"));
    obj.insert("superseded_by".to_string(), json!("autopilot_goal"));
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
}

/// 供 Cursor hook / 工具读取当前任务的 RFV 账本（无覆盖则用 `active_task.json`）。
pub fn read_rfv_loop_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_rfv_loop: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    let path = rfv_loop_state_path(repo_root, &task_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read RFV_LOOP_STATE: {err}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse RFV_LOOP_STATE: {err}"))?;
    Ok(Some(value))
}

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
            } else if let Some(s) = v.as_str() {
                Some(vec![json!(s)])
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn value_array_or_empty(payload: &Value, key: &str) -> Result<Vec<Value>, String> {
    let Some(v) = payload.get(key) else {
        return Ok(Vec::new());
    };
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(arr) = v.as_array() else {
        return Err(format!("{key} must be array (or null), got {v:?}"));
    };
    Ok(arr.clone())
}

fn clamp_max_rounds(raw: u64) -> (u64, bool) {
    if raw > MAX_ROUNDS_HARD_CAP {
        (MAX_ROUNDS_HARD_CAP, true)
    } else {
        (raw, false)
    }
}

/// stdio：`framework_rfv_loop`
pub fn framework_rfv_loop(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_rfv_loop_impl(payload)
    } else {
        crate::task_write_lock::apply_task_ledger_mutation(|| framework_rfv_loop_impl(payload))
    }
}

fn framework_rfv_loop_impl(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_rfv_loop requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_rfv_loop: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    let repo_root = resolve_repo_root_arg(Some(repo_root.as_path()))?;
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();

    let task_id_override = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match operation.as_str() {
        "status" => {
            let state = read_rfv_loop_state(&repo_root, task_id_override)?;
            let tid = if let Some(t) = task_id_override {
                t.to_string()
            } else {
                read_active_task_id(&repo_root).unwrap_or_default()
            };
            let path = if tid.is_empty() {
                PathBuf::new()
            } else {
                rfv_loop_state_path(&repo_root, &tid)
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": state,
            }))
        }
        "start" | "upsert" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_rfv_loop start requires task_id in payload or active_task.json"
                        .to_string()
                })?;
            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "framework_rfv_loop start requires non-empty goal".to_string())?;
            let requested_max = payload
                .get("max_rounds")
                .and_then(Value::as_u64)
                .unwrap_or(3);
            let (max_rounds, capped) = clamp_max_rounds(requested_max);
            let allow_external = payload
                .get("allow_external_research")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let parallel_external = payload
                .get("parallel_external_with_review")
                .and_then(Value::as_bool)
                .unwrap_or(true);

            let mut obj = Map::new();
            obj.insert("schema_version".to_string(), json!(RFV_LOOP_SCHEMA_VERSION));
            obj.insert("goal".to_string(), json!(goal));
            obj.insert("max_rounds".to_string(), json!(max_rounds));
            obj.insert("max_rounds_requested".to_string(), json!(requested_max));
            obj.insert("max_rounds_capped".to_string(), json!(capped));
            obj.insert("allow_external_research".to_string(), json!(allow_external));
            obj.insert(
                "parallel_external_with_review".to_string(),
                json!(parallel_external),
            );
            obj.insert(
                "review_scope".to_string(),
                json!(payload
                    .get("review_scope")
                    .and_then(Value::as_str)
                    .unwrap_or("")),
            );
            obj.insert(
                "fix_scope".to_string(),
                json!(payload
                    .get("fix_scope")
                    .and_then(Value::as_str)
                    .unwrap_or("")),
            );
            obj.insert(
                "verify_commands".to_string(),
                Value::Array(value_string_list(&payload, "verify_commands")),
            );
            obj.insert(
                "stop_when".to_string(),
                Value::Array(value_string_list(&payload, "stop_when")),
            );
            obj.insert("loop_status".to_string(), json!("active"));
            obj.insert("current_round".to_string(), json!(0));
            obj.insert("rounds".to_string(), json!([]));
            obj.insert("updated_at".to_string(), json!(now_iso()));
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
            }

            let path = rfv_loop_state_path(&repo_root, &task_id);
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            let goal_state_cleared =
                crate::autopilot_goal::deactivate_goal_for_conflict_with_rfv(&repo_root, &task_id)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": value,
                "goal_state_cleared": goal_state_cleared,
                "warning": if capped {
                    Some(format!(
                        "max_rounds requested {requested_max} exceeds hard cap {MAX_ROUNDS_HARD_CAP}; stored max_rounds={max_rounds}"
                    ))
                } else {
                    None
                },
            }))
        }
        "append_round" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_rfv_loop append_round requires task_id or active_task.json"
                        .to_string()
                })?;
            let path = rfv_loop_state_path(&repo_root, &task_id);
            let mut state = read_rfv_loop_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| format!("RFV_LOOP_STATE missing at {}", path.display()))?;

            let round_n = payload
                .get("round")
                .and_then(Value::as_u64)
                .ok_or_else(|| "append_round requires round (u64)".to_string())?;

            let obj = state
                .as_object_mut()
                .ok_or_else(|| "RFV_LOOP_STATE root must be object".to_string())?;
            let max_rounds = obj
                .get("max_rounds")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_ROUNDS_HARD_CAP);
            if round_n > max_rounds {
                return Err(format!("round {round_n} exceeds max_rounds {max_rounds}"));
            }

            let review_summary = payload
                .get("review_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let external_research_summary = payload
                .get("external_research_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let fix_summary = payload
                .get("fix_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let raw_verify = payload
                .get("verify_result")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN");
            let verify_result = normalize_verify_result(raw_verify)?;
            let supervisor_decision = payload
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .unwrap_or("continue")
                .to_ascii_lowercase();
            let reason = payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            // Optional "adversarial depth" fields: stored as-is (array) for audit; no new state machine.
            // Shapes are minimally validated here so later rollups can trust arrays.
            let adversarial_findings = value_array_or_empty(&payload, "adversarial_findings")?;
            let falsification_tests = value_array_or_empty(&payload, "falsification_tests")?;

            // Cross-link this round's verify claim against EVIDENCE_INDEX successful rows
            // recorded since the previous round (audit trail; not a hard block — supervisor
            // still owns the call, but the discrepancy lands in `cross_check`).
            let (evidence_refs, cross_check_label) =
                cross_link_evidence(&repo_root, &task_id, obj, &verify_result);

            let mut entry_map = serde_json::Map::new();
            entry_map.insert("round".to_string(), json!(round_n));
            entry_map.insert("review_summary".to_string(), json!(review_summary));
            entry_map.insert(
                "external_research_summary".to_string(),
                json!(external_research_summary),
            );
            entry_map.insert("fix_summary".to_string(), json!(fix_summary));
            entry_map.insert("verify_result".to_string(), json!(verify_result));
            entry_map.insert(
                "supervisor_decision".to_string(),
                json!(supervisor_decision),
            );
            entry_map.insert("reason".to_string(), json!(reason));
            entry_map.insert("at".to_string(), json!(now_iso()));
            entry_map.insert("evidence_refs".to_string(), Value::Array(evidence_refs));
            if !adversarial_findings.is_empty() {
                entry_map.insert(
                    "adversarial_findings".to_string(),
                    Value::Array(adversarial_findings),
                );
            }
            if !falsification_tests.is_empty() {
                entry_map.insert(
                    "falsification_tests".to_string(),
                    Value::Array(falsification_tests),
                );
            }
            if let Some(label) = cross_check_label {
                entry_map.insert("cross_check".to_string(), json!(label));
            }
            let entry = Value::Object(entry_map);

            let rounds = obj
                .get_mut("rounds")
                .and_then(|r| r.as_array_mut())
                .ok_or_else(|| "RFV_LOOP_STATE.rounds missing".to_string())?;
            rounds.push(entry);

            obj.insert("current_round".to_string(), json!(round_n));
            obj.insert("updated_at".to_string(), json!(now_iso()));

            let loop_status = match supervisor_decision.as_str() {
                "close" | "closed" => "closed",
                "block" | "blocked" => "blocked",
                _ => {
                    if round_n >= max_rounds {
                        "closed"
                    } else {
                        "active"
                    }
                }
            };
            obj.insert("loop_status".to_string(), json!(loop_status));

            write_atomic_json(&path, &state)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "append_round",
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": state,
            }))
        }
        _ => Err(format!(
            "framework_rfv_loop: unknown operation '{operation}'"
        )),
    }
}

fn rfv_loop_requests_continuation(state: &Value) -> bool {
    state
        .get("loop_status")
        .and_then(Value::as_str)
        .map(|s| s.eq_ignore_ascii_case("active"))
        .unwrap_or(false)
}

fn rfv_followup_compact_line(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut out = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

/// 已解析的 `RFV_LOOP_STATE` 上构建 RFV 续跑提示（与 [`build_rfv_loop_followup_message`] 文案一致）。
pub fn build_rfv_loop_followup_message_from_state(
    repo_root: &Path,
    task_id: &str,
    state: &Value,
) -> Option<String> {
    if !rfv_loop_hook_enabled() {
        return None;
    }
    if !rfv_loop_requests_continuation(state) {
        return None;
    }
    let goal = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or("(no goal in RFV_LOOP_STATE)");
    let current = state
        .get("current_round")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_r = state.get("max_rounds").and_then(Value::as_u64).unwrap_or(0);
    let ext = state
        .get("allow_external_research")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if router_rs_goal_prompt_verbose() {
        let path = rfv_loop_state_path(repo_root, task_id);
        let mut lines = vec![
            "RFV_LOOP_CONTINUE: `RFV_LOOP_STATE.json` 显示多轮 review-fix-verify 仍在进行（`loop_status=active`）。".to_string(),
            format!("Path: {}", path.display()),
            format!("Goal: {}", goal),
            format!("Progress: round {current} / max_rounds {max_r} (达到 stop_when 或 append_round close 后可结束)"),
        ];
        if ext {
            lines.push(
                "本任务允许外部调研：下一轮可并行 external research + internal review，再进入 fix / verify；每轮结束请 `framework_rfv_loop` append_round。"
                    .to_string(),
            );
        } else {
            lines.push(
                "下一轮请按 review → fix → verify 顺序起独立 subagent，并在每轮末 `append_round` 落盘。"
                    .to_string(),
            );
        }
        let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
        if !nudges.rfv_loop_continue_reasoning_depth.is_empty() {
            lines.push(nudges.rfv_loop_continue_reasoning_depth.clone());
        }
        crate::harness_operator_nudges::push_math_reasoning_line(&mut lines, &nudges);
        return Some(lines.join("\n"));
    }
    let rel = format!("artifacts/current/{task_id}/RFV_LOOP_STATE.json");
    let gshort = rfv_followup_compact_line(goal, 120);
    let ext_note = if ext { " · ext ok" } else { "" };
    let mut lines = vec![
        format!("RFV_LOOP_CONTINUE: active · r {current}/{max_r}{ext_note} · `{rel}`"),
        format!("Goal: {gshort}"),
    ];
    lines.push(if ext {
        "Next: ext+review→fix→verify；轮末 `framework_rfv_loop` append_round。".to_string()
    } else {
        "Next: review→fix→verify；轮末 append_round。".to_string()
    });
    let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
    if !nudges.rfv_loop_continue_reasoning_depth.is_empty() {
        lines.push(nudges.rfv_loop_continue_reasoning_depth.clone());
    }
    crate::harness_operator_nudges::push_math_reasoning_line(&mut lines, &nudges);
    Some(lines.join("\n"))
}

/// Cursor stop / beforeSubmit：`loop_status=active` 时提示继续下一轮 RFV。
/// 默认紧凑；`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 与 Goal / AUTOPILOT_DRIVE 共用同一 verbose 开关。
pub fn build_rfv_loop_followup_message(repo_root: &Path) -> Option<String> {
    let state = read_rfv_loop_state(repo_root, None).ok()??;
    let task_id = read_active_task_id(repo_root)?;
    build_rfv_loop_followup_message_from_state(repo_root, &task_id, &state)
}

/// preCompact 用的一行摘要（不分配大段 followup）。
pub fn rfv_loop_precompact_hint(repo_root: &Path) -> Option<String> {
    let state = read_rfv_loop_state(repo_root, None).ok()??;
    if !rfv_loop_requests_continuation(&state) {
        return None;
    }
    let current = state
        .get("current_round")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_r = state.get("max_rounds").and_then(Value::as_u64).unwrap_or(0);
    Some(format!(
        "RFV active r{current}/{max_r} — `RFV_LOOP_STATE.json`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rfv_start_append_roundtrip() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"rfv-task"}"#,
        )
        .expect("pointer");

        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "rfv-task",
            "goal": "harden loop",
            "max_rounds": 100,
            "allow_external_research": true,
            "verify_commands": ["cargo test -q"],
            "stop_when": ["verifier pass", "max_rounds"],
        }))
        .expect("start");

        framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 1u64,
            "review_summary": "r1",
            "external_research_summary": "web: none",
            "fix_summary": "f1",
            "verify_result": "PASS",
            "adversarial_findings": [
                {"id":"A1","hypothesis":"panic on empty input","severity":"high"}
            ],
            "falsification_tests": [
                {"id":"T1","command":"cargo test -q","expect":"pass"}
            ],
            "supervisor_decision": "continue",
            "reason": "ok",
        }))
        .expect("append");

        let st = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "status",
        }))
        .expect("status");
        let gs = st["rfv_loop_state"].as_object().expect("obj");
        assert_eq!(gs["current_round"], json!(1));
        assert_eq!(gs["loop_status"], json!("active"));
        let rounds = gs["rounds"].as_array().expect("rounds");
        let r1 = rounds[0].as_object().expect("round1 obj");
        assert!(r1.get("adversarial_findings").is_some());
        assert!(r1.get("falsification_tests").is_some());
        let _via_api = read_rfv_loop_state(&repo, None)
            .expect("read api")
            .expect("state");

        let msg = build_rfv_loop_followup_message(&repo).expect("rfv followup");
        assert!(msg.contains("RFV_LOOP_CONTINUE"));
        assert!(
            msg.contains("artifacts/current/") && msg.contains("RFV_LOOP_STATE.json"),
            "compact followup should use relative path; msg={msg:?}"
        );
        assert!(
            msg.contains("推理深度") && msg.contains("EVIDENCE_INDEX"),
            "registry nudge should append; msg={msg:?}"
        );

        let prior = std::env::var("ROUTER_RS_GOAL_PROMPT_VERBOSE").ok();
        std::env::set_var("ROUTER_RS_GOAL_PROMPT_VERBOSE", "1");
        let msg_v = build_rfv_loop_followup_message(&repo).expect("rfv verbose");
        assert!(
            msg_v.contains("loop_status=active"),
            "verbose env should restore long RFV banner; msg={msg_v:?}"
        );
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_GOAL_PROMPT_VERBOSE", v),
            None => std::env::remove_var("ROUTER_RS_GOAL_PROMPT_VERBOSE"),
        }

        let _ = fs::remove_dir_all(&repo);
    }

    /// P0-A: invalid `verify_result` is rejected (not silently coerced).
    #[test]
    fn append_round_rejects_unknown_verify_result() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-vr-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-vr")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-vr"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-vr",
            "goal": "verify enum",
            "max_rounds": 5u64,
        }))
        .expect("start");
        let err = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "kinda passed",
        }))
        .expect_err("invalid verify_result must error");
        assert!(
            err.contains("verify_result must be one of"),
            "unexpected error: {err}"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    /// P1-B: PASS round with no successful EVIDENCE_INDEX rows surfaces `cross_check=no_evidence_window`.
    #[test]
    fn append_round_marks_pass_without_evidence() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-cl-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        let task_dir = repo.join("artifacts/current/t-cl");
        fs::create_dir_all(&task_dir).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-cl"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-cl",
            "goal": "cross-link",
            "max_rounds": 3u64,
        }))
        .expect("start");
        // No EVIDENCE_INDEX yet → PASS should land with no_evidence_window.
        let out = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "PASS",
        }))
        .expect("append");
        let rounds = out["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds array");
        let r1 = &rounds[0];
        assert_eq!(r1["cross_check"], json!("no_evidence_window"));
        assert!(r1["evidence_refs"].as_array().expect("refs").is_empty());

        // Now write a successful EVIDENCE row newer than the round timestamp and append round 2.
        // Use a timestamp far in the future so it deterministically beats round 1's `at`.
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"recorded_at":"2099-12-31T23:59:59Z","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
        let out2 = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 2u64,
            "verify_result": "PASS",
        }))
        .expect("append 2");
        let rounds2 = out2["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds 2");
        let r2 = &rounds2[1];
        assert!(
            r2.get("cross_check").is_none(),
            "expected cross_check absent on PASS-with-evidence; round={r2}"
        );
        assert!(
            !r2["evidence_refs"].as_array().expect("refs2").is_empty(),
            "expected non-empty evidence_refs; round={r2}"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    /// RFV 与 GOAL 同 task 互斥：RFV start 应删除已存在的 GOAL_STATE。
    #[test]
    fn rfv_start_clears_goal_same_task() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-rfv-mutex-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-mx")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"rfv-mx"}"#,
        )
        .expect("pointer");
        let rr = repo.display().to_string();

        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "rfv-mx",
            "goal": "macro first",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        let gpath = crate::autopilot_goal::goal_state_path_for_task(&repo, "rfv-mx");
        assert!(gpath.is_file());
        assert!(crate::autopilot_goal::build_autopilot_drive_followup_message(&repo).is_some());

        let out = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "rfv-mx",
            "goal": "rfv mode",
            "max_rounds": 2u64,
        }))
        .expect("rfv start");
        assert_eq!(out["goal_state_cleared"], json!(true));
        assert!(!gpath.is_file());
        assert!(crate::autopilot_goal::build_autopilot_drive_followup_message(&repo).is_none());

        let _ = fs::remove_dir_all(&repo);
    }
}
