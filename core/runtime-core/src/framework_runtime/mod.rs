use serde_json::{Value, json};
use std::path::Path;

// ── Modules extracted to framework-extra (L4) ──
pub use framework_extra::alias;
pub use framework_extra::closeout;
pub use framework_extra::contract_summary;
pub use framework_extra::evidence;
pub use framework_extra::framework_doctor;
pub use framework_extra::prompt_compression;
pub use framework_extra::session_artifacts;
pub use framework_extra::snapshot;
pub use framework_extra::statusline;
pub use framework_extra::util;

// ── Modules that remain in runtime-core (deep coupling) ──
pub use fr_utils::constants;
pub use fr_exec::evolution_observer;
pub use fr_utils::io_utils;
pub use fr_utils::json_io;
pub use fr_utils::json_value;
pub use fr_exec::live_execute;
pub use framework_extra::orchestration_controller;
pub use fr_contracts::pre_tool_use_guard;
pub use framework_kernel::repo_roots;
pub use fr_exec::runtime_view;
pub use fr_exec::sandbox_control;
pub mod stdio_dispatch;
pub mod tool_handlers;
pub use fr_utils::stdio_op_registry;
pub use fr_exec::trace_attach;
pub use fr_exec::trace_stream_io;
pub use fr_exec::trace_transport;
pub use fr_utils::types;

// Re-export json_io functions needed by cli/common.rs (cycle-breaking extraction).
pub use json_io::{parse_json_input, print_json_value};
// Re-export json_value functions for sibling-module backward compat.
pub use json_value::{
    first_nonempty, nonempty_string, safe_slug, stable_line_items, value_bool_or_none,
    value_string_list, value_text,
};

// Re-exports from existing submodules (backward compat).
pub use alias::build_framework_alias_envelope;
pub use constants::FRAMEWORK_ALIAS_SCHEMA_VERSION;
pub use constants::{
    FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION, FRAMEWORK_RUNTIME_AUTHORITY,
    FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION, FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
};
pub use crate::stdio_payload_types::*;
pub use framework_doctor::{DoctorResult, run_continuity_audit, run_framework_doctor};
pub use orchestration_controller::{
    build_background_control_response, build_runtime_control_plane_payload,
    build_runtime_integrator_payload, build_runtime_metric_record,
    build_runtime_observability_exporter_descriptor, build_runtime_observability_health_snapshot,
    build_runtime_observability_metric_catalog_payload, runtime_observability_dashboard_schema,
};
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
pub use framework_extra::route_manifest_fallback::route_task_with_manifest_fallback;
pub use framework_extra::route_manifest_fallback::{
    manifest_fallback_path, resolve_runtime_declared_manifest_fallback,
};
pub use sandbox_control::build_sandbox_control_response;
pub use session_artifacts::write_framework_session_artifacts;
pub use snapshot::{
    build_framework_runtime_snapshot_envelope, build_framework_runtime_snapshot_envelope_with_level,
};
pub use statusline::build_framework_statusline;
pub use stdio_dispatch::{dispatch_stdio_json_request, dispatch_stdio_json_request_payload};
pub use stdio_op_registry::{StdioOpDomain, classify_stdio_op};
pub use types::FrameworkAliasBuildOptions;
pub use trace_attach::attach_runtime_event_transport;
pub use trace_stream_io::{inspect_trace_stream, replay_trace_stream};
#[cfg(test)]
pub use stdio_op_registry::{
    is_framework_stdio_op, is_routing_stdio_op, is_runtime_stdio_op, is_trace_stdio_op,
};
pub use trace_attach::{
    cleanup_attached_runtime_event_transport, subscribe_attached_runtime_events,
};
pub use trace_stream_io::{write_trace_compaction_delta, write_trace_metadata};
pub use trace_runtime::sha256_hex;

// Re-export from new submodules for backward-compatible paths.
pub use closeout::{
    closeout_programmatic_enforcement_enabled, closeout_record_path_for_task,
    closeout_stop_followup_for_completion_text, evaluate_closeout_record_file_for_task,
    first_task_id_from_registry,
};
pub use contract_summary::build_framework_contract_summary_envelope;
pub use evidence::{
    append_evidence_index_merged_row, extract_post_tool_duration_ms, framework_hook_evidence_append,
    post_tool_call_succeeded, try_append_post_tool_shell_evidence,
};
pub use util::{
    current_local_timestamp, hash_file_for_test, is_terminal, supervisor_contract,
};

use constants::CURRENT_ARTIFACT_DIR;
use types::FrameworkRuntimeView;

/// Thin wrappers to `runtime_view` kept in `mod.rs` for sibling-module access.
pub fn load_framework_runtime_view(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> FrameworkRuntimeView {
    runtime_view::load_framework_runtime_view(repo_root, artifact_root_override, task_id_override)
}

pub fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    runtime_view::classify_runtime_continuity(snapshot)
}

pub fn workspace_name_from_root(repo_root: &Path) -> String {
    runtime_view::workspace_name_from_root(repo_root)
}

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
        util::truncate_utf8_chars(task_line.trim(), 200)
    };
    let summary = if summary_text.trim().is_empty() {
        "Automatic continuity checkpoint. No summary text was provided; refine in the next turn."
            .to_string()
    } else {
        util::truncate_utf8_chars(summary_text.trim(), 8000)
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

