// Task pointer management: read/write/neutralize active_task.json and focus_task.json pointers.
// Extracted from state_manager.rs during module split.

use crate::utils::atomic_write::write_atomic_json;
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
    read_pointer_task_id(repo_root, "active_task_id", "active_task.json")
}

pub fn read_focus_task_id(repo_root: &Path) -> Option<String> {
    read_pointer_task_id(repo_root, "focus_task_id", "focus_task.json")
}

fn read_pointer_task_id(repo_root: &Path, key: &str, legacy_filename: &str) -> Option<String> {
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
        // Fallback: TASK_POINTERS.json has `tasks` array but no top-level key.
        // Try `tasks[0].task_id` as the implied focus task.
        if key == "focus_task_id" {
            if let Some(arr) = data.get("tasks").and_then(Value::as_array) {
                if let Some(first) = arr.first() {
                    if let Some(tid) = first.get("task_id").and_then(Value::as_str) {
                        let tid = tid.trim().to_string();
                        if !tid.is_empty() {
                            let _ = crate::utils::path_guard::safe_task_id_component(&tid)?;
                            return Some(tid);
                        }
                    }
                }
            }
        }
    }
    // Fallback: legacy single-file format (active_task.json / focus_task.json)
    let path = repo_root.join("artifacts/current").join(legacy_filename);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Some(tid) = parse_task_id_from_pointer_json(&raw) {
            return Some(tid);
        }
    }
    // Fallback: task_registry.json's focus_task_id
    if key == "focus_task_id" {
        let registry_path = repo_root.join("artifacts/current/task_registry.json");
        if let Ok(raw) = fs::read_to_string(&registry_path) {
            if let Ok(data) = serde_json::from_str::<Value>(&raw) {
                if let Some(tid) = data.get("focus_task_id").and_then(Value::as_str) {
                    let tid = tid.trim().to_string();
                    if !tid.is_empty() {
                        let _ = crate::utils::path_guard::safe_task_id_component(&tid)?;
                        return Some(tid);
                    }
                }
            }
        }
    }
    None
}

/// Read `active_task.json` and `focus_task.json` task ids in one back-to-back pair (smaller
/// TOCTOU window than two independent helper calls). Used by [`crate::task_state::read_task_pointers`]
/// and hydration entrypoints.
pub fn read_task_pointer_pair(repo_root: &Path) -> (Option<String>, Option<String>) {
    // Try TASK_POINTERS.json first (Phase 3C consolidated file)
    let pointers_path = repo_root.join("artifacts/current/TASK_POINTERS.json");
    if pointers_path.is_file()
        && let Ok(raw) = fs::read_to_string(&pointers_path)
            && let Ok(data) = serde_json::from_str::<Value>(&raw) {
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
    // Fallback: legacy 3-file format
    let active_path = repo_root.join("artifacts/current/active_task.json");
    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    let active_raw = fs::read_to_string(&active_path).ok();
    let focus_raw = fs::read_to_string(&focus_path).ok();
    let active_task_id = active_raw
        .as_deref()
        .and_then(parse_task_id_from_pointer_json);
    let focus_task_id = focus_raw
        .as_deref()
        .and_then(parse_task_id_from_pointer_json);
    if active_task_id.is_some() || focus_task_id.is_some() {
        return (active_task_id, focus_task_id);
    }
    // Fallback: try TASK_POINTERS.json `tasks[0]` as implied focus
    let focus_from_tasks = (|| -> Option<String> {
        let raw = fs::read_to_string(&pointers_path).ok()?;
        let data: Value = serde_json::from_str(&raw).ok()?;
        let arr = data.get("tasks").and_then(Value::as_array)?;
        let first = arr.first()?;
        let tid = first.get("task_id")?.as_str()?.trim().to_string();
        if tid.is_empty() { return None; }
        crate::utils::path_guard::safe_task_id_component(&tid)?;
        Some(tid)
    })();
    if let Some(tid) = focus_from_tasks {
        return (None, Some(tid));
    }
    // Fallback: task_registry.json focus_task_id
    let registry_path = repo_root.join("artifacts/current/task_registry.json");
    let focus_from_registry = (|| -> Option<String> {
        let raw = fs::read_to_string(&registry_path).ok()?;
        let data: Value = serde_json::from_str(&raw).ok()?;
        let tid = data.get("focus_task_id")?.as_str()?.trim().to_string();
        if tid.is_empty() { return None; }
        crate::utils::path_guard::safe_task_id_component(&tid)?;
        Some(tid)
    })();
    (None, focus_from_registry)
}

fn pointer_file_matches_task_id(path: &Path, task_id: &str) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| parse_task_id_from_pointer_json(&raw))
        .is_some_and(|id| id == task_id)
}

