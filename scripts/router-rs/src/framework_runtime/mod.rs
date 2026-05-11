use crate::closeout_enforcement::{
    evaluate_closeout_record_value, evaluate_closeout_record_value_with_context,
    CloseoutEvidenceContext,
};
use crate::router_env_flags::router_rs_env_enabled_default_true;
use chrono::{Local, SecondsFormat};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

mod alias;
mod constants;
mod continuity_digest;
mod prompt_compression;
mod repo_roots;
mod runtime_view;
mod session_artifacts;
mod statusline;
mod types;

pub use alias::build_framework_alias_envelope;
// Used by `crate::framework_runtime::FRAMEWORK_ALIAS_SCHEMA_VERSION` consumers; not referenced in this module body.
#[allow(unused_imports)]
pub use constants::FRAMEWORK_ALIAS_SCHEMA_VERSION;
// Retained for external callers.
#[allow(unused_imports)]
pub use constants::FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION;
pub use constants::{
    FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION, FRAMEWORK_RUNTIME_AUTHORITY,
    FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION, FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
};
pub use continuity_digest::build_framework_continuity_digest_prompt;
pub use prompt_compression::build_framework_prompt_compression_envelope;
pub use repo_roots::{
    framework_root_from_executable_path, is_framework_root, resolve_repo_root_arg,
};
pub use session_artifacts::write_framework_session_artifacts;
pub use statusline::build_framework_statusline;
pub use types::FrameworkAliasBuildOptions;

use constants::{
    CLOSEOUT_COMPLETION_STATUSES, CURRENT_ARTIFACT_DIR, EVIDENCE_INDEX_FILENAME,
    EVIDENCE_INDEX_SCHEMA_VERSION, NEXT_ACTIONS_FILENAME, SESSION_SUMMARY_FILENAME,
    SUPERVISOR_STATE_FILENAME, TASK_REGISTRY_SCHEMA_VERSION, TRACE_METADATA_FILENAME,
};
use types::FrameworkRuntimeView;

pub fn build_framework_runtime_snapshot_envelope(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, artifact_root_override, task_id_override);
    let continuity = classify_runtime_continuity(&snapshot);
    let continuity_route = continuity
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity_route.first().map(|item| value_text(Some(item)))
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
            "control_plane_present": snapshot.active_task_pointer_present
                && snapshot.focus_task_pointer_present
                && snapshot.task_registry_present
                && !snapshot.supervisor_state.is_empty(),
            "control_plane_missing": missing_control_plane_anchors(&snapshot),
            "control_plane_inconsistency_reasons": snapshot.control_plane_inconsistency_reasons,
            "active_task_id": snapshot.active_task_id,
            "focus_task_id": snapshot.focus_task_id,
            "known_task_ids": snapshot.known_task_ids,
            "recoverable_task_ids": snapshot.recoverable_task_ids,
            "parallel_task_count": snapshot.known_task_ids.len(),
            "registered_tasks": snapshot.registered_tasks,
            "collected_at": snapshot.collected_at,
            "session_summary_present": !snapshot.session_summary_text.trim().is_empty(),
            "next_action_count": continuity
                .get("next_actions")
                .and_then(Value::as_array)
                .map(|rows| rows.len())
                .unwrap_or(0),
            "evidence_count": normalize_evidence_index(&snapshot.evidence_index).len(),
            "trace_skill_count": continuity_route.len(),
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
                "current_pointer_root": snapshot.mirror_root.display().to_string(),
                "supervisor_state": repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
            },
        }
    }))
}

pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let workspace = workspace_name_from_root(repo_root);
    let continuity_route = continuity
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity_route.first().map(|item| value_text(Some(item)))
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
    let goal = if is_active {
        contract.get("goal").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let scope = if is_active {
        value_string_list(contract.get("scope"))
    } else {
        Vec::<String>::new()
    };
    let forbidden_scope = if is_active {
        value_string_list(contract.get("forbidden_scope"))
    } else {
        Vec::<String>::new()
    };
    let acceptance_criteria = if is_active {
        value_string_list(contract.get("acceptance_criteria"))
    } else {
        Vec::<String>::new()
    };
    let evidence_required = if is_active {
        value_string_list(contract.get("evidence_required"))
    } else {
        Vec::<String>::new()
    };
    let active_phase = if is_active {
        nonempty_string(snapshot.supervisor_state.get("active_phase"))
    } else {
        Option::<String>::None
    };
    let next_actions = if is_active {
        continuity
            .get("next_actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::<Value>::new()
    };
    let open_blockers = if is_active {
        blocker_list
    } else {
        Vec::<String>::new()
    };
    let session_summary: Map<String, Value> = parse_session_summary(&snapshot.session_summary_text);
    let evidence_count = normalize_evidence_index(&snapshot.evidence_index).len();
    let contract_digest_input = json!({
        "workspace": workspace.clone(),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "goal": goal,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
        "active_phase": active_phase,
        "primary_owner": primary_owner.clone(),
        "next_actions": next_actions,
        "open_blockers": open_blockers,
        "trace_skills": continuity_route.clone(),
        "evidence_count": evidence_count,
    });
    let contract_digest = stable_json_sha256(&contract_digest_input)?;
    let session_summary_value = Value::Object(session_summary.clone());
    let prompt_lines = build_contract_guard_prompt_lines(
        &contract_digest,
        &continuity,
        &contract_digest_input,
        &session_summary_value,
        snapshot.current_root.as_path(),
    );
    Ok(json!({
        "schema_version": FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "contract_summary": {
            "ok": true,
            "workspace": workspace,
            "contract_digest": contract_digest,
            "contract_digest_algorithm": "sha256",
            "contract_guard": {
                "contract_active": is_active,
                "drift_classes": ["scope_drift", "owner_drift", "evidence_drift", "contract_digest_drift"],
                "fail_closed_when": [
                    "expected contract_digest differs from live contract_digest",
                    "proposed owner differs from primary_owner without explicit contract update intent",
                    "proposed goal/task changes while continuity is active",
                    "verification/evidence requirements are dropped before completion"
                ],
                "update_requires_explicit_user_intent": true
            },
            "prompt_lines": prompt_lines,
            "continuity": continuity,
            "goal": contract_digest_input.get("goal").cloned().unwrap_or(Value::Null),
            "scope": contract_digest_input.get("scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "forbidden_scope": contract_digest_input.get("forbidden_scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "acceptance_criteria": contract_digest_input.get("acceptance_criteria").cloned().unwrap_or(Value::Array(Vec::new())),
            "evidence_required": contract_digest_input.get("evidence_required").cloned().unwrap_or(Value::Array(Vec::new())),
            "active_phase": contract_digest_input.get("active_phase").cloned().unwrap_or(Value::Null),
            "primary_owner": primary_owner,
            "next_actions": contract_digest_input.get("next_actions").cloned().unwrap_or(Value::Array(Vec::new())),
            "open_blockers": contract_digest_input.get("open_blockers").cloned().unwrap_or(Value::Array(Vec::new())),
            "trace_skills": continuity_route,
            "session_summary": session_summary,
            "evidence_count": evidence_count,
            "artifacts_root": snapshot.current_root.display().to_string(),
            "recent_completed_execution": continuity.get("recent_completed_execution").cloned().unwrap_or(Value::Null),
            "recovery_hints": continuity.get("recovery_hints").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        }
    }))
}

fn stable_json_sha256(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|err| format!("serialize contract digest input failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn build_contract_guard_prompt_lines(
    contract_digest: &str,
    continuity: &Value,
    digest_input: &Value,
    session_summary: &Value,
    artifact_root: &Path,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("contract_digest: sha256:{contract_digest}"));
    lines.push(format!(
        "continuity: state={} can_resume={}",
        value_text(continuity.get("state")),
        continuity
            .get("can_resume")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    let task = value_text(continuity.get("task"));
    if !task.is_empty() {
        lines.push(format!("task: {task}"));
    } else if let Some(task) = nonempty_string(session_summary.get("task")) {
        lines.push(format!("task: {task}"));
    }
    if let Some(owner) = nonempty_string(digest_input.get("primary_owner")) {
        lines.push(format!("owner: {owner}"));
    }
    if let Some(phase) = nonempty_string(digest_input.get("active_phase")) {
        lines.push(format!("phase: {phase}"));
    }
    for (label, key) in [
        ("goal", "goal"),
        ("scope", "scope"),
        ("forbidden_scope", "forbidden_scope"),
        ("acceptance", "acceptance_criteria"),
        ("evidence", "evidence_required"),
        ("blockers", "open_blockers"),
    ] {
        let line = compact_contract_value_line(label, digest_input.get(key));
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.push(format!("artifacts: {}", artifact_root.display()));
    lines.truncate(12);
    lines
}

fn compact_contract_value_line(label: &str, value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match value {
        Value::Null => String::new(),
        Value::String(text) if text.trim().is_empty() => String::new(),
        Value::String(text) => format!("{label}: {}", compact_contract_text(text, 140)),
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Array(items) => {
            let joined = items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ");
            if joined.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&joined, 180))
            }
        }
        _ => {
            let text = value_text(Some(value));
            if text.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&text, 140))
            }
        }
    }
}

