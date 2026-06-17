use crate::closeout_enforcement::{
    CloseoutEvidenceContext, evaluate_closeout_record_value,
    evaluate_closeout_record_value_with_context,
};
use chrono::{Local, SecondsFormat};
use tracing::instrument;
use hex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

mod alias;
pub use framework_runtime::codex_hooks_duplicate;
pub use framework_runtime::constants;
pub use framework_runtime::evolution_observer;
mod framework_doctor;
pub use framework_runtime::io_utils;
pub use framework_runtime::json_io;
pub use framework_runtime::json_value;
pub use framework_runtime::live_execute;
pub(crate) mod orchestration_controller;
pub use framework_runtime::pre_tool_use_guard;
mod prompt_compression;
pub use framework_runtime::repo_roots;
pub mod route_manifest_fallback;
pub use framework_runtime::runtime_view;
pub use framework_runtime::sandbox_control;
mod session_artifacts;
mod statusline;
pub mod stdio_dispatch;
pub use framework_runtime::stdio_op_registry;
pub use framework_runtime::trace_attach;
pub use framework_runtime::trace_stream_io;
pub use framework_runtime::trace_transport;
pub use framework_runtime::types;

use json_io::{read_json_strict, read_text_if_exists};
// Re-export json_io functions needed by cli/common.rs (cycle-breaking extraction).
pub use json_io::{parse_json_input, print_json_value};
use json_value::{
    first_nonempty, nonempty_string, safe_slug, stable_line_items, value_bool_or_none,
    value_string_list, value_text,
};

use crate::atomic_write::write_atomic_text;

pub use alias::build_framework_alias_envelope;
// Used by `crate::framework_runtime::FRAMEWORK_ALIAS_SCHEMA_VERSION` consumers; not referenced in this module body.
#[allow(unused_imports)]
pub use constants::FRAMEWORK_ALIAS_SCHEMA_VERSION;
// Retained for external callers.
pub use codex_hooks_duplicate::eprint_codex_hooks_duplicate_warnings;
#[allow(unused_imports)]
pub use constants::FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION;
pub use constants::{
    FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION, FRAMEWORK_RUNTIME_AUTHORITY,
    FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION, FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
};
// DoctorResult is re-exported for external consumers; not referenced within this module.
pub use crate::stdio_payload_types::*;
#[allow(unused_imports)]
pub use framework_doctor::{DoctorResult, run_continuity_audit, run_framework_doctor};
pub use orchestration_controller::{
    build_background_control_response, build_runtime_control_plane_payload,
    build_runtime_integrator_payload, build_runtime_metric_record,
    build_runtime_observability_exporter_descriptor, build_runtime_observability_health_snapshot,
    build_runtime_observability_metric_catalog_payload, runtime_observability_dashboard_schema,
};
#[allow(unused_imports)]
pub use pre_tool_use_guard::{
    PRE_TOOL_USE_GUARD_SCHEMA_VERSION, PRE_TOOL_USE_GUARD_STDIO_OP, PreToolUseGuardRequest,
    PreToolUseGuardResponse, PreToolUseGuardVerdict, evaluate_pre_tool_use_guard,
    evaluate_pre_tool_use_guard_value, host_requires_strict_pre_tool_fallback,
    pre_tool_use_guard_contract,
};
pub use prompt_compression::build_framework_prompt_compression_envelope;
pub use repo_roots::{
    framework_root_from_executable_path, is_framework_root, resolve_repo_root_arg,
};
pub use route_manifest_fallback::route_task_with_manifest_fallback;
pub use route_manifest_fallback::{
    manifest_fallback_path, resolve_runtime_declared_manifest_fallback,
};
pub use sandbox_control::build_sandbox_control_response;
pub use session_artifacts::write_framework_session_artifacts;
pub use statusline::build_framework_statusline;
pub use stdio_dispatch::{dispatch_stdio_json_request, dispatch_stdio_json_request_payload};
pub use stdio_op_registry::{StdioOpDomain, classify_stdio_op};
pub use types::FrameworkAliasBuildOptions;
// Public re-exports for browser-mcp crate
pub use trace_attach::attach_runtime_event_transport;
pub use trace_stream_io::{inspect_trace_stream, replay_trace_stream};
// Crate-internal re-exports
#[cfg(test)]
pub use stdio_op_registry::{
    is_framework_stdio_op, is_routing_stdio_op, is_runtime_stdio_op, is_trace_stdio_op,
};
pub use trace_attach::{
    cleanup_attached_runtime_event_transport, subscribe_attached_runtime_events,
};
pub use trace_stream_io::{sha256_hex, write_trace_compaction_delta, write_trace_metadata};

