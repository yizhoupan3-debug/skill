use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION: &str =
    "router-rs-framework-runtime-snapshot-v1";
pub const FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION: &str =
    "router-rs-framework-contract-summary-v1";
pub const FRAMEWORK_RUNTIME_AUTHORITY: &str = "rust-framework-runtime-read-model";

const CURRENT_ARTIFACT_DIR: &str = "current";
const ACTIVE_TASK_POINTER_NAME: &str = "active_task.json";
const SESSION_SUMMARY_FILENAME: &str = "SESSION_SUMMARY.md";
const NEXT_ACTIONS_FILENAME: &str = "NEXT_ACTIONS.json";
const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
const TRACE_METADATA_FILENAME: &str = "TRACE_METADATA.json";
const SUPERVISOR_STATE_FILENAME: &str = ".supervisor_state.json";
const NEXT_ACTIONS_SCHEMA_VERSION: &str = "next-actions-v2";
const EVIDENCE_INDEX_SCHEMA_VERSION: &str = "evidence-index-v2";
const TRACE_METADATA_SCHEMA_VERSION: &str = "trace-metadata-v2";
const SUPERVISOR_STATE_SCHEMA_VERSION: &str = "supervisor-state-v2";
const TERMINAL_STORY_STATES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
const TERMINAL_PHASES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
const TERMINAL_VERIFICATION_STATUSES: &[&str] = &[
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];
const STALE_STORY_STATES: &[&str] = &["stale", "expired", "invalid"];

#[derive(Debug, Clone)]
struct FrameworkRuntimeView {
    session_summary_text: String,
    next_actions: Value,
    evidence_index: Value,
    trace_metadata: Value,
    supervisor_state: Map<String, Value>,
    artifact_base: PathBuf,
    current_root: PathBuf,
    mirror_root: PathBuf,
    task_root: PathBuf,
    active_task_id: Option<String>,
    collected_at: String,
}

pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    let base = if let Some(path) = repo_root {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|err| format!("resolve current directory failed: {err}"))?
    };
    Ok(base.canonicalize().unwrap_or(base))
}

pub fn build_framework_runtime_snapshot_envelope(repo_root: &Path) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root);
    let continuity = classify_runtime_continuity(&snapshot);
    let trace_skills = normalize_trace_skills(&snapshot.trace_metadata);
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            trace_skills.first().cloned()
        } else {
            Some(direct)
        }
    };
    let verification_status = snapshot
        .supervisor_state
        .get("verification")
        .and_then(Value::as_object)
        .and_then(|verification| nonempty_string(verification.get("verification_status")));
    Ok(json!({
        "schema_version": FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "runtime_snapshot": {
            "ok": true,
            "workspace": workspace_name_from_root(repo_root),
            "artifact_base": snapshot.artifact_base.display().to_string(),
            "current_root": snapshot.current_root.display().to_string(),
            "mirror_root": snapshot.mirror_root.display().to_string(),
            "task_root": snapshot.task_root.display().to_string(),
            "active_task_id": snapshot.active_task_id,
            "collected_at": snapshot.collected_at,
            "session_summary_present": !snapshot.session_summary_text.trim().is_empty(),
            "next_action_count": normalize_next_actions(&snapshot.next_actions).len(),
            "evidence_count": normalize_evidence_index(&snapshot.evidence_index).len(),
            "trace_skill_count": trace_skills.len(),
            "continuity": continuity,
            "supervisor_state": {
                "task_id": nonempty_string(snapshot.supervisor_state.get("task_id")),
                "task_summary": nonempty_string(snapshot.supervisor_state.get("task_summary")),
                "active_phase": nonempty_string(snapshot.supervisor_state.get("active_phase")),
                "primary_owner": primary_owner,
                "verification_status": verification_status,
            },
            "paths": {
                "session_summary": snapshot.current_root.join(SESSION_SUMMARY_FILENAME).display().to_string(),
                "next_actions": snapshot.current_root.join(NEXT_ACTIONS_FILENAME).display().to_string(),
                "evidence_index": snapshot.current_root.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
                "trace_metadata": snapshot.current_root.join(TRACE_METADATA_FILENAME).display().to_string(),
                "bridge_mirror_root": snapshot.mirror_root.display().to_string(),
                "supervisor_state": repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
            },
        }
    }))
}

pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let trace_skills = normalize_trace_skills(&snapshot.trace_metadata);
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            trace_skills.first().cloned()
        } else {
            Some(direct)
        }
    };
    let blocker_list = snapshot
        .supervisor_state
        .get("blockers")
        .and_then(Value::as_object)
        .and_then(|blockers| blockers.get("open_blockers"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let is_active = continuity.get("state").and_then(Value::as_str) == Some("active")
        && continuity.get("can_resume").and_then(Value::as_bool) == Some(true);
    Ok(json!({
        "schema_version": FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "contract_summary": {
            "ok": true,
            "workspace": workspace_name_from_root(repo_root),
            "continuity": continuity,
            "goal": if is_active { contract.get("goal").cloned().unwrap_or(Value::Null) } else { Value::Null },
            "scope": if is_active { value_string_list(contract.get("scope")) } else { Vec::<String>::new() },
            "forbidden_scope": if is_active { value_string_list(contract.get("forbidden_scope")) } else { Vec::<String>::new() },
            "acceptance_criteria": if is_active { value_string_list(contract.get("acceptance_criteria")) } else { Vec::<String>::new() },
            "evidence_required": if is_active { value_string_list(contract.get("evidence_required")) } else { Vec::<String>::new() },
            "active_phase": if is_active { nonempty_string(snapshot.supervisor_state.get("active_phase")) } else { Option::<String>::None },
            "primary_owner": primary_owner,
            "next_actions": if is_active { normalize_next_actions(&snapshot.next_actions) } else { Vec::<String>::new() },
            "open_blockers": if is_active { blocker_list } else { Vec::<String>::new() },
            "trace_skills": trace_skills,
            "session_summary": parse_session_summary(&snapshot.session_summary_text),
            "evidence_count": normalize_evidence_index(&snapshot.evidence_index).len(),
            "artifacts_root": snapshot.current_root.display().to_string(),
            "recent_completed_execution": continuity.get("recent_completed_execution").cloned().unwrap_or(Value::Null),
            "recovery_hints": continuity.get("recovery_hints").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        }
    }))
}

fn load_framework_runtime_view(repo_root: &Path) -> FrameworkRuntimeView {
    let artifact_base = repo_root.join("artifacts");
    let mirror_root = artifact_base.join(CURRENT_ARTIFACT_DIR);
    let supervisor_state = normalize_supervisor_state(&read_json_if_exists(
        &repo_root.join(SUPERVISOR_STATE_FILENAME),
    ));
    let pointer = read_json_if_exists(&mirror_root.join(ACTIVE_TASK_POINTER_NAME));
    let active_task_id = {
        let direct = safe_slug(&value_text(supervisor_state.get("task_id")));
        if direct.is_empty() {
            let pointer_task_id = safe_slug(&value_text(pointer.get("task_id")));
            if pointer_task_id.is_empty() {
                None
            } else {
                Some(pointer_task_id)
            }
        } else {
            Some(direct)
        }
    };
    let task_root = active_task_id
        .as_ref()
        .map(|task_id| mirror_root.join(task_id))
        .unwrap_or_else(|| mirror_root.clone());
    let pointer_task_id = safe_slug(&value_text(pointer.get("task_id")));
    let mirror_matches_selected = active_task_id
        .as_ref()
        .map(|task_id| task_id == &pointer_task_id)
        .unwrap_or(false);
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
        artifact_base,
        current_root: preferred_root,
        mirror_root,
        task_root,
        active_task_id,
        collected_at: current_local_timestamp(),
    }
}

fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    let summary = parse_session_summary(&snapshot.session_summary_text);
    let supervisor = &snapshot.supervisor_state;
    let verification = supervisor
        .get("verification")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let continuity = supervisor
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let contract = supervisor_contract(supervisor);
    let trace_task = value_text(snapshot.trace_metadata.get("task"));
    let summary_task = value_text(summary.get("task"));
    let supervisor_task = first_nonempty(&[
        value_text(supervisor.get("task_summary")),
        value_text(supervisor.get("task_id")),
    ]);
    let task = first_nonempty(&[summary_task.clone(), trace_task.clone(), supervisor_task]);
    let summary_phase = value_text(summary.get("phase"));
    let supervisor_phase = value_text(supervisor.get("active_phase"));
    let phase = first_nonempty(&[summary_phase.clone(), supervisor_phase.clone()]);
    let verification_status = value_text(verification.get("verification_status"));
    let summary_status = value_text(summary.get("status"));
    let story_state = value_text(continuity.get("story_state"));
    let status = first_nonempty(&[
        summary_status.clone(),
        verification_status.clone(),
        story_state.clone(),
    ]);
    let route = normalize_trace_skills(&snapshot.trace_metadata);
    let mut next_actions = normalize_next_actions(&snapshot.next_actions);
    if next_actions.is_empty() {
        next_actions = value_string_list(supervisor.get("next_actions"));
    }
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
    let summary_terminal = is_terminal(&summary_phase, TERMINAL_PHASES)
        || is_terminal(&summary_status, TERMINAL_VERIFICATION_STATUSES);
    let supervisor_terminal = is_terminal(&supervisor_phase, TERMINAL_PHASES)
        || is_terminal(&verification_status, TERMINAL_VERIFICATION_STATUSES)
        || is_terminal(&story_state, TERMINAL_STORY_STATES);
    let terminal_reasons = stable_line_items(vec![
        terminal_reason("summary phase is terminal", &summary_phase, TERMINAL_PHASES),
        terminal_reason(
            "summary status is terminal",
            &summary_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason("supervisor phase is terminal", &supervisor_phase, TERMINAL_PHASES),
        terminal_reason(
            "verification status is terminal",
            &verification_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason(
            "continuity story_state is terminal",
            &story_state,
            TERMINAL_STORY_STATES,
        ),
    ]);
    let inconsistency_reasons = stable_line_items(vec![
        if !summary_task.is_empty()
            && !trace_task.is_empty()
            && !looks_same_identity(&summary_task, &trace_task)
        {
            format!("session summary task '{summary_task}' disagrees with trace task '{trace_task}'")
        } else {
            String::new()
        },
        if summary_terminal
            && !supervisor_terminal
            && (!supervisor_phase.is_empty() || !verification_status.is_empty())
        {
            "session summary marks the task terminal while supervisor still looks active".to_string()
        } else {
            String::new()
        },
        if supervisor_terminal
            && !summary_terminal
            && (!summary_phase.is_empty() || !summary_status.is_empty())
        {
            "supervisor marks the task terminal while the session summary still looks active".to_string()
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
    ]);
    let now = Local::now().fixed_offset();
    let stale_reasons = stable_line_items(vec![
        if is_terminal(&story_state, STALE_STORY_STATES) {
            format!("continuity story_state is stale: {story_state}")
        } else {
            String::new()
        },
        if value_bool_or_none(continuity.get("resume_allowed")) == Some(false)
            && terminal_reasons.is_empty()
        {
            "continuity explicitly disallows resume".to_string()
        } else {
            String::new()
        },
        match parse_iso_timestamp(continuity.get("active_lease_expires_at")) {
            Some(expires_at) if expires_at < now => {
                format!(
                    "active lease expired at {}",
                    value_text(continuity.get("active_lease_expires_at"))
                )
            }
            _ => String::new(),
        },
        if snapshot.session_summary_text.trim().is_empty()
            && terminal_reasons.is_empty()
            && (!task.is_empty()
                || !supervisor_phase.is_empty()
                || !verification_status.is_empty()
                || !next_actions.is_empty())
        {
            "session summary mirror is missing while supervisor still looks active".to_string()
        } else {
            String::new()
        },
        if !value_text(continuity.get("state_reason")).is_empty()
            && (is_terminal(&story_state, STALE_STORY_STATES)
                || value_bool_or_none(continuity.get("resume_allowed")) == Some(false))
        {
            format!("state reason: {}", value_text(continuity.get("state_reason")))
        } else {
            String::new()
        },
    ]);
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
    let state = if !has_any_runtime_signal {
        "missing"
    } else if !inconsistency_reasons.is_empty() {
        "inconsistent"
    } else if !terminal_reasons.is_empty() {
        "completed"
    } else if !stale_reasons.is_empty() {
        "stale"
    } else {
        "active"
    };
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
            "Reconcile SESSION_SUMMARY.md, TRACE_METADATA.json, and .supervisor_state.json before injecting continuity.",
            "Treat the current snapshot as blocked until the supervisor rewrites a consistent continuity bundle."
        ]),
        _ => json!([]),
    };
    json!({
        "state": state,
        "can_resume": state == "active",
        "task": task,
        "phase": phase,
        "status": status,
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "current_execution": if state == "active" && !task.is_empty() { current_execution } else { Value::Null },
        "recent_completed_execution": if state == "completed" && !task.is_empty() { recent_completed_execution } else { Value::Null },
        "stale_reasons": stale_reasons,
        "terminal_reasons": terminal_reasons,
        "inconsistency_reasons": inconsistency_reasons,
        "recovery_hints": recovery_hints,
        "continuity": {
            "story_state": if story_state.is_empty() { None::<String> } else { Some(story_state) },
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
            "bridge_mirror_root": snapshot.mirror_root.display().to_string(),
            "supervisor_state": snapshot.artifact_base.parent().unwrap_or(&snapshot.artifact_base).join(SUPERVISOR_STATE_FILENAME).display().to_string(),
        }
    })
}

fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn read_json_if_exists(path: &Path) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Map::new())),
        Err(_) => Value::Object(Map::new()),
    }
}

fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
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
            items.iter()
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
        payload.get("next_actions").or_else(|| payload.get("actions"))
    };
    actions
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|item| value_text(Some(item)))
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
        payload.get("matched_skills").or_else(|| payload.get("skills"))
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
    terminal_values.iter().any(|candidate| lowered == *candidate)
}

fn looks_same_identity(left: &str, right: &str) -> bool {
    let left_token = safe_slug(&left.to_ascii_lowercase());
    let right_token = safe_slug(&right.to_ascii_lowercase());
    if left_token.is_empty() || right_token.is_empty() {
        return true;
    }
    left_token == right_token || left_token.contains(&right_token) || right_token.contains(&left_token)
}
