//! Runtime I/O 薄壳：再导出 `framework_runtime/` 函数。
//!
//! `dispatch_runtime_output_mode_stdio` / `handles_runtime_output_stdio_op` 已移入
//! `framework_runtime/stdio_op_registry.rs`；`write_*` payload 函数已移入
//! `framework_runtime/trace_transport.rs` — cli 侧仅保留再导出以维持向后兼容。

pub use fr_exec::trace_transport::{
    write_checkpoint_resume_manifest_payload, write_text_payload, write_transport_binding_payload,
};
pub use fr_utils::stdio_op_registry::{
    dispatch_runtime_output_mode_stdio, handles_runtime_output_stdio_op,
};
pub use framework_kernel::json_value::{optional_non_empty_string, required_non_empty_string};

#[cfg(test)]
pub use fr_exec::trace_attach::{
    attach_runtime_event_transport, cleanup_attached_runtime_event_transport,
    subscribe_attached_runtime_events,
};
#[cfg(test)]
pub use fr_exec::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata,
};
#[cfg(test)]
pub use runtime_core::trace_runtime::sha256_hex;

#[cfg(test)]
pub use fr_utils::stdio_op_registry::{
    StdioOpDomain, classify_stdio_op, is_framework_stdio_op, is_routing_stdio_op,
    is_runtime_stdio_op, is_trace_stdio_op,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
pub use runtime_core::framework_runtime::stdio_dispatch::dispatch_stdio_json_request_payload;

// Re-exports for test prelude (functions moved to framework-extra / fr-exec during refactor).
#[cfg(test)]
pub use fr_exec::live_execute::{
    DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
    build_live_execute_prompt, build_live_execute_response, execute_request,
    extract_chat_completion_content, live_execute_http_client, normalize_chat_completions_endpoint,
    perform_live_execute_with_sender, validate_live_execute_aggregator_base_url,
};
