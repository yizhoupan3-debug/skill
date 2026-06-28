use core_errors::FrameworkError;
use fr_utils::constants::{
    CURRENT_ARTIFACT_DIR, EVIDENCE_INDEX_FILENAME, EVIDENCE_INDEX_SCHEMA_VERSION,
    FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY, FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
    TASK_POINTERS_FILENAME, TASK_POINTERS_SCHEMA_VERSION, TERMINAL_STORY_STATES,
    TERMINAL_VERIFICATION_STATUSES,
};
use fr_utils::json_io::read_json_strict;
use fr_utils::json_value::{build_task_id, safe_slug, value_bool_or_none, value_text};
use fr_utils::types::TaskRegistryEntry;
use fr_utils::util::{defaulted_payload_text, required_payload_text};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn resolve_session_repo_root_for_task_ledger(
    payload: &Value,
) -> Result<Option<PathBuf>, FrameworkError> {
    let rr = value_text(payload.get("repo_root"));
    if rr.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(&rr);
    if !path.is_dir() {
        fs::create_dir_all(&path)?;
    }
    Ok(Some(framework_kernel::repo_roots::resolve_repo_root_arg(
        Some(path.as_path()),
    )?))
}

/// Write framework session artifacts — limited to TASK_POINTERS.json + EVIDENCE_INDEX.json.
///
/// Previous iterations also wrote SESSION_SUMMARY.md, NEXT_ACTIONS.json,
/// TRACE_METADATA.json, .supervisor_state.json, active_task.json, focus_task.json,
/// and task_registry.json. All removed per v10 Wave 2c.
pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, FrameworkError> {
    let run = || -> Result<Value, FrameworkError> {
        let output_dir = value_text(payload.get("output_dir"));
        if output_dir.is_empty() {
            return Err(FrameworkError::validation(
                "framework session artifact writer requires output_dir",
            ));
        }
        let task = required_payload_text(&payload, "task", "framework session artifact writer")?;
        let phase = defaulted_payload_text(&payload, "phase", "implementation");
        let status = defaulted_payload_text(&payload, "status", "in_progress");
        let task_id = resolve_session_task_id(&payload, &task);
        let focus = value_bool_or_none(payload.get("focus")).unwrap_or(false);
        let update_registry_only_if_known =
            value_bool_or_none(payload.get("update_registry_only_if_known")).unwrap_or(false);
        let repo_root = value_text(payload.get("repo_root"));
        let output_root = PathBuf::from(&output_dir);
        let primary_dir = if payload.get("task_id").is_some() || !repo_root.is_empty() {
            output_root.join(&task_id)
        } else {
            output_root.clone()
        };
        let evidence_path = primary_dir.join(EVIDENCE_INDEX_FILENAME);
        let mut changed_paths: Vec<String> = Vec::new();

        // ── EVIDENCE_INDEX.json (when payload has evidence array) ──
        if let Some(evidence_items) = payload.get("evidence").and_then(Value::as_array) {
            let evidence_payload = build_evidence_index_payload(evidence_items);
            if let Some(parent) = evidence_path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_json_if_changed(&evidence_path, &evidence_payload)?;
            changed_paths.push(evidence_path.display().to_string());
        }

        // ── TASK_POINTERS.json entry ──
        let mirror_root = output_root.join(CURRENT_ARTIFACT_DIR);
        let updated_at = framework_kernel::time::current_local_timestamp();
        let registry_known = task_id_known_in_task_pointers(&mirror_root, &task_id);
        let should_touch_registry = !update_registry_only_if_known || registry_known;
        if should_touch_registry {
            if write_task_pointers_entry(
                &mirror_root,
                TaskRegistryEntry {
                    task_id: &task_id,
                    task: &task,
                    phase: &phase,
                    status: &status,
                    resume_allowed: Some(
                        !crate::util::is_terminal(&status, TERMINAL_VERIFICATION_STATUSES)
                            && !crate::util::is_terminal(&status, TERMINAL_STORY_STATES),
                    ),
                    updated_at: &updated_at,
                    focus_task_id: if focus { Some(task_id.as_str()) } else { None },
                },
            )? {
                changed_paths.push(
                    mirror_root
                        .join(TASK_POINTERS_FILENAME)
                        .display()
                        .to_string(),
                );
            }
        }

        Ok(json!({
            "ok": true,
            "schema_version": FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
            "task_id": task_id,
            "task": task,
            "phase": phase,
            "status": status,
            "changed_paths": changed_paths,
        }))
    };
    match resolve_session_repo_root_for_task_ledger(&payload)? {
        Some(resolved) => {
            let _guard = core_state_utils::task_write_lock::acquire_task_ledger_repo_lock(
                &resolved,
                std::time::Duration::from_secs(30),
            )?;
            run()
        }
        None => run(),
    }
}