use constants::{
    CLOSEOUT_COMPLETION_STATUSES, CURRENT_ARTIFACT_DIR, EVIDENCE_INDEX_FILENAME,
    EVIDENCE_INDEX_SCHEMA_VERSION, NEXT_ACTIONS_FILENAME, SESSION_SUMMARY_FILENAME,
    SUPERVISOR_STATE_FILENAME, TASK_POINTERS_FILENAME, TASK_REGISTRY_SCHEMA_VERSION,
    TRACE_METADATA_FILENAME,
};
use types::FrameworkRuntimeView;

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
    let snapshot = load_framework_runtime_view(repo_root, artifact_root_override, task_id_override);
    let continuity = classify_runtime_continuity(&snapshot);
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
    let control_plane_missing = missing_control_plane_anchors(&snapshot);
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
        "workspace": workspace_name_from_root(repo_root),
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
            .unwrap_or(json!(constants::TASK_REGISTRY_SCHEMA_VERSION)),
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
#[cfg(feature = "codegraph")]
fn codegraph_index_snapshot(repo_root: &Path) -> Value {
    match codegraph_rs::CodeGraphIndex::open(repo_root) {
        Ok(index) => match index.index_stats() {
            Ok(stats) => json!({
                "enabled": true,
                "db_path": index.db_path().display().to_string(),
                "node_count": stats.node_count,
                "edge_count": stats.edge_count,
                "file_count": stats.file_count,
                "indexed_at": stats.indexed_at,
                "db_size_bytes": stats.db_size_bytes,
            }),
            Err(e) => json!({
                "enabled": true,
                "error": format!("stats query failed: {e}"),
            }),
        },
        Err(e) => json!({
            "enabled": false,
            "error": format!("open failed: {e}"),
        }),
    }
}

#[cfg(not(feature = "codegraph"))]
fn codegraph_index_snapshot(_repo_root: &Path) -> Value {
    json!({"enabled": false})
}

#[instrument(level = "debug", skip_all)]
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
    let evidence_count = count_evidence_rows(&snapshot.evidence_index);
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
    let host_harness = build_host_harness_summary_fragment(repo_root)?;
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
            "host_harness": host_harness,
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
    Ok(hex::encode(hasher.finalize()))
}

struct CachedRegistry {
    content: Value,
    mtime: Option<SystemTime>,
}

static REGISTRY_CACHE: Mutex<Option<CachedRegistry>> = Mutex::new(None);

