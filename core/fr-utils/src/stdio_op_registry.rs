//! Stdio JSON op domain registry.

use core_errors::FrameworkError;
use serde_json::Value;

/// Domain classification for a stdio operation.
///
/// Each variant corresponds to a namespace of op strings: Routing (skill/router dispatch),
/// Runtime (execution, background, session control), Trace (event recording/replay), or Framework
/// (snapshot, contract, goal/RFV). Use to group ops for dispatch routing and contract validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdioOpDomain {
    Routing,
    Runtime,
    Trace,
    Framework,
    Tool,
}

const ROUTING_STDIO_OPS: &[&str] = &[
    "route",
    "search_skills",
    "hook_policy",
    "pre_tool_use_guard",
    "concurrency_defaults",
    "route_report",
    "route_resolution",
    "route_policy",
    "route_snapshot",
    "compile_profile_bundle",
    "compile_profile_artifacts",
    "closeout_evaluate",
    "closeout_contract",
    "hook_event_routing_contract",
    "eval_route",
    "eval_route_contract",
];

const RUNTIME_STDIO_OPS: &[&str] = &[
    "execute",
    "execution_contract_bundle",
    "normalize_execution_kernel_metadata_contract",
    "normalize_execution_kernel_contract",
    "validate_execution_kernel_steady_state_metadata",
    "decode_execution_response",
    "runtime_observability_health_snapshot",
    "background_control",
    "background_state",
    "orchestrator",
    "describe_transport",
    "describe_handoff",
    "checkpoint_resume_manifest",
    "runtime_checkpoint_control_plane",
    "write_transport_binding",
    "write_checkpoint_resume_manifest",
    "attach_runtime_event_transport",
    "subscribe_attached_runtime_events",
    "cleanup_attached_runtime_event_transport",
    "runtime_storage",
    "control_plane_contracts",
];

const TRACE_STDIO_OPS: &[&str] = &[
    "trace_record_event",
    "trace_stream_replay",
    "trace_stream_inspect",
    "trace_compact",
    "write_trace_compaction_delta",
    "write_trace_metadata",
];

const FRAMEWORK_STDIO_OPS: &[&str] = &[
    "framework_runtime_snapshot",
    "framework_contract_summary",
    "framework_prompt_compression",
    "framework_resolve_content",
    "framework_session_artifact_write",
    "framework_hook_evidence_append",
    "framework_goal_drive",
    "framework_alias",
    "task_ledger_dispatch",
];

const TOOL_STDIO_OPS: &[&str] = &[
    "route_tool",
    "search_tools",
    "tool_registry_status",
    // tool_decision_report and tool_eval removed: stubs never implemented
];

fn op_in_domain(op: &str, domain_ops: &[&str]) -> bool {
    domain_ops.contains(&op)
}

/// Classify a stdio operation string into its domain.
///
/// Returns `Some(StdioOpDomain)` if the op is recognized, or `None` for unknown ops. Use as the
/// primary entry point for routing incoming stdio requests to the correct dispatch handler.
pub fn classify_stdio_op(op: &str) -> Option<StdioOpDomain> {
    if op_in_domain(op, ROUTING_STDIO_OPS) {
        Some(StdioOpDomain::Routing)
    } else if op_in_domain(op, RUNTIME_STDIO_OPS) {
        Some(StdioOpDomain::Runtime)
    } else if op_in_domain(op, TRACE_STDIO_OPS) {
        Some(StdioOpDomain::Trace)
    } else if op_in_domain(op, FRAMEWORK_STDIO_OPS) {
        Some(StdioOpDomain::Framework)
    } else if op_in_domain(op, TOOL_STDIO_OPS) {
        Some(StdioOpDomain::Tool)
    } else {
        None
    }
}

/// Check whether a stdio operation belongs to the Routing domain.
/// Use for domain-specific dispatch filtering or contract reporting.
pub fn is_routing_stdio_op(op: &str) -> bool {
    op_in_domain(op, ROUTING_STDIO_OPS)
}

