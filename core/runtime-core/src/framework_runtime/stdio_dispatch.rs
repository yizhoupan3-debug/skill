//! Stdio JSON request dispatch.
//!
//! # Dispatch patterns
//!
//! This module uses two dispatch patterns:
//!
//! 1. **`parse_and_dispatch<T,R,F>()`** — for operations with structured request
//!    and response types.  The payload is deserialized into `T`, handed to `F`,
//!    and the return value is serialized.  Used when the handler has well-defined
//!    typed parameters (e.g. `TraceRecordEventRequestPayload`).
//!
//! 2. **Manual field extraction** — for operations with ad-hoc or optional fields
//!    that don't fit a shared struct (e.g. `session_supervisor` reads raw `Value`
//!    and delegates to the hook).  The handler calls `payload.get("field")`,
//!    extracts what it needs, and returns a serialized `Value` directly.
//!
//! Both patterns produce `Result<Value, FrameworkError>` at the dispatch level.
//! The choice depends on whether the operation's shape is stable enough to
//! warrant a dedicated struct.  When in doubt, prefer `parse_and_dispatch`.

use core_errors::FrameworkError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[cfg(feature = "l5-state")]
use crate::background_state::handle_background_state_operation;
use crate::closeout_validation::{
    CloseoutEvidenceContext, closeout_contract, evaluate_closeout_record_value,
    evaluate_closeout_record_value_with_context,
};
use crate::eval_route::{eval_route_contract, run_eval_route};
use crate::execution_contract::{
    build_execution_contract_bundle, decode_execution_response_value,
    normalize_execution_kernel_contract_value, normalize_execution_kernel_metadata_contract_value,
    validate_execution_kernel_steady_state_metadata_value,
};
use crate::framework_profile::{
    build_control_plane_contract_descriptors, build_profile_artifact_bundle, build_profile_bundle,
    load_framework_profile,
};
use crate::hook_event_routing::hook_event_routing_contract;
use crate::hook_policy::evaluate_hook_policy_value;
use crate::route::{
    ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION, RouteDecision, RouteSnapshotEnvelopePayload,
    RouteSnapshotRequestPayload, SkillRecord, build_route_diff_report, build_route_policy,
    build_route_resolution, build_route_snapshot, build_search_results_payload,
    filter_record_indices_for_host, filter_records_for_host, load_inline_records,
    load_records_cached_for_stdio, search_skills_subset,
};
use crate::runtime_storage::{
    RuntimeStorageRequestPayload, build_checkpoint_control_plane_compiler_payload,
    runtime_storage_operation,
};
use crate::stdio_payload_types::{
    BackgroundControlRequestPayload, ExecuteRequestPayload,
    TraceMetadataWriteRequestPayload,
    TraceStreamInspectRequestPayload, TraceStreamReplayRequestPayload,
};
use crate::stdio_transport::{
    StdioJsonRequestPayload, StdioJsonResponsePayload, runtime_concurrency_defaults_payload,
};
use crate::task_command;
use crate::trace_runtime::{
    TraceRecordEventRequestPayload, record_trace_event,
};
use fr_runtime::trace_attach::{
    attach_runtime_event_transport, cleanup_attached_runtime_event_transport,
    subscribe_attached_runtime_events,
};
use fr_runtime::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, write_trace_metadata,
};
use fr_runtime::trace_transport::{
    write_checkpoint_resume_manifest_payload, write_transport_binding_payload,
};
use fr_runtime::stdio_op_registry::{
    dispatch_runtime_output_mode_stdio, handles_runtime_output_stdio_op,
};
use framework_extra::route_manifest_fallback::route_task_with_manifest_fallback;

