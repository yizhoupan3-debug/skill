//! Framework runtime snapshot building.
//!
//! Functions for building the runtime snapshot envelope (`build_framework_runtime_snapshot_envelope`
//! and `build_framework_runtime_snapshot_envelope_with_level`) as well as internal helpers.

use framework_runtime::constants::{
    FRAMEWORK_RUNTIME_AUTHORITY, FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION, SESSION_SUMMARY_FILENAME,
    NEXT_ACTIONS_FILENAME, EVIDENCE_INDEX_FILENAME, TRACE_METADATA_FILENAME,
    SUPERVISOR_STATE_FILENAME, TASK_REGISTRY_SCHEMA_VERSION,
};
use framework_runtime::json_value::{nonempty_string, value_text};
use framework_runtime::runtime_view;
use serde_json::{Value, json};
use std::path::Path;
use tracing::instrument;

use crate::util::{count_evidence_rows, truncate_utf8_chars};

#[instrument(level = "debug", skip_all)]
pub fn build_framework_runtime_snapshot_envelope(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> Result<Value, String> {
    build_framework_runtime_snapshot_envelope_with_level(
        repo_root,
        artifact_root_override,
        task_id_override,
        "summary",
    )
}

/// Detail level for snapshot output: "summary" (compact, default) or "full".
#[instrument(level = "debug", skip_all, fields(detail_level))]
pub fn build_framework_runtime_snapshot_envelope_with_level(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
    detail_level: &str,
) -> Result<Value, String> {
    let is_full = detail_level == "full";
    let snapshot =
        runtime_view::load_framework_runtime_view(repo_root, artifact_root_override, task_id_override);
    let continuity = runtime_view::classify_runtime_continuity(&snapshot);
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity
                .get("route")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .map(|item| value_text(Some(item)))
        } else {
            Some(direct)
        }
    };
    let verification_status = snapshot
        .supervisor_state
        .get("verification")
        .and_then(Value::as_object)
        .and_then(|verification| nonempty_string(verification.get("verification_status")));

    // --- known_task_ids: summary keeps recent 3 (still an array for backward compat) ---
    let known_task_ids = if is_full {
        Value::Array(
            snapshot
                .known_task_ids
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        )
    } else {
        let recent: Vec<Value> = snapshot
            .known_task_ids
            .iter()
            .take(3)
            .map(|s| Value::String(s.clone()))
            .collect();
        Value::Array(recent)
    };

    // --- registered_tasks: summary strips task description to 80 chars ---
    let registered_tasks = if is_full {
        snapshot.registered_tasks.clone()
    } else {
        build_compact_registered_tasks(&snapshot.registered_tasks)
    };

    // --- continuity: summary strips empty arrays/null fields ---
    let continuity_value = if is_full {
        continuity.clone()
    } else {
        build_compact_continuity(&continuity)
    };

    // --- paths: summary omits full paths map ---
    let paths_value = if is_full {
        json!({
            "session_summary": snapshot.current_root.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": snapshot.current_root.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": snapshot.current_root.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": snapshot.current_root.join(TRACE_METADATA_FILENAME).display().to_string(),
            "current_pointer_root": snapshot.mirror_root.display().to_string(),
            "supervisor_state": repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
        })
    } else {
        json!({
            "artifact_base": snapshot.artifact_base.display().to_string(),
        })
    };

    // --- control_plane_missing: summary skips if empty ---
    let control_plane_missing = runtime_view::missing_control_plane_anchors(&snapshot);
    let control_plane_missing_value = if is_full || !control_plane_missing.is_empty() {
        json!(control_plane_missing)
    } else {
        Value::Array(vec![])
    };

    // --- control_plane_inconsistency_reasons: summary skips if empty ---
    let cp_inconsistency = &snapshot.control_plane_inconsistency_reasons;
    let cp_inconsistency_value = if is_full || !cp_inconsistency.is_empty() {
        json!(cp_inconsistency)
    } else {
        Value::Array(vec![])
    };

    // --- recoverable_task_ids: summary skips if empty ---
    let recoverable_value = if is_full || !snapshot.recoverable_task_ids.is_empty() {
        json!(snapshot.recoverable_task_ids)
    } else {
        Value::Array(vec![])
    };

    let mut runtime_snapshot = json!({
        "ok": true,
        "workspace": framework_runtime::runtime_view::workspace_name_from_root(repo_root),
        "detail_level": if is_full { "full" } else { "summary" },
        "control_plane_present": snapshot.task_pointers_present
            && !snapshot.supervisor_state.is_empty(),
        "active_task_id": snapshot.active_task_id,
        "focus_task_id": snapshot.focus_task_id,
        "known_task_ids": known_task_ids,
        "parallel_task_count": snapshot.known_task_ids.len(),
        "registered_tasks": registered_tasks,
        "collected_at": snapshot.collected_at,
        "session_summary_present": !snapshot.session_summary_text.trim().is_empty(),
        "next_action_count": continuity
            .get("next_actions")
            .and_then(Value::as_array)
            .map(|rows| rows.len())
            .unwrap_or(0),
        "evidence_count": count_evidence_rows(&snapshot.evidence_index),
        "trace_skill_count": continuity.get("route").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "continuity": continuity_value,
        "supervisor_state": {
            "task_id": nonempty_string(snapshot.supervisor_state.get("task_id")),
            "task_summary": nonempty_string(snapshot.supervisor_state.get("task_summary")),
            "active_phase": nonempty_string(snapshot.supervisor_state.get("active_phase")),
            "primary_owner": primary_owner,
            "verification_status": verification_status,
        },
        "paths": paths_value,
        "code_index": codegraph_index_snapshot(repo_root),
    });

    // Only include these fields when non-empty (both modes) or in full mode
    if is_full {
        // In full mode, always include verbose path fields
        if let Some(obj) = runtime_snapshot.as_object_mut() {
            obj.insert(
                "artifact_base".to_string(),
                json!(snapshot.artifact_base.display().to_string()),
            );
            obj.insert(
                "current_root".to_string(),
                json!(snapshot.current_root.display().to_string()),
            );
            obj.insert(
                "mirror_root".to_string(),
                json!(snapshot.mirror_root.display().to_string()),
            );
            obj.insert(
                "task_root".to_string(),
                json!(snapshot.task_root.display().to_string()),
            );
            obj.insert(
                "control_plane_missing".to_string(),
                control_plane_missing_value,
            );
            obj.insert(
                "control_plane_inconsistency_reasons".to_string(),
                cp_inconsistency_value,
            );
            obj.insert("recoverable_task_ids".to_string(), recoverable_value);
        }
    } else if let Some(obj) = runtime_snapshot.as_object_mut() {
        // In summary mode, only include these if they have content
        if !control_plane_missing.is_empty() {
            obj.insert(
                "control_plane_missing".to_string(),
                control_plane_missing_value,
            );
        }
        if !cp_inconsistency.is_empty() {
            obj.insert(
                "control_plane_inconsistency_reasons".to_string(),
                cp_inconsistency_value,
            );
        }
        if !snapshot.recoverable_task_ids.is_empty() {
            obj.insert("recoverable_task_ids".to_string(), recoverable_value);
        }
        obj.insert("_truncated".to_string(), json!(true));
    }

    Ok(json!({
        "schema_version": FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "runtime_snapshot": runtime_snapshot,
    }))
}