/// Machine-readable per-host harness surface from `RUNTIME_REGISTRY.json` (for contract-summary / audits).
fn build_host_harness_summary_fragment(repo_root: &Path) -> Result<Value, String> {
    let path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if !path.is_file() {
        return Err(format!(
            "RUNTIME_REGISTRY missing at {} — cannot build host_harness fragment",
            path.display()
        ));
    }
    let mtime = fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    {
        let guard = REGISTRY_CACHE.lock().expect("registry cache");
        if let Some(ref cached) = *guard {
            if cached.mtime == mtime {
                return Ok(cached.content.clone());
            }
        }
    }
    let v = read_json_strict(&path)?;
    let projections = v
        .get("host_projections")
        .and_then(Value::as_object)
        .ok_or_else(|| "RUNTIME_REGISTRY missing host_projections".to_string())?;
    let mut hosts: Vec<String> = projections.keys().cloned().collect();
    hosts.sort();
    let mut out = Map::new();
    for host in hosts {
        let proj = projections
            .get(&host)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("host_projections.{host} must be an object"))?;
        out.insert(
            host,
            json!({
                "harness_capabilities": proj.get("harness_capabilities").cloned().unwrap_or(Value::Null),
                "harness_capability_exceptions": proj.get("harness_capability_exceptions").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    let result = Value::Object(out);
    {
        let mut guard = REGISTRY_CACHE.lock().expect("registry cache");
        *guard = Some(CachedRegistry { content: result.clone(), mtime });
    }
    Ok(result)
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
    let normalized = text.split_whitespace().fold(String::new(), |mut acc, w| {
        if !acc.is_empty() {
            acc.push(' ');
        }
        acc.push_str(w);
        acc
    });
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
    crate::path_guard::reject_unsafe_path(path)?;
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

/// Compute SHA-256 hex digest of a file (used by integration tests across crates).
pub fn hash_file_for_test(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("read file failed for {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn write_json_if_changed_unlocked(path: &Path, payload: &Value) -> Result<bool, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize JSON payload failed: {err}"))?
    );
    write_text_if_changed_unlocked(path, &serialized)
}

pub fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
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

fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// Stable task id when no active/focus pointer exists (review-only sessions).
pub(crate) const CONTINUITY_SESSION_CHECKPOINT_TASK_ID: &str = "continuity-session";

/// 带可选 task_id 的版本（用于 Desktop MCP session_checkpoint tool）。
///
/// `repointer_focus`: when true, rewrite active/focus/supervisor (explicit user checkpoint).
/// `update_registry_only_if_known`: when true, never append a new registry row for unknown ids.
pub fn build_automatic_continuity_checkpoint_payload_with_task_id(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
    task_id: Option<&str>,
    repointer_focus: bool,
    update_registry_only_if_known: bool,
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
    let mut payload = json!({
        "output_dir": output_dir.to_string_lossy(),
        "repo_root": repo_root.to_string_lossy(),
        "task": task,
        "summary": summary,
        "phase": "execution",
        "status": "in_progress",
        "focus": repointer_focus,
        "update_registry_only_if_known": update_registry_only_if_known,
        "next_actions": [
            "Open artifacts/current/SESSION_SUMMARY.md on the next session.",
            "Optional: run `router-rs framework snapshot --repo-root <repo>` for a compact runtime read model.",
        ],
        "trace_metadata": {
            "checkpoint_kind": "automatic_stop_hook",
        }
    });
    if let Some(tid) = task_id.filter(|s| !s.is_empty())
        && let Some(obj) = payload.as_object_mut() {
            obj.insert("task_id".to_string(), serde_json::json!(tid));
        }
    payload
}

const MAX_POST_TOOL_EVIDENCE_ARTIFACTS: usize = 120;

fn continuity_post_tool_evidence_env_enabled() -> bool {
    crate::router_env_flags::router_rs_continuity_post_tool_evidence_enabled()
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

fn coerce_duration_ms_value(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(n) = value.as_i64() {
        return n.try_into().ok();
    }
    if let Some(n) = value.as_f64() {
        return Some(n.round() as u64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<u64>().ok();
    }
    None
}

/// Parse `tool_output` JSON string once; returns `None` when the field is missing, not a string,
/// or fails to parse. Used by `extract_post_tool_duration_ms` and `extract_codex_tool_exit_hint`
/// to avoid double-parsing the same payload.
fn parse_tool_output_json(event: &Value) -> Option<&'static Value> {
    // Leak the parsed Value so it lives for 'static — small one-shot objects in hook path.
    let text = event.get("tool_output").and_then(Value::as_str)?;
    let parsed: Value = serde_json::from_str(text).ok()?;
    Some(Box::leak(Box::new(parsed)))
}

/// PostToolUse journal: tool execution duration when the host payload carries it.
pub fn extract_post_tool_duration_ms(event: &Value) -> Option<u64> {
    let candidates: [&Option<&Value>; 10] = [
        &event.get("duration_ms"),
        &event.get("durationMs"),
        &event.get("tool_duration_ms"),
        &event.get("toolDurationMs"),
        &event.get("execution_time_ms"),
        &event.get("executionTimeMs"),
        &event.get("tool_output").and_then(|v| v.get("duration_ms")),
        &event.get("tool_output").and_then(|v| v.get("durationMs")),
        &event
            .get("tool_output")
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("duration_ms")),
        &event.get("result").and_then(|v| v.get("duration_ms")),
    ];
    if let Some(parsed) = parse_tool_output_json(event) {
        if let Some(ms) = coerce_duration_ms_value(parsed.get("duration_ms")) {
            return Some(ms);
        }
        if let Some(ms) = coerce_duration_ms_value(parsed.get("durationMs")) {
            return Some(ms);
        }
    }
    for candidate in candidates {
        if let Some(ms) = coerce_duration_ms_value(*candidate) {
            return Some(ms);
        }
    }
    None
}

/// PostToolUse journal: infer success from exit code / error flags when present.
pub fn post_tool_call_succeeded(event: &Value) -> bool {
    if event
        .get("is_error")
        .and_then(Value::as_bool)
        .is_some_and(|v| v)
    {
        return false;
    }
    if event.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    match extract_codex_tool_exit_hint(event) {
        Some(0) => true,
        Some(_) => false,
        None => true,
    }
}

/// 从 Codex `PostToolUse` 载荷中提取退出码（兼容嵌套 `tool_output` / JSON 字符串）。
fn extract_codex_tool_exit_hint(event: &Value) -> Option<i64> {
    let candidates: [&Option<&Value>; 7] = [
        &event.get("exit_code"),
        &event.get("exitCode"),
        &event.get("tool_output").and_then(|v| v.get("exit_code")),
        &event.get("tool_output").and_then(|v| v.get("exitCode")),
        &event
            .get("tool_output")
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("exit_code")),
        &event.get("result").and_then(|v| v.get("exit_code")),
        &event.get("response").and_then(|v| v.get("exit_code")),
    ];
    if let Some(parsed) = parse_tool_output_json(event) {
        if let Some(code) = coerce_exit_code_value(parsed.get("exit_code")) {
            return Some(code);
        }
        if let Some(code) = coerce_exit_code_value(parsed.get("exitCode")) {
            return Some(code);
        }
    }
    for candidate in candidates {
        if let Some(code) = coerce_exit_code_value(*candidate) {
            return Some(code);
        }
    }
    None
}

/// Task id resolution for evidence append helpers: explicit override wins, then task_view pointers.
fn resolve_evidence_append_task_id(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Option<String> {
    task_id_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            let view = crate::task_state::resolve_task_view(repo_root, None);
            view.task_id.filter(|s| !s.is_empty())
        })
}

