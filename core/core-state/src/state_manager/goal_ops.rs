// Goal state management: start, checkpoint, pause, resume, complete, block, clear.
// Extracted from state_manager.rs during module split.

use crate::utils::atomic_write::write_atomic_json;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use super::pointer_ops::{
    ensure_task_directory, neutralize_task_pointers_for_task, sync_task_pointers_after_goal_drive,
};
use super::quality_gate_ops::deactivate_quality_gate_for_conflict_with_goal_drive;
use super::{REQUIRES_COMPLETION_EVIDENCE_KEY, goal_state_path_for_task, read_goal_state, read_goal_type_from_state};
use core_state_types::task_state_types::GoalType;

fn resolve_task_id_strict(payload: &Value) -> Result<String, String> {
    payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            "goal_state_manage: task_id is required in payload (multi-agent safe mode)".to_string()
        })
}

fn resolve_framework_goal_drive_repo(payload: &Value) -> Result<PathBuf, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "framework_goal_drive requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_goal_drive: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    Ok(repo_root.to_path_buf())
}

/// stdio / CLI：`framework_goal_drive`
pub fn framework_goal_drive(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_goal_drive_impl(payload)
    } else {
        let repo_root = resolve_framework_goal_drive_repo(&payload)?;
        crate::utils::task_write_lock::apply_task_ledger_mutation(
            &repo_root,
            || framework_goal_drive_impl(payload),
        ).map_err(|e| e.to_string())
    }
}

fn resolve_session_id(payload: &Value) -> String {
    // 1. Explicit from payload
    if let Some(sid) = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return sid.to_string();
    }
    // 2. Environment variables: scan for any *_SESSION_ID
    for (key, val) in std::env::vars() {
        if key.ends_with("_SESSION_ID") {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    // 3. No session identity available — return empty (legacy / no-isolation mode).
    //    Earlier versions auto-generated an "auto-{nanos}" token here, but that
    //    broke stale detection across sessions (auto-tokens are unique per creation,
    //    so they could never match in a later read where env is also absent).
    String::new()
}

fn base_goal_object(
    goal: String,
    non_goals: Vec<Value>,
    done_when: Vec<Value>,
    validation_commands: Vec<Value>,
    drive_until_done: bool,
    requires_completion_evidence: bool,
    current_horizon: Option<String>,
    session_id: String,
) -> Map<String, Value> {
    use super::GOAL_STATE_SCHEMA_VERSION;
    let mut m = Map::new();
    m.insert(
        "schema_version".to_string(),
        json!(GOAL_STATE_SCHEMA_VERSION),
    );
    m.insert("drive_until_done".to_string(), json!(drive_until_done));
    m.insert(
        REQUIRES_COMPLETION_EVIDENCE_KEY.to_string(),
        json!(requires_completion_evidence),
    );
    m.insert("status".to_string(), json!("running"));
    m.insert("goal".to_string(), json!(goal));
    m.insert("session_id".to_string(), json!(session_id));
    m.insert("non_goals".to_string(), Value::Array(non_goals));
    m.insert("done_when".to_string(), Value::Array(done_when));
    m.insert(
        "validation_commands".to_string(),
        Value::Array(validation_commands),
    );
    m.insert(
        "current_horizon".to_string(),
        json!(current_horizon.unwrap_or_default()),
    );
    m.insert("checkpoints".to_string(), json!([]));
    m.insert("blocker".to_string(), Value::Null);
    m.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));
    m
}

pub(crate) fn count_nonempty_string_items(values: &[Value]) -> usize {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .count()
}

pub(crate) fn value_string_list(payload: &Value, key: &str) -> Vec<Value> {
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

fn value_has_nonempty_string_item(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)),
        Some(Value::String(s)) => !s.trim().is_empty(),
        _ => false,
    }
}

/// Validate the institutional drive-until-done contract.
///
/// When `drive_until_done` is true, a goal must carry a minimally deep contract:
/// at least one non-goal, at least two done-when items, and at least one
/// validation command.  This prevents goals that are "one-step done" from
/// entering driving mode.
///
/// `context` is a label like "start", "amend", or "resume" used in error messages.
pub(crate) fn validate_drive_contract(
    drive_until_done: bool,
    non_goals: &[Value],
    done_when: &[Value],
    validation_commands: &[Value],
    context: &str,
) -> Result<(), String> {
    if !drive_until_done {
        return Ok(());
    }
    if count_nonempty_string_items(non_goals) == 0 {
        return Err(format!(
            "framework_goal_drive {context}: drive_until_done goal requires non-empty non_goals"
        ));
    }
    if count_nonempty_string_items(done_when) < 2 {
        return Err(format!(
            "framework_goal_drive {context}: drive_until_done goal requires >=2 done_when items"
        ));
    }
    if count_nonempty_string_items(validation_commands) == 0 {
        return Err(format!(
            "framework_goal_drive {context}: drive_until_done goal requires non-empty validation_commands"
        ));
    }
    Ok(())
}

fn goal_requires_completion_evidence(state: &Value) -> bool {
    if let Some(b) = state
        .get(REQUIRES_COMPLETION_EVIDENCE_KEY)
        .and_then(Value::as_bool)
    {
        return b;
    }
    state
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || value_has_nonempty_string_item(state.get("validation_commands"))
        || value_has_nonempty_string_item(state.get("done_when"))
}

