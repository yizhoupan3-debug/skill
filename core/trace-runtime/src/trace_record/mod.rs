// Schema version constants — visible to all submodules via `use super::CONSTANT;`
pub(super) const TRACE_RECORD_EVENT_SCHEMA_VERSION: &str = "router-rs-trace-record-event-v1";
pub(super) const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
pub(super) const TRACE_EVENT_SCHEMA_VERSION: &str = "runtime-trace-v2";
pub(super) const TRACE_EVENT_SINK_SCHEMA_VERSION: &str = "runtime-trace-sink-v2";

mod record;
mod types;
mod util;

#[cfg(test)]
mod tests;

// Re-exports — makes all `pub` items from submodules accessible at crate level
pub use record::record_trace_event;
pub use types::{
    TraceRecordEventRequestPayload,
    TraceRecordEventResponsePayload,
};
pub use util::{
    build_trace_cursor, hydrate_trace_event, sha256_hex, trace_event_object,
    trace_event_string_field, trace_event_usize_field,
};