pub fn append_evidence_index_merged_row(
    repo_root: &Path,
    task_id_override: Option<&str>,
    entry: Map<String, Value>,
) -> Result<(), String> {
    // 解析 entry 中的签名字段用于去重（精确去重：command_preview + recorded_at）
    let entry_signature = entry
        .get("command_preview")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let entry_recorded_at = entry
        .get("recorded_at")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();

    let resolved_task_id = resolve_evidence_append_task_id(repo_root, task_id_override);

    // Lightweight readiness check: avoid full 9-file snapshot rebuild
    let current_root = repo_root.join("artifacts/current");
    let active_pointer_exists = current_root.join(TASK_POINTERS_FILENAME).is_file();
    let summary_exists = current_root.join(SESSION_SUMMARY_FILENAME).is_file();
    if !active_pointer_exists && !summary_exists {
        eprintln!(
            "[router-rs] warning: evidence append skipped \u{2014} no active continuity session \
             (no active/focus task pointer and no SESSION_SUMMARY at {})",
            current_root.join(SESSION_SUMMARY_FILENAME).display()
        );
        return Ok(());
    }

    // Write evidence to task-local subdirectory when a task_id is resolved,
    // matching the read path in FrameworkRuntimeView and
    // task_evidence_artifacts_summary_for_task.
    let evidence_path = match resolved_task_id {
        Some(ref tid) => current_root.join(tid).join(EVIDENCE_INDEX_FILENAME),
        None => current_root.join(EVIDENCE_INDEX_FILENAME),
    };
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create evidence dir: {err}"))?;
    }

    let tx_payload = {
        let _evidence_lock = crate::runtime_storage::acquire_runtime_path_lock(&evidence_path)?;

        let existing = read_json_strict(&evidence_path)?;
        let mut rows: Vec<Map<String, Value>> = normalize_evidence_index(&existing);

        let is_duplicate = rows.iter().any(|row| {
            let sig_cmd = row
                .get("command_preview")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let sig_at = row
                .get("recorded_at")
                .and_then(Value::as_str)
                .unwrap_or_default();
            sig_cmd == entry_signature && sig_at == entry_recorded_at
        });
        let tx_payload = Value::Object(entry.clone());
        if !is_duplicate {
            rows.push(entry);
        }

        if rows.len() > MAX_POST_TOOL_EVIDENCE_ARTIFACTS {
            // Keep all success=true rows + latest N non-success rows
            let mut success_rows: Vec<Map<String, Value>> = Vec::new();
            let mut other_rows: Vec<Map<String, Value>> = Vec::new();
            for row in rows.drain(..) {
                if row.get("success").and_then(Value::as_bool) == Some(true) {
                    success_rows.push(row);
                } else {
                    other_rows.push(row);
                }
            }
            let budget = MAX_POST_TOOL_EVIDENCE_ARTIFACTS.saturating_sub(success_rows.len());
            if other_rows.len() > budget {
                let drain = other_rows.len() - budget;
                other_rows.drain(0..drain);
            }
            rows = success_rows;
            rows.extend(other_rows);
        }
        let payload = json!({
            "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
            "artifacts": rows.into_iter().map(Value::Object).collect::<Vec<Value>>(),
        });
        write_json_if_changed_unlocked(&evidence_path, &payload)?;
        tx_payload
    };
    if let Some(tid) = resolved_task_id {
        let tx = crate::task_ledger::LedgerTransaction {
            ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            tx_type: "evidence".to_string(),
            payload: tx_payload,
            idempotency_key: None,
            seq: None,
            schema_version: Some(1),
        };
        if let Err(e) = crate::task_ledger::append_transaction(repo_root, &tid, tx) {
            eprintln!("[router-rs] failed to append evidence transaction to TASK_LEDGER: {e}");
        }
        crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &tid);
    }
    Ok(())
}