use fr_runtime::pre_tool_use_guard::evaluate_pre_tool_use_guard_value;
use fr_runtime::live_execute::execute_request;
use fr_runtime::trace_transport::{
    build_checkpoint_resume_manifest, build_trace_handoff_descriptor,
    build_trace_transport_descriptor,
};
use fr_runtime::stdio_op_registry::{StdioOpDomain, classify_stdio_op};
use fr_runtime::types::FrameworkAliasBuildOptions;
use framework_extra::alias::build_framework_alias_envelope;
use framework_extra::closeout::evaluate_closeout_record_file_for_task;
use framework_extra::contract_summary::build_framework_contract_summary_envelope;
use framework_extra::evidence::framework_hook_evidence_append;
use framework_extra::orchestration_controller::build_runtime_observability_health_snapshot;
use framework_extra::session_artifacts::write_framework_session_artifacts;
use framework_extra::snapshot::build_framework_runtime_snapshot_envelope_with_level;
use framework_core::json_value::{
    optional_bool, optional_non_empty_string, required_non_empty_string,
};
use framework_core::repo_roots::resolve_repo_root_arg;
use quality_gate;

pub fn dispatch_stdio_json_request_payload(
    request: StdioJsonRequestPayload,
) -> StdioJsonResponsePayload {
    let id = request.id.clone();
    use std::panic::{AssertUnwindSafe, catch_unwind};
    match catch_unwind(AssertUnwindSafe(|| {
        dispatch_stdio_json_request(&request.op, request.payload)
    })) {
        Ok(Ok(payload)) => StdioJsonResponsePayload {
            id: request.id,
            ok: true,
            payload: Some(payload),
            error: None,
        },
        Ok(Err(error)) => StdioJsonResponsePayload {
            id: request.id,
            ok: false,
            payload: None,
            error: Some(error.to_string()),
        },
        Err(panic_err) => {
            let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic in dispatch_stdio_json_request".to_string()
            };
            tracing::error!(panic = %msg, "panic caught in stdio dispatch");
            StdioJsonResponsePayload {
                id,
                ok: false,
                payload: None,
                error: Some(format!("dispatch panic: {msg}")),
            }
        }
    }
}

pub fn dispatch_stdio_json_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    // Runtime output mode dispatch: `handles_runtime_output_stdio_op()` and
    // `dispatch_runtime_output_mode_stdio()` are intentional stubs in `fr_utils`.
    // The real implementation lives in `runtime-core::orchestration_controller` and
    // is registered via host-projection hooks at runtime. These stubs break the
    // `cli ↔ framework-runtime` circular dependency (framework-runtime cannot
    // depend on runtime-core). See `fr_runtime::stdio_op_registry` for details.
    if handles_runtime_output_stdio_op(op) {
        let Some(result) = dispatch_runtime_output_mode_stdio(op, payload) else {
            return Err(FrameworkError::validation(format!(
                "runtime output mode dispatch drifted for {op}"
            )));
        };
        return result;
    }
    match classify_stdio_op(op) {
        Some(StdioOpDomain::Routing) => dispatch_routing_stdio_request(op, payload),
        Some(StdioOpDomain::Runtime) => dispatch_runtime_stdio_request(op, payload),
        Some(StdioOpDomain::Trace) => dispatch_trace_stdio_request(op, payload),
        Some(StdioOpDomain::Framework) => dispatch_framework_stdio_request(op, payload),
        Some(StdioOpDomain::Tool) => dispatch_tool_stdio_request(op, payload),
        None => Err(FrameworkError::validation(format!(
            "unsupported stdio operation: {op}"
        ))),
    }
}

fn parse_payload<T: DeserializeOwned>(payload: Value, context: &str) -> Result<T, FrameworkError> {
    serde_json::from_value(payload)
        .map_err(|err| FrameworkError::validation(format!("parse {context} input failed: {err}")))
}

fn serialize_payload<T: Serialize>(value: T, context: &str) -> Result<Value, FrameworkError> {
    serde_json::to_value(value).map_err(|err| {
        FrameworkError::validation(format!("serialize {context} output failed: {err}"))
    })
}

fn parse_and_dispatch<T, R, F>(
    payload: Value,
    context: &str,
    handler: F,
) -> Result<Value, FrameworkError>
where
    T: DeserializeOwned,
    R: Serialize,
    F: FnOnce(T) -> Result<R, FrameworkError>,
{
    let request = parse_payload::<T>(payload, context)?;
    serialize_payload(handler(request)?, context)
}

