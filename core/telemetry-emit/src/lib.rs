//! L0 unified telemetry emit primitives.
//!
//! Sits at L0 so every crate can use it without layering violations.
//! Provides structured emit wrappers for `TelemetryEvent`, `MetricCounter`
//! counters, and convenience macros that pair `tracing::*!` with telemetry.
//!
//! ## Re-exports
//!
//! - `emit_telemetry`, `TelemetryEvent`, `TelemetryWriter` — re-exported from
//!   `framework-kernel` for single-import convenience.
//! - All `emit_*` functions — safe wrappers that call `framework_kernel::emit_telemetry`.
//! - `MetricCounter` — structured counter/gauge with labels.
//! - `emit_warn!`, `emit_info!`, `emit_error!` — macros that pair `tracing::*!`
//!   with telemetry emission.

pub use framework_kernel::{TelemetryEvent, TelemetryWriter, emit_telemetry};
pub use telemetry_types::PredictionOutcomeCheck;

mod emit;
mod macros;
mod metric;

pub use emit::*;
pub use metric::*;
