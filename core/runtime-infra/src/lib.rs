//! runtime-infra: shared infrastructure (B0-level semantics, L4 dependencies).
//!
//! Extracted from `runtime-core/src/infrastructure/` per ADR-010 §10.3.

pub mod kernel_bootstrap;
pub mod stdio_transport;
pub mod telemetry_emit;
