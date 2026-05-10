//! Framework runtime view + continuity classification helpers.
//!
//! This module is intended to host the "runtime view" read-model builder and the
//! continuity classification logic extracted from `framework_runtime/mod.rs`.

use super::constants::*;
use super::types::*;

use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::ops::Not;
use std::path::{Path, PathBuf};

pub(super) fn load_framework_runtime_view(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> FrameworkRuntimeView {
    let mut control_plane_parse_errors: Vec<String> = Vec::new();
    let artifact_base =
        artifact_root_override.map_or_else(|| repo_root.join("artifacts"), Path::to_path_buf);
    let mirror_root = artifact_base.join(CURRENT_ARTIFACT_DIR);
    let supervisor_state_path = repo_root.join(SUPERVISOR_STATE_FILENAME);
    let supervisor_state = if supervisor_state_path.is_file() {
        normalize_supervisor_state(&read_json_control_plane_field(
            &supervisor_state_path,
            ".supervisor_state.json",
            &mut control_plane_parse_errors,
        ))
    } else {
        Map::new()
    };
    let active_task_pointer_path = mirror_root.join(ACTIVE_TASK_POINTER_NAME);
    let focus_task_pointer_path = mirror_root.join(FOCUS_TASK_POINTER_NAME);
    let task_registry_path = mirror_root.join(TASK_REGISTRY_NAME);
    let active_task_pointer_present = active_task_pointer_path.is_file();
    let focus_task_pointer_present = focus_task_pointer_path.is_file();
    let task_registry_present = task_registry_path.is_file();
    let pointer = read_json_control_plane_field(
        &active_task_pointer_path,
        "active_task.json",
        &mut control_plane_parse_errors,
    );
    let focus_pointer = read_json_control_plane_field(
        &focus_task_pointer_path,
        "focus_task.json",
        &mut control_plane_parse_errors,
    );
    let (registered_tasks, mut known_task_ids, mut recoverable_task_ids) =
        normalized_task_registry(&read_json_control_plane_field(
            &task_registry_path,
            "task_registry.json",
            &mut control_plane_parse_errors,
        ));
    let registry_task_ids_before_selection = known_task_ids.clone();
    let focus_task_id = {
        let direct = safe_slug(&value_text(focus_pointer.get("task_id")));
        if direct.is_empty().not() {
            Some(direct)
        } else {
            None
        }
    };
    let supervisor_task_id = safe_slug(&value_text(supervisor_state.get("task_id")));
    let pointer_task_id = safe_slug(&value_text(pointer.get("task_id")));
    let mut control_plane_inconsistency_reasons = control_plane_parse_errors;
    control_plane_inconsistency_reasons.extend(stable_line_items(vec![
        if !supervisor_task_id.is_empty()
            && focus_task_id
                .as_ref()
                .is_some_and(|task_id| task_id != &supervisor_task_id)
        {
            format!(
                "supervisor task_id '{supervisor_task_id}' disagrees with focus task pointer '{}'",
                focus_task_id.clone().unwrap_or_default()
            )
        } else {
            String::new()
        },
        if !supervisor_task_id.is_empty()
            && !pointer_task_id.is_empty()
            && supervisor_task_id != pointer_task_id
        {
            format!(
                "supervisor task_id '{supervisor_task_id}' disagrees with active task pointer '{pointer_task_id}'"
            )
        } else {
            String::new()
        },
        if focus_task_id
            .as_ref()
            .is_some_and(|task_id| !pointer_task_id.is_empty() && task_id != &pointer_task_id)
        {
            format!(
                "focus task pointer '{}' disagrees with active task pointer '{pointer_task_id}'",
                focus_task_id.clone().unwrap_or_default()
            )
        } else {
            String::new()
        },
    ]));
    let active_task_id = {
        let direct = safe_slug(task_id_override.unwrap_or(""));
        if direct.is_empty().not() {
            Some(direct)
        } else if let Some(focus_task_id) = focus_task_id.clone() {
            Some(focus_task_id)
        } else if pointer_task_id.is_empty().not() {
            Some(pointer_task_id.clone())
        } else {
            supervisor_task_id
                .is_empty()
                .not()
                .then_some(supervisor_task_id.clone())
        }
    };
    if task_id_override.is_none() {
        if let Some(task_id) = active_task_id.as_ref() {
            if task_registry_present
                && !registry_task_ids_before_selection
                    .iter()
                    .any(|existing| existing == task_id)
            {
                control_plane_inconsistency_reasons.push(format!(
                    "selected task_id '{task_id}' is missing from task_registry.json"
                ));
            }
        }
    }
    if let Some(task_id) = active_task_id.clone() {
        if !known_task_ids.iter().any(|existing| existing == &task_id) {
            known_task_ids.push(task_id.clone());
        }
        if supervisor_state
            .get("continuity")
            .and_then(Value::as_object)
            .and_then(|continuity| value_bool_or_none(continuity.get("resume_allowed")))
            == Some(true)
            && !recoverable_task_ids
                .iter()
                .any(|existing| existing == &task_id)
        {
            recoverable_task_ids.push(task_id);
        }
    }
    if let Some(task_id) = focus_task_id.clone() {
        if !known_task_ids.iter().any(|existing| existing == &task_id) {
            known_task_ids.push(task_id);
        }
    }
    let task_root = active_task_id
        .as_ref()
        .map_or_else(|| mirror_root.clone(), |task_id| mirror_root.join(task_id));
    let mirror_matches_selected = active_task_id
        .as_ref()
        .is_some_and(|task_id| task_id == &pointer_task_id);
    let preferred_root = if task_root.exists() {
        task_root.clone()
    } else if active_task_id.is_none() || mirror_matches_selected {
        mirror_root.clone()
    } else {
        task_root.clone()
    };
    let read_task_or_mirror = |file_name: &str| -> PathBuf {
        let preferred = preferred_root.join(file_name);
        if preferred.exists() {
            return preferred;
        }
        let mirror = mirror_root.join(file_name);
        if mirror.exists() {
            return mirror;
        }
        preferred
    };

    FrameworkRuntimeView {
        session_summary_text: read_text_if_exists(&read_task_or_mirror(SESSION_SUMMARY_FILENAME)),
        next_actions: read_json_if_exists(&read_task_or_mirror(NEXT_ACTIONS_FILENAME)),
        evidence_index: read_json_if_exists(&read_task_or_mirror(EVIDENCE_INDEX_FILENAME)),
        trace_metadata: read_json_if_exists(&read_task_or_mirror(TRACE_METADATA_FILENAME)),
        supervisor_state,
        routing_runtime_version: load_routing_runtime_version(repo_root),
        repo_root: repo_root.to_path_buf(),
        artifact_base,
        current_root: preferred_root,
        mirror_root,
        task_root,
        active_task_pointer_present,
        focus_task_pointer_present,
        task_registry_present,
        active_task_id,
        focus_task_id,
        control_plane_inconsistency_reasons,
        known_task_ids,
        recoverable_task_ids,
        registered_tasks,
        collected_at: current_local_timestamp(),
    }
}

pub(super) fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    let summary = parse_session_summary(&snapshot.session_summary_text);
    let supervisor = &snapshot.supervisor_state;
    let verification = object_field(supervisor, "verification");
    let continuity = object_field(supervisor, "continuity");
    let contract = supervisor_contract(supervisor);
    let trace_task = value_text(snapshot.trace_metadata.get("task"));
    let summary_task = value_text(summary.get("task"));
    let supervisor_task = first_nonempty(&[
        value_text(supervisor.get("task_summary")),
        value_text(supervisor.get("task_id")),
    ]);
    let task = first_nonempty(&[
        summary_task.clone(),
        trace_task.clone(),
        supervisor_task.clone(),
    ]);
    let summary_phase = value_text(summary.get("phase"));
    let supervisor_phase = value_text(supervisor.get("active_phase"));
    let verification_status = value_text(verification.get("verification_status"));
    let summary_status = value_text(summary.get("status"));
    let story_state = value_text(continuity.get("story_state"));
    let summary_terminal = is_terminal(&summary_phase, TERMINAL_PHASES)
        || is_terminal(&summary_status, TERMINAL_VERIFICATION_STATUSES);
    let supervisor_terminal = is_terminal(&supervisor_phase, TERMINAL_PHASES)
        || is_terminal(&verification_status, TERMINAL_VERIFICATION_STATUSES)
        || is_terminal(&story_state, TERMINAL_STORY_STATES);
    let supervisor_terminal_overrides_summary = supervisor_terminal
        && !summary_terminal
        && (summary_task.is_empty()
            || supervisor_task.is_empty()
            || looks_same_identity(&summary_task, &supervisor_task));
    let phase = if supervisor_terminal_overrides_summary {
        first_nonempty(&[supervisor_phase.clone(), summary_phase.clone()])
    } else {
        first_nonempty(&[summary_phase.clone(), supervisor_phase.clone()])
    };
    let status = if supervisor_terminal_overrides_summary {
        first_nonempty(&[
            verification_status.clone(),
            story_state.clone(),
            summary_status.clone(),
        ])
    } else {
        first_nonempty(&[
            summary_status.clone(),
            verification_status.clone(),
            story_state.clone(),
        ])
    };
    let authoritative_status = if status.is_empty() {
        synthesized_status(supervisor)
    } else {
        status.clone()
    };
    let next_actions = authoritative_next_actions(&snapshot.next_actions, supervisor);
    let route = authoritative_route(
        &snapshot.trace_metadata,
        supervisor,
        &task,
        &authoritative_status,
        snapshot.routing_runtime_version,
    );
    let blockers = supervisor
        .get("blockers")
        .and_then(Value::as_object)
        .and_then(|blockers| blockers.get("open_blockers"))
        .and_then(Value::as_array)
        .map(|items| {
            stable_line_items(
                items
                    .iter()
                    .map(|item| value_text(Some(item)))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    let scope = value_string_list(contract.get("scope"));
    let forbidden_scope = value_string_list(contract.get("forbidden_scope"));
    let acceptance_criteria = value_string_list(contract.get("acceptance_criteria"));
    let evidence_required = value_string_list(contract.get("evidence_required"));
    let evidence_count = normalize_evidence_index(&snapshot.evidence_index).len();
    let evidence_missing =
        evidence_count == 0 && (!evidence_required.is_empty() || !acceptance_criteria.is_empty());
    let missing_recovery_anchors = stable_line_items(vec![
        if snapshot.session_summary_text.trim().is_empty() {
            "SESSION_SUMMARY".to_string()
        } else {
            String::new()
        },
        if object_has_any_signal(&snapshot.next_actions).not() {
            "NEXT_ACTIONS".to_string()
        } else {
            String::new()
        },
        if object_has_any_signal(&snapshot.trace_metadata).not() {
            "TRACE_METADATA".to_string()
        } else {
            String::new()
        },
        if supervisor.is_empty() {
            "SUPERVISOR_STATE".to_string()
        } else {
            String::new()
        },
    ]);
    let terminal_reasons = terminal_continuity_reasons(
        &summary_phase,
        &summary_status,
        &supervisor_phase,
        &verification_status,
        &story_state,
    );
    let inconsistency_reasons = stable_line_items(vec![
        if !summary_task.is_empty()
            && !trace_task.is_empty()
            && !looks_same_identity(&summary_task, &trace_task)
        {
            format!(
                "session summary task '{summary_task}' disagrees with trace task '{trace_task}'"
            )
        } else {
            String::new()
        },
        if summary_terminal
            && !supervisor_terminal
            && (!supervisor_phase.is_empty() || !verification_status.is_empty())
        {
            "session summary marks the task terminal while supervisor still looks active"
                .to_string()
        } else {
            String::new()
        },
        if supervisor_terminal
            && !summary_terminal
            && (!summary_phase.is_empty() || !summary_status.is_empty())
            && !supervisor_terminal_overrides_summary
        {
            "supervisor marks the task terminal while the session summary still looks active"
                .to_string()
        } else {
            String::new()
        },
        if value_bool_or_none(continuity.get("resume_allowed")) == Some(true)
            && !terminal_reasons.is_empty()
        {
            "continuity.resume_allowed=true conflicts with terminal lifecycle metadata".to_string()
        } else {
            String::new()
        },
        join_lines(&snapshot.control_plane_inconsistency_reasons),
    ]);
    let stale_reasons = stale_continuity_reasons(
        StaleContinuityInputs {
            continuity: &continuity,
            story_state: &story_state,
            task: &task,
            supervisor_phase: &supervisor_phase,
            verification_status: &verification_status,
            next_actions: &next_actions,
            session_summary_missing: snapshot.session_summary_text.trim().is_empty(),
            terminal_reasons_empty: terminal_reasons.is_empty(),
        },
        Local::now().fixed_offset(),
    );
    let current_execution = json!({
        "task": task,
        "phase": phase,
        "status": if status.is_empty() && (!task.is_empty() || !next_actions.is_empty() || !blockers.is_empty()) {
            "in_progress".to_string()
        } else {
            status.clone()
        },
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
    });
    let recent_completed_execution = json!({
        "task": task,
        "phase": if phase.is_empty() {
            first_nonempty(&[story_state.clone(), supervisor_phase.clone()])
        } else {
            phase.clone()
        },
        "status": if status.is_empty() { "completed".to_string() } else { status.clone() },
        "route": route,
        "follow_up_notes": next_actions,
        "terminal_reasons": terminal_reasons,
    });
    let has_any_runtime_signal = !snapshot.session_summary_text.trim().is_empty()
        || object_has_any_signal(&snapshot.next_actions)
        || object_has_any_signal(&snapshot.evidence_index)
        || object_has_any_signal(&snapshot.trace_metadata)
        || !supervisor.is_empty();
    let missing_control_plane_anchors = missing_control_plane_anchors(snapshot);
    let has_missing_control_plane_anchors = !missing_control_plane_anchors.is_empty();
    let has_missing_recovery_anchors = !missing_recovery_anchors.is_empty();
    let state = if !has_any_runtime_signal
        || (task.is_empty() && (has_missing_recovery_anchors || has_missing_control_plane_anchors))
    {
        "missing"
    } else if !inconsistency_reasons.is_empty() {
        "inconsistent"
    } else if !terminal_reasons.is_empty() {
        "completed"
    } else if has_missing_recovery_anchors || has_missing_control_plane_anchors {
        "inconsistent"
    } else if !stale_reasons.is_empty() {
        "stale"
    } else {
        "active"
    };
    let can_resume = state == "active"
        && !has_missing_recovery_anchors
        && !has_missing_control_plane_anchors
        && !task.is_empty();
    let recovery_hints = match state {
        "missing" => json!([
            "Refresh SESSION_SUMMARY.md, NEXT_ACTIONS.json, TRACE_METADATA.json, and .supervisor_state.json before injecting continuity."
        ]),
        "completed" => json!([
            "Keep this task only as recent-completed context; do not inject it as current execution.",
            "Start a new standalone task before resuming related work."
        ]),
        "stale" => json!([
            "Re-read the live continuity artifacts and rebuild a fresh active task before injecting execution context.",
            "Do not continue from the stale snapshot without a new supervisor-owned continuity refresh."
        ]),
        "inconsistent" => json!([
            "Reconcile SESSION_SUMMARY.md, NEXT_ACTIONS.json, TRACE_METADATA.json, artifacts/current pointers, task_registry.json, and .supervisor_state.json before injecting continuity.",
            "Treat the current snapshot as blocked until the supervisor rewrites a consistent continuity bundle."
        ]),
        _ => json!([]),
    };
    json!({
        "state": state,
        "can_resume": can_resume,
        "task": task,
        "phase": phase,
        "status": status,
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "evidence_count": evidence_count,
        "evidence_missing": evidence_missing,
        "verification_status": if verification_status.is_empty() { Value::Null } else { Value::String(verification_status.clone()) },
        "missing_recovery_anchors": missing_recovery_anchors,
        "missing_control_plane_anchors": missing_control_plane_anchors,
        "current_execution": if state == "active" && !task.is_empty() { current_execution } else { Value::Null },
        "recent_completed_execution": if state == "completed" && !task.is_empty() { recent_completed_execution } else { Value::Null },
        "stale_reasons": stale_reasons,
        "terminal_reasons": terminal_reasons,
        "inconsistency_reasons": inconsistency_reasons,
        "recovery_hints": recovery_hints,
        "continuity": {
            "story_state": nonempty_string(Some(&Value::String(story_state))),
            "resume_allowed": value_bool_or_none(continuity.get("resume_allowed")),
            "last_updated_at": nonempty_string(continuity.get("last_updated_at")),
            "active_lease_expires_at": nonempty_string(continuity.get("active_lease_expires_at")),
            "state_reason": nonempty_string(continuity.get("state_reason")),
        },
        "summary_fields": summary,
        "paths": {
            "session_summary": snapshot.current_root.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": snapshot.current_root.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": snapshot.current_root.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": snapshot.current_root.join(TRACE_METADATA_FILENAME).display().to_string(),
            "task_root": snapshot.task_root.display().to_string(),
            "current_pointer_root": snapshot.mirror_root.display().to_string(),
            "supervisor_state": snapshot.repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
        }
    })
}

pub(super) fn missing_control_plane_anchors(snapshot: &FrameworkRuntimeView) -> Vec<String> {
    stable_line_items(vec![
        if snapshot.active_task_pointer_present {
            String::new()
        } else {
            ACTIVE_TASK_POINTER_NAME.to_string()
        },
        if snapshot.focus_task_pointer_present {
            String::new()
        } else {
            FOCUS_TASK_POINTER_NAME.to_string()
        },
        if snapshot.task_registry_present {
            String::new()
        } else {
            TASK_REGISTRY_NAME.to_string()
        },
        if snapshot.supervisor_state.is_empty() {
            SUPERVISOR_STATE_FILENAME.to_string()
        } else {
            String::new()
        },
    ])
}

pub(super) fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

pub(super) fn read_json_if_exists(path: &Path) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Map::new())),
        Err(_) => Value::Object(Map::new()),
    }
}