/// Load TASK_POINTERS.json, creating parent dirs if needed.
fn load_task_pointers_json(repo_root: &Path) -> Result<Value, String> {
    let mirror = repo_root.join("artifacts/current");
    fs::create_dir_all(&mirror).map_err(|e| format!("mkdir {}: {e}", mirror.display()))?;
    let pointers_path = mirror.join("TASK_POINTERS.json");
    Ok(match fs::read_to_string(&pointers_path) {
        Ok(raw) => serde_json::from_str::<Value>(&raw)
            .map_err(|e| format!("parse TASK_POINTERS.json: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(format!("read TASK_POINTERS.json: {e}")),
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
                "updated_at": updated_at,
            }));
        }
    } else {
        obj.insert(
            "tasks".to_string(),
            json!([{
                "task_id": task_id,
                "task": task_label,
                "updated_at": updated_at,
            }]),
        );
    }
}

/// Write `artifacts/current/active_task.json` (`{"task_id":…}` only).
pub fn write_active_task_pointer(repo_root: &Path, task_id: &str) -> Result<(), String> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    if let Some(obj) = pointers.as_object_mut() {
        obj.insert("schema_version".to_string(), json!("task-pointers-v1"));
        obj.insert("active_task_id".to_string(), json!(task_id));
    }
    write_atomic_json(
        &repo_root.join("artifacts/current/TASK_POINTERS.json"),
        &pointers,
    )
}

fn write_focus_task_pointer_minimal(
    repo_root: &Path,
    task_id: &str,
    task_label: &str,
) -> Result<(), String> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    let updated_at = framework_kernel::time::now_iso();
    if let Some(obj) = pointers.as_object_mut() {
        obj.insert("schema_version".to_string(), json!("task-pointers-v1"));
        obj.insert("focus_task_id".to_string(), json!(task_id));
        upsert_tasks_array_entry(obj, task_id, task_label, &updated_at);
    }
    write_atomic_json(
        &repo_root.join("artifacts/current/TASK_POINTERS.json"),
        &pointers,
    )
}

/// Atomically write both `active_task_id` and `focus_task_id` in one operation,
/// including the `tasks` array label update. Prefer this over two independent
/// `write_active_task_pointer` + `write_focus_task_pointer_minimal` calls.
pub fn set_task_focus(
    repo_root: &Path,
    task_id: &str,
    task_label: &str,
) -> Result<(), String> {
    crate::utils::path_guard::validate_task_id_component(task_id)?;
    let mut pointers = load_task_pointers_json(repo_root)?;
    let updated_at = framework_kernel::time::now_iso();
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
pub fn sync_task_pointers_after_goal_drive(
    repo_root: &Path,
    task_id: &str,
    goal_label: &str,
    payload: &Value,
) -> Result<(), String> {
    write_active_task_pointer(repo_root, task_id)?;
    if goal_drive_set_focus_from_payload(payload) {
        write_focus_task_pointer_minimal(repo_root, task_id, goal_label)?;
    }
    Ok(())
}

/// Remove active/focus pointers when they reference `task_id` (complete / clear).
pub fn neutralize_task_pointers_for_task(repo_root: &Path, task_id: &str) -> Result<(), String> {
    // Update TASK_POINTERS.json (Phase 3C consolidated file)
    let pointers_path = repo_root.join("artifacts/current/TASK_POINTERS.json");
    if pointers_path.is_file() {
        let raw = fs::read_to_string(&pointers_path)
            .map_err(|e| format!("read TASK_POINTERS.json: {e}"))?;
        let mut data: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("parse TASK_POINTERS.json: {e}"))?;
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
                tasks.retain(|entry| {
                    entry.get("task_id").and_then(Value::as_str) != Some(task_id)
                });
                if tasks.len() != len_before {
                    changed = true;
                }
            }
        }
        if changed {
            write_atomic_json(&pointers_path, &data)
                .map_err(|e| format!("write TASK_POINTERS.json: {e}"))?;
        }
    }
    // Also clean up legacy files if they exist
    let active_path = repo_root.join("artifacts/current/active_task.json");
    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    if pointer_file_matches_task_id(&active_path, task_id) {
        fs::remove_file(&active_path)
            .map_err(|e| format!("remove legacy active_task.json: {e}"))?;
    }
    if pointer_file_matches_task_id(&focus_path, task_id) {
        fs::remove_file(&focus_path)
            .map_err(|e| format!("remove legacy focus_task.json: {e}"))?;
    }
    Ok(())
}

pub fn ensure_task_directory(
    repo_root: &Path,
    task_id: &str,
) -> Result<std::path::PathBuf, String> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    let task_dir = repo_root.join("artifacts/current").join(tid);
    if !task_dir.is_dir() {
        fs::create_dir_all(&task_dir)
            .map_err(|e| format!("failed to create task directory '{}': {e}", tid))?;
    }
    Ok(task_dir)
}