fn compact_contract_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut compact = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    compact.push_str("...");
    compact
}

fn load_framework_runtime_view(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> FrameworkRuntimeView {
    runtime_view::load_framework_runtime_view(repo_root, artifact_root_override, task_id_override)
}

fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    runtime_view::classify_runtime_continuity(snapshot)
}

fn missing_control_plane_anchors(snapshot: &FrameworkRuntimeView) -> Vec<String> {
    runtime_view::missing_control_plane_anchors(snapshot)
}

fn workspace_name_from_root(repo_root: &Path) -> String {
    runtime_view::workspace_name_from_root(repo_root)
}

fn write_text_if_changed_unlocked(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    write_atomic_text(path, content)?;
    Ok(true)
}

#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_RDONLY)
        .open(parent)
        .map_err(|err| {
            format!(
                "open parent dir for fsync failed {}: {err}",
                parent.display()
            )
        })?;
    dir.sync_all()
        .map_err(|err| format!("fsync parent dir failed for {}: {err}", parent.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn write_atomic_text(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt")
    ));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp_path)
        .map_err(|err| format!("open temp file failed for {}: {err}", tmp_path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    file.sync_all()
        .map_err(|err| format!("fsync temp file failed for {}: {err}", tmp_path.display()))?;
    drop(file);
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;
    fsync_parent_dir(path)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn hash_file_for_test(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("read file failed for {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_json_if_changed_unlocked(path: &Path, payload: &Value) -> Result<bool, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize JSON payload failed: {err}"))?
    );
    write_text_if_changed_unlocked(path, &serialized)
}