/// `framework hook-evidence-append`：供 Cursor hook 等外部进程写入一条验证记录。
///
/// JSON：`repo_root`（可选）、`task_id`（可选）、`command_preview`（必填）、`exit_code`（可选）、`source`（可选，默认 `external_hook`）。
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
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let exit_code = payload
        .get("exit_code")
        .and_then(|v| coerce_exit_code_value(Some(v)));

    let cursor_hook = source.trim().to_ascii_lowercase().starts_with("cursor_");
    let preview_lower = preview_trim.to_ascii_lowercase();
    if !cursor_hook && !shell_command_looks_like_verification(&preview_lower) {
        crate::telemetry_emit::emit_hook_fired("hook_evidence_append", "skip");
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

    // Programmatic verification of physical artifact association (L1 Truthfulness)
    let artifact_ok = detect_and_verify_physical_artifact(&repo_root, &preview_lower);
    if !artifact_ok {
        entry.insert("artifact_verification_failed".to_string(), json!(true));
    }

    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0 && artifact_ok));
    } else {
        entry.insert("success".to_string(), json!(artifact_ok));
    }
    append_evidence_index_merged_row(&repo_root, task_id.as_deref(), entry)?;
    let success = exit_code.map(|ec| ec == 0).unwrap_or(true) && artifact_ok;
    crate::telemetry_emit::emit_hook_fired(
        "hook_evidence_append",
        if success { "append:ok" } else { "append:warn" },
    );
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

fn shell_command_looks_like_verification(command_lower: &str) -> bool {
    // Fast reject: skip 50+ contains checks when no seed keyword is present.
    if !command_lower.contains("cargo")
        && !command_lower.contains("test")
        && !command_lower.contains("check")
        && !command_lower.contains("make")
        && !command_lower.contains("npm")
        && !command_lower.contains("pytest")
        && !command_lower.contains("yarn")
        && !command_lower.contains("pnpm")
        && !command_lower.contains("bun")
        && !command_lower.contains("vitest")
        && !command_lower.contains("jest")
        && !command_lower.contains("rake")
        && !command_lower.contains("go ")
        && !command_lower.contains("dotnet")
        && !command_lower.contains("maturin")
        && !command_lower.contains("tox")
        && !command_lower.contains("uv run")
        && !command_lower.contains("just")
        && !command_lower.contains("ruff")
        && !command_lower.contains("mypy")
        && !command_lower.contains("deno")
        && !command_lower.contains("lint")
        && !command_lower.contains("tsc")
        && !command_lower.contains("eslint")
        && !command_lower.contains("prettier")
        && !command_lower.contains("biome")
        && !command_lower.contains("gradle")
        && !command_lower.contains("mvn")
        && !command_lower.contains("flutter")
        && !command_lower.contains("swift")
        && !command_lower.contains("dart")
        && !command_lower.contains("playwright")
        && !command_lower.contains("nx ")
        && !command_lower.contains("scripts/verify")
        && !command_lower.contains("verify.sh")
        && !command_lower.contains("nextest")
        && !command_lower.contains("policy")
        && !command_lower.contains("verify_cursor")
        && !command_lower.contains("python")
        && !command_lower.contains("lean")
        && !command_lower.contains("coq")
        && !command_lower.contains("isabelle")
        && !command_lower.contains("lake ")
        && !command_lower.contains("z3 ")
    {
        return false;
    }

    // Original (Rust / Python / JS test runners + lint).
    command_lower.contains("cargo test")
        || command_lower.contains("cargo check")
        || command_lower.contains("cargo clippy")
        || command_lower.contains("cargo build")
        || command_lower.contains("cargo fmt")
        || command_lower.contains("cargo nextest")
        || command_lower.contains("cargo hack")
        || command_lower.contains("nextest")
        || command_lower.contains("pytest")
        || command_lower.contains("npm test")
        || command_lower.contains("pnpm test")
        || command_lower.contains("yarn test")
        || command_lower.contains("make test")
        || command_lower.contains("make check")
        || command_lower.contains("make ci")
        || command_lower.contains("make verify")
        || command_lower.contains("go test")
        || command_lower.contains("go vet")
        || command_lower.contains("dotnet test")
        || command_lower.contains("maturin")
        || command_lower.contains("tox")
        || command_lower.contains("uv run")
        || command_lower.contains("just test")
        || command_lower.contains("just check")
        || command_lower.contains("vitest")
        || command_lower.contains("jest")
        || command_lower.contains("ruby test")
        || command_lower.contains("rake test")
        || command_lower.contains("verify_cursor_hooks")
        || command_lower.contains("policy_contracts")
        || command_lower.contains("ruff check")
        || command_lower.contains("ruff format")
        || command_lower.contains("mypy")
        || command_lower.contains("deno test")
        || command_lower.contains("bun test")
        // pnpm / bun tooling.
        || command_lower.contains("pnpm lint")
        || command_lower.contains("pnpm check")
        || command_lower.contains("bun lint")
        // TypeScript / JS tooling (no `test` keyword).
        || command_lower.contains("tsc --noemit")
        || command_lower.contains("tsc -p")
        || command_lower.contains("eslint")
        || command_lower.contains("prettier --check")
        || command_lower.contains("biome check")
        || command_lower.contains("biome ci")
        // JVM ecosystems.
        || command_lower.contains("gradle test")
        || command_lower.contains("gradlew test")
        || command_lower.contains("gradle check")
        || command_lower.contains("mvn test")
        || command_lower.contains("mvn verify")
        || command_lower.contains("mvn package")
        // Mobile / Dart / Swift tooling.
        || command_lower.contains("flutter test")
        || command_lower.contains("swift test")
        || command_lower.contains("swift build")
        || command_lower.contains("dart analyze")
        // E2E / cross-runner test frameworks.
        || command_lower.contains("playwright test")
        || command_lower.contains("nx test")
        || command_lower.contains("nx affected")
        // Repo-local verifier scripts (any path under scripts/ ending with verify*).
        || command_lower.contains("scripts/verify")
        || command_lower.contains("/verify.sh")
        || command_lower.contains("./verify.sh")
        || command_lower.contains("task test")
        || command_lower.contains("task check")
        // Formal / math toolchains: shared with `harness_context_signals` (`formal_toolchain`).
        || crate::formal_toolchain::ascii_lower_contains_formal_toolchain_tokens(command_lower)
}