fn dispatch_stdio_closeout_evaluate(payload: Value) -> Result<Value, FrameworkError> {
    let repo_root = optional_non_empty_string(&payload, "repo_root");
    let task_id = optional_non_empty_string(&payload, "task_id");
    let result = if let (Some(repo_root), Some(task_id), Some(record_path)) = (
        repo_root.as_deref(),
        task_id.as_deref(),
        optional_non_empty_string(&payload, "record_path").as_deref(),
    ) {
        Ok(evaluate_closeout_record_file_for_task(
            Path::new(repo_root),
            task_id,
            Path::new(record_path),
        )?)
    } else {
        let record_value = payload
            .get("record")
            .cloned()
            .unwrap_or_else(|| payload.clone());
        if let (Some(repo_root), Some(task_id)) = (repo_root.as_deref(), task_id.as_deref()) {
            let ctx = CloseoutEvidenceContext::new(Some(task_id.trim().to_string()), Path::new(repo_root));
            Ok(evaluate_closeout_record_value_with_context(
                record_value,
                &ctx,
            )?)
        } else {
            Ok(evaluate_closeout_record_value(record_value)?)
        }
    };
    if let Ok(ref response) = result {
        let action = if response.get("closeout_allowed").and_then(Value::as_bool) == Some(true) {
            "allow"
        } else {
            "block"
        };
        tracing::debug!("closeout_evaluate: {action}");
    }
    result
}

fn dispatch_routing_stdio_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    match op {
        "route" => dispatch_stdio_route(payload),
        "search_skills" => dispatch_stdio_search_skills(payload),
        "hook_policy" => evaluate_hook_policy_value(payload),
        "pre_tool_use_guard" => Ok(evaluate_pre_tool_use_guard_value(payload)?),
        "concurrency_defaults" => serialize_payload(
            runtime_concurrency_defaults_payload(),
            "concurrency defaults",
        ),
        "route_report" => dispatch_stdio_route_report(payload),
        "route_resolution" => dispatch_stdio_route_resolution(payload),
        "route_policy" => dispatch_stdio_route_policy(payload),
        "route_snapshot" => dispatch_stdio_route_snapshot(payload),
        "compile_profile_bundle" => dispatch_stdio_compile_profile_bundle(payload),
        "compile_profile_artifacts" => dispatch_stdio_compile_profile_artifacts(payload),
        "closeout_evaluate" => dispatch_stdio_closeout_evaluate(payload),
        "closeout_contract" => Ok(closeout_contract()),
        "hook_event_routing_contract" => Ok(hook_event_routing_contract()),
        "eval_route" => dispatch_stdio_eval_route(payload),
        "eval_route_contract" => Ok(eval_route_contract()),
        _ => Err(FrameworkError::validation(format!(
            "unsupported routing stdio operation: {op}"
        ))),
    }
}