fn read_json_strict(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read json failed for {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse json failed for {}: {err}", path.display()))
}

pub(super) fn read_json_control_plane_field(
    path: &Path,
    label: &str,
    parse_errors: &mut Vec<String>,
) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match read_json_strict(path) {
        Ok(value) => value,
        Err(err) => {
            parse_errors.push(format!(
                "invalid control-plane json ({label}, {}): {err}",
                path.display()
            ));
            Value::Object(Map::new())
        }
    }
}

pub(super) fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn object_field(map: &Map<String, Value>, key: &str) -> Map<String, Value> {
    map.get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn terminal_continuity_reasons(
    summary_phase: &str,
    summary_status: &str,
    supervisor_phase: &str,
    verification_status: &str,
    story_state: &str,
) -> Vec<String> {
    stable_line_items(vec![
        terminal_reason("summary phase is terminal", summary_phase, TERMINAL_PHASES),
        terminal_reason(
            "summary status is terminal",
            summary_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason(
            "supervisor phase is terminal",
            supervisor_phase,
            TERMINAL_PHASES,
        ),
        terminal_reason(
            "verification status is terminal",
            verification_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason(
            "continuity story_state is terminal",
            story_state,
            TERMINAL_STORY_STATES,
        ),
    ])
}

fn stale_continuity_reasons(
    input: StaleContinuityInputs<'_>,
    now: DateTime<FixedOffset>,
) -> Vec<String> {
    let resume_allowed = value_bool_or_none(input.continuity.get("resume_allowed"));
    let state_reason = value_text(input.continuity.get("state_reason"));
    stable_line_items(vec![
        if is_terminal(input.story_state, STALE_STORY_STATES) {
            format!("continuity story_state is stale: {}", input.story_state)
        } else {
            String::new()
        },
        if resume_allowed == Some(false) && input.terminal_reasons_empty {
            "continuity explicitly disallows resume".to_string()
        } else {
            String::new()
        },
        match parse_iso_timestamp(input.continuity.get("active_lease_expires_at")) {
            Some(expires_at) if expires_at < now => format!(
                "active lease expired at {}",
                value_text(input.continuity.get("active_lease_expires_at"))
            ),
            _ => String::new(),
        },
        if input.session_summary_missing
            && input.terminal_reasons_empty
            && (!input.task.is_empty()
                || !input.supervisor_phase.is_empty()
                || !input.verification_status.is_empty()
                || !input.next_actions.is_empty())
        {
            "session summary mirror is missing while supervisor still looks active".to_string()
        } else {
            String::new()
        },
        if !state_reason.is_empty()
            && (is_terminal(input.story_state, STALE_STORY_STATES) || resume_allowed == Some(false))
        {
            format!("state reason: {state_reason}")
        } else {
            String::new()
        },
    ])
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn parse_session_summary(text: &str) -> Map<String, Value> {
    let mut result = Map::new();
    for line in text.lines() {
        if !line.starts_with("- ") {
            continue;
        }
        let body = &line[2..];
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        result.insert(
            key.trim().to_string(),
            Value::String(value.trim().to_string()),
        );
    }
    result
}

fn normalized_task_registry(payload: &Value) -> (Value, Vec<String>, Vec<String>) {
    let focus_task_id = safe_slug(&value_text(payload.get("focus_task_id")));
    normalize_task_registry_rows(focus_task_id, registry_rows_from_payload(payload))
}

fn registry_rows_from_payload(payload: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(items) = payload.get("tasks").and_then(Value::as_array) {
        for item in items {
            let Some(row) = item.as_object() else {
                continue;
            };
            let task_id = safe_slug(&value_text(row.get("task_id")));
            if task_id.is_empty() {
                continue;
            }
            let task = value_text(row.get("task"));
            let task_value = if task.is_empty() {
                Value::String(task_id.clone())
            } else {
                Value::String(task)
            };
            rows.push(json!({
                "task_id": task_id,
                "task": task_value,
                "updated_at": nonempty_string(row.get("updated_at")),
                "status": nonempty_string(row.get("status")),
                "phase": nonempty_string(row.get("phase")),
                "resume_allowed": value_bool_or_none(row.get("resume_allowed")),
            }));
        }
    }
    rows
}

fn normalize_task_registry_rows(
    focus_task_id: String,
    mut rows: Vec<Value>,
) -> (Value, Vec<String>, Vec<String>) {
    rows.sort_by(|left, right| {
        registry_task_sort_key(right)
            .cmp(&registry_task_sort_key(left))
            .then_with(|| value_text(right.get("task_id")).cmp(&value_text(left.get("task_id"))))
    });

    let mut seen = HashSet::new();
    let mut tasks = Vec::new();
    let mut known_task_ids = Vec::new();
    let mut recoverable_task_ids = Vec::new();
    let mut overflow_count = 0usize;
    for row in rows {
        let task_id = safe_slug(&value_text(row.get("task_id")));
        if task_id.is_empty() || !seen.insert(task_id.clone()) {
            continue;
        }
        if value_bool_or_none(row.get("resume_allowed")) == Some(true) {
            recoverable_task_ids.push(task_id.clone());
        }
        known_task_ids.push(task_id);
        if tasks.len() >= 128 {
            overflow_count += 1;
            continue;
        }
        tasks.push(row);
    }
    tasks.sort_by(|left, right| {
        let left_focus = value_text(left.get("task_id")) == focus_task_id;
        let right_focus = value_text(right.get("task_id")) == focus_task_id;
        right_focus
            .cmp(&left_focus)
            .then_with(|| registry_task_sort_key(right).cmp(&registry_task_sort_key(left)))
            .then_with(|| value_text(left.get("task_id")).cmp(&value_text(right.get("task_id"))))
    });
    (
        json!({
            "schema_version": TASK_REGISTRY_SCHEMA_VERSION,
            "focus_task_id": if focus_task_id.is_empty() { Value::Null } else { Value::String(focus_task_id) },
            "tasks": tasks,
            "task_count": known_task_ids.len(),
            "recoverable_task_count": recoverable_task_ids.len(),
            "truncated": overflow_count > 0,
            "overflow_count": overflow_count,
        }),
        known_task_ids,
        recoverable_task_ids,
    )
}

fn registry_task_sort_key(row: &Value) -> String {
    first_nonempty(&[
        value_text(row.get("updated_at")),
        value_text(row.get("task_id")),
    ])
}

fn normalize_supervisor_state(payload: &Value) -> Map<String, Value> {
    let source = payload.as_object().cloned().unwrap_or_default();
    let mut normalized = source.clone();
    normalized.insert(
        "schema_version".to_string(),
        Value::String(SUPERVISOR_STATE_SCHEMA_VERSION.to_string()),
    );

    let delegation = source
        .get("delegation")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            for key in [
                "delegation_plan_created",
                "spawn_attempted",
                "spawn_block_reason",
                "fallback_mode",
                "delegated_sidecars",
            ] {
                if let Some(value) = source.get(key) {
                    map.insert(key.to_string(), value.clone());
                }
            }
            map
        });
    normalized.insert("delegation".to_string(), Value::Object(delegation));

    let verification = source
        .get("verification")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            for key in ["verification_status", "last_verification_summary"] {
                if let Some(value) = source.get(key) {
                    map.insert(key.to_string(), value.clone());
                }
            }
            map
        });
    normalized.insert("verification".to_string(), Value::Object(verification));

    let mut continuity = source
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for key in [
        "story_state",
        "resume_allowed",
        "last_updated_at",
        "active_lease_expires_at",
        "state_reason",
    ] {
        if !continuity.contains_key(key) {
            if let Some(value) = source.get(key) {
                continuity.insert(key.to_string(), value.clone());
            }
        }
    }
    normalized.insert("continuity".to_string(), Value::Object(continuity));

    let blockers = source
        .get("blockers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            if let Some(value) = source.get("open_blockers") {
                map.insert("open_blockers".to_string(), value.clone());
            }
            map
        });
    normalized.insert("blockers".to_string(), Value::Object(blockers));
    normalized
}