/// Check whether a stdio operation belongs to the Runtime domain.
/// Use for domain-specific dispatch filtering or contract reporting.
pub fn is_runtime_stdio_op(op: &str) -> bool {
    op_in_domain(op, RUNTIME_STDIO_OPS)
}

/// Check whether a stdio operation belongs to the Trace domain.
/// Use for domain-specific dispatch filtering or contract reporting.
pub fn is_trace_stdio_op(op: &str) -> bool {
    op_in_domain(op, TRACE_STDIO_OPS)
}

/// Check whether a stdio operation belongs to the Framework domain.
/// Use for domain-specific dispatch filtering or contract reporting.
pub fn is_framework_stdio_op(op: &str) -> bool {
    op_in_domain(op, FRAMEWORK_STDIO_OPS)
}

/// ── Runtime output mode dispatch (stub) ──
///
/// These functions are intentionally stubbed here. The real implementation
/// lives in `runtime-core::framework_runtime::orchestration_controller` and is
/// registered via host-projection hooks at runtime. The stubs break the
/// `cli ↔ framework-runtime` circular dependency — the framework-runtime crate
/// cannot depend on `runtime-core`.
///
/// `handles_runtime_output_stdio_op` returns `false`, so the dispatch branch
/// is never entered. If someone re-enables it, the `None` from dispatch is
/// caught by the caller in `stdio_dispatch.rs` and returns an error.
pub fn dispatch_runtime_output_mode_stdio(
    _op: &str,
    _payload: Value,
) -> Option<Result<Value, FrameworkError>> {
    None
}

/// Returns `false` for all runtime output mode operations.
///
/// The real dispatch is handled in `runtime-core::orchestration_controller` via host-projection
/// hooks. This stub returns `false` so the dispatch branch in `stdio_dispatch.rs` is never entered.
pub fn handles_runtime_output_stdio_op(_op: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn classify_routing_ops() {
        assert_eq!(classify_stdio_op("route"), Some(StdioOpDomain::Routing));
        assert_eq!(
            classify_stdio_op("search_skills"),
            Some(StdioOpDomain::Routing)
        );
        assert_eq!(
            classify_stdio_op("eval_route"),
            Some(StdioOpDomain::Routing)
        );
    }

    #[test]
    fn classify_runtime_ops() {
        assert_eq!(classify_stdio_op("execute"), Some(StdioOpDomain::Runtime));
        assert_eq!(
            classify_stdio_op("orchestrator"),
            Some(StdioOpDomain::Runtime)
        );
    }

    #[test]
    fn classify_trace_ops() {
        assert_eq!(
            classify_stdio_op("trace_record_event"),
            Some(StdioOpDomain::Trace)
        );
        assert_eq!(
            classify_stdio_op("trace_stream_replay"),
            Some(StdioOpDomain::Trace)
        );
    }

    #[test]
    fn classify_framework_ops() {
        assert_eq!(
            classify_stdio_op("framework_goal_drive"),
            Some(StdioOpDomain::Framework)
        );
    }

    #[test]
    fn classify_unknown_op_returns_none() {
        assert_eq!(classify_stdio_op("unknown_op"), None);
        assert_eq!(classify_stdio_op(""), None);
    }

    #[test]
    fn domain_predicates_match() {
        assert!(is_routing_stdio_op("route"));
        assert!(!is_routing_stdio_op("execute"));
        assert!(is_runtime_stdio_op("execute"));
        assert!(!is_runtime_stdio_op("route"));
        assert!(is_trace_stdio_op("trace_record_event"));
        assert!(!is_trace_stdio_op("route"));
        assert!(is_framework_stdio_op("framework_goal_drive"));
        assert!(!is_framework_stdio_op("route"));
    }

    #[test]
    fn dispatch_stub_returns_none() {
        assert!(dispatch_runtime_output_mode_stdio("test", serde_json::Value::Null).is_none());
        assert!(!handles_runtime_output_stdio_op("test"));
    }
}