fn dispatch_runtime_stdio_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    match op {
        "execute" => {
            let request = parse_payload::<ExecuteRequestPayload>(payload, "execute")?;
            serialize_payload(execute_request(request)?, "execute")
        }
        "execution_contract_bundle" => Ok(Value::Object(build_execution_contract_bundle())),
        "normalize_execution_kernel_metadata_contract" => {
            if payload.as_object().is_some_and(Map::is_empty) || payload.is_null() {
                normalize_execution_kernel_metadata_contract_value(None)
            } else {
                normalize_execution_kernel_metadata_contract_value(Some(&payload))
            }
        }
        "normalize_execution_kernel_contract" => {
            let kernel_contract = payload.get("kernel_contract").ok_or_else(|| {
                "execution-kernel contract payload is missing kernel_contract.".to_string()
            })?;
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            normalize_execution_kernel_contract_value(kernel_contract, response_shape)
        }
        "validate_execution_kernel_steady_state_metadata" => {
            let metadata = payload.get("metadata").ok_or_else(|| {
                "execution-kernel validation payload is missing metadata.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let response_shape = payload.get("response_shape").and_then(Value::as_str);
            validate_execution_kernel_steady_state_metadata_value(
                metadata,
                kernel_contract,
                response_shape,
            )
        }
        "decode_execution_response" => {
            let execution_payload = payload.get("payload").ok_or_else(|| {
                "execution response decode payload is missing payload.".to_string()
            })?;
            let kernel_contract = payload.get("kernel_contract");
            let dry_run = payload.get("dry_run").and_then(Value::as_bool);
            decode_execution_response_value(execution_payload, kernel_contract, dry_run)
        }
        "runtime_observability_health_snapshot" => serialize_payload(
            build_runtime_observability_health_snapshot(),
            "runtime observability health snapshot",
        ),
        "background_control" => parse_and_dispatch::<BackgroundControlRequestPayload, _, _>(
            payload,
            "background control",
            |p| framework_extra::orchestration_controller::build_background_control_response(p),
        ),
        #[cfg(feature = "l5-state")]
        "background_state" => handle_background_state_operation(payload),
        #[cfg(not(feature = "l5-state"))]
        "background_state" => Err(FrameworkError::hook(
            "background_state requires L5 state feature (compile-time gate)",
        )),
        "session_supervisor" => {
            framework_core::runtime_hooks::try_hooks()
                .ok_or_else(|| FrameworkError::hook("runtime hooks not registered"))?
                .handle_orchestrator_operation(payload)
        }
        "describe_transport" => {
            build_trace_transport_descriptor(payload)
        }
        "describe_handoff" => {
            build_trace_handoff_descriptor(payload)
        }
        "checkpoint_resume_manifest" => {
            build_checkpoint_resume_manifest(payload)
        }
        "runtime_checkpoint_control_plane" => {
            build_checkpoint_control_plane_compiler_payload(payload)
        }
        "write_transport_binding" => {
            write_transport_binding_payload(payload)
        }
        "write_checkpoint_resume_manifest" => {
            write_checkpoint_resume_manifest_payload(payload)
        }
        "attach_runtime_event_transport" => {
            attach_runtime_event_transport(payload)
        }
        "subscribe_attached_runtime_events" => {
            subscribe_attached_runtime_events(payload)
        }
        "cleanup_attached_runtime_event_transport" => {
            cleanup_attached_runtime_event_transport(payload)
        }
        "runtime_storage" => parse_and_dispatch::<RuntimeStorageRequestPayload, _, _>(
            payload,
            "runtime storage",
            |p| runtime_storage_operation(p),
        ),
        "control_plane_contracts" => serialize_payload(
            build_control_plane_contract_descriptors(),
            "control plane contracts",
        ),
        _ => Err(FrameworkError::validation(format!(
            "unsupported runtime stdio operation: {op}"
        ))),
    }
}

fn dispatch_trace_stdio_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    match op {
        "trace_record_event" => parse_and_dispatch::<TraceRecordEventRequestPayload, _, _>(
            payload,
            "trace record event",
            |p| record_trace_event(p).map_err(|e| FrameworkError::validation(e.to_string())),
        ),
        "trace_stream_replay" => parse_and_dispatch::<TraceStreamReplayRequestPayload, _, _>(
            payload,
            "trace stream replay",
            |p| replay_trace_stream(p),
        ),
        "trace_stream_inspect" => parse_and_dispatch::<TraceStreamInspectRequestPayload, _, _>(
            payload,
            "trace stream inspect",
            |p| inspect_trace_stream(p),
        ),
        "write_trace_metadata" => parse_and_dispatch::<TraceMetadataWriteRequestPayload, _, _>(
            payload,
            "trace metadata write",
            |p| write_trace_metadata(p),
        ),
        _ => Err(FrameworkError::validation(format!(
            "unsupported trace stdio operation: {op}"
        ))),
    }
}