fn join_lines(values: &[String]) -> String {
    values
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ")
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub(crate) fn read_json_strict(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read json failed for {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse json failed for {}: {err}", path.display()))
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

fn required_payload_text(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    let Some(v) = payload.get(key) else {
        return Err(format!("{context}: missing required field {key:?}"));
    };
    let s = value_text(Some(v));
    if s.trim().is_empty() {
        return Err(format!("{context}: required field {key:?} is empty"));
    }
    Ok(s)
}

fn defaulted_payload_text(payload: &Value, key: &str, fallback: &str) -> String {
    let s = payload
        .get(key)
        .map(|v| value_text(Some(v)))
        .unwrap_or_default();
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s
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
            "focus_task_id": if focus_task_id.is_empty() {
                Value::Null
            } else {
                Value::String(focus_task_id)
            },
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

pub(crate) fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// 构建自动连续性检查点载荷（非完成态 `status=in_progress`，用于 Codex Stop 等钩子）。
///
/// `task_line` 用作路由摘要标题；`summary_text` 写入 SESSION_SUMMARY 正文片段。
pub fn build_automatic_continuity_checkpoint_payload(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
) -> Value {
    let output_dir = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let task = if task_line.trim().is_empty() {
        "session-checkpoint".to_string()
    } else {
        truncate_utf8_chars(task_line.trim(), 200)
    };
    let summary = if summary_text.trim().is_empty() {
        "Automatic continuity checkpoint. No summary text was provided; refine in the next turn."
            .to_string()
    } else {
        truncate_utf8_chars(summary_text.trim(), 8000)
    };
    json!({
        "output_dir": output_dir.to_string_lossy(),
        "repo_root": repo_root.to_string_lossy(),
        "task": task,
        "summary": summary,
        "phase": "execution",
        "status": "in_progress",
        "focus": true,
        "next_actions": [
            "Open artifacts/current/SESSION_SUMMARY.md on the next session.",
            "Optional: run `router-rs framework snapshot --repo-root <repo>` for a compact runtime read model.",
        ],
        "trace_metadata": {
            "checkpoint_kind": "automatic_stop_hook",
        }
    })
}

const MAX_POST_TOOL_EVIDENCE_ARTIFACTS: usize = 120;

fn continuity_post_tool_evidence_env_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE")
}

fn extract_codex_shell_command_preview(event: &Value) -> Option<String> {
    let input = event.get("tool_input").and_then(Value::as_object)?;
    let cmd = input
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            input
                .get("cmd")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            input
                .get("script")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            input
                .get("arguments")
                .and_then(Value::as_object)
                .and_then(|a| a.get("command"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })?;
    Some(truncate_utf8_chars(cmd, 2000))
}

fn coerce_exit_code_value(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(n) = value.as_u64() {
        return Some(n as i64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<i64>().ok();
    }
    None
}

/// 从 Codex `PostToolUse` 载荷中提取退出码（兼容嵌套 `tool_output` / JSON 字符串）。
fn extract_codex_tool_exit_hint(event: &Value) -> Option<i64> {
    let candidates: Vec<Option<&Value>> = vec![
        event.get("exit_code"),
        event.get("exitCode"),
        event.get("tool_output").and_then(|v| v.get("exit_code")),
        event.get("tool_output").and_then(|v| v.get("exitCode")),
        event
            .get("tool_output")
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("exit_code")),
        event.get("result").and_then(|v| v.get("exit_code")),
        event.get("response").and_then(|v| v.get("exit_code")),
    ];
    if let Some(to) = event.get("tool_output") {
        if let Some(text) = to.as_str() {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                if let Some(code) = coerce_exit_code_value(parsed.get("exit_code")) {
                    return Some(code);
                }
                if let Some(code) = coerce_exit_code_value(parsed.get("exitCode")) {
                    return Some(code);
                }
            }
        }
    }
    for candidate in candidates {
        if let Some(code) = coerce_exit_code_value(candidate) {
            return Some(code);
        }
    }
    None
}

fn append_evidence_index_merged_row(
    repo_root: &Path,
    entry: Map<String, Value>,
) -> Result<(), String> {
    if !continuity_post_tool_evidence_env_enabled() {
        return Ok(());
    }
    let _guard = crate::task_write_lock::task_ledger_write_lock()
        .lock()
        .map_err(|_| "task ledger write lock poisoned".to_string())?;
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    if !continuity_session_ready_for_evidence_append(&snapshot) {
        return Ok(());
    }

    let evidence_path = snapshot.current_root.join(EVIDENCE_INDEX_FILENAME);
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create evidence dir: {err}"))?;
    }
    // Cross-process lock: evidence append is read-modify-write and must not lose updates
    // when Cursor/Codex hooks (or parallel tests) race on the same task directory.
    let _evidence_lock = crate::runtime_storage::acquire_runtime_path_lock(&evidence_path)?;

    let existing = read_json_strict(&evidence_path)?;
    let mut rows: Vec<Map<String, Value>> = normalize_evidence_index(&existing);
    rows.push(entry);
    if rows.len() > MAX_POST_TOOL_EVIDENCE_ARTIFACTS {
        let drain = rows.len() - MAX_POST_TOOL_EVIDENCE_ARTIFACTS;
        rows.drain(0..drain);
    }
    let payload = json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": rows.into_iter().map(Value::Object).collect::<Vec<Value>>(),
    });
    write_json_if_changed_unlocked(&evidence_path, &payload)?;
    if let Some(tid) = crate::autopilot_goal::read_active_task_id(repo_root) {
        if !tid.is_empty() {
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &tid);
        }
    }
    Ok(())
}

