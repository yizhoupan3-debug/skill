// Schema version constants — visible to all submodules via `use super::CONSTANT;`
pub(super) const TRACE_RECORD_EVENT_SCHEMA_VERSION: &str = "router-rs-trace-record-event-v1";
pub(super) const TRACE_COMPACT_SCHEMA_VERSION: &str = "router-rs-trace-compact-v1";
pub(super) const TRACE_COMPACTION_RESULT_SCHEMA_VERSION: &str = "runtime-trace-compaction-result-v1";
pub(super) const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
pub(super) const TRACE_EVENT_SCHEMA_VERSION: &str = "runtime-trace-v2";
pub(super) const TRACE_EVENT_SINK_SCHEMA_VERSION: &str = "runtime-trace-sink-v2";
pub(super) const TRACE_REPLAY_CURSOR_SCHEMA_VERSION: &str = "runtime-trace-cursor-v1";
pub(super) const TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION: &str = "runtime-trace-compaction-snapshot-v1";
pub(super) const TRACE_COMPACTION_DELTA_SCHEMA_VERSION: &str = "runtime-trace-compaction-delta-v1";
pub(super) const TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION: &str = "runtime-trace-artifact-ref-v1";
pub(super) const TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION: &str = "runtime-trace-compaction-manifest-v1";

mod types;
mod record;
mod compact;
mod util;

#[cfg(test)]
mod tests;

// Re-exports — makes all `pub` items from submodules accessible at crate level
pub use types::*;
pub use record::*;
pub use compact::*;
pub use util::*;