fn dispatch_framework_stdio_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    match op {
        "framework_runtime_snapshot" => dispatch_stdio_framework_runtime_snapshot(payload),
        "framework_contract_summary" => dispatch_stdio_framework_contract_summary(payload),
        "framework_session_artifact_write" => {
            write_framework_session_artifacts(payload)
        }
        "framework_hook_evidence_append" => {
            framework_hook_evidence_append(payload)
        }
        "framework_goal_drive" => runtime_infra::kernel_utils::framework_goal_drive(payload),
        "framework_quality_gate_loop" => {
            let repo_root_str = payload
                .get("repo_root")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let repo_root = std::path::Path::new(repo_root_str);
            let task_id = payload
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if task_id.is_empty() {
                return Err(FrameworkError::validation(
                    "framework_quality_gate: task_id is required".to_string(),
                ));
            }
            let goal = payload.get("goal").and_then(|v| v.as_str()).unwrap_or("");
            let scene = payload
                .get("scene")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(quality_gate::scene::GENERAL);
            let sub_scene = payload
                .get("sub_scene")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let round = payload.get("round").and_then(|v| v.as_u64()).unwrap_or(1);
            let output_data = payload.get("output_data").cloned();
            let verdict = crate::qg_entry::trigger(
                repo_root,
                task_id,
                scene,
                goal,
                sub_scene,
                round,
                tokio::runtime::Handle::try_current().ok(),
                output_data,
            );
            Ok(serde_json::to_value(&verdict)?)
        }
        "framework_alias" => dispatch_stdio_framework_alias(payload),
        "task_ledger_dispatch" => task_command::dispatch_task_ledger_command_envelope(payload),
        _ => Err(FrameworkError::validation(format!(
            "unsupported framework stdio operation: {op}"
        ))),
    }
}

fn dispatch_tool_stdio_request(op: &str, payload: Value) -> Result<Value, FrameworkError> {
    match op {
        "route_tool" => {
            let query = required_non_empty_string(&payload, "query", "stdio route_tool")?;
            let registry_path = resolve_tool_registry_path_from_payload(&payload)?;
            let decision = tool_routing_engine::routing::route_tool(&query, &registry_path)?
                .ok_or_else(|| {
                    FrameworkError::not_found(format!(
                        "route_tool: no matching tool found for query '{query}'"
                    ))
                })?;
            serde_json::to_value(&decision).map_err(|e| FrameworkError::validation(e.to_string()))
        }
        "search_tools" => {
            let query = required_non_empty_string(&payload, "query", "stdio search_tools")?;
            let top_k = payload.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let registry_path = resolve_tool_registry_path_from_payload(&payload)?;
            let records = mcp_tool_registry::load_tool_records_cached(&registry_path)?;
            let results = tool_routing_engine::search::search_tools(&query, &records, top_k);
            serde_json::to_value(&results).map_err(|e| FrameworkError::validation(e.to_string()))
        }
        "tool_registry_status" => {
            let registry_path = resolve_tool_registry_path_from_payload(&payload)?;
            let records = mcp_tool_registry::load_tool_records_cached(&registry_path)?;
            let status = serde_json::json!({
                "schema_version": mcp_tool_registry::EXPECTED_SCHEMA,
                "total_count": records.len(),
                "builtin_count": records.iter().filter(|r| r.layer == "builtin").count(),
                "research_count": records.iter().filter(|r| r.layer == "research").count(),
                "independent_count": records.iter().filter(|r| r.layer == "independent").count(),
                "external_count": records.iter().filter(|r| r.layer == "external").count(),
            });
            Ok(status)
        }
        _ => Err(FrameworkError::validation(format!(
            "unsupported tool stdio operation: {op}"
        ))),
    }
}

/// Resolve tool registry path from payload, using hooks or falling back to repo_root.
fn resolve_tool_registry_path_from_payload(
    payload: &Value,
) -> Result<std::path::PathBuf, FrameworkError> {
    if let Some(path) = mcp_tool_registry::resolve_tool_registry_path() {
        return Ok(path);
    }
    let repo_root = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    Ok(std::path::PathBuf::from(repo_root)
        .join(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH))
}