fn resolve_session_task_id(payload: &Value, task: &str) -> String {
    let direct = safe_slug(&value_text(payload.get("task_id")));
    let candidate = if direct.is_empty() {
        build_task_id(task, None)
    } else {
        direct
    };
    if is_unsafe_task_id_slug(&candidate) {
        build_task_id(task, None)
    } else {
        candidate
    }
}

fn is_unsafe_task_id_slug(slug: &str) -> bool {
    slug.is_empty()
        || slug.contains("..")
        || slug.contains('/')
        || slug.contains('\\')
        || slug.starts_with('.')
}

fn build_evidence_index_payload(entries: &[Value]) -> Value {
    json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": entries,
    })
}

fn task_id_known_in_task_pointers(mirror_root: &Path, task_id: &str) -> bool {
    let registry_path = mirror_root.join(TASK_POINTERS_FILENAME);
    let Ok(existing) = read_json_strict(&registry_path) else {
        return false;
    };
    crate::util::registry_rows_from_payload(&existing)
        .iter()
        .any(|row| safe_slug(&value_text(row.get("task_id"))) == task_id)
}

fn write_task_pointers_entry(
    mirror_root: &Path,
    entry: TaskRegistryEntry<'_>,
) -> Result<bool, FrameworkError> {
    let existing =
        read_json_strict(&mirror_root.join(TASK_POINTERS_FILENAME)).unwrap_or_else(|_| json!({}));
    let focus_task = entry.focus_task_id.map_or_else(
        || safe_slug(&value_text(existing.get("focus_task_id"))),
        ToString::to_string,
    );
    let mut rows = crate::util::registry_rows_from_payload(&existing);
    let mut replaced = false;
    for row in &mut rows {
        let Some(map) = row.as_object_mut() else {
            continue;
        };
        if safe_slug(&value_text(map.get("task_id"))) != entry.task_id {
            continue;
        }
        map.insert(
            "task_id".to_string(),
            Value::String(entry.task_id.to_string()),
        );
        map.insert("task".to_string(), Value::String(entry.task.to_string()));
        map.insert(
            "updated_at".to_string(),
            Value::String(entry.updated_at.to_string()),
        );
        map.insert(
            "status".to_string(),
            Value::String(entry.status.to_string()),
        );
        map.insert("phase".to_string(), Value::String(entry.phase.to_string()));
        map.insert(
            "resume_allowed".to_string(),
            entry.resume_allowed.map_or(Value::Null, Value::Bool),
        );
        replaced = true;
        break;
    }
    if !replaced {
        rows.push(json!({
            "task_id": entry.task_id,
            "task": entry.task,
            "updated_at": entry.updated_at,
            "status": entry.status,
            "phase": entry.phase,
            "resume_allowed": entry.resume_allowed,
        }));
    }
    let compacted = crate::util::normalize_task_registry_rows(focus_task, rows).0;
    // Merge compacted registry back into TASK_POINTERS.json
    let mut out = json!({
        "schema_version": TASK_POINTERS_SCHEMA_VERSION,
    });
    if let Some(obj) = out.as_object_mut() {
        if let Some(tid) = existing.get("active_task_id") {
            obj.insert("active_task_id".to_string(), tid.clone());
        }
        if let Some(ftid) = compacted.get("focus_task_id") {
            obj.insert("focus_task_id".to_string(), ftid.clone());
        }
        if let Some(tasks) = compacted.get("tasks") {
            obj.insert("tasks".to_string(), tasks.clone());
        }
    }
    write_json_if_changed(&mirror_root.join(TASK_POINTERS_FILENAME), &out)
}

pub(super) use core_state_utils::json_io::write_json_if_changed;
