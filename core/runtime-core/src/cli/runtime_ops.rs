//! Runtime I/O 薄壳：再导出 `framework_runtime/` 函数。
//!
//! `dispatch_runtime_output_mode_stdio` / `handles_runtime_output_stdio_op` 已移入
//! `framework_runtime/stdio_op_registry.rs`；`write_*` payload 函数已移入
//! `framework_runtime/trace_transport.rs` — cli 侧仅保留再导出以维持向后兼容。

pub use crate::framework_runtime::json_value::{
    optional_non_empty_string, required_non_empty_string,
};
pub use crate::framework_runtime::stdio_op_registry::{
    dispatch_runtime_output_mode_stdio, handles_runtime_output_stdio_op,
};
pub use crate::framework_runtime::trace_transport::{
    write_checkpoint_resume_manifest_payload, write_text_payload,
    write_transport_binding_payload,
};

#[cfg(test)]
pub use crate::framework_runtime::{
    attach_runtime_event_transport, cleanup_attached_runtime_event_transport, inspect_trace_stream,
    replay_trace_stream, sha256_hex, subscribe_attached_runtime_events,
    write_trace_compaction_delta, write_trace_metadata,
};

#[cfg(test)]
pub use crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
pub use crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request_payload;
#[cfg(test)]
pub use crate::framework_runtime::stdio_op_registry::{
    StdioOpDomain, classify_stdio_op, is_framework_stdio_op, is_routing_stdio_op,
    is_runtime_stdio_op, is_trace_stdio_op,
};