/// `framework hook-evidence-append`：供 Cursor hook 等外部进程写入一条验证记录。
///
/// JSON：`repo_root`（可选）、`command_preview`（必填）、`exit_code`（可选）、`source`（可选，默认 `external_hook`）。
pub fn framework_hook_evidence_append(payload: Value) -> Result<Value, String> {
    let explicit = payload.get("repo_root").and_then(|v| {
        let s = value_text(Some(v));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    });
    let repo_root = resolve_repo_root_arg(explicit.as_deref())?;
    let preview = required_payload_text(&payload, "command_preview", "hook evidence append")?;
    let preview_trim = preview.trim();
    if preview_trim.is_empty() {
        return Err("hook evidence append requires non-empty command_preview".to_string());
    }
    let source = defaulted_payload_text(&payload, "source", "external_hook");
    let exit_code = payload
        .get("exit_code")
        .and_then(|v| coerce_exit_code_value(Some(v)));

    let cursor_hook = source.trim().to_ascii_lowercase().starts_with("cursor_");
    if !cursor_hook && !shell_command_looks_like_verification(preview_trim) {
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "command_preview did not match verification heuristics",
            "schema_version": "router-rs-hook-evidence-append-v1",
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
        }));
    }

    let preview_store = truncate_utf8_chars(preview_trim, 2000);
    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("external_hook_verification"));
    entry.insert("source".to_string(), json!(source.trim()));
    entry.insert("command_preview".to_string(), json!(preview_store));
    entry.insert("recorded_at".to_string(), json!(current_local_timestamp()));
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    append_evidence_index_merged_row(&repo_root, entry)?;
    Ok(json!({
        "ok": true,
        "skipped": false,
        "schema_version": "router-rs-hook-evidence-append-v1",
        "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
    }))
}

