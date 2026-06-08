//! B0 root: shared traits for dependency inversion and telemetry pipeline.
pub mod telemetry;
pub mod tokenizer;

pub use telemetry::{
    emit_telemetry, global_telemetry_writer, install_global_telemetry_writer, LogAggregator,
    LogAggregatorHandle, MpscTelemetryWriter, PredictionOutcomeCheck, TelemetryEvent,
    TelemetryWriter,
};
pub use tokenizer::{
    has_parallel_review_candidate_context, install_tokenizer_provider, tokenize_query,
    TokenizerProvider,
};
