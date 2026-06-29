// Task pointer management: read/write/neutralize active_task.json and focus_task.json pointers.
// Extracted from state_manager.rs during module split.

use crate::utils::atomic_write::write_atomic_json;
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn parse_task_id_from_pointer_json(raw: &str) -> Option<String> {
    let data: Value = serde_json::from_str(raw).ok()?;
    let t = data
        .get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    crate::utils::path_guard::safe_task_id_component(&t)?;
    Some(t)
}

pub fn read_primary_task_id(repo_root: &Path) -> Option<String> {
    let (active, focus) = read_task_pointer_pair(repo_root);
    active.or(focus)
}

pub fn read_active_task_id(repo_root: &Path) -> Option<String> {
    read_pointer_task_id(repo_root, "active_task_id")
}

pub fn read_focus_task_id(repo_root: &Path) -> Option<String> {
    read_pointer_task_id(repo_root, "focus_task_id")
}

fn read_pointer_task_id(repo_root: &Path, key: &str) -> Option<String> {
    let pointers_path = repo_root.join("artifacts/current/TASK_POINTERS.json");
    if pointers_path.is_file() {
        let raw = fs::read_to_string(&pointers_path).ok()?;
        let data: Value = serde_json::from_str(&raw).ok()?;
        if let Some(tid) = data.get(key).and_then(Value::as_str) {
            let tid = tid.trim().to_string();
            if !tid.is_empty() {
                crate::utils::path_guard::safe_task_id_component(&tid)?;
                return Some(tid);
            }
        }
    }
    None
}

/// Read active and focus task IDs from the consolidated TASK_POINTERS.json.
/// Used by [`crate::task_state::read_task_pointers`] and hydration entrypoints.
pub fn read_task_pointer_pair(repo_root: &Path) -> (Option<String>, Option<String>) {
    let pointers_path = repo_root.join("artifacts/current/TASK_POINTERS.json");
    if pointers_path.is_file()
        && let Ok(raw) = fs::read_to_string(&pointers_path)
        && let Ok(data) = serde_json::from_str::<Value>(&raw)
    {
        let parse = |key: &str| -> Option<String> {
            let tid = data.get(key)?.as_str()?.trim().to_string();
            if tid.is_empty() {
                return None;
            }
            crate::utils::path_guard::safe_task_id_component(&tid)?;
            Some(tid)
        };
        let active = parse("active_task_id");
        let focus = parse("focus_task_id");
        if active.is_some() || focus.is_some() {
            return (active, focus);
        }
    }
    (None, None)
}

fn pointer_file_matches_task_id(path: &Path, task_id: &str) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| parse_task_id_from_pointer_json(&raw))
        .is_some_and(|id| id == task_id)
}

/// Load TASK_POINTERS.json, creating parent dirs if needed.
fn load_task_pointers_json(repo_root: &Path) -> Result<Value, FrameworkError> {
    let mirror = repo_root.join("artifacts/current");
    fs::create_dir_all(&mirror)?;
    let pointers_path = mirror.join("TASK_POINTERS.json");
    Ok(match fs::read_to_string(&pointers_path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "TASK_POINTERS.json parse failed ({}), resetting to empty state",
                    e
                );
                json!({})
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(FrameworkError::Io(e)),
    })
}

/// Upsert a single entry in the TASK_POINTERS.json `tasks` array.
/// Updates existing entry by `task_id` match, or pushes a new entry.
fn upsert_tasks_array_entry(
    obj: &mut serde_json::Map<String, Value>,
    task_id: &str,
    task_label: &str,
    updated_at: &str,
) {
    if let Some(tasks) = obj.get_mut("tasks").and_then(|v| v.as_array_mut()) {
        let mut found = false;
        for entry in tasks.iter_mut() {
            if entry.get("task_id").and_then(Value::as_str) == Some(task_id) {
                if let Some(e) = entry.as_object_mut() {
                    e.insert("task".to_string(), json!(task_label));
                    e.insert("label".to_string(), json!(task_label));
                    e.insert("updated_at".to_string(), json!(updated_at));
                }
                found = true;
                break;
            }
        }
        if !found {
            tasks.push(json!({
                "task_id": task_id,
                "task": task_label,
                "label": task_label,
                "updated_at": updated_at,
            }));
        }
    } else {
        obj.insert(
            "tasks".to_string(),
            json!([{
                "task_id": task_id,
                "task": task_label,
                "label": task_label,
                "updated_at": updated_at,
            }]),
        );
    }
}