fn codex_tool_name_normalized(event: &Value) -> String {
    event
        .get("tool_name")
        .or(event.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn tool_name_is_shell_like(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n == "bash"
        || n == "sh"
        || n == "zsh"
        || n == "shell"
        || n.contains("terminal")
        || n.contains("shell")
        || n == "functions.run_terminal_cmd"
        || n == "run_terminal_cmd"
        || n == "powershell"
        || n == "pwsh"
}

fn shell_command_looks_like_verification(command: &str) -> bool {
    let c = command.to_ascii_lowercase();
    // Original (Rust / Python / JS test runners + lint).
    c.contains("cargo test")
        || c.contains("cargo check")
        || c.contains("cargo clippy")
        || c.contains("cargo build")
        || c.contains("cargo fmt")
        || c.contains("cargo nextest")
        || c.contains("cargo hack")
        || c.contains("nextest")
        || c.contains("pytest")
        || c.contains("npm test")
        || c.contains("pnpm test")
        || c.contains("yarn test")
        || c.contains("make test")
        || c.contains("make check")
        || c.contains("make ci")
        || c.contains("make verify")
        || c.contains("go test")
        || c.contains("go vet")
        || c.contains("dotnet test")
        || c.contains("maturin")
        || c.contains("tox")
        || c.contains("uv run")
        || c.contains("just test")
        || c.contains("just check")
        || c.contains("vitest")
        || c.contains("jest")
        || c.contains("ruby test")
        || c.contains("rake test")
        || c.contains("verify_cursor_hooks")
        || c.contains("policy_contracts")
        || c.contains("ruff check")
        || c.contains("ruff format")
        || c.contains("mypy")
        || c.contains("deno test")
        || c.contains("bun test")
        // TypeScript / JS tooling (no `test` keyword).
        || c.contains("tsc --noemit")
        || c.contains("tsc -p")
        || c.contains("eslint")
        || c.contains("prettier --check")
        || c.contains("biome check")
        || c.contains("biome ci")
        // JVM ecosystems.
        || c.contains("gradle test")
        || c.contains("gradlew test")
        || c.contains("gradle check")
        || c.contains("mvn test")
        || c.contains("mvn verify")
        || c.contains("mvn package")
        // E2E / cross-runner test frameworks.
        || c.contains("playwright test")
        || c.contains("nx test")
        || c.contains("nx affected")
        // Repo-local verifier scripts (any path under scripts/ ending with verify*).
        || c.contains("scripts/verify")
        || c.contains("/verify.sh")
        || c.contains("./verify.sh")
        || c.contains("task test")
        || c.contains("task check")
        // Formal / math toolchains (narrow tokens; avoid bare `python` as the only signal).
        || c.contains("sympy")
        || c.contains(" z3 ")
        || c.starts_with("z3 ")
        || c.contains("\tz3 ")
        || c.contains(" z3\t")
        || c.contains("lean4")
        || c.contains(" lean ")
        || c.trim_start().starts_with("lean ")
        || c.contains("coqc")
        || c.contains("coqchk")
        || c.contains("lake build")
        || c.contains("lake test")
        || c.contains("lake check")
        || c.contains("lake exe")
        || c.contains("isabelle build")
}

#[cfg(test)]
mod shell_command_verification_heuristic_tests {
    use super::shell_command_looks_like_verification;

    #[test]
    fn matrix_math_formal_and_build_tools() {
        assert!(shell_command_looks_like_verification(
            "python -c \"import sympy; print(sympy.simplify(1))\""
        ));
        assert!(shell_command_looks_like_verification("z3 /tmp/proof.smt2"));
        assert!(shell_command_looks_like_verification("  z3  /tmp/x.smt2"));
        assert!(shell_command_looks_like_verification("lean --version"));
        assert!(shell_command_looks_like_verification(
            "lake build && lake test"
        ));
        assert!(shell_command_looks_like_verification(
            "coqc -Q theories Foo.v"
        ));
        assert!(shell_command_looks_like_verification(
            "coqchk -silent Foo.vo"
        ));
        assert!(shell_command_looks_like_verification("isabelle build -D ."));
        assert!(shell_command_looks_like_verification("cargo test -q"));
        assert!(shell_command_looks_like_verification("pytest -q"));
    }

    #[test]
    fn matrix_rejects_bare_python_and_random_strings() {
        assert!(!shell_command_looks_like_verification("python foo.py"));
        assert!(!shell_command_looks_like_verification(
            "python -c \"print(1)\""
        ));
        assert!(!shell_command_looks_like_verification("echo hello"));
        assert!(!shell_command_looks_like_verification("leaning tower")); // not `lean ` token
    }
}

fn continuity_session_ready_for_evidence_append(snapshot: &FrameworkRuntimeView) -> bool {
    if snapshot.active_task_pointer_present {
        return true;
    }
    let summary_path = snapshot.current_root.join(SESSION_SUMMARY_FILENAME);
    summary_path.is_file()
}

/// 在宿主 `PostToolUse` 中追加一条「终端类验证命令」到 `EVIDENCE_INDEX.json`（与 session 写入共用锁）。
///
/// `kind` 用于区分来源（如 `codex_post_tool_verification` / `cursor_post_tool_verification`）。
/// 仅在连续性已初始化且命令启发式匹配验证类时写入。`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭。
pub fn try_append_post_tool_shell_evidence(
    repo_root: &Path,
    event: &Value,
    kind: &str,
) -> Result<(), String> {
    if !continuity_post_tool_evidence_env_enabled() {
        return Ok(());
    }
    let tool_name = codex_tool_name_normalized(event);
    if !tool_name_is_shell_like(&tool_name) {
        return Ok(());
    }
    let Some(command_preview) = extract_codex_shell_command_preview(event) else {
        return Ok(());
    };
    if !shell_command_looks_like_verification(&command_preview) {
        return Ok(());
    }

    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let exit_hint = extract_codex_tool_exit_hint(event);
    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!(kind));
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(command_preview));
    entry.insert("recorded_at".to_string(), json!(current_local_timestamp()));
    if !session_id.is_empty() {
        entry.insert("session_id".to_string(), json!(session_id));
    }
    if let Some(ec) = exit_hint {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    append_evidence_index_merged_row(repo_root, entry)?;
    Ok(())
}

/// Whether programmatic closeout enforcement is enabled in the current process.
///
/// - **Enabled** in CI / GitHub Actions by default.
/// - **Disabled** locally when `ROUTER_RS_CLOSEOUT_ENFORCEMENT` is unset.
/// - Explicitly disable with `ROUTER_RS_CLOSEOUT_ENFORCEMENT=0|false|off|no`.
pub fn closeout_programmatic_enforcement_enabled() -> bool {
    !closeout_enforcement_disabled_by_env()
}

/// Default location for a task's closeout record.
pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("closeout")
        .join(format!("{}.json", task_id.trim()))
}