fn apply_optional_goal_fields_from_payload(obj: &mut Map<String, Value>, payload: &Value) {
    if let Some(lp) = payload
        .get("lifecycle_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("lifecycle_profile".to_string(), json!(lp));
    }
    if let Some(gt) = payload
        .get("goal_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("goal_type".to_string(), json!(gt));
    }
    if let Some(st) = payload
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("status".to_string(), json!(st));
    }
    crate::goal_prediction::merge_prediction_from_payload(obj, payload);
}

fn task_evidence_artifacts_for_task(repo_root: &Path, task_id: &str) -> Vec<Value> {
    use super::EVIDENCE_INDEX_FILENAME;
    if task_id.trim().is_empty() {
        return Vec::new();
    }
    if crate::utils::path_guard::safe_task_id_component(task_id).is_none() {
        return Vec::new();
    }
    let Ok(goal_path) = goal_state_path_for_task(repo_root, task_id) else {
        return Vec::new();
    };
    let Some(parent) = goal_path.parent() else {
        return Vec::new();
    };
    let path = parent.join(EVIDENCE_INDEX_FILENAME);
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

fn evidence_row_is_self_attested(entry: &Value) -> bool {
    if entry
        .get("tool_call_id")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
    {
        return false;
    }
    entry.get("kind").and_then(Value::as_str) == Some("mcp_record_evidence")
        || entry.get("source").and_then(Value::as_str) == Some("mcp_record_evidence")
}

/// True when at least one successful evidence row exists and every successful row is MCP
/// self-attested (`mcp_record_evidence` without host-bound `tool_call_id`).
pub fn task_evidence_success_only_self_attested(repo_root: &Path, task_id: &str) -> bool {
    let artifacts = task_evidence_artifacts_for_task(repo_root, task_id);
    let mut saw_success = false;
    let mut saw_non_self_attested_success = false;
    for entry in artifacts {
        if !super::evidence_index_entry_implies_success(&entry) {
            continue;
        }
        saw_success = true;
        if !evidence_row_is_self_attested(&entry) {
            saw_non_self_attested_success = true;
        }
    }
    saw_success && !saw_non_self_attested_success
}

/// 指定 `task_id` 任务目录下 `EVIDENCE_INDEX.json`：是否存在非空 `artifacts`、是否至少有一条成功验证记录。
/// 单条 artifact 的「成功」判定下沉到 [`super::evidence_index_entry_implies_success`]，
/// 与 `rfv_loop` 共用一份口径。
pub fn task_evidence_artifacts_summary_for_task(repo_root: &Path, task_id: &str) -> (bool, bool) {
    use super::EVIDENCE_INDEX_FILENAME;
    if task_id.trim().is_empty() {
        return (false, false);
    }
    if crate::utils::path_guard::safe_task_id_component(task_id).is_none() {
        return (false, false);
    }
    let Ok(goal_path) = goal_state_path_for_task(repo_root, task_id) else {
        return (false, false);
    };
    let Some(parent) = goal_path.parent() else {
        return (false, false);
    };
    let path = parent.join(EVIDENCE_INDEX_FILENAME);
    if !path.is_file() {
        return (false, false);
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return (false, false);
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return (false, false);
    };
    let Some(arr) = val.get("artifacts").and_then(Value::as_array) else {
        return (false, false);
    };
    if arr.is_empty() {
        return (false, false);
    }
    let any_ok = arr.iter().any(super::evidence_index_entry_implies_success);
    (true, any_ok)
}

fn framework_goal_drive_impl(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_goal_drive requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_goal_drive: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    let repo_root = repo_root.canonicalize().unwrap_or(repo_root);
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
            use super::pointer_ops::read_task_pointer_pair;
            let tid = if let Some(t) = task_id_override {
                t.to_string()
            } else {
                let (active, focus) = read_task_pointer_pair(&repo_root);
                active.or(focus).unwrap_or_default()
            };
            let read_override = match task_id_override {
                Some(_) => task_id_override,
                None if tid.is_empty() => None,
                None => Some(tid.as_str()),
            };
            let state = read_goal_state(&repo_root, read_override)?;
            let path = if tid.is_empty() {
                PathBuf::new()
            } else {
                goal_state_path_for_task(&repo_root, &tid).unwrap_or_else(|_| PathBuf::new())
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "goal_state_path": path.display().to_string(),
                "goal_state": state,
            }))
        }
        "start" | "upsert" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "framework_goal_drive start requires non-empty goal".to_string())?;
            let drive_until_done = payload
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let requires_completion_evidence = if drive_until_done {
                true
            } else {
                payload
                    .get(REQUIRES_COMPLETION_EVIDENCE_KEY)
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            };
            let non_goals = value_string_list(&payload, "non_goals");
            let done_when = value_string_list(&payload, "done_when");
            let validation_commands = value_string_list(&payload, "validation_commands");

            // Institutional contract: when `drive_until_done` is true, a goal must not be "one-step done".
            // Enforce a minimally deep goal contract at creation time.
            validate_drive_contract(drive_until_done, &non_goals, &done_when, &validation_commands, "start")?;

            let session_id = resolve_session_id(&payload);
            let mut obj = base_goal_object(
                goal.to_string(),
                non_goals,
                done_when,
                validation_commands,
                drive_until_done,
                requires_completion_evidence,
                payload
                    .get("current_horizon")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                session_id,
            );
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
            }
            if let Some(cg) = payload.get("completion_gates")
                && !cg.is_null() {
                    obj.insert("completion_gates".to_string(), cg.clone());
                }
            apply_optional_goal_fields_from_payload(&mut obj, &payload);
            ensure_task_directory(&repo_root, &task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: framework_kernel::time::now_iso(),
                tx_type: "goal_state".to_string(),
                payload: value.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
            let quality_gate_superseded =
                deactivate_quality_gate_for_conflict_with_goal_drive(&repo_root, &task_id)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            sync_task_pointers_after_goal_drive(&repo_root, &task_id, goal, &payload)?;
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "status": "running",
                "quality_gate_superseded": quality_gate_superseded,
            }))
        }
        "checkpoint" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let quality_gate_superseded =
                deactivate_quality_gate_for_conflict_with_goal_drive(&repo_root, &task_id)?;
            let note = payload
                .get("note")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "framework_goal_drive checkpoint requires non-empty note".to_string()
                })?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
            let arr = state
                .as_object_mut()
                .and_then(|o| o.get_mut("checkpoints"))
                .and_then(|c| c.as_array_mut())
                .ok_or_else(|| "GOAL_STATE.checkpoints corrupt".to_string())?;
            arr.push(json!({
                "at": framework_kernel::time::now_iso(),
                "note": note,
                "type": payload.get("checkpoint_type").and_then(Value::as_str).unwrap_or("milestone"),
                "done_when_covers": payload.get("done_when_covers").cloned().unwrap_or(json!([])),
                "evidence_refs": payload.get("evidence_refs").cloned().unwrap_or(json!([])),
            }));
            if let Some(o) = state.as_object_mut() {
                o.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));
                crate::goal_prediction::merge_prediction_from_payload(o, &payload);
            }
            write_atomic_json(&path, &state)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: framework_kernel::time::now_iso(),
                tx_type: "goal_state".to_string(),
                payload: state.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "checkpoint",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "quality_gate_superseded": quality_gate_superseded,
            }))
        }
        "pause" => set_terminal_flags(
            &repo_root,
            Some(resolve_task_id_strict(&payload)?),
            "paused",
            Some(false),
            None,
        ),
        "resume" => {
            let drive_until_done = payload
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            resume_goal_running(
                &repo_root,
                Some(resolve_task_id_strict(&payload)?),
                drive_until_done,
                &payload,
            )
        }
        "complete" => {
            let task_id = resolve_task_id_strict(&payload)?;
            let state = read_goal_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| "GOAL_STATE missing for completion gate check".to_string())?;
            if goal_requires_completion_evidence(&state) {
                let (_, evidence_ok) =
                    task_evidence_artifacts_summary_for_task(&repo_root, task_id.as_str());
                if !evidence_ok {
                    return Err(
                        "framework_goal_drive complete requires successful EVIDENCE_INDEX row"
                            .to_string(),
                    );
                }
            }
            if let Some(gates) = crate::task_state::parse_goal_completion_gates(&state) {
                let view = crate::task_state::resolve_task_view(&repo_root, Some(task_id.as_str()));
                crate::task_state::validate_goal_completion_gates(&view, &gates)?;
            }

            // Loop goal: complete = iteration complete, NOT goal termination.
            // Keep status=running, do NOT archive or neutralize pointers.
            let goal_type = read_goal_type_from_state(&state);
            if goal_type == GoalType::Loop {
                let goal_path = goal_state_path_for_task(&repo_root, &task_id)?;
                let mut loop_state = read_goal_state(&repo_root, Some(&task_id))?
                    .ok_or_else(|| format!("GOAL_STATE missing at {}", goal_path.display()))?;
                if let Some(obj) = loop_state.as_object_mut() {
                    let count = obj.get("iteration_count").and_then(Value::as_u64).unwrap_or(0);
                    obj.insert("iteration_count".to_string(), json!(count + 1));
                    obj.insert("last_iteration_completed_at".to_string(), json!(framework_kernel::time::now_iso()));
                    obj.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));
                }
                write_atomic_json(&goal_path, &loop_state)?;
                let tx = crate::task_ledger::LedgerTransaction {
                    ts: framework_kernel::time::now_iso(),
                    tx_type: "goal_iteration_completed".to_string(),
                    payload: loop_state.clone(),
                    idempotency_key: None,
                    seq: None,
                    schema_version: Some(1),
                };
                crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                    .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
                crate::task_state_aggregate::sync_task_state_aggregate_best_effort(&repo_root, &task_id);
                return Ok(json!({
                    "ok": true,
                    "operation": "iteration_completed",
                    "task_id": task_id,
                    "iteration_count": loop_state.get("iteration_count").and_then(Value::as_u64).unwrap_or(0),
                }));
            }

            // Linear goal complete: terminal flags + neutralize + archive
            let out = set_terminal_flags(
                &repo_root,
                Some(task_id.clone()),
                "completed",
                Some(false),
                None,
            )?;
            neutralize_task_pointers_for_task(&repo_root, &task_id)?;
            // Mark GOAL_STATE.json as archived (do NOT delete — preserve for tooling/queries)
            let goal_path = goal_state_path_for_task(&repo_root, &task_id)?;
            if goal_path.is_file() {
                match fs::read_to_string(&goal_path) {
                    Ok(raw) => {
                        if let Ok(mut goal_val) = serde_json::from_str::<Value>(&raw)
                            && let Some(obj) = goal_val.as_object_mut() {
                                obj.insert("archived".to_string(), json!(true));
                                obj.insert(
                                    "completed_at".to_string(),
                                    json!(framework_kernel::time::now_iso()),
                                );
                                obj.insert(
                                    "updated_at".to_string(),
                                    json!(framework_kernel::time::now_iso()),
                                );
                                let _ = write_atomic_json(&goal_path, &goal_val);
                            }
                    }
                    Err(e) => warn!("failed to read GOAL_STATE.json for archive annotation: {e}"),
                }
            }
            Ok(out)
        }
        "block" => {
            let blocker = payload
                .get("blocker")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "framework_goal_drive block requires non-empty blocker".to_string()
                })?;
            set_terminal_flags(
                &repo_root,
                Some(resolve_task_id_strict(&payload)?),
                "blocked",
                Some(false),
                Some(blocker.to_string()),
            )
        }
        "clear" => clear_goal_state(&repo_root, Some(resolve_task_id_strict(&payload)?)),
        "amend" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
            let obj = state
                .as_object_mut()
                .ok_or_else(|| "GOAL_STATE root must be object".to_string())?;

            // Only mutable states can be amended
            let status = obj.get("status").and_then(Value::as_str).unwrap_or("");
            if status == "completed"
                || obj.get("archived").and_then(Value::as_bool).unwrap_or(false)
            {
                return Err(
                    "framework_goal_drive amend: cannot amend a completed/archived goal"
                        .to_string(),
                );
            }
            // Stale goals from another session cannot be amended
            if obj.get("stale").and_then(Value::as_bool).unwrap_or(false) {
                return Err(
                    "framework_goal_drive amend: cannot amend a stale goal (session_id mismatch)"
                        .to_string(),
                );
            }

            let keep_progress = payload
                .get("keep_progress")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let mut has_amend = false;

            if let Some(v) = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                obj.insert("goal".to_string(), json!(v));
                has_amend = true;
            }
            if let Some(arr) = payload.get("non_goals").and_then(Value::as_array) {
                let cleaned: Vec<Value> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| json!(s))
                    .collect();
                obj.insert("non_goals".to_string(), json!(cleaned));
                has_amend = true;
            }
            if let Some(arr) = payload.get("done_when").and_then(Value::as_array) {
                let cleaned: Vec<Value> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| json!(s))
                    .collect();
                obj.insert("done_when".to_string(), json!(cleaned));
                has_amend = true;
            }
            if let Some(arr) = payload.get("validation_commands").and_then(Value::as_array) {
                let cleaned: Vec<Value> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| json!(s))
                    .collect();
                obj.insert("validation_commands".to_string(), json!(cleaned));
                has_amend = true;
            }
            if let Some(gt) = payload
                .get("goal_type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                obj.insert("goal_type".to_string(), json!(gt));
                has_amend = true;
            }

            if !keep_progress {
                obj.insert("checkpoints".to_string(), json!([]));
            }

            if !has_amend {
                return Err(
                    "framework_goal_drive amend requires at least one field to update: \
                     goal, non_goals, done_when, or validation_commands"
                        .to_string(),
                );
            }

            // Amend drive-contract revalidation:
            // If the goal has drive_until_done=true, the resulting contract fields
            // must still satisfy the same completeness constraints as start.
            // This prevents amend from silently subcontracting a drive goal.
            let goal_drive = obj.get("drive_until_done").and_then(Value::as_bool).unwrap_or(false);
            let amend_non_goals = obj.get("non_goals")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let amend_done_when = obj.get("done_when")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let amend_validation = obj.get("validation_commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            validate_drive_contract(goal_drive, &amend_non_goals, &amend_done_when, &amend_validation, "amend")?;

            obj.insert(
                "amended_at".to_string(),
                json!(framework_kernel::time::now_iso()),
            );
            obj.insert(
                "updated_at".to_string(),
                json!(framework_kernel::time::now_iso()),
            );

            write_atomic_json(&path, &state)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: framework_kernel::time::now_iso(),
                tx_type: "goal_state".to_string(),
                payload: state.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "amend",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
            }))
        }
        _ => Err(format!(
            "framework_goal_drive: unknown operation '{operation}'"
        )),
    }
}