fn dispatch_stdio_route(payload: Value) -> Result<Value, FrameworkError> {
    let query = required_non_empty_string(&payload, "query", "stdio route")?;
    let session_id = optional_non_empty_string(&payload, "session_id")
        .unwrap_or_else(|| "route-cli".to_string());
    let allow_overlay = payload
        .get("allow_overlay")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let first_turn = payload
        .get("first_turn")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let owned_inline_records = if payload.get("skills").is_some() {
        Some(load_inline_records(&payload)?)
    } else {
        None
    };
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let host_id = optional_non_empty_string(&payload, "host_id");
    let cached_records;
    let records: &[SkillRecord] = if let Some(items) = owned_inline_records.as_ref() {
        items.as_slice()
    } else {
        cached_records = load_records_cached_for_stdio(runtime_path.as_deref())?;
        cached_records.as_ref()
    };
    let owned_host_records = if host_id.is_some() {
        Some(filter_records_for_host(records, host_id.as_deref())?)
    } else {
        None
    };
    let route_records = if let Some(items) = owned_host_records.as_ref() {
        items.as_slice()
    } else {
        records
    };
    // Always use route_task_with_manifest_fallback to ensure host_id-based
    // manifest lookup. Previously when inline records were provided we called
    // route_task() directly, which bypassed the manifest fallback. RD-7 fix.
    let decision = route_task_with_manifest_fallback(
        route_records,
        host_id.as_deref(),
        &query,
        &session_id,
        allow_overlay,
        first_turn,
    )?;
    serialize_payload(decision, "route")
}

fn dispatch_stdio_search_skills(payload: Value) -> Result<Value, FrameworkError> {
    let query = required_non_empty_string(&payload, "query", "stdio search_skills")?;
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(5);
    let runtime_path = optional_non_empty_string(&payload, "runtime_path").map(PathBuf::from);
    let host_id = optional_non_empty_string(&payload, "host_id");
    let records = load_records_cached_for_stdio(runtime_path.as_deref())?;
    let host_indices = filter_record_indices_for_host(&records, host_id.as_deref())
        .map_err(|e| FrameworkError::validation(format!("host filter: {e}")))?;
    let matches = search_skills_subset(&records, Some(&host_indices), &query, limit);
    let resolved = build_search_results_payload(&query, matches);
    serialize_payload(resolved, "search")
}

fn dispatch_stdio_route_report(payload: Value) -> Result<Value, FrameworkError> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route report")?;
    let route_decision = payload
        .get("route_decision")
        .cloned()
        .filter(|value| !value.is_null())
        .map(serde_json::from_value::<RouteDecision>)
        .transpose()
        .map_err(|err| {
            FrameworkError::validation(format!("parse route decision contract failed: {err}"))
        })?;
    let rust_snapshot = match payload.get("rust_route_snapshot") {
        Some(raw) if !raw.is_null() => serde_json::from_value(raw.clone()).map_err(|err| {
            FrameworkError::validation(format!("parse rust route snapshot failed: {err}"))
        })?,
        _ => route_decision
            .as_ref()
            .map(|decision| decision.route_snapshot.clone())
            .ok_or_else(|| {
                FrameworkError::validation(
                    "route_report requires rust_route_snapshot or route_decision",
                )
            })?,
    };
    serialize_payload(
        build_route_diff_report(&mode, rust_snapshot, route_decision.as_ref())?,
        "route report",
    )
}

fn dispatch_stdio_route_resolution(payload: Value) -> Result<Value, FrameworkError> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route resolution")?;
    let decision_value = payload
        .get("route_decision")
        .cloned()
        .ok_or_else(|| FrameworkError::validation("route_resolution requires route_decision"))?;
    let decision = serde_json::from_value::<RouteDecision>(decision_value).map_err(|err| {
        FrameworkError::validation(format!("parse route resolution input failed: {err}"))
    })?;
    serialize_payload(
        build_route_resolution(&mode, &decision)?,
        "route resolution",
    )
}