/// Evaluate a materialized closeout record JSON file, attaching an EvidenceContext (R8) when possible.
pub fn evaluate_closeout_record_file_for_task(
    repo_root: &Path,
    task_id: &str,
    record_path: &Path,
) -> Result<Value, String> {
    let tid = task_id.trim();
    if tid.is_empty() {
        return Err("task_id is empty".to_string());
    }
    let text = std::fs::read_to_string(record_path).map_err(|err| {
        format!(
            "read closeout record failed ({}): {err}",
            record_path.display()
        )
    })?;
    let record: Value = serde_json::from_str(&text).map_err(|err| {
        format!(
            "parse closeout record JSON failed ({}): {err}",
            record_path.display()
        )
    })?;
    let (rows_non_empty, has_success) =
        crate::autopilot_goal::task_evidence_artifacts_summary_for_task(repo_root, tid);
    let ctx = CloseoutEvidenceContext {
        evidence_rows_non_empty: rows_non_empty,
        has_successful_verification: has_success,
    };
    evaluate_closeout_record_value_with_context(record, &ctx)
        .map_err(|err| format!("closeout record evaluation failed: {err}"))
}

fn in_ci_like_environment() -> bool {
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        return true;
    }
    match std::env::var("CI") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !t.is_empty() && !matches!(t.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => false,
    }
}

fn closeout_enforcement_disabled_by_env() -> bool {
    match std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "0" | "false" | "off" | "no")
        }
        // 未设置：本地个人场景默认软门禁；CI/GitHub Actions 默认硬门禁（团队/审计友好）。
        Err(_) => !in_ci_like_environment(),
    }
}

