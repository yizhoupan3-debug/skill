#![deny(clippy::unwrap_used, clippy::expect_used)]
pub mod error;
pub use error::TraceError;
pub use trace_record::{
    build_trace_cursor, compact_trace_stream, hydrate_trace_event, record_trace_event,
    sha256_hex, trace_event_object, trace_event_string_field, trace_event_usize_field,
    TraceCompactRequestPayload, TraceCompactResponsePayload, TraceRecordEventRequestPayload,
    TraceRecordEventResponsePayload, TraceTextWrite,
};
mod trace_record;
