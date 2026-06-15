//! Runtime I/O 薄壳：`write_*` 载荷与 `framework_runtime/` 再导出。

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::common::validate_write_path;
use crate::runtime_envelope_ids::{
    CHECKPOINT_MANIFEST_WRITE_AUTHORITY, CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
    TRANSPORT_BINDING_WRITE_AUTHORITY, TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
    WRITE_TEXT_PAYLOAD_TEMP_COUNTER,
};

pub use crate::framework_runtime::json_value::{
    optional_non_empty_string, required_non_empty_string,
};
use crate::framework_runtime::trace_transport::{
    build_checkpoint_resume_manifest, build_trace_transport_payload,
};
#[cfg(test)]
pub use crate::framework_runtime::{
    attach_runtime_event_transport, cleanup_attached_runtime_event_transport, inspect_trace_stream,
    replay_trace_stream, sha256_hex, subscribe_attached_runtime_events,
    write_trace_compaction_delta, write_trace_metadata,
};
include!("runtime_ops.inc");

#[cfg(test)]
pub use crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
pub use crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request_payload;
#[cfg(test)]
pub use crate::framework_runtime::stdio_op_registry::{
    StdioOpDomain, classify_stdio_op, is_framework_stdio_op, is_routing_stdio_op,
    is_runtime_stdio_op, is_trace_stdio_op,
};

#[cfg(test)]
#[allow(unused_imports)]
pub use crate::framework_runtime::live_execute::payload_text_signals_deep_research;
#[cfg(test)]
pub use crate::framework_runtime::live_execute::{
    DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
    LiveExecuteResult, build_live_execute_prompt, build_live_execute_response, execute_request,
    extract_chat_completion_content, live_execute_http_client, normalize_chat_completions_endpoint,
    perform_live_execute, perform_live_execute_with_sender,
    validate_live_execute_aggregator_base_url,
};

#[cfg(test)]
pub use crate::framework_runtime::{
    build_background_control_response, build_runtime_observability_health_snapshot,
    build_sandbox_control_response,
};
pub use crate::framework_runtime::{
    build_runtime_control_plane_payload, build_runtime_integrator_payload,
    build_runtime_metric_record, build_runtime_observability_exporter_descriptor,
    build_runtime_observability_metric_catalog_payload, runtime_observability_dashboard_schema,
};

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

pub fn dispatch_runtime_output_mode_stdio(
    op: &str,
    payload: Value,
) -> Option<Result<Value, String>> {
    RUNTIME_OUTPUT_MODES
        .iter()
        .find(|mode| mode.stdio_op == Some(op))
        .map(|mode| mode.call(payload))
}

pub fn handles_runtime_output_stdio_op(op: &str) -> bool {
    RUNTIME_OUTPUT_MODES
        .iter()
        .any(|mode| mode.stdio_op == Some(op))
}