/// Apply closeout enforcement to a session-artifact write payload.
///
/// Returns:
/// - `Ok(Some(eval))` when status claims completion and a valid record was
///   provided that passes evaluation. The envelope is attached to the
///   response so callers see the evidence summary alongside the write.
/// - `Ok(None)` when status is not a completion claim. In that case
///   any incidental `closeout_record` is intentionally **not** parsed —
///   in-progress / planning / execution checkpoints often carry placeholder
///   or partial records, and `deny_unknown_fields` plus strict R-rule
///   evaluation would otherwise turn a benign in-progress write into a hard
///   error. Pre-completion validation is the caller's responsibility (run
///   `closeout evaluate` separately) so the artifact-write path stays
///   resilient against in-progress draft records.
/// - `Ok(None)` when status claims completion but programmatic enforcement is off:
///   explicit `ROUTER_RS_CLOSEOUT_ENFORCEMENT`=`0`/`false`/`off`/`no`, **or** the variable is unset
///   while not in CI/GitHub Actions（本地默认软；响应中不附带 `closeout_evaluation`）。
///   团队/CI：未设置且检测到 `CI` 或 `GITHUB_ACTIONS` 时默认硬门禁。
///   Note: `ROUTER_RS_CLOSEOUT_ENFORCEMENT` **set to empty string** is still “set” for this branch
///   resolution — it does **not** receive the unset/local-soft treatment.
/// - `Err(reason)` only when:
///   - status claims completion but no `closeout_record` is provided, or
///   - status claims completion and the provided record fails evaluation
///     (`closeout_allowed=false` or parse error).
fn enforce_closeout_for_session_payload(payload: &Value) -> Result<Option<Value>, String> {
    let status_lower = value_text(payload.get("status")).to_ascii_lowercase();
    let claims_completion = CLOSEOUT_COMPLETION_STATUSES
        .iter()
        .any(|allowed| *allowed == status_lower);
    if !claims_completion {
        return Ok(None);
    }
    if closeout_enforcement_disabled_by_env() {
        return Ok(None);
    }
    let closeout_record = payload.get("closeout_record").cloned().ok_or_else(|| {
        "framework session artifact write claims completion (status in {completed,done,passed,...}) but no closeout_record was provided. \
         A closeout record is required so closeout_enforcement can verify completion evidence (verification_status, commands_run, artifacts_checked, summary). \
         Re-issue the request with a closeout_record matching configs/framework/CLOSEOUT_RECORD_SCHEMA.json.".to_string()
    })?;
    // Try to attach an EvidenceContext so R8 (`claimed_passed_without_evidence_index_rows`) runs.
    // Both repo_root and task_id must resolve from the write payload; otherwise fall back to the
    // record-only evaluator (R7 still catches the most common self-attestation pattern).
    let repo_root_str = value_text(payload.get("repo_root"));
    let task_id_str = value_text(payload.get("task_id"));
    let evaluation = if !repo_root_str.is_empty() && !task_id_str.is_empty() {
        let repo_root = PathBuf::from(&repo_root_str);
        let (rows_non_empty, has_success) =
            crate::autopilot_goal::task_evidence_artifacts_summary_for_task(
                &repo_root,
                &task_id_str,
            );
        let ctx = CloseoutEvidenceContext {
            evidence_rows_non_empty: rows_non_empty,
            has_successful_verification: has_success,
        };
        evaluate_closeout_record_value_with_context(closeout_record, &ctx)
            .map_err(|err| format!("closeout enforcement failed: {err}"))?
    } else {
        evaluate_closeout_record_value(closeout_record)
            .map_err(|err| format!("closeout enforcement failed: {err}"))?
    };
    let allowed = evaluation
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !allowed {
        let violations = evaluation
            .get("violations")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        let missing = evaluation
            .get("missing_evidence")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        return Err(format!(
            "closeout_enforcement blocked completion: closeout_allowed=false. \
             violations={violations} missing_evidence={missing}. \
             Resolve violations or downgrade status before re-issuing the artifact write."
        ));
    }
    Ok(Some(evaluation))
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

fn supervisor_contract(state: &Map<String, Value>) -> Map<String, Value> {
    state
        .get("execution_contract")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn is_terminal(value: &str, terminal_values: &[&str]) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    terminal_values
        .iter()
        .any(|candidate| lowered == *candidate)
}

#[cfg(test)]
mod resolve_repo_root_tests {
    use super::resolve_repo_root_arg;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn resolve_repo_root_walks_up_from_scripts_router_rs_subdir() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-root-resolve-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        fs::write(
            tmp.join("configs/framework/RUNTIME_REGISTRY.json"),
            r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{}}"#,
        )
        .unwrap();
        fs::create_dir_all(tmp.join("scripts/router-rs/src")).unwrap();
        fs::write(
            tmp.join("scripts/router-rs/Cargo.toml"),
            "[package]\nname = \"router-rs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let subdir = tmp.join("scripts/router-rs/src");
        let resolved = resolve_repo_root_arg(Some(subdir.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_unchanged_when_no_framework_markers() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-no-marker-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let resolved = resolve_repo_root_arg(Some(tmp.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_from_cargo_manifest_dir_matches_framework_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let resolved = resolve_repo_root_arg(Some(manifest_dir.as_path())).unwrap();
        let expect = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("skill repo root should resolve");
        assert_eq!(
            resolved, expect,
            "router-rs crate cwd must resolve to framework repo root for continuity/RUNTIME_REGISTRY"
        );
    }
}
