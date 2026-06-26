//! L0 unified telemetry emit primitives.
//!
//! Sits at L0 so every crate can use it without layering violations.
//! Provides structured emit wrappers for `TelemetryEvent` and hook action helpers.
//!
//! ## Re-exports
//!
//! - `emit_telemetry`, `TelemetryEvent`, `TelemetryWriter` — re-exported from
//!   `framework-kernel` for single-import convenience.
//! - All `emit_*` functions — safe wrappers that call `framework_kernel::emit_telemetry`.
//!
//! ## Lint
//!
//! Denies unwrap/expect unconditionally.

#![deny(clippy::unwrap_used, clippy::expect_used)]

pub use framework_kernel::{TelemetryEvent, TelemetryWriter, emit_telemetry};
pub use telemetry_types::PredictionOutcomeCheck;

mod emit;

pub use emit::*;
