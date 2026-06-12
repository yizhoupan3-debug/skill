//! B0 root: shared traits for dependency inversion and telemetry pipeline.

// ── B0 core modules ──
pub mod framework_host_targets;
pub mod repo_roots;
pub mod router_self;
pub mod runtime_registry;
pub mod telemetry;
pub mod tokenizer;

// ── leaf modules migrated from runtime-core ──
pub mod skill_repo;
pub mod stdio_payload_types;

// ── migrated from framework-profile crate ──
pub mod framework_profile;

pub use telemetry::{
    emit_telemetry, global_telemetry_writer, install_global_telemetry_writer, LogAggregator,
    LogAggregatorHandle, MpscTelemetryWriter, PredictionOutcomeCheck, TelemetryEvent,
    TelemetryWriter,
};
pub use tokenizer::{
    has_parallel_review_candidate_context, install_tokenizer_provider, tokenize_query,
    TokenizerProvider,
};