fn detect_and_verify_physical_artifact(repo_root: &Path, command_lower: &str) -> bool {
    let max_delta = 15; // 15s safe time window for mtime verification to accommodate slow disks

    // Dynamic bypass: Skip physical filesystem assertions during Rust target integration tests
    let repo_path_str = repo_root.to_string_lossy();
    if repo_path_str.contains("target/tmp")
        || repo_path_str.contains("post-tool-evidence-append")
        || repo_path_str.contains("cursor-post-tool-evidence-append")
    {
        return true;
    }

    if command_lower.contains("cargo test")
        || command_lower.contains("cargo check")
        || command_lower.contains("cargo clippy")
        || command_lower.contains("cargo build")
    {
        let target_dir = repo_root.join("target");
        if target_dir.is_dir() {
            if is_modified_recently(&target_dir, max_delta) {
                return true;
            }
            let debug_dir = target_dir.join("debug");
            if debug_dir.is_dir() && is_modified_recently(&debug_dir, max_delta) {
                return true;
            }
            return false;
        }
        return false;
    }

    if command_lower.contains("pytest") {
        let py_cache = repo_root.join(".pytest_cache");
        if py_cache.is_dir() && is_modified_recently(&py_cache, max_delta) {
            return true;
        }
        let junit = repo_root.join("junit.xml");
        if junit.is_file() && is_modified_recently(&junit, max_delta) {
            return true;
        }
        return false;
    }

    true
}

fn is_modified_recently(path: &std::path::Path, max_delta_secs: u64) -> bool {
    use std::time::SystemTime;
    if let Ok(metadata) = std::fs::metadata(path)
        && let Ok(modified) = metadata.modified() {
            let now = SystemTime::now();
            if let Ok(elapsed) = now.duration_since(modified) {
                return elapsed.as_secs() <= max_delta_secs;
            }
            if let Ok(elapsed) = modified.duration_since(now) {
                return elapsed.as_secs() <= max_delta_secs;
            }
        }
    false
}

#[cfg(test)]
mod shell_command_verification_heuristic_tests {
    use super::shell_command_looks_like_verification;

    fn check(cmd: &str) -> bool {
        shell_command_looks_like_verification(&cmd.to_ascii_lowercase())
    }