/// Write `artifacts/current/active_task.json` (`{"task_id":…}` only).
/// Always upserts the `tasks` array to prevent D5 auto-pass vulnerability
/// (validate_complete_transition checks task existence via GOAL_STATE.json,
/// but maintaining a consistent tasks array is good practice).
pub fn write_active_task_pointer(repo_root: &Path, task_id: &str) -> Result<(), FrameworkError> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    let updated_at = framework_core::time::now_iso();
    if let Some(obj) = pointers.as_object_mut() {
        obj.insert("schema_version".to_string(), json!("task-pointers-v1"));
        obj.insert("active_task_id".to_string(), json!(task_id));
        upsert_tasks_array_entry(obj, task_id, task_id, &updated_at);
    }
    write_atomic_json(
        &repo_root.join("artifacts/current/TASK_POINTERS.json"),
        &pointers,
    )
}

/// Atomically write both `active_task_id` and `focus_task_id` in one operation,
pub fn set_task_focus(
    repo_root: &Path,
    task_id: &str,
    task_label: &str,
) -> Result<(), FrameworkError> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    let updated_at = framework_core::time::now_iso();
    if let Some(obj) = pointers.as_object_mut() {
        obj.insert("schema_version".to_string(), json!("task-pointers-v1"));
        obj.insert("active_task_id".to_string(), json!(task_id));
        obj.insert("focus_task_id".to_string(), json!(task_id));
        upsert_tasks_array_entry(obj, task_id, task_label, &updated_at);
    }
    write_atomic_json(
        &repo_root.join("artifacts/current/TASK_POINTERS.json"),
        &pointers,
    )
}

fn goal_drive_set_focus_from_payload(payload: &Value) -> bool {
    payload
        .get("set_focus")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// After `start`/`resume`, keep continuity pointers aligned with the task that owns GOAL_STATE.
/// Single atomic write: sets active_task_id, conditionally sets focus_task_id,
/// and always updates the tasks array. Avoids the intermediate state that
/// two separate writes would expose to readers (P2-002).
pub fn sync_task_pointers_after_goal_drive(
    repo_root: &Path,
    task_id: &str,
    goal_label: &str,
    payload: &Value,
) -> Result<(), FrameworkError> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    let updated_at = framework_core::time::now_iso();
    if let Some(obj) = pointers.as_object_mut() {
        obj.insert("schema_version".to_string(), json!("task-pointers-v1"));
        obj.insert("active_task_id".to_string(), json!(task_id));
        upsert_tasks_array_entry(obj, task_id, goal_label, &updated_at);
        if goal_drive_set_focus_from_payload(payload) {
            obj.insert("focus_task_id".to_string(), json!(task_id));
        }
    }
    write_atomic_json(
        &repo_root.join("artifacts/current/TASK_POINTERS.json"),
        &pointers,
    )
}

/// Remove active/focus pointers when they reference `task_id` (complete / clear).
pub fn neutralize_task_pointers_for_task(
    repo_root: &Path,
    task_id: &str,
) -> Result<(), FrameworkError> {
    // Update TASK_POINTERS.json (Phase 3C consolidated file)
    let pointers_path = repo_root.join("artifacts/current/TASK_POINTERS.json");
    if pointers_path.is_file() {
        let raw = fs::read_to_string(&pointers_path).map_err(FrameworkError::Io)?;
        let mut data: Value = serde_json::from_str(&raw).unwrap_or_else(|e| {
            tracing::warn!("TASK_POINTERS.json parse failed in neutralize ({e}), treating as empty");
            json!({})
        });
        let mut changed = false;
        if let Some(obj) = data.as_object_mut() {
            if obj.get("active_task_id").and_then(Value::as_str) == Some(task_id) {
                obj.remove("active_task_id");
                changed = true;
            }
            if obj.get("focus_task_id").and_then(Value::as_str) == Some(task_id) {
                obj.remove("focus_task_id");
                changed = true;
            }
            // Also remove from `tasks` array to prevent fallback reads
            // (read_task_pointer_pair falls back to tasks[0].task_id)
            if let Some(tasks) = obj.get_mut("tasks").and_then(|v| v.as_array_mut()) {
                let len_before = tasks.len();
                tasks.retain(|entry| entry.get("task_id").and_then(Value::as_str) != Some(task_id));
                if tasks.len() != len_before {
                    changed = true;
                }
            }
        }
        if changed {
            write_atomic_json(&pointers_path, &data)?;
        }
    }
    // Also clean up legacy files if they exist
    let active_path = repo_root.join("artifacts/current/active_task.json");
    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    if pointer_file_matches_task_id(&active_path, task_id) {
        fs::remove_file(&active_path).map_err(FrameworkError::Io)?;
    }
    if pointer_file_matches_task_id(&focus_path, task_id) {
        fs::remove_file(&focus_path).map_err(FrameworkError::Io)?;
    }
    Ok(())
}

pub fn ensure_task_directory(
    repo_root: &Path,
    task_id: &str,
) -> Result<std::path::PathBuf, FrameworkError> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    let task_dir = repo_root.join("artifacts/current").join(tid);
    if !task_dir.is_dir() {
        fs::create_dir_all(&task_dir)?;
    }
    Ok(task_dir)
}
