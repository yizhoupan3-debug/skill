#![deny(clippy::unwrap_used, clippy::expect_used)]
//! runtime-exit-gate: quality gate + closeout enforcement (L4).
//!
//! Extracted from `runtime-core/src/exit_gate/` per ADR-010 §10.3.

pub mod quality_gate;
pub mod schema_drift;
pub mod harness_ops;
