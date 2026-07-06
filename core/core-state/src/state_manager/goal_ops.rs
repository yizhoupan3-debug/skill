// Goal state management: start, checkpoint, pause, resume, complete, block, clear.
// Extracted from state_manager.rs during module split.

use crate::transition_validation::{TaskTransition, validate_transition};
use crate::utils::atomic_write::write_atomic_json;
use core_errors::FrameworkError;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use super::pointer_ops::{
    ensure_task_directory, neutralize_task_pointers_for_task, sync_task_pointers_after_goal_drive,
};
use super::{
    REQUIRES_COMPLETION_EVIDENCE_KEY, goal_state_path_for_task, read_goal_state,
};

/// 从 goal state 中剥离运行时注入的临时字段（stale/stale_reason），
/// 防止它们被持久化到 GOAL_STATE.json 中。
fn strip_stale_annotations(state: &mut Value) {
    if let Some(obj) = state.as_object_mut() {
        obj.remove("stale");
        obj.remove("stale_reason");
    }
}

/// Append a ledger transaction with bounded retries, then fall back to dirty-marking.
///
/// Used after `write_atomic_json` has already succeeded — if ledger append fails,
/// retry up to `MAX_LEDGER_RETRIES` times with a short sleep. If all retries fail,
/// mark the GOAL_STATE as `_dirty` and re-write for recovery on next hydrate.
///
/// Returns `Ok(())` in all cases (best-effort): the dirty flag ensures eventual consistency.
fn append_transaction_with_retry(
    repo_root: &Path,
    task_id: &str,
    tx: crate::task_ledger::LedgerTransaction,
    goal_path: &Path,
    state: &mut Value,
    context: &str,
) {
    const MAX_LEDGER_RETRIES: u32 = 3;
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

    let mut last_err = None;
    for attempt in 0..MAX_LEDGER_RETRIES {
        match crate::task_ledger::append_transaction_assuming_l1_held(repo_root, task_id, tx.clone()) {
            Ok(()) => {
                if attempt > 0 {
                    tracing::info!(
                        task_id = %task_id,
                        attempt = attempt + 1,
                        context = context,
                        "TASK_LEDGER append succeeded after retry"
                    );
                }
                return;
            }
            Err(e) => {
                if attempt + 1 < MAX_LEDGER_RETRIES {
                    tracing::warn!(
                        task_id = %task_id,
                        attempt = attempt + 1,
                        error = %e,
                        context = context,
                        "TASK_LEDGER append failed — retrying"
                    );
                    std::thread::sleep(RETRY_DELAY);
                }
                last_err = Some(e);
            }
        }
    }

    // All retries exhausted — mark dirty for recovery on next hydrate.
    let e = last_err.unwrap();
    tracing::error!(
        task_id = %task_id,
        error = %e,
        context = context,
        retries = MAX_LEDGER_RETRIES,
        "CRITICAL: GOAL_STATE written but TASK_LEDGER append failed after retries — \
         setting _dirty flag for next hydrate cycle"
    );
    if let Some(obj) = state.as_object_mut() {
        obj.insert("_dirty".to_string(), json!(true));
        obj.insert("_dirty_reason".to_string(), json!(format!(
            "ledger append failed after {MAX_LEDGER_RETRIES} retries ({context}): {e}"
        )));
    }
    let _ = write_atomic_json(goal_path, state);
}

/// Commit a goal mutation: strip stale annotations → write GOAL_STATE.json → append ledger.
///
/// This is the standard write path for all goal mutation operations. It replaces the
/// previously duplicated 3-step pattern (strip → write → ledger) found in 10+ locations.
///
/// Returns the JSON response on success, or propagates the first error.
fn commit_goal_mutation(
    repo_root: &Path,
    task_id: &str,
    state: &mut Value,
    tx_label: &str,
) -> Result<(), FrameworkError> {
    strip_stale_annotations(state);
    let path = goal_state_path_for_task(repo_root, task_id)?;
    write_atomic_json(&path, state)?;
    let tx = crate::task_ledger::LedgerTransaction::new(tx_label, state.clone())
        .with_schema_version(1);
    append_transaction_with_retry(repo_root, task_id, tx, &path, state, tx_label);
    Ok(())
}

/// Transition a goal to review_pending with blockers, commit, and return a structured response.
///
/// Replaces the 3 near-identical blocks in the complete path (max_iterations, QG blocked,
/// QG hook error).
fn transition_to_review_pending(
    repo_root: &Path,
    task_id: &str,
    state: &mut Value,
    blockers: Vec<Value>,
    operation_label: &str,
    extra_fields: Vec<(String, Value)>,
) -> Result<Value, FrameworkError> {
    if let Some(obj) = state.as_object_mut() {
        obj.insert("status".to_string(), json!("review_pending"));
        obj.insert("blockers".to_string(), json!(blockers));
        obj.insert(
            "updated_at".to_string(),
            json!(framework_core::time::now_iso()),
        );
    }
    commit_goal_mutation(repo_root, task_id, state, &format!("goal_{operation_label}"))?;
    let mut response = json!({
        "ok": true,
        "operation": operation_label,
        "task_id": task_id,
        "status": "review_pending",
        "blockers": state.get("blockers"),
    });
    for (key, val) in extra_fields {
        response[key] = val;
    }
    Ok(response)
}

/// Check whether a goal status transition is allowed by the state machine.
/// Returns true only for transitions explicitly listed in the matrix.
fn is_valid_goal_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        // Operational transitions
        ("running", "paused")
            | ("running", "blocked")
            | ("running", "review_pending")
            | ("running", "completed")
            | ("running", "failed")
        // Recovery transitions
            | ("paused", "running")
            | ("blocked", "running")
            | ("review_pending", "running")
        // Terminal transitions (one-way)
            | ("paused", "failed")
            | ("blocked", "failed")
            | ("review_pending", "failed")
    )
}