/// Compact version of registered_tasks for summary mode:
/// keeps `status`, `task` (truncated to 80 chars), `updated_at`, and `task_id`.
fn build_compact_registered_tasks(registered_tasks: &Value) -> Value {
    let Some(tasks) = registered_tasks
        .as_object()
        .and_then(|o| o.get("tasks"))
        .and_then(Value::as_array)
    else {
        return registered_tasks.clone();
    };
    let compact_tasks: Vec<Value> = tasks
        .iter()
        .map(|row| {
            let task_text = value_text(row.get("task"));
            let truncated = truncate_utf8_chars(&task_text, 80);
            let mut compact = json!({
                "task_id": row.get("task_id"),
                "status": row.get("status"),
                "updated_at": row.get("updated_at"),
            });
            if !truncated.is_empty() {
                compact
                    .as_object_mut()
                    .unwrap()
                    .insert("task".to_string(), Value::String(truncated));
            }
            compact
        })
        .collect();
    let task_count = registered_tasks
        .as_object()
        .and_then(|o| o.get("task_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let focus_task_id = registered_tasks
        .as_object()
        .and_then(|o| o.get("focus_task_id"))
        .cloned();
    let truncated_flag = registered_tasks
        .as_object()
        .and_then(|o| o.get("truncated"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut result = json!({
        "schema_version": registered_tasks
            .as_object()
            .and_then(|o| o.get("schema_version"))
            .cloned()
            .unwrap_or(json!(TASK_REGISTRY_SCHEMA_VERSION)),
        "task_count": task_count,
        "tasks": compact_tasks,
        "truncated": truncated_flag,
    });
    if let Some(fti) = focus_task_id {
        result
            .as_object_mut()
            .unwrap()
            .insert("focus_task_id".to_string(), fti);
    }
    result
}

/// Compact version of continuity for summary mode:
/// strips verbose/redundant fields (paths, summary_fields, nested continuity,
/// recovery_hints). Keeps all core fields including nulls and empty arrays
/// that callers test for (e.g. missing_recovery_anchors, current_execution).
fn build_compact_continuity(continuity: &Value) -> Value {
    let Some(obj) = continuity.as_object() else {
        return continuity.clone();
    };
    // Fields to drop entirely in summary mode (always verbose or redundant)
    const SKIP_KEYS: &[&str] = &[
        "paths",
        "summary_fields",
        "recovery_hints",
        "continuity", // nested inner continuity block
    ];
    let mut compact = serde_json::Map::new();
    for (key, val) in obj {
        if SKIP_KEYS.contains(&key.as_str()) {
            continue;
        }
        compact.insert(key.clone(), val.clone());
    }
    Value::Object(compact)
}

/// Build code_index snapshot from codegraph database (when feature enabled).
#[allow(unexpected_cfgs)]
#[cfg(feature = "codegraph")]
fn codegraph_index_snapshot(repo_root: &Path) -> Value {
    match codegraph_rs::CodeGraphIndex::open(repo_root) {
        Ok(index) => {
            // Index stats — best-effort; failures logged below
            let stats = match index.index_stats() {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("[codegraph] index_stats query failed: {e}");
                    None
                }
            };
            // Dead code count — lightweight COUNT(*) query, no row data transfer
            let dead_code_count = index
                .count_dead_code_only(Some("rust"))
                .ok()
                .unwrap_or(0);
            // Recent indexed files — agent gets a sense of what's covered
            let recent_files: Vec<Value> = match index.list_files() {
                Ok(files) => files
                    .into_iter()
                    .filter(|f| f.symbol_count > 0)
                    .take(5)
                    .map(|f| {
                        json!({
                            "path": f.path,
                            "language": f.language,
                            "symbol_count": f.symbol_count,
                        })
                    })
                    .collect(),
                Err(e) => {
                    tracing::warn!("[codegraph] list_files query failed: {e}");
                    vec![]
                }
            };
            // Freshness check — true if indexed within the last hour
            let fresh_enough = stats
                .as_ref()
                .and_then(|s| s.indexed_at.as_deref())
                .and_then(|t| {
                    match chrono::DateTime::parse_from_rfc3339(t) {
                        Ok(dt) => {
                            let now = chrono::Utc::now();
                            Some((now - dt.with_timezone(&chrono::Utc)).num_minutes() < 60)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[codegraph] failed to parse indexed_at ({t:?}): {e}",
                            );
                            None
                        }
                    }
                })
                .unwrap_or(false);

            json!({
                "enabled": true,
                "db_path": index.db_path().display().to_string(),
                "stats": stats,
                "dead_code_count": dead_code_count,
                "recent_files": recent_files,
                "fresh_enough": fresh_enough,
            })
        }
        Err(e) => json!({
            "enabled": false,
            "error": format!("open failed: {e}"),
        }),
    }
}

#[allow(unexpected_cfgs)]
#[cfg(not(feature = "codegraph"))]
fn codegraph_index_snapshot(_repo_root: &Path) -> Value {
    json!({"enabled": false})
}
