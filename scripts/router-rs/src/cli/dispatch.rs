//! 子命令 `dispatch_*`（不含 stdio）。
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::args::*;
use super::common::{
    manifest_fallback_path, parse_json_input, print_json_value, route_task_with_manifest_fallback,
};
use super::runtime_ops::{
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata,
};

use crate::browser_mcp::{
    resolve_browser_mcp_attach_artifact, run_browser_mcp_stdio_loop, BrowserAttachConfig,
};
use crate::claude_hooks::run_claude_hook_cli;
use crate::closeout_enforcement::{closeout_enforcement_contract, evaluate_closeout_record_value};
use crate::codex_hooks::{
    build_codex_hook_projection, codex_host_entrypoint_provider, install_codex_cli_hooks,
    resolve_codex_home, run_codex_audit_hook, InstallMode,
};
use crate::eval_route::{eval_route_contract, run_eval_route};
use crate::framework_profile::{
    build_codex_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use crate::framework_runtime::{
    build_framework_alias_envelope, build_framework_contract_summary_envelope,
    build_framework_prompt_compression_envelope, build_framework_runtime_snapshot_envelope,
    build_framework_statusline, framework_hook_evidence_append, resolve_repo_root_arg,
    write_framework_session_artifacts, FrameworkAliasBuildOptions,
};
use crate::hook_policy::{evaluate_hook_policy, hook_policy_contract, HookPolicyEvaluateRequest};
use crate::host_entrypoint_sync::sync_host_entrypoints;
use crate::host_integration::run_host_integration_from_args;
use crate::review_gate::run_review_gate;
use crate::route::{
    build_search_results_payload, load_records, load_records_from_manifest, search_skills,
    MatchRow, SearchResultsPayload,
};
use crate::router_self;
use crate::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, runtime_backend_family_catalog_payload,
    runtime_backend_family_parity_payload, runtime_storage_operation,
};
use crate::task_command;
use crate::task_state;
use crate::trace_runtime::{
    compact_trace_stream, record_trace_event, TraceCompactRequestPayload,
    TraceRecordEventRequestPayload,
};

use crate::runtime_storage::RuntimeStorageRequestPayload;

include!("dispatch_body.txt");