    #[test]
    fn matrix_math_formal_and_build_tools() {
        assert!(check(
            "python -c \"import sympy; print(sympy.simplify(1))\""
        ));
        assert!(check("z3 /tmp/proof.smt2"));
        assert!(check("  z3  /tmp/x.smt2"));
        assert!(check("lean --version"));
        assert!(check("lake build && lake test"));
        assert!(check("coqc -Q theories Foo.v"));
        assert!(check("coqchk -silent Foo.vo"));
        assert!(check("isabelle build -D ."));
        assert!(check("cargo test -q"));
        assert!(check("pytest -q"));
    }

    #[test]
    fn matrix_rejects_bare_python_and_random_strings() {
        assert!(!check("python foo.py"));
        assert!(!check("python -c \"print(1)\""));
        assert!(!check("echo hello"));
        assert!(!check("leaning tower")); // not `lean ` token
    }

    #[test]
    fn test_physical_artifact_checks() {
        use super::detect_and_verify_physical_artifact;
        let temp_dir = std::env::temp_dir().join(format!(
            "router-rs-test-artifact-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Non-verification commands should be bypassed and return true by default
        assert!(detect_and_verify_physical_artifact(
            &temp_dir,
            "python foo.py"
        ));

        // 2. Pytest should return false when pytest_cache / junit.xml are missing
        assert!(!detect_and_verify_physical_artifact(&temp_dir, "pytest -v"));

        // 3. Cargo test should return false when target directory is missing
        assert!(!detect_and_verify_physical_artifact(
            &temp_dir,
            "cargo test"
        ));

        // 4. Simulate pytest generating .pytest_cache folder -> pytest passes
        let pytest_cache = temp_dir.join(".pytest_cache");
        std::fs::create_dir_all(&pytest_cache).unwrap();
        assert!(detect_and_verify_physical_artifact(&temp_dir, "pytest -v"));

        // 5. Simulate cargo generating target folder -> cargo test passes
        let target_dir = temp_dir.join("target");
        std::fs::create_dir_all(&target_dir).unwrap();
        assert!(detect_and_verify_physical_artifact(&temp_dir, "cargo test"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

/// 在宿主 `PostToolUse` 中追加一条「终端类验证命令」到 `EVIDENCE_INDEX.json`（与 session 写入共用锁）。
///
/// `kind` 用于区分来源（如 `codex_post_tool_verification` / `cursor_post_tool_verification`）。
/// 仅在连续性已初始化且命令启发式匹配验证类时写入。默认关；`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` 开启。
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
    let command_lower = command_preview.to_ascii_lowercase();
    if !shell_command_looks_like_verification(&command_lower) {
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

    // Programmatic verification of physical artifact association (L1 Truthfulness)
    let artifact_ok = detect_and_verify_physical_artifact(repo_root, &command_lower);
    if !artifact_ok {
        entry.insert("artifact_verification_failed".to_string(), json!(true));
    }

    if let Some(ec) = exit_hint {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0 && artifact_ok));
    } else {
        entry.insert("success".to_string(), json!(artifact_ok));
    }
    // Pointer 机制已移除：从 event 中提取 task_id 显式传递
    let task_id_from_event = event
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    append_evidence_index_merged_row(repo_root, task_id_from_event, entry)?;
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
pub fn closeout_record_path_for_task(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    // SECURITY: Validate task_id to prevent path traversal attacks.
    // Only allow alphanumeric characters, hyphens, and underscores.
    let sanitized = task_id.trim();
    if sanitized.is_empty() {
        return Err("task_id cannot be empty".to_string());
    }
    if !sanitized
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "task_id contains invalid characters (only alphanumeric, hyphen, underscore allowed): {:?}",
            sanitized
        ));
    }

    let path = repo_root
        .join("artifacts")
        .join("closeout")
        .join(format!("{}.json", sanitized));

    // SECURITY: Verify the resolved path is still within the expected directory.
    // This prevents any remaining path traversal attempts (e.g., via symlinks).
    let closeout_dir = repo_root.join("artifacts").join("closeout");
    let canonical_path = std::fs::canonicalize(&path).or_else(|_| {
        std::fs::canonicalize(&closeout_dir).map(|p| p.join(format!("{}.json", sanitized)))
    });
    if let Ok(canonical) = canonical_path {
        let canonical_dir = std::fs::canonicalize(&closeout_dir)
            .map_err(|e| format!("failed to canonicalize closeout directory: {}", e))?;
        if !canonical.starts_with(&canonical_dir) {
            return Err("path traversal detected".to_string());
        }
    }

    Ok(path)
}

struct CachedTaskRegistry {
    content: Value,
    mtime: Option<SystemTime>,
}

static TASK_REGISTRY_CACHE: Mutex<Option<CachedTaskRegistry>> = Mutex::new(None);

/// 从 task_registry.json 中读取 task_id（pointer 机制移除后的回退）。
/// 优先返回 focus_task_id，再返回 tasks 数组中第一个。
pub fn first_task_id_from_registry(repo_root: &Path) -> Option<String> {
    let registry_path = repo_root.join("artifacts/current/task_registry.json");
    let mtime = fs::metadata(&registry_path)
        .ok()
        .and_then(|m| m.modified().ok());
    {
        let guard = TASK_REGISTRY_CACHE.lock().expect("task registry cache");
        if let Some(ref cached) = *guard {
            if cached.mtime == mtime {
                return extract_first_task_id_from_value(&cached.content);
            }
        }
    }
    let raw = fs::read_to_string(&registry_path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    {
        let mut guard = TASK_REGISTRY_CACHE.lock().expect("task registry cache");
        *guard = Some(CachedTaskRegistry { content: data.clone(), mtime });
    }
    extract_first_task_id_from_value(&data)
}

fn extract_first_task_id_from_value(data: &Value) -> Option<String> {
    if let Some(focus) = data.get("focus_task_id").and_then(Value::as_str) {
        let focus = focus.trim();
        if !focus.is_empty() {
            return Some(focus.to_string());
        }
    }
    let tasks = data.get("tasks").and_then(Value::as_array)?;
    for row in tasks {
        if let Some(tid) = row.get("task_id").and_then(Value::as_str) {
            let tid = tid.trim();
            if !tid.is_empty() {
                return Some(tid.to_string());
            }
        }
    }
    None
}

fn count_evidence_rows(evidence_index: &Value) -> usize {
    evidence_index
        .get("artifacts")
        .or_else(|| evidence_index.get("evidence"))
        .and_then(Value::as_array)
        .map(|rows| rows.len())
        .unwrap_or(0)
}

/// Evaluate a materialized closeout record JSON file, attaching an EvidenceContext (R8) when possible.
/// Shared Stop/closeout guard when assistant or user text claims completion (Cursor/Codex parity).
pub fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> {
    if text.trim().is_empty() || !crate::hook_common::contains_completion_claim_token(text) {
        return None;
    }
    // Pointer 机制已移除：先尝试 resolve_task_view，再回退到 task_registry.json
    let tid = crate::task_state::resolve_task_view(repo_root, None)
        .task_id
        .filter(|s| !s.is_empty())
        .or_else(|| first_task_id_from_registry(repo_root));
    let tid = tid?;
    if !closeout_programmatic_enforcement_enabled() {
        return None;
    }
    let record_path = closeout_record_path_for_task(repo_root, &tid).ok()?;
    if !record_path.is_file() {
        return Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={tid} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估。",
            record_path.display()
        ));
    }
    let eval = evaluate_closeout_record_file_for_task(repo_root, &tid, &record_path).ok()?;
    if eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    Some(format!(
        "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluation_failed path={}",
        record_path.display()
    ))
}

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
        crate::goal_drive::task_evidence_artifacts_summary_for_task(repo_root, tid);
    let goal_state = crate::goal_drive::read_goal_state(repo_root, Some(tid))
        .ok()
        .flatten();
    let goal_prediction = goal_state
        .as_ref()
        .and_then(core_state::goal_prediction::read_goal_prediction);
    let ctx = CloseoutEvidenceContext {
        task_id: Some(tid.to_string()),
        evidence_rows_non_empty: rows_non_empty,
        has_successful_verification: has_success,
        goal_prediction,
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
            !t.is_empty() && !is_false_ci_value(&t)
        }
        Err(_) => false,
    }
}

#[inline]
fn is_false_ci_value(s: &str) -> bool {
    s == "0" || s == "false" || s == "off" || s == "no"
}

fn closeout_enforcement_disabled_by_env() -> bool {
    match std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            is_false_ci_value(&t)
        }
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
            crate::goal_drive::task_evidence_artifacts_summary_for_task(
                &repo_root,
                &task_id_str,
            );
        let goal_state = crate::goal_drive::read_goal_state(&repo_root, Some(&task_id_str))
            .ok()
            .flatten();
        let goal_prediction = goal_state
            .as_ref()
            .and_then(core_state::goal_prediction::read_goal_prediction);
        let ctx = CloseoutEvidenceContext {
            task_id: Some(task_id_str.trim().to_string()),
            evidence_rows_non_empty: rows_non_empty,
            has_successful_verification: has_success,
            goal_prediction,
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
#[path = "mod_tests.rs"]
mod tests;