/// Merge or replace array fields during amend (GOAL-009).
/// When `merge` is true, new items are appended to the existing array.
/// When `merge` is false (default), the existing array is replaced entirely.
fn merge_or_replace_array(map: &mut Map<String, Value>, key: &str, new_items: &[Value], merge: bool) {
    if merge {
        if new_items.is_empty() {
            return; // nothing to merge
        }
        if let Some(existing) = map.get_mut(key).and_then(|v| v.as_array_mut()) {
            // P3-008: Deduplicate on merge — only append items not already present
            for item in new_items.iter() {
                if !existing.contains(item) {
                    existing.push(item.clone());
                }
            }
        } else {
            map.insert(key.to_string(), json!(new_items));
        }
    } else {
        map.insert(key.to_string(), json!(new_items));
    }
}

fn resolve_task_id_strict(payload: &Value) -> Result<String, FrameworkError> {
    payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            FrameworkError::validation(
                "goal_state_manage: task_id is required in payload (multi-agent safe mode)",
            )
        })
}

fn resolve_framework_goal_drive_repo(payload: &Value) -> Result<PathBuf, FrameworkError> {
    let repo_root = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| FrameworkError::validation("framework_goal_drive requires repo_root"))?;
    if !repo_root.is_dir() {
        return Err(FrameworkError::not_found(format!(
            "framework_goal_drive: repo_root is not a directory: {}",
            repo_root.display()
        )));
    }
    Ok(repo_root.to_path_buf())
}