fn clear_goal_state(repo_root: &Path, task_id_resolved: Option<String>) -> Result<Value, String> {
    let task_id = task_id_resolved.ok_or_else(|| {
        "goal_state_manage: task_id is required (multi-agent safe mode)".to_string()
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let existed = path.is_file();
    if existed {
        fs::remove_file(&path).map_err(|err| format!("remove GOAL_STATE: {err}"))?;
    }
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    neutralize_task_pointers_for_task(repo_root, &task_id)?;
    Ok(json!({
        "ok": true,
        "operation": "clear",
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "removed": existed,
    }))
}

fn resume_goal_running(
    repo_root: &Path,
    task_id_resolved: Option<String>,
    drive_until_done: bool,
    payload: &Value,
) -> Result<Value, String> {
    let task_id = task_id_resolved.ok_or_else(|| {
        "goal_state_manage: task_id is required (multi-agent safe mode)".to_string()
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let mut state = read_goal_state(repo_root, Some(&task_id))?
        .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "GOAL_STATE root must be object".to_string())?;
    obj.insert("status".to_string(), json!("running"));
    obj.insert("drive_until_done".to_string(), json!(drive_until_done));
    obj.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));

    // Resume drive-contract revalidation:
    // When drive_until_done is being set to true, the existing contract fields
    // must already satisfy the completeness constraints — otherwise resume
    // could silently elevate a lightweight goal into driving mode without
    // the required depth.
    let resume_non_goals = obj.get("non_goals")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let resume_done_when = obj.get("done_when")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let resume_validation = obj.get("validation_commands")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    validate_drive_contract(drive_until_done, &resume_non_goals, &resume_done_when, &resume_validation, "resume")?;

    write_atomic_json(&path, &state)?;
    let tx = crate::task_ledger::LedgerTransaction {
        ts: framework_kernel::time::now_iso(),
        tx_type: "goal_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    crate::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
        .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
    let quality_gate_superseded = deactivate_quality_gate_for_conflict_with_goal_drive(repo_root, &task_id)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    let goal_label = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or(task_id.as_str());
    sync_task_pointers_after_goal_drive(repo_root, &task_id, goal_label, payload)?;
    Ok(json!({
        "ok": true,
        "operation": "resume",
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "goal_state": state,
        "quality_gate_superseded": quality_gate_superseded,
    }))
}

fn set_terminal_flags(
    repo_root: &Path,
    task_id_resolved: Option<String>,
    status: &str,
    drive_until_done: Option<bool>,
    blocker: Option<String>,
) -> Result<Value, String> {
    let task_id = task_id_resolved.ok_or_else(|| {
        "goal_state_manage: task_id is required (multi-agent safe mode)".to_string()
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let mut state = read_goal_state(repo_root, Some(&task_id))?
        .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "GOAL_STATE root must be object".to_string())?;
    obj.insert("status".to_string(), json!(status));
    if let Some(d) = drive_until_done {
        obj.insert("drive_until_done".to_string(), json!(d));
    }
    match blocker {
        Some(b) => obj.insert("blocker".to_string(), json!(b)),
        None if status == "blocked" => None,
        None => obj.insert("blocker".to_string(), Value::Null),
    };
    obj.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));
    write_atomic_json(&path, &state)?;
    let tx = crate::task_ledger::LedgerTransaction {
        ts: framework_kernel::time::now_iso(),
        tx_type: "goal_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    crate::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
        .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    Ok(json!({
        "ok": true,
        "operation": status,
        "task_id": task_id,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::state_manager::EVIDENCE_INDEX_FILENAME;
    use crate::task_ledger::task_ledger_path;
    use serde_json::json;

    // ── unique_repo ──────────────────────────────────────────────────────────

    fn unique_repo(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "router-rs-goal-ops-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn start_drive_goal(repo: &Path, task_id: &str) -> Value {
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": task_id,
            "goal": "drive goal test",
            "non_goals": ["n1", "n2"],
            "done_when": ["d1", "d2", "d3"],
            "validation_commands": ["cargo test"],
            "drive_until_done": true,
        }))
        .expect("start drive goal")
    }

    fn write_evidence_success(repo: &Path, task_id: &str) {
        let dir = repo.join("artifacts/current").join(task_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(EVIDENCE_INDEX_FILENAME),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"x","exit_code":0}]}"#,
        )
        .expect("write EVIDENCE_INDEX.json");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // validate_drive_contract
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn drive_contract_skips_when_not_driving() {
        assert!(validate_drive_contract(false, &[], &[], &[], "test").is_ok());
    }

    #[test]
    fn drive_contract_rejects_no_non_goals() {
        let err = validate_drive_contract(true, &[], &[json!("d1"), json!("d2")], &[json!("cargo test")], "test").unwrap_err();
        assert!(err.contains("non_goals"), "{err}");
    }

    #[test]
    fn drive_contract_rejects_less_than_two_done_when() {
        let err = validate_drive_contract(true, &[json!("n1")], &[json!("d1")], &[json!("cargo test")], "test").unwrap_err();
        assert!(err.contains("done_when"), "{err}");
    }

    #[test]
    fn drive_contract_rejects_no_validation_commands() {
        let err = validate_drive_contract(true, &[json!("n1")], &[json!("d1"), json!("d2")], &[], "test").unwrap_err();
        assert!(err.contains("validation_commands"), "{err}");
    }

    #[test]
    fn drive_contract_accepts_minimally_valid() {
        assert!(validate_drive_contract(true, &[json!("n1")], &[json!("d1"), json!("d2")], &[json!("cargo test")], "test").is_ok());
    }

    #[test]
    fn drive_contract_ignores_empty_string_items() {
        // Empty strings should not count toward the contract
        let err = validate_drive_contract(true, &[json!(""), json!("n1")], &[json!(""), json!("d1"), json!("d2")], &[json!("")], "test").unwrap_err();
        assert!(err.contains("validation_commands"), "empty validation cmd should not satisfy contract: {err}");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // goal_requires_completion_evidence
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn requires_evidence_when_drive_until_done() {
        let state = json!({"drive_until_done": true});
        assert!(goal_requires_completion_evidence(&state));
    }

    #[test]
    fn requires_evidence_when_explicit_flag() {
        let state = json!({"requires_completion_evidence": true});
        assert!(goal_requires_completion_evidence(&state));
    }

    #[test]
    fn requires_evidence_when_validation_commands_nonempty() {
        let state = json!({"validation_commands": ["cargo test"]});
        assert!(goal_requires_completion_evidence(&state));
    }

    #[test]
    fn requires_evidence_when_done_when_nonempty() {
        let state = json!({"done_when": ["d1"]});
        assert!(goal_requires_completion_evidence(&state));
    }

    #[test]
    fn not_requires_evidence_when_no_triggers() {
        let state = json!({"drive_until_done": false});
        assert!(!goal_requires_completion_evidence(&state));
        let state2 = json!({});
        assert!(!goal_requires_completion_evidence(&state2));
    }

    #[test]
    fn explicit_flag_is_authoritative() {
        // requires_completion_evidence explicitly set — checked first, overrides drive_until_done
        let state = json!({"drive_until_done": false, "requires_completion_evidence": true});
        assert!(goal_requires_completion_evidence(&state));

        // Explicit false overrides drive_until_done=true (key presence wins)
        let state2 = json!({"drive_until_done": true, "requires_completion_evidence": false});
        assert!(!goal_requires_completion_evidence(&state2), "explicit false is authoritative");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — start
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_start_creates_goal_state_file() {
        let repo = unique_repo("start-file");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        let out = start_drive_goal(&repo, "t-start");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["task_id"], json!("t-start"));

        let path = repo.join("artifacts/current/t-start/GOAL_STATE.json");
        assert!(path.is_file(), "GOAL_STATE.json must exist");

        let raw = fs::read_to_string(&path).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["schema_version"], json!("router-rs-goal-v1"));
        assert_eq!(goal["status"], json!("running"));
        assert_eq!(goal["goal"], json!("drive goal test"));
        assert_eq!(goal["drive_until_done"], json!(true));
        assert_eq!(goal[REQUIRES_COMPLETION_EVIDENCE_KEY], json!(true));
        assert_eq!(goal["checkpoints"], json!([]));

        // TASK_LEDGER should have the goal_state transaction
        let ledger_path = task_ledger_path(&repo, "t-start").unwrap();
        assert!(ledger_path.is_file(), "TASK_LEDGER.jsonl must exist");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_start_rejects_drive_without_contract() {
        let repo = unique_repo("start-reject");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-bad",
            "goal": "incomplete drive",
            "drive_until_done": true,
            // missing non_goals, done_when, validation_commands
        }))
        .unwrap_err();
        assert!(err.contains("non_goals"), "must reject: {err}");
        assert!(!repo.join("artifacts/current/t-bad/GOAL_STATE.json").is_file());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_start_non_drive_succeeds_with_minimal_fields() {
        let repo = unique_repo("start-nd");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-simple",
            "goal": "simple task",
            "drive_until_done": false,
        }))
        .expect("non-drive start must succeed");
        assert_eq!(out["ok"], json!(true));
        assert!(repo.join("artifacts/current/t-simple/GOAL_STATE.json").is_file());
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — status
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_status_reads_back_goal_state() {
        let repo = unique_repo("status");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-st");
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "status",
            "task_id": "t-st",
        }))
        .expect("status");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["task_id"], json!("t-st"));
        assert!(out["goal_state"].is_object());
        assert_eq!(out["goal_state"]["status"], json!("running"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_status_without_override_uses_pointer() {
        let repo = unique_repo("status-ptr");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-sp");
        // Pointer was set by start via sync_task_pointers_after_goal_drive
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "status",
            // no task_id — should resolve from pointer
        }))
        .expect("status with pointer");
        assert_eq!(out["task_id"], json!("t-sp"));
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — checkpoint
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_checkpoint_appends_to_goal_state() {
        let repo = unique_repo("cp");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-cp");
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "checkpoint",
            "task_id": "t-cp",
            "note": "milestone one",
        }))
        .expect("checkpoint");
        assert_eq!(out["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-cp/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        let cps = goal["checkpoints"].as_array().unwrap();
        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0]["note"], json!("milestone one"));
        assert_eq!(cps[0]["type"], json!("milestone"));
        assert!(cps[0]["at"].is_string());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_checkpoint_rejects_empty_note() {
        let repo = unique_repo("cp-empty");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-cpe");
        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "checkpoint",
            "task_id": "t-cpe",
            "note": "",
        }))
        .unwrap_err();
        assert!(err.contains("non-empty note"), "{err}");
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — pause / resume
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_pause_sets_status_paused_and_drive_false() {
        let repo = unique_repo("pause");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-pa");
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "pause",
            "task_id": "t-pa",
        }))
        .expect("pause");
        assert_eq!(out["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-pa/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["status"], json!("paused"));
        assert_eq!(goal["drive_until_done"], json!(false));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_pause_resume_cycle_toggles_status() {
        let repo = unique_repo("pause-resume");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-pr");

        // pause
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "pause",
            "task_id": "t-pr",
        }))
        .expect("pause");

        // resume
        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "resume",
            "task_id": "t-pr",
        }))
        .expect("resume");
        assert_eq!(out["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-pr/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["status"], json!("running"));
        assert_eq!(goal["drive_until_done"], json!(true));
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — complete
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_complete_succeeds_with_evidence() {
        let repo = unique_repo("complete-ev");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-ce");
        write_evidence_success(&repo, "t-ce");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-ce",
        }))
        .expect("complete");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["operation"], json!("completed"));

        // Verify goal state is marked archived
        let raw = fs::read_to_string(repo.join("artifacts/current/t-ce/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["archived"], json!(true));
        assert!(goal["completed_at"].is_string());
        assert_eq!(goal["status"], json!("completed"));

        // Verify pointers neutralized
        let (active, focus) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert!(active.is_none() || active.as_deref() != Some("t-ce"),
            "active pointer should be neutralized: {active:?}");
        assert!(focus.is_none() || focus.as_deref() != Some("t-ce"),
            "focus pointer should be neutralized: {focus:?}");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_rejects_missing_evidence() {
        let repo = unique_repo("complete-no-ev");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-cne");

        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-cne",
        }))
        .unwrap_err();
        assert!(err.contains("EVIDENCE"), "must reject without evidence: {err}");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_non_drive_does_not_require_evidence() {
        let repo = unique_repo("complete-nd");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-nd",
            "goal": "no drive needed",
            "drive_until_done": false,
        }))
        .expect("start non-drive");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-nd",
        }))
        .expect("non-drive complete must succeed without evidence");
        assert_eq!(out["ok"], json!(true));
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — block
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_block_sets_blocker_and_status() {
        let repo = unique_repo("block");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-bl");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "block",
            "task_id": "t-bl",
            "blocker": "waiting for dependency X",
        }))
        .expect("block");
        assert_eq!(out["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-bl/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["status"], json!("blocked"));
        assert_eq!(goal["blocker"], json!("waiting for dependency X"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_block_rejects_empty_blocker() {
        let repo = unique_repo("block-empty");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-ble");

        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "block",
            "task_id": "t-ble",
            "blocker": "",
        }))
        .unwrap_err();
        assert!(err.contains("non-empty blocker"), "{err}");
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — clear
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_clear_removes_goal_state_and_neutralizes_pointers() {
        let repo = unique_repo("clear");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-cl");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "clear",
            "task_id": "t-cl",
        }))
        .expect("clear");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["removed"], json!(true));

        assert!(!repo.join("artifacts/current/t-cl/GOAL_STATE.json").is_file(),
            "GOAL_STATE must be deleted");

        // Pointers neutralized
        let (active, focus) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert!(active.is_none() || active.as_deref() != Some("t-cl"));
        assert!(focus.is_none() || focus.as_deref() != Some("t-cl"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_clear_noop_on_missing_goal_state() {
        let repo = unique_repo("clear-miss");
        fs::create_dir_all(repo.join("artifacts/current/t-cm")).unwrap();

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "clear",
            "task_id": "t-cm",
        }))
        .expect("clear on missing goal");
        assert_eq!(out["removed"], json!(false));

        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // framework_goal_drive — amend
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_amend_updates_goal_field() {
        let repo = unique_repo("amend-ok");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-am");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "amend",
            "task_id": "t-am",
            "goal": "revised goal",
        }))
        .expect("amend");
        assert_eq!(out["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-am/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["goal"], json!("revised goal"));
        assert!(goal["amended_at"].is_string());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_amend_rejects_completed_goal() {
        let repo = unique_repo("amend-complete");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        // Non-drive goal — no evidence needed
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-ac",
            "goal": "to complete",
            "drive_until_done": false,
        }))
        .expect("start");
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-ac",
        }))
        .expect("complete");

        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "amend",
            "task_id": "t-ac",
            "goal": "wont work",
        }))
        .unwrap_err();
        assert!(err.contains("completed"), "{err}");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_amend_revalidates_drive_contract() {
        let repo = unique_repo("amend-contract");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        // Start with drive_until_done=true (satisfies contract: 2 non_goals, 3 done_when, 1 validation_cmd)
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-amc",
            "goal": "amend contract test",
            "non_goals": ["n1"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test"],
            "drive_until_done": true,
        }))
        .expect("start");

        // Amend to remove a done_when — should succeed since drive contract still met
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "amend",
            "task_id": "t-amc",
            "done_when": ["d1"],  // only 1 now — drive contract violated!
        }))
        .unwrap_err();
        // Note: amend with drive_until_done=true requires >=2 done_when

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_amend_requires_at_least_one_field() {
        let repo = unique_repo("amend-empty");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-ae");

        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "amend",
            "task_id": "t-ae",
            // no goal/non_goals/done_when/validation_commands
        }))
        .unwrap_err();
        assert!(err.contains("at least one field"), "{err}");
        let _ = fs::remove_dir_all(&repo);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // resolve_session_id
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn session_id_from_payload_takes_precedence() {
        let payload = json!({"session_id": "from-payload"});
        let sid = resolve_session_id(&payload);
        assert_eq!(sid, "from-payload");
    }

    #[test]
    fn session_id_falls_back_to_env_when_no_explicit() {
        let payload = json!({});
        // Returns first *_SESSION_ID env var match, or empty if none exist.
        // Either is acceptable — test validates no panic/crash in the fallback path.
        let _ = resolve_session_id(&payload);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // value_string_list helper
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn value_string_list_from_array() {
        let payload = json!({"items": ["a", "b", "c"]});
        let result = value_string_list(&payload, "items");
        assert_eq!(result, vec![json!("a"), json!("b"), json!("c")]);
    }

    #[test]
    fn value_string_list_from_single_string() {
        let payload = json!({"name": "hello"});
        let result = value_string_list(&payload, "name");
        assert_eq!(result, vec![json!("hello")]);
    }

    #[test]
    fn value_string_list_returns_empty_for_missing_key() {
        let payload = json!({});
        let result = value_string_list(&payload, "nope");
        assert_eq!(result, Vec::<Value>::new());
    }

    #[test]
    fn value_string_list_filters_non_strings() {
        let payload = json!({"items": ["a", 42, false, "b"]});
        let result = value_string_list(&payload, "items");
        assert_eq!(result, vec![json!("a"), json!("b")]);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // count_nonempty_string_items
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn count_nonempty_counts_correctly() {
        let items = [json!("a"), json!(""), json!("b"), json!("  "), json!("c")];
        assert_eq!(count_nonempty_string_items(&items), 3);
    }

    #[test]
    fn count_nonempty_zero_when_all_empty() {
        let items = [json!(""), json!("  ")];
        assert_eq!(count_nonempty_string_items(&items), 0);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // goal_state_path_for_task — path safety
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn goal_state_path_rejects_unsafe_task_id() {
        let repo = Path::new("/tmp");
        for bad in ["", "../x", "a/b", ".."] {
            let err = goal_state_path_for_task(repo, bad).unwrap_err();
            assert!(err.contains("safe path component"), "bad id {bad:?}: {err}");
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Loop goal: complete semantics
    // ═══════════════════════════════════════════════════════════════════════════

    fn start_loop_goal(repo: &Path, task_id: &str) {
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": task_id,
            "goal": "loop goal test",
            "goal_type": "loop",
            "drive_until_done": false,
        }))
        .expect("start loop goal");
    }

    #[test]
    fn loop_goal_complete_increments_iteration_count() {
        let repo = unique_repo("loop-iter");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_loop_goal(&repo, "t-li");

        let out = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-li",
        }))
        .expect("complete iteration 1");
        assert_eq!(out["operation"], json!("iteration_completed"));
        assert_eq!(out["iteration_count"], json!(1));

        // Verify GOAL_STATE on disk
        let raw = fs::read_to_string(repo.join("artifacts/current/t-li/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["iteration_count"], json!(1));
        assert!(goal.get("last_iteration_completed_at").and_then(Value::as_str).is_some());
        assert_eq!(goal["status"], json!("running"),
            "loop goal must remain running after iteration complete");
        assert!(goal.get("archived").is_none(),
            "loop goal must NOT be archived after iteration complete");

        // Complete again → iteration_count=2
        let out2 = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-li",
        }))
        .expect("complete iteration 2");
        assert_eq!(out2["iteration_count"], json!(2));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn loop_goal_complete_does_not_neutralize_pointers() {
        let repo = unique_repo("loop-ptr");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_loop_goal(&repo, "t-lp");

        // Active pointer should point to our loop task
        let (active, _) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert_eq!(active.as_deref(), Some("t-lp"),
            "loop goal should have active pointer");

        // Complete iteration
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-lp",
        }))
        .expect("complete iteration");

        // Pointers must still reference the task (not neutralized)
        let (active2, _) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert_eq!(active2.as_deref(), Some("t-lp"),
            "loop goal pointers must NOT be neutralized after iteration complete");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn loop_goal_keeps_running_after_iteration_complete() {
        let repo = unique_repo("loop-status");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_loop_goal(&repo, "t-ls");

        // Complete iteration
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-ls",
        }))
        .expect("complete iteration");

        // Status must still be running, not completed, not archived
        let st = read_goal_state(&repo, Some("t-ls"))
            .expect("read")
            .expect("state");
        assert_eq!(st["status"], json!("running"),
            "loop goal status must remain running");
        assert!(st.get("archived").is_none(),
            "loop goal must not have archived flag");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn loop_goal_complete_with_drive_until_done_requires_evidence() {
        let repo = unique_repo("loop-ev");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();

        // Start loop goal with drive_until_done=true (requires evidence)
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-lev",
            "goal": "loop with evidence",
            "goal_type": "loop",
            "drive_until_done": true,
            "non_goals": ["n1"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
        }))
        .expect("start loop drive");

        // Without evidence, complete must be rejected
        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-lev",
        }))
        .unwrap_err();
        assert!(err.contains("EVIDENCE"), "loop drive goal should require evidence: {err}");

        let _ = fs::remove_dir_all(&repo);
    }
}