fn dispatch_stdio_route_policy(payload: Value) -> Result<Value, FrameworkError> {
    let mode = required_non_empty_string(&payload, "mode", "stdio route policy")?;
    serialize_payload(build_route_policy(&mode)?, "route policy")
}

fn dispatch_stdio_route_snapshot(payload: Value) -> Result<Value, FrameworkError> {
    let request =
        serde_json::from_value::<RouteSnapshotRequestPayload>(payload).map_err(|err| {
            FrameworkError::validation(format!("parse route snapshot input failed: {err}"))
        })?;
    let snapshot = build_route_snapshot(
        &request.engine,
        &request.selected_skill,
        request.overlay_skill.as_deref(),
        &request.layer,
        request.score,
        &request.reasons,
        0,
    );
    serialize_payload(
        RouteSnapshotEnvelopePayload {
            snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
            authority: ROUTE_AUTHORITY.to_string(),
            route_snapshot: snapshot,
        },
        "route snapshot",
    )
}

fn dispatch_stdio_framework_runtime_snapshot(payload: Value) -> Result<Value, FrameworkError> {
    let repo_root_str =
        required_non_empty_string(&payload, "repo_root", "stdio framework runtime snapshot")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root_str)))?;
    let artifact_root = optional_non_empty_string(&payload, "artifact_source_dir");
    let task_id = optional_non_empty_string(&payload, "task_id");
    let detail_level = optional_non_empty_string(&payload, "detail_level")
        .unwrap_or_else(|| "summary".to_string());
    serialize_payload(
        build_framework_runtime_snapshot_envelope_with_level(
            repo_root.as_path(),
            artifact_root.as_deref().map(Path::new),
            task_id.as_deref(),
            &detail_level,
        )?,
        "framework runtime snapshot",
    )
}

fn dispatch_stdio_framework_contract_summary(payload: Value) -> Result<Value, FrameworkError> {
    let repo_root =
        required_non_empty_string(&payload, "repo_root", "stdio framework contract summary")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root)))?;
    serialize_payload(
        build_framework_contract_summary_envelope(repo_root.as_path())?,
        "framework contract summary",
    )
}

fn dispatch_stdio_framework_alias(payload: Value) -> Result<Value, FrameworkError> {
    let repo_root = required_non_empty_string(&payload, "repo_root", "stdio framework alias")?;
    let repo_root = resolve_repo_root_arg(Some(Path::new(&repo_root)))?;
    let alias_name = required_non_empty_string(&payload, "alias", "stdio framework alias")?;
    let max_lines = payload
        .get("max_lines")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(4);
    let compact = payload
        .get("compact")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host_id = payload.get("host_id").and_then(Value::as_str);
    serialize_payload(
        build_framework_alias_envelope(
            repo_root.as_path(),
            &alias_name,
            FrameworkAliasBuildOptions {
                max_lines,
                compact,
                host_id,
            },
        )?,
        "framework alias",
    )
}

fn dispatch_stdio_compile_profile_bundle(payload: Value) -> Result<Value, FrameworkError> {
    let profile_path = required_non_empty_string(&payload, "profile_path", "stdio profile bundle")?;
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let bundle = build_profile_bundle(profile)?;
    serialize_payload(bundle, "profile bundle")
}

fn dispatch_stdio_compile_profile_artifacts(payload: Value) -> Result<Value, FrameworkError> {
    let profile_path =
        required_non_empty_string(&payload, "profile_path", "stdio profile artifacts")?;
    let full = optional_bool(&payload, "full").unwrap_or(false);
    let profile = load_framework_profile(Path::new(&profile_path))?;
    let artifacts = build_profile_artifact_bundle(profile, full)?;
    serialize_payload(artifacts, "host profile artifacts")
}

fn dispatch_stdio_eval_route(payload: Value) -> Result<Value, FrameworkError> {
    let cases_path = required_non_empty_string(&payload, "cases", "stdio eval route")?;
    let runtime = optional_non_empty_string(&payload, "runtime");
    let report = run_eval_route(Path::new(&cases_path), runtime.as_deref().map(Path::new))?;
    serialize_payload(report, "eval route report")
}