/// stdio / CLI：`framework_goal_drive`
pub fn framework_goal_drive(payload: Value) -> Result<Value, FrameworkError> {
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
    m.insert("checkpoints".to_string(), json!([]));
    m.insert("blocker".to_string(), Value::Null);
    m.insert(
        "updated_at".to_string(),
        json!(framework_core::time::now_iso()),
    );
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
            } else {
                v.as_str().map(|s| vec![json!(s)])
            }
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
) -> Result<(), FrameworkError> {
    if !drive_until_done {
        return Ok(());
    }
    if count_nonempty_string_items(non_goals) == 0 {
        return Err(FrameworkError::validation(format!(
            "framework_goal_drive {context}: drive_until_done goal requires non-empty non_goals"
        )));
    }
    if count_nonempty_string_items(done_when) < 2 {
        return Err(FrameworkError::validation(format!(
            "framework_goal_drive {context}: drive_until_done goal requires >=2 done_when items"
        )));
    }
    if count_nonempty_string_items(validation_commands) == 0 {
        return Err(FrameworkError::validation(format!(
            "framework_goal_drive {context}: drive_until_done goal requires non-empty validation_commands"
        )));
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

fn apply_optional_goal_fields_from_payload(
    obj: &mut Map<String, Value>,
    payload: &Value,
) -> Result<(), FrameworkError> {
    if let Some(st) = payload
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("status".to_string(), json!(st));
    }
    crate::goal_prediction::merge_prediction_from_payload(obj, payload);
    Ok(())
}

/// Verify the integrity hash chain of evidence artifacts (EV-F007).
/// Logs a warning if the chain is broken but does not block.
fn verify_evidence_chain_integrity(artifacts: &[Value]) {
    let mut prev_hash = "genesis";
    for (idx, entry) in artifacts.iter().enumerate() {
        let stored_hash = entry.get("chain_hash").and_then(Value::as_str).unwrap_or("");
        if stored_hash.is_empty() {
            // Pre-chain entries (before EV-F007) are acceptable.
            prev_hash = "";
            continue;
        }
        if prev_hash.is_empty() {
            // Previous entry had no hash — chain starts here.
            prev_hash = stored_hash;
            continue;
        }
        // We can't recompute the full hash without the original content,
        // but we can check that consecutive hashes are non-empty and distinct
        // (a basic sanity check). Full verification would require storing the
        // content alongside, which is too expensive for the read path.
        if stored_hash == prev_hash {
            tracing::warn!(
                index = idx,
                hash = %stored_hash,
                "evidence chain integrity: duplicate hash at index {idx} — possible tampering"
            );
        }
        prev_hash = stored_hash;
    }
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
    let artifacts = val.get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // EV-F007: Verify evidence chain integrity (advisory — warn but don't block).
    verify_evidence_chain_integrity(&artifacts);
    artifacts
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
/// 与 `quality_gate_loop` 共用一份口径。
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

fn framework_goal_drive_impl(payload: Value) -> Result<Value, FrameworkError> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| FrameworkError::validation("framework_goal_drive requires repo_root"))?;
    if !repo_root.is_dir() {
        return Err(FrameworkError::not_found(format!(
            "framework_goal_drive: repo_root is not a directory: {}",
            repo_root.display()
        )));
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
            let goal_state_summary = match state {
                Some(ref s) => json!({
                    "status": s.get("status"),
                    "goal": s.get("goal").and_then(|g| g.as_str()).map(|g| {
                        if g.len() > 120 { format!("{}…", &g[..g.floor_char_boundary(120)]) } else { g.to_string() }
                    }),
                    "done_when_count": s.get("done_when").and_then(|d| d.as_array()).map(|a| a.len()),
                    "checkpoint_count": s.get("checkpoints").and_then(|c| c.as_array()).map(|a| a.len()),
                    "blocker": s.get("blocker"),
                }),
                None => json!(null),
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "goal_state_path": path.display().to_string(),
                "goal_state": goal_state_summary,
            }))
        }
        "start" | "upsert" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;

            // Idempotent start guard: reject if GOAL_STATE.json already exists.
            let existing = read_goal_state(&repo_root, Some(&task_id))?;
            if existing.is_some() {
                return Err(FrameworkError::validation(format!(
                    "goal already exists for task '{task_id}' — use amend or clear to modify"
                )));
            }

            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    FrameworkError::validation("framework_goal_drive start requires non-empty goal")
                })?;
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
            validate_drive_contract(
                drive_until_done,
                &non_goals,
                &done_when,
                &validation_commands,
                "start",
            )?;

            let session_id = resolve_session_id(&payload);
            let mut obj = base_goal_object(
                goal.to_string(),
                non_goals,
                done_when,
                validation_commands,
                drive_until_done,
                requires_completion_evidence,
                session_id,
            );
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
            }
            if let Some(cg) = payload.get("completion_gates")
                && !cg.is_null()
            {
                obj.insert("completion_gates".to_string(), cg.clone());
            }
            apply_optional_goal_fields_from_payload(&mut obj, &payload)?;
            ensure_task_directory(&repo_root, &task_id)?;
            let mut value = Value::Object(obj);
            commit_goal_mutation(&repo_root, &task_id, &mut value, "goal_state")?;
            // Pointer sync is best-effort: goal state already committed.
            // Log warning on failure but don't block — subsequent operations
            // can fall back to read_goal_state or discover_goal_state.
            if let Err(e) = sync_task_pointers_after_goal_drive(&repo_root, &task_id, goal, &payload) {
                tracing::warn!(
                    task_id = %task_id,
                    error = %e,
                    "goal start: TASK_POINTERS sync failed (non-fatal) — goal state committed"
                );
            }
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "status": "running",
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
                    FrameworkError::validation(
                        "framework_goal_drive checkpoint requires non-empty note",
                    )
                })?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?.ok_or_else(|| {
                FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
            })?;

            // GOAL-005: Stale guard — cannot checkpoint a stale goal
            if state.get("stale").and_then(Value::as_bool) == Some(true) {
                return Err(FrameworkError::validation(
                    "cannot checkpoint a stale goal — session_id does not match current session",
                ));
            }

            // GOAL-012: Checkpoint size limit
            let max_checkpoints = 100u64;
            if let Some(cps) = state.get("checkpoints").and_then(Value::as_array) {
                if cps.len() as u64 >= max_checkpoints {
                    return Err(FrameworkError::validation(format!(
                        "checkpoint limit reached ({max_checkpoints} max)"
                    )));
                }
            }

            // T-F-005: Enforce checkpoint note length limit to prevent GOAL_STATE.json bloat.
            // Use char-aware truncation to avoid panicking on multi-byte UTF-8 boundaries (e.g. CJK).
            const MAX_CHECKPOINT_NOTE_LEN: usize = 2048;
            let note_clamped = if note.len() > MAX_CHECKPOINT_NOTE_LEN {
                tracing::warn!(
                    note_len = note.len(),
                    max = MAX_CHECKPOINT_NOTE_LEN,
                    "checkpoint note exceeds length limit — clamping"
                );
                // Find a valid char boundary at or before MAX_CHECKPOINT_NOTE_LEN
                let mut idx = MAX_CHECKPOINT_NOTE_LEN;
                while idx > 0 && !note.is_char_boundary(idx) {
                    idx -= 1;
                }
                &note[..idx]
            } else {
                note
            };

            let arr = state
                .as_object_mut()
                .and_then(|o| o.get_mut("checkpoints"))
                .and_then(|c| c.as_array_mut())
                .ok_or_else(|| FrameworkError::validation("GOAL_STATE.checkpoints corrupt"))?;
            arr.push(json!({
                "at": framework_core::time::now_iso(),
                "note": note_clamped,
                "type": payload.get("checkpoint_type").and_then(Value::as_str).unwrap_or("milestone"),
                "done_when_covers": payload.get("done_when_covers").cloned().unwrap_or(json!([])),
                "evidence_refs": payload.get("evidence_refs").cloned().unwrap_or(json!([])),
            }));
            if let Some(o) = state.as_object_mut() {
                o.insert(
                    "updated_at".to_string(),
                    json!(framework_core::time::now_iso()),
                );
                crate::goal_prediction::merge_prediction_from_payload(o, &payload);
            }
            commit_goal_mutation(&repo_root, &task_id, &mut state, "goal_state")?;
            Ok(json!({
                "ok": true,
                "operation": "checkpoint",
                "task_id": task_id,
            }))
        }
        "fail" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?.ok_or_else(|| {
                FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
            })?;
            // Stale guard
            if state.get("stale").and_then(Value::as_bool) == Some(true) {
                return Err(FrameworkError::validation(
                    "cannot fail a stale goal — session_id does not match current session",
                ));
            }
            // Cannot fail already-terminal goals
            let current_status = state.get("status").and_then(Value::as_str).unwrap_or("");
            if current_status == "completed" || current_status == "failed" {
                return Err(FrameworkError::validation(format!(
                    "goal is already in '{current_status}' status — cannot fail"
                )));
            }
            let obj = state
                .as_object_mut()
                .ok_or_else(|| FrameworkError::validation("GOAL_STATE root must be object"))?;
            obj.insert("status".to_string(), json!("failed"));
            obj.insert(
                "failed_at".to_string(),
                json!(framework_core::time::now_iso()),
            );
            obj.insert(
                "updated_at".to_string(),
                json!(framework_core::time::now_iso()),
            );
            // Record failure reason if provided
            if let Some(reason) = payload.get("reason").and_then(Value::as_str) {
                obj.insert("failure_reason".to_string(), json!(reason));
            }
            commit_goal_mutation(&repo_root, &task_id, &mut state, "goal_failed")?;
            // Neutralize pointers since the goal is now terminal
            neutralize_task_pointers_for_task(&repo_root, &task_id)?;

            // A5: Advisory warning when max_iterations reached but caller
            // explicitly failed the goal (not a blocking guard — fail is user-intent).
            let max_iter_warning = state
                .get("max_iterations")
                .and_then(Value::as_u64)
                .map(|max| {
                    let current = state.get("iteration_count").and_then(Value::as_u64).unwrap_or(0);
                    if current + 1 >= max {
                        Some(format!(
                            "max_iterations ({max}) reached at iteration_count={current}; use clear + restart to continue"
                        ))
                    } else {
                        None
                    }
                })
                .flatten();

            let mut resp = json!({
                "ok": true,
                "operation": "fail",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
            });
            if let Some(ref warning) = max_iter_warning {
                resp["warning"] = json!(warning);
            }
            Ok(resp)
        }
        "pause" => set_terminal_flags(
            &repo_root,
            Some(resolve_task_id_strict(&payload)?),
            "paused",
            None, // Don't overwrite drive_until_done — preserve for resume
            None,
        ),
        "resume" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;

            // A6: When drive_until_done is not explicitly provided, preserve
            // the existing value from GOAL_STATE instead of defaulting to true.
            let _path = goal_state_path_for_task(&repo_root, &task_id)?;
            let existing_state = read_goal_state(&repo_root, Some(&task_id))?.unwrap_or_default();
            let existing_drive = existing_state
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let drive_until_done = payload
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(existing_drive);
            resume_goal_running(
                &repo_root,
                Some(task_id),
                drive_until_done,
                &payload,
            )
        }
        "complete" => {
            let task_id = resolve_task_id_strict(&payload)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?.ok_or_else(|| {
                FrameworkError::validation("GOAL_STATE missing for completion gate check")
            })?;

            // GOAL-002: Status guard
            let status = state.get("status").and_then(Value::as_str).unwrap_or("");
            if status != "running" && status != "review_pending" {
                return Err(FrameworkError::validation(format!(
                    "cannot complete a goal in '{status}' status — must be 'running' or 'review_pending'"
                )));
            }

            // GOAL-002: Stale guard (check before stripping)
            if state.get("stale").and_then(Value::as_bool) == Some(true) {
                return Err(FrameworkError::validation(
                    "cannot complete a stale goal — session_id does not match current session",
                ));
            }

            // Phase B: validate_transition is the authoritative blocking anti-fraud gate.
            // Only applies to goals that require completion evidence (drive_until_done,
            // validation_commands, done_when, or explicit requires_completion_evidence).
            if goal_requires_completion_evidence(&state) {
                let transition_v =
                    validate_transition(&repo_root, &task_id, TaskTransition::Complete);
                if !transition_v.passed {
                    return Err(FrameworkError::validation(format!(
                        "validate_transition blocked: {}",
                        transition_v.reason
                    )));
                }
            }

            if let Some(gates) = crate::task_state::parse_goal_completion_gates(&state) {
                let view = crate::task_state::resolve_task_view(&repo_root, Some(task_id.as_str()));
                crate::task_state::validate_goal_completion_gates(&view, &gates)?;
            }

            // ── GOAL-008: max_iterations check (before QG) ──
            // Check BEFORE the QG gate so that iteration limits are not masked
            // by a consistently-blocking QG (P1-008).
            if let Some(max_iter) = state.get("max_iterations").and_then(Value::as_u64) {
                let current = state.get("iteration_count").and_then(Value::as_u64).unwrap_or(0);
                let next_count = current + 1;
                if next_count >= max_iter {
                    // Increment iteration_count so that retry→complete will see
                    // next_count >= max_iter and pass through (P1-004 livelock fix).
                    if let Some(obj) = state.as_object_mut() {
                        obj.insert("iteration_count".to_string(), json!(next_count));
                    }
                    let blockers = vec![json!({
                        "finding": "max_iterations reached",
                        "severity": "info",
                    })];
                    return transition_to_review_pending(
                        &repo_root, &task_id, &mut state, blockers,
                        "max_iterations_reached", vec![],
                    );
                }
            }

            // ── D4/D9: Auto-trigger QGEntry on goal complete ──
            // Two-stage exit gate: Stage 1 anti-fraud + Stage 2 scene-dispatched checker chain.
            // If the QG gate blocks, transition to review_pending instead of completing the iteration.
            if let Some(hooks) = framework_core::runtime_hooks::try_hooks() {
                let goal_text = state.get("goal").and_then(Value::as_str).unwrap_or("");
                let round = state
                    .get("iteration_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let goal_scene = state
                    .get("scene")
                    .and_then(Value::as_str)
                    .unwrap_or("general");
                let qg_payload = serde_json::json!({
                    "repo_root": repo_root.to_string_lossy().to_string(),
                    "task_id": task_id,
                    "scene": goal_scene,
                    "goal": goal_text,
                    "round": round + 1,
                });
                match hooks.evaluate_quality_gate(qg_payload) {
                    Ok(verdict) => {
                        let passed = verdict
                            .get("passed")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        if !passed {
                            let blockers: Vec<Value> = verdict
                                .get("blockers")
                                .cloned()
                                .unwrap_or(json!([]))
                                .as_array()
                                .cloned()
                                .unwrap_or_default();
                            return transition_to_review_pending(
                                &repo_root, &task_id, &mut state, blockers,
                                "quality_gate_blocked",
                                vec![("reason".to_string(), verdict.get("reason").cloned().unwrap_or(Value::Null))],
                            );
                        }
                    }
                    Err(e) => {
                        // P1-007 / F9: QG hook error → degrade to review_pending
                        // (consistent with runner behavior). QG failure means
                        // "needs review", not "system error".
                        tracing::error!(
                            "QG auto-trigger hook error: {e} — degrading to review_pending (fail-closed)"
                        );
                        let blockers = vec![json!({"id": "qg_hook_error", "description": format!("{e}")})];
                        return transition_to_review_pending(
                            &repo_root, &task_id, &mut state, blockers,
                            "quality_gate_blocked",
                            vec![("reason".to_string(), json!(format!("quality gate hook error: {e}")))],
                        );
                    }
                }
            } else {
                // P1-003: hooks not available — log warning but continue (fail-open by design)
                tracing::warn!(
                    "QG auto-trigger: RuntimeCoreHooks not registered — quality gate skipped. \
                     QG checkers will NOT run for this complete. \
                     Set up hooks in RUNTIME_REGISTRY.json to enable QG evaluation."
                );
            }

            // Complete = iteration complete, NOT goal termination.
            // Keep status=running, do NOT archive or neutralize pointers.

            // P3-011: Warn on rapid consecutive completes (same timestamp)
            if let Some(last) = state.get("last_iteration_completed_at").and_then(Value::as_str) {
                if last == framework_core::time::now_iso() {
                    tracing::warn!(
                        "rapid consecutive complete detected for task '{task_id}'                          — last_iteration_completed_at is identical; iteration_count may inflate"
                    );
                }
            }

            let mut loop_state = state; // reuse the single read from above
            if let Some(obj) = loop_state.as_object_mut() {
                let count = obj
                    .get("iteration_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                obj.insert("iteration_count".to_string(), json!(count + 1));
                obj.insert(
                    "last_iteration_completed_at".to_string(),
                    json!(framework_core::time::now_iso()),
                );
                obj.insert(
                    "updated_at".to_string(),
                    json!(framework_core::time::now_iso()),
                );
            }
            commit_goal_mutation(&repo_root, &task_id, &mut loop_state, "goal_iteration_completed")?;

            // Enrich response with loop semantics context for model awareness
            let iteration_count = loop_state.get("iteration_count").and_then(Value::as_u64).unwrap_or(0);
            let drive_until_done = loop_state.get("drive_until_done").and_then(Value::as_bool).unwrap_or(false);
            let done_when = loop_state.get("done_when").cloned().unwrap_or(json!([]));
            let status = loop_state.get("status").and_then(Value::as_str).unwrap_or("running");

            let mut response = json!({
                "ok": true,
                "operation": "iteration_completed",
                "task_id": task_id,
                "iteration_count": iteration_count,
                "status": status,
                "drive_until_done": drive_until_done,
                "done_when": done_when,
            });

            // Loop continuation hint: guide the model to keep driving
            if drive_until_done && status == "running" {
                response["next_action"] = json!("continue");
            }

            Ok(response)
        }
        "retry" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?.ok_or_else(|| {
                FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
            })?;

            // GOAL-005: Stale guard
            if state.get("stale").and_then(Value::as_bool) == Some(true) {
                return Err(FrameworkError::validation(
                    "cannot retry a stale goal — session_id does not match current session",
                ));
            }

            let obj = state
                .as_object_mut()
                .ok_or_else(|| FrameworkError::validation("GOAL_STATE root must be object"))?;
            let current_status = obj.get("status").and_then(Value::as_str).unwrap_or("");
            if current_status != "review_pending" {
                return Err(FrameworkError::validation(format!(
                    "cannot retry a goal in '{current_status}' status — must be 'review_pending'"
                )));
            }
            obj.insert("status".to_string(), json!("running"));
            obj.insert("blockers".to_string(), Value::Null);
            obj.insert(
                "updated_at".to_string(),
                json!(framework_core::time::now_iso()),
            );
            commit_goal_mutation(&repo_root, &task_id, &mut state, "goal_state")?;
            let goal_label = state
                .get("goal")
                .and_then(Value::as_str)
                .unwrap_or(task_id.as_str());
            // Pointer sync is best-effort (non-fatal).
            if let Err(e) = sync_task_pointers_after_goal_drive(&repo_root, &task_id, goal_label, &payload) {
                tracing::warn!(
                    task_id = %task_id,
                    error = %e,
                    "goal retry: TASK_POINTERS sync failed (non-fatal)"
                );
            }
            Ok(json!({
                "ok": true,
                "operation": "retry",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
            }))
        }
        "block" => {
            let blocker = payload
                .get("blocker")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    FrameworkError::validation(
                        "framework_goal_drive block requires non-empty blocker",
                    )
                })?;
            set_terminal_flags(
                &repo_root,
                Some(resolve_task_id_strict(&payload)?),
                "blocked",
                Some(false),
                Some(blocker.to_string()),
            )
        }
        "unblock" => set_terminal_flags(
            &repo_root,
            Some(resolve_task_id_strict(&payload)?),
            "running",
            None,
            None,
        ),
        "clear" => clear_goal_state(&repo_root, Some(resolve_task_id_strict(&payload)?), &payload),
        "amend" => {
            let task_id = resolve_task_id_strict(&payload)?;
            crate::utils::path_guard::validate_task_id_component(&task_id)?;
            let path = goal_state_path_for_task(&repo_root, &task_id)?;
            let mut state = read_goal_state(&repo_root, Some(&task_id))?.ok_or_else(|| {
                FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
            })?;
            let obj = state
                .as_object_mut()
                .ok_or_else(|| FrameworkError::validation("GOAL_STATE root must be object"))?;

            // Only mutable states can be amended.
            // 'completed' is reserved for future state machine use; currently unreachable
            // since loop semantics keep goals at "running" after iteration complete.
            let status = obj.get("status").and_then(Value::as_str).unwrap_or("");
            if status == "completed"
                || obj
                    .get("archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                return Err(FrameworkError::validation(
                    "framework_goal_drive amend: cannot amend a completed/archived goal",
                ));
            }
            // Stale goals from another session cannot be amended
            if obj.get("stale").and_then(Value::as_bool).unwrap_or(false) {
                return Err(FrameworkError::validation(
                    "framework_goal_drive amend: cannot amend a stale goal (session_id mismatch)",
                ));
            }

            let keep_progress = payload
                .get("keep_progress")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            // GOAL-009: merge flag — when true, append to existing arrays instead of replacing
            let merge = payload.get("merge").and_then(Value::as_bool).unwrap_or(false);
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
                merge_or_replace_array(obj, "non_goals", &cleaned, merge);
                has_amend = true;
            }
            if let Some(arr) = payload.get("done_when").and_then(Value::as_array) {
                let cleaned: Vec<Value> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| json!(s))
                    .collect();
                merge_or_replace_array(obj, "done_when", &cleaned, merge);
                has_amend = true;
            }
            if let Some(arr) = payload.get("validation_commands").and_then(Value::as_array) {
                let cleaned: Vec<Value> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| json!(s))
                    .collect();
                merge_or_replace_array(obj, "validation_commands", &cleaned, merge);
                has_amend = true;
            }

            // P3-014: Amend metadata and completion_gates
            if let Some(cg) = payload.get("completion_gates")
                && !cg.is_null()
            {
                obj.insert("completion_gates".to_string(), cg.clone());
                has_amend = true;
            }
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
                has_amend = true;
            }

            // P2-020: Amend drive_until_done field
            if let Some(dud) = payload.get("drive_until_done").and_then(Value::as_bool) {
                obj.insert("drive_until_done".to_string(), json!(dud));
                // Recompute requires_completion_evidence when drive_until_done changes:
                // if drive_until_done is now true, evidence is required.
                if dud {
                    obj.insert(REQUIRES_COMPLETION_EVIDENCE_KEY.to_string(), json!(true));
                }
                has_amend = true;
            }

            // P2-017: Prevent contradiction — drive_until_done=true with requires_completion_evidence=false
            let current_drive = obj.get("drive_until_done").and_then(Value::as_bool).unwrap_or(false);
            if current_drive {
                let current_evidence = obj.get(REQUIRES_COMPLETION_EVIDENCE_KEY).and_then(Value::as_bool).unwrap_or(true);
                if !current_evidence {
                    return Err(FrameworkError::validation(format!(
                        "framework_goal_drive amend: drive_until_done=true requires {} to be true",
                        REQUIRES_COMPLETION_EVIDENCE_KEY,
                    )));
                }
            }

            if !keep_progress {
                obj.insert("checkpoints".to_string(), json!([]));
            }

            if !has_amend {
                return Err(FrameworkError::validation(
                    "framework_goal_drive amend requires at least one field to update: \
                     goal, non_goals, done_when, validation_commands, or drive_until_done",
                ));
            }

            // Amend drive-contract revalidation:
            // If the goal has drive_until_done=true, the resulting contract fields
            // must still satisfy the same completeness constraints as start.
            // This prevents amend from silently subcontracting a drive goal.
            let goal_drive = obj
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let amend_non_goals = obj
                .get("non_goals")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let amend_done_when = obj
                .get("done_when")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let amend_validation = obj
                .get("validation_commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            validate_drive_contract(
                goal_drive,
                &amend_non_goals,
                &amend_done_when,
                &amend_validation,
                "amend",
            )?;

            obj.insert(
                "amended_at".to_string(),
                json!(framework_core::time::now_iso()),
            );
            obj.insert(
                "updated_at".to_string(),
                json!(framework_core::time::now_iso()),
            );

            commit_goal_mutation(&repo_root, &task_id, &mut state, "goal_state")?;
            Ok(json!({
                "ok": true,
                "operation": "amend",
                "task_id": task_id,
            }))
        }
        _ => Err(FrameworkError::validation(format!(
            "framework_goal_drive: unknown operation '{operation}'"
        ))),
    }
}

