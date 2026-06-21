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
use super::rfv_ops::deactivate_rfv_for_conflict_with_goal_drive;
use super::{REQUIRES_COMPLETION_EVIDENCE_KEY, goal_state_path_for_task, now_iso, read_goal_state};

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
        crate::utils::task_write_lock::apply_task_ledger_mutation(&repo_root, || {
            framework_goal_drive_impl(payload)
        })
    }
}

/// Cache-invalidation hook called after every goal-state mutation.
///
/// Currently a no-op because the routing record cache lives in
/// `routing_engine` (across crate boundary) and is invalidated via
/// file-system mtime checks rather than in-process signals.
/// Retained as a seam so a future in-process cache can plug in without
/// touching every call-site.
fn invalidate_route_records_cache_on_write() {
    // No in-process route records cache; no-op for goal drive writes.
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
    // 2. Environment variables
    for env_key in &[
        "CLAUDE_SESSION_ID",
        "CURSOR_SESSION_ID",
        "OPENCODE_SESSION_ID",
    ] {
        if let Ok(sid) = std::env::var(env_key) {
            let trimmed = sid.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    // 3. Auto-generate from SystemTime nanos
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("auto-{nanos}")
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
    m.insert("updated_at".to_string(), json!(now_iso()));
    m
}

fn count_nonempty_string_items(values: &[Value]) -> usize {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .count()
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
                .unwrap_or(true);
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
            if drive_until_done {
                if count_nonempty_string_items(&non_goals) == 0 {
                    return Err(
                        "framework_goal_drive start requires non-empty non_goals (drive_until_done=true)"
                            .to_string(),
                    );
                }
                if count_nonempty_string_items(&done_when) < 2 {
                    return Err(
                        "framework_goal_drive start requires >=2 done_when items (drive_until_done=true)"
                            .to_string(),
                    );
                }
                if count_nonempty_string_items(&validation_commands) == 0 {
                    return Err(
                        "framework_goal_drive start requires non-empty validation_commands (drive_until_done=true)"
                            .to_string(),
                    );
                }
            }

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
            // Ensure task directory exists before writing GOAL_STATE
            ensure_task_directory(&repo_root, &task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                tx_type: "goal_state".to_string(),
                payload: value.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
            invalidate_route_records_cache_on_write();
            let rfv_loop_superseded =
                deactivate_rfv_for_conflict_with_goal_drive(&repo_root, &task_id)?;
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
                "rfv_loop_superseded": rfv_loop_superseded,
            }))
        }
        "checkpoint" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
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
                "at": now_iso(),
                "note": note,
                "type": payload.get("checkpoint_type").and_then(Value::as_str).unwrap_or("milestone"),
                "done_when_covers": payload.get("done_when_covers").cloned().unwrap_or(json!([])),
                "evidence_refs": payload.get("evidence_refs").cloned().unwrap_or(json!([])),
            }));
            if let Some(o) = state.as_object_mut() {
                o.insert("updated_at".to_string(), json!(now_iso()));
                crate::goal_prediction::merge_prediction_from_payload(o, &payload);
            }
            write_atomic_json(&path, &state)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                tx_type: "goal_state".to_string(),
                payload: state.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx)
                .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
            invalidate_route_records_cache_on_write();
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "checkpoint",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
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
            let out = set_terminal_flags(
                &repo_root,
                Some(task_id.clone()),
                "completed",
                Some(false),
                None,
            )?;
            neutralize_task_pointers_for_task(&repo_root, &task_id)?;
            // Auto-delete GOAL_STATE.json after successful completion (session-scoped goal)
            let goal_path = goal_state_path_for_task(&repo_root, &task_id)?;
            if goal_path.is_file()
                && let Err(e) = fs::remove_file(&goal_path) {
                    warn!("failed to remove completed GOAL_STATE.json: {e}");
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
    invalidate_route_records_cache_on_write();
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
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    let tx = crate::task_ledger::LedgerTransaction {
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        tx_type: "goal_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    crate::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
        .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
    invalidate_route_records_cache_on_write();
    let rfv_loop_superseded = deactivate_rfv_for_conflict_with_goal_drive(repo_root, &task_id)?;
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
        "rfv_loop_superseded": rfv_loop_superseded,
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
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    let tx = crate::task_ledger::LedgerTransaction {
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        tx_type: "goal_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    crate::task_ledger::append_transaction_assuming_l1_held(repo_root, &task_id, tx)
        .map_err(|e| format!("TASK_LEDGER append failed: {e}"))?;
    invalidate_route_records_cache_on_write();
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    Ok(json!({
        "ok": true,
        "operation": status,
        "task_id": task_id,
    }))
}
