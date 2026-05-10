//! Live execute、沙箱/后台控制面、trace 传输 helpers、stdio 分发。

use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::args::*;
use super::common::{
    append_text_with_process_lock, manifest_fallback_path, route_task_with_manifest_fallback,
};

use crate::autopilot_goal;
use crate::background_state::handle_background_state_operation;
use crate::closeout_enforcement::{closeout_enforcement_contract, evaluate_closeout_record_value};
use crate::eval_route::{eval_route_contract, run_eval_route};
use crate::execution_contract::{
    build_execution_contract_bundle, build_execution_kernel_contracts_by_mode,
    build_execution_kernel_metadata_contract, build_steady_state_execution_kernel_metadata,
    decode_execution_response_value, normalize_execution_kernel_contract_value,
    normalize_execution_kernel_metadata_contract_value,
    validate_execution_kernel_steady_state_metadata_value, EXECUTION_AUTHORITY,
    EXECUTION_MODEL_ID_SOURCE, EXECUTION_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY, EXECUTION_SCHEMA_VERSION,
};
use crate::framework_profile::{
    build_codex_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use crate::framework_runtime::{
    self, build_framework_alias_envelope, build_framework_contract_summary_envelope,
    build_framework_prompt_compression_envelope, build_framework_runtime_snapshot_envelope,
    framework_hook_evidence_append, resolve_repo_root_arg, write_framework_session_artifacts,
    FrameworkAliasBuildOptions,
};
use crate::hook_policy::evaluate_hook_policy_value;
use crate::rfv_loop;
use crate::route::{
    build_route_diff_report, build_route_policy, build_route_resolution, build_route_snapshot,
    build_search_results_payload, load_inline_records, load_records_cached_for_stdio,
    load_records_from_manifest, route_task, search_skills, RouteDecision,
    RouteDecisionSnapshotPayload, RouteSnapshotEnvelopePayload, RouteSnapshotRequestPayload,
    SkillRecord, ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
};
use crate::runtime_envelope_ids::*;
use crate::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, resolve_storage_backend,
    runtime_backend_family_catalog_payload, runtime_backend_family_parity_payload,
    runtime_storage_operation, storage_artifact_exists, storage_read_text, ResolvedStorageBackend,
    RuntimeStorageRequestPayload,
};
use crate::session_supervisor::handle_session_supervisor_operation;
use crate::stdio_transport::runtime_concurrency_defaults_payload;
use crate::stdio_transport::{StdioJsonRequestPayload, StdioJsonResponsePayload};
use crate::task_command;
use crate::trace_runtime::{
    compact_trace_stream, record_trace_event, TraceCompactRequestPayload,
    TraceRecordEventRequestPayload,
};

include!("runtime_ops.inc");

// Merged from `cli_modes.rs` (must follow `include!` so builder fns exist; avoids import cycle).
struct RuntimeOutputMode {
    stdio_op: Option<&'static str>,
    run_stdio: fn(Value) -> Result<Value, String>,
}

impl RuntimeOutputMode {
    fn call(&self, payload: Value) -> Result<Value, String> {
        (self.run_stdio)(payload)
    }
}

const RUNTIME_OUTPUT_MODES: &[RuntimeOutputMode] = &[
    RuntimeOutputMode {
        stdio_op: Some("runtime_integrator"),
        run_stdio: |_| Ok(build_runtime_integrator_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_control_plane"),
        run_stdio: |_| Ok(build_runtime_control_plane_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_exporter_descriptor"),
        run_stdio: |_| Ok(build_runtime_observability_exporter_descriptor()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_metric_catalog"),
        run_stdio: |_| Ok(build_runtime_observability_metric_catalog_payload()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_observability_dashboard_schema"),
        run_stdio: |_| Ok(runtime_observability_dashboard_schema()),
    },
    RuntimeOutputMode {
        stdio_op: Some("runtime_metric_record"),
        run_stdio: |payload| build_runtime_metric_record(payload),
    },
];

pub(crate) fn dispatch_runtime_output_mode_stdio(
    op: &str,
    payload: Value,
) -> Option<Result<Value, String>> {
    RUNTIME_OUTPUT_MODES
        .iter()
        .find(|mode| mode.stdio_op == Some(op))
        .map(|mode| mode.call(payload))
}

pub(crate) fn handles_runtime_output_stdio_op(op: &str) -> bool {
    RUNTIME_OUTPUT_MODES
        .iter()
        .any(|mode| mode.stdio_op == Some(op))
}