fn clear_goal_state(
    repo_root: &Path,
    task_id_resolved: Option<String>,
    payload: &Value,
) -> Result<Value, FrameworkError> {
    let task_id = task_id_resolved.ok_or_else(|| {
        FrameworkError::validation("goal_state_manage: task_id is required (multi-agent safe mode)")
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let existed = path.is_file();

    // P2-015: Stale guard — check before clearing (consistent with all other mutation ops).
    // Stale goals owned by another session should not be silently cleared.
    if existed {
        if let Ok(Some(state)) = read_goal_state(repo_root, Some(&task_id)) {
            if state.get("stale").and_then(Value::as_bool) == Some(true) {
                // Stale guard: require force flag to clear stale goals
                let force = payload.get("force").and_then(Value::as_bool).unwrap_or(false);
                if !force {
                    return Err(FrameworkError::validation(
                        "cannot clear a stale goal — session_id does not match current session. \
                         Use force=true to override."
                    ));
                }
                tracing::warn!(
                    task_id = %task_id,
                    "clear_goal_state: stale goal cleared with force=true"
                );
            }
        }
    }

    if existed {
        fs::remove_file(&path)?;
        // T-F-004: Record goal_cleared in TASK_LEDGER for audit trail completeness.
        let clear_tx = crate::task_ledger::LedgerTransaction::new(
            "goal_cleared",
            json!({"task_id": task_id, "cleared_at": framework_core::time::now_iso()}),
        )
        .with_schema_version(1);
        if let Err(e) = crate::task_ledger::append_transaction_assuming_l1_held(
            repo_root, &task_id, clear_tx,
        ) {
            tracing::warn!(
                task_id = %task_id,
                error = %e,
                "failed to append goal_cleared transaction to TASK_LEDGER"
            );
        }
    }
    // Pointer cleanup is best-effort after goal state removal.
    if let Err(e) = neutralize_task_pointers_for_task(repo_root, &task_id) {
        tracing::warn!(
            task_id = %task_id,
            error = %e,
            "clear_goal_state: TASK_POINTERS cleanup failed (non-fatal)"
        );
    }
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
) -> Result<Value, FrameworkError> {
    let task_id = task_id_resolved.ok_or_else(|| {
        FrameworkError::validation("goal_state_manage: task_id is required (multi-agent safe mode)")
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let mut state = read_goal_state(repo_root, Some(&task_id))?.ok_or_else(|| {
        FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
    })?;

    // GOAL-005: Stale guard — cannot resume a stale goal
    if state.get("stale").and_then(Value::as_bool) == Some(true) {
        return Err(FrameworkError::validation(
            "cannot resume a stale goal — session_id does not match current session",
        ));
    }

    // P1-005: State guard — only allow resume on paused goals.
    // Reject resume on running, blocked, or review_pending goals.
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    if status == "running" {
        return Err(FrameworkError::validation(
            "cannot resume a goal already in 'running' status — goal is already active",
        ));
    }
    if status == "blocked" {
        return Err(FrameworkError::validation(
            "cannot resume a blocked goal — use unblock or clear instead",
        ));
    }
    if status == "review_pending" {
        return Err(FrameworkError::validation(
            "cannot resume a goal in 'review_pending' status — use retry instead",
        ));
    }

    let obj = state
        .as_object_mut()
        .ok_or_else(|| FrameworkError::validation("GOAL_STATE root must be object"))?;
    obj.insert("status".to_string(), json!("running"));
    obj.insert("drive_until_done".to_string(), json!(drive_until_done));
    obj.insert(
        "updated_at".to_string(),
        json!(framework_core::time::now_iso()),
    );

    // Resume drive-contract revalidation:
    // When drive_until_done is being set to true, the existing contract fields
    // must already satisfy the completeness constraints — otherwise resume
    // could silently elevate a lightweight goal into driving mode without
    // the required depth.
    let resume_non_goals = obj
        .get("non_goals")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let resume_done_when = obj
        .get("done_when")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let resume_validation = obj
        .get("validation_commands")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    validate_drive_contract(
        drive_until_done,
        &resume_non_goals,
        &resume_done_when,
        &resume_validation,
        "resume",
    )?;

    commit_goal_mutation(repo_root, &task_id, &mut state, "goal_state")?;
    let goal_label = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or(task_id.as_str());
    // Pointer sync is best-effort (non-fatal).
    if let Err(e) = sync_task_pointers_after_goal_drive(repo_root, &task_id, goal_label, payload) {
        tracing::warn!(
            task_id = %task_id,
            error = %e,
            "goal resume: TASK_POINTERS sync failed (non-fatal)"
        );
    }
    Ok(json!({
        "ok": true,
        "operation": "resume",
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "goal_state": state,
    }))
}

fn set_terminal_flags(
    repo_root: &Path,
    task_id_resolved: Option<String>,
    status: &str,
    drive_until_done: Option<bool>,
    blocker: Option<String>,
) -> Result<Value, FrameworkError> {
    let task_id = task_id_resolved.ok_or_else(|| {
        FrameworkError::validation("goal_state_manage: task_id is required (multi-agent safe mode)")
    })?;
    let path = goal_state_path_for_task(repo_root, &task_id)?;
    let mut state = read_goal_state(repo_root, Some(&task_id))?.ok_or_else(|| {
        FrameworkError::not_found(format!("GOAL_STATE missing at {}", path.display()))
    })?;

    // GOAL-005: Stale guard — cannot pause/block a stale goal
    if state.get("stale").and_then(Value::as_bool) == Some(true) {
        return Err(FrameworkError::validation(
            "cannot modify a stale goal — session_id does not match current session",
        ));
    }

    let obj = state
        .as_object_mut()
        .ok_or_else(|| FrameworkError::validation("GOAL_STATE root must be object"))?;

    let current = obj.get("status").and_then(Value::as_str).unwrap_or("");
    // T-SG-001: Validate transition against the formal state machine matrix.
    // This single check covers: invalid status, terminal states, archived, cross-state.
    if !is_valid_goal_transition(current, status) {
        return Err(FrameworkError::validation(format!(
            "goal transition '{current}' → '{status}' is not allowed by the state machine"
        )));
    }
    // Idempotent pause/block guard: reject same-state transition.
    if !current.is_empty() && current == status {
        return Err(FrameworkError::validation(format!(
            "goal is already in '{status}' state"
        )));
    }
    obj.insert("status".to_string(), json!(status));
    if let Some(d) = drive_until_done {
        obj.insert("drive_until_done".to_string(), json!(d));
    }
    match blocker {
        Some(b) => obj.insert("blocker".to_string(), json!(b)),
        None if status == "blocked" => None,
        None => obj.insert("blocker".to_string(), Value::Null),
    };
    obj.insert(
        "updated_at".to_string(),
        json!(framework_core::time::now_iso()),
    );
    commit_goal_mutation(repo_root, &task_id, &mut state, "goal_state")?;
    Ok(json!({
        "ok": true,
        "operation": status,
        "task_id": task_id,
    }))
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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
        let err = validate_drive_contract(
            true,
            &[],
            &[json!("d1"), json!("d2")],
            &[json!("cargo test")],
            "test",
        )
        .unwrap_err();
        assert!(err.to_string().contains("non_goals"), "{err}");
    }

    #[test]
    fn drive_contract_rejects_less_than_two_done_when() {
        let err = validate_drive_contract(
            true,
            &[json!("n1")],
            &[json!("d1")],
            &[json!("cargo test")],
            "test",
        )
        .unwrap_err();
        assert!(err.to_string().contains("done_when"), "{err}");
    }

    #[test]
    fn drive_contract_rejects_no_validation_commands() {
        let err = validate_drive_contract(
            true,
            &[json!("n1")],
            &[json!("d1"), json!("d2")],
            &[],
            "test",
        )
        .unwrap_err();
        assert!(err.to_string().contains("validation_commands"), "{err}");
    }

    #[test]
    fn drive_contract_accepts_minimally_valid() {
        assert!(
            validate_drive_contract(
                true,
                &[json!("n1")],
                &[json!("d1"), json!("d2")],
                &[json!("cargo test")],
                "test"
            )
            .is_ok()
        );
    }

    #[test]
    fn drive_contract_ignores_empty_string_items() {
        // Empty strings should not count toward the contract
        let err = validate_drive_contract(
            true,
            &[json!(""), json!("n1")],
            &[json!(""), json!("d1"), json!("d2")],
            &[json!("")],
            "test",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("validation_commands"),
            "empty validation cmd should not satisfy contract: {err}"
        );
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
        assert!(
            !goal_requires_completion_evidence(&state2),
            "explicit false is authoritative"
        );
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
        assert!(err.to_string().contains("non_goals"), "must reject: {err}");
        assert!(
            !repo
                .join("artifacts/current/t-bad/GOAL_STATE.json")
                .is_file()
        );
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
        assert!(
            repo.join("artifacts/current/t-simple/GOAL_STATE.json")
                .is_file()
        );
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
        assert!(err.to_string().contains("non-empty note"), "{err}");
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
        // drive_until_done is preserved (not overwritten to false) so resume can restore it
        assert_eq!(goal["drive_until_done"], json!(true));
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

        // resume — preserves the pause-set drive_until_done value (A6 fix)
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
        // A6: pause preserves drive_until_done, so resume sees the original value
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
        assert_eq!(out["operation"], json!("iteration_completed"));
        assert_eq!(out["iteration_count"], json!(1));

        // Goal stays running (not archived/neutralized) — loop semantics
        let raw = fs::read_to_string(repo.join("artifacts/current/t-ce/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["status"], json!("running"));
        assert_eq!(goal["iteration_count"], json!(1));
        assert!(
            goal.get("last_iteration_completed_at")
                .and_then(Value::as_str)
                .is_some()
        );
        assert!(
            goal.get("archived").is_none(),
            "goal must NOT be archived after iteration complete"
        );

        // Pointers NOT neutralized
        let (active, _focus) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert_eq!(
            active.as_deref(),
            Some("t-ce"),
            "active pointer must not be neutralized: {active:?}"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_complete_rejects_missing_evidence() {
        let repo = unique_repo("complete-no-ev");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        start_drive_goal(&repo, "t-cne");
        // Create TASK_POINTERS.json to satisfy D5 "task exists" check
        fs::write(
            repo.join("artifacts/current/t-cne/TASK_POINTERS.json"),
            r#"{"schema_version":"task-pointers-v1","task_id":"t-cne","entries":[]}"#,
        )
        .unwrap();

        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-cne",
        }))
        .unwrap_err();
        assert!(
            err.to_string().contains("validate_transition blocked"),
            "must reject without evidence: {err}"
        );
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
        assert!(err.to_string().contains("non-empty blocker"), "{err}");
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

        assert!(
            !repo
                .join("artifacts/current/t-cl/GOAL_STATE.json")
                .is_file(),
            "GOAL_STATE must be deleted"
        );

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
    fn goal_amend_succeeds_after_iteration_complete() {
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

        // Goals stay running after iteration complete — amend should succeed
        let amend = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "amend",
            "task_id": "t-ac",
            "goal": "revised after iteration",
        }))
        .expect("amend should succeed after iteration complete");
        assert_eq!(amend["ok"], json!(true));

        let raw = fs::read_to_string(repo.join("artifacts/current/t-ac/GOAL_STATE.json")).unwrap();
        let goal: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(goal["goal"], json!("revised after iteration"));
        assert_eq!(goal["status"], json!("running"));

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
        assert!(err.to_string().contains("at least one field"), "{err}");
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
            assert!(
                err.to_string().contains("safe path component"),
                "bad id {bad:?}: {err}"
            );
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
        assert!(
            goal.get("last_iteration_completed_at")
                .and_then(Value::as_str)
                .is_some()
        );
        assert_eq!(
            goal["status"],
            json!("running"),
            "loop goal must remain running after iteration complete"
        );
        assert!(
            goal.get("archived").is_none(),
            "loop goal must NOT be archived after iteration complete"
        );

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
        assert_eq!(
            active.as_deref(),
            Some("t-lp"),
            "loop goal should have active pointer"
        );

        // Complete iteration
        framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-lp",
        }))
        .expect("complete iteration");

        // Pointers must still reference the task (not neutralized)
        let (active2, _) = super::super::pointer_ops::read_task_pointer_pair(&repo);
        assert_eq!(
            active2.as_deref(),
            Some("t-lp"),
            "loop goal pointers must NOT be neutralized after iteration complete"
        );

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
        assert_eq!(
            st["status"],
            json!("running"),
            "loop goal status must remain running"
        );
        assert!(
            st.get("archived").is_none(),
            "loop goal must not have archived flag"
        );

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
                        "drive_until_done": true,
            "non_goals": ["n1"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["echo ok"],
        }))
        .expect("start loop drive");

        // Create TASK_POINTERS.json to satisfy D5 "task exists" check
        fs::write(
            repo.join("artifacts/current/t-lev/TASK_POINTERS.json"),
            r#"{"schema_version":"task-pointers-v1","task_id":"t-lev","entries":[]}"#,
        )
        .unwrap();

        // Without evidence, complete must be rejected
        let err = framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "t-lev",
        }))
        .unwrap_err();
        assert!(
            err.to_string().contains("validate_transition blocked"),
            "loop drive goal should require evidence: {err}"
        );

        let _ = fs::remove_dir_all(&repo);
    }
}