fn normalize_evidence_index(payload: &Value) -> Vec<Map<String, Value>> {
    let items = if payload.get("schema_version").and_then(Value::as_str)
        == Some(EVIDENCE_INDEX_SCHEMA_VERSION)
    {
        payload.get("artifacts")
    } else {
        payload.get("artifacts").or_else(|| payload.get("evidence"))
    };
    items
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_object().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_next_actions(payload: &Value) -> Vec<String> {
    let actions = if payload.get("schema_version").and_then(Value::as_str)
        == Some(NEXT_ACTIONS_SCHEMA_VERSION)
    {
        payload.get("next_actions")
    } else {
        payload
            .get("next_actions")
            .or_else(|| payload.get("actions"))
    };
    actions
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(coerce_next_action_line)
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_trace_skills(payload: &Value) -> Vec<String> {
    let skills = if payload.get("schema_version").and_then(Value::as_str)
        == Some(TRACE_METADATA_SCHEMA_VERSION)
    {
        payload.get("matched_skills")
    } else {
        payload
            .get("matched_skills")
            .or_else(|| payload.get("skills"))
    };
    skills
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn coerce_next_action_line(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.trim().to_string();
    }
    if let Some(map) = value.as_object() {
        for key in ["title", "summary", "action", "label", "details"] {
            let text = value_text(map.get(key));
            if !text.is_empty() {
                return text;
            }
        }
    }
    value_text(Some(value))
}

fn authoritative_next_actions(
    snapshot_payload: &Value,
    supervisor_state: &Map<String, Value>,
) -> Vec<String> {
    let supervisor_actions = supervisor_state
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|rows| {
            stable_line_items(
                rows.iter()
                    .map(coerce_next_action_line)
                    .filter(|item| !item.is_empty())
                    .collect(),
            )
        })
        .unwrap_or_default();
    if supervisor_actions.is_empty() {
        normalize_next_actions(snapshot_payload)
    } else {
        supervisor_actions
    }
}

fn fallback_route_from_supervisor(supervisor_state: &Map<String, Value>) -> Vec<String> {
    let controller = supervisor_state
        .get("controller")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    stable_line_items(vec![
        value_text(controller.get("gate")),
        value_text(controller.get("primary_owner")),
        value_text(controller.get("overlay")),
        value_text(controller.get("owner_lane")),
        value_text(supervisor_state.get("primary_owner")),
    ])
}

fn trace_payload_identity_matches(
    payload: &Value,
    task: &str,
    status: &str,
    current_routing_runtime_version: u64,
) -> bool {
    if !payload.is_object() {
        return false;
    }
    let payload_task = value_text(payload.get("task"));
    if payload_task.is_empty() && !normalize_trace_skills(payload).is_empty() {
        return true;
    }
    let payload_status = value_text(payload.get("verification_status"));
    if !looks_same_identity(&payload_task, task) {
        return false;
    }
    if !payload_status.is_empty() && payload_status != status {
        return false;
    }
    if let Some(version) = payload
        .get("routing_runtime_version")
        .and_then(Value::as_u64)
    {
        if version != current_routing_runtime_version {
            return false;
        }
    }
    true
}

fn authoritative_route(
    trace_payload: &Value,
    supervisor_state: &Map<String, Value>,
    task: &str,
    status: &str,
    current_routing_runtime_version: u64,
) -> Vec<String> {
    if trace_payload_identity_matches(trace_payload, task, status, current_routing_runtime_version)
    {
        let route = normalize_trace_skills(trace_payload);
        if !route.is_empty() {
            return route;
        }
    }
    fallback_route_from_supervisor(supervisor_state)
}

fn supervisor_contract(state: &Map<String, Value>) -> Map<String, Value> {
    state
        .get("execution_contract")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn parse_iso_timestamp(value: Option<&Value>) -> Option<DateTime<FixedOffset>> {
    let text = value_text(value);
    if text.is_empty() {
        return None;
    }
    let normalized = if text.ends_with('Z') {
        format!("{}+00:00", &text[..text.len() - 1])
    } else {
        text
    };
    DateTime::parse_from_rfc3339(&normalized).ok()
}

fn load_routing_runtime_version(repo_root: &Path) -> u64 {
    let runtime_path = repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json");
    read_json_if_exists(&runtime_path)
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(1)
}

fn synthesized_status(supervisor_state: &Map<String, Value>) -> String {
    let verification = supervisor_state
        .get("verification")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let continuity = supervisor_state
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    first_nonempty(&[
        value_text(verification.get("verification_status")),
        value_text(continuity.get("story_state")),
        value_text(supervisor_state.get("active_phase")),
        "in_progress".to_string(),
    ])
}

fn object_has_any_signal(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Object(map) => !map.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn terminal_reason(prefix: &str, value: &str, terminal_values: &[&str]) -> String {
    if is_terminal(value, terminal_values) {
        format!("{prefix}: {value}")
    } else {
        String::new()
    }
}

fn is_terminal(value: &str, terminal_values: &[&str]) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    terminal_values
        .iter()
        .any(|candidate| lowered == *candidate)
}

fn looks_same_identity(left: &str, right: &str) -> bool {
    let left_token = safe_slug(&left.to_ascii_lowercase());
    let right_token = safe_slug(&right.to_ascii_lowercase());
    if left_token.is_empty() || right_token.is_empty() {
        return true;
    }
    left_token == right_token
        || left_token.contains(&right_token)
        || right_token.contains(&left_token)
}

fn join_lines(values: &[String]) -> String {
    values
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ")
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string()
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn nonempty_string(value: Option<&Value>) -> Option<String> {
    let text = value_text(value);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn value_bool_or_none(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn first_nonempty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn stable_line_items(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let value = item.trim().to_string();
        if value.is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        result.push(value);
    }
    result
}
