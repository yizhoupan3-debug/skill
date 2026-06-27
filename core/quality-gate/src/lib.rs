//! quality-gate: QG Route — pluggable checker trait, registry, and aggregate.
//!
//! This crate implements the QG Route concept from the v10 architecture:
//! a scene-dispatched, composable quality gate that runs a chain of
//! `GateChecker` implementations and aggregates their results.
//!
//! Key design decisions (roadmap §6):
//! - `GateChecker::check()` is synchronous; async checkers use `runtime_handle`
//! - `CheckResult.passed` is a checker-level self-judgment; `GateVerdict.passed`
//!   is determined by aggregate rules (§2.5)
//! - No checker-level severity in `CheckResult` — severity belongs to `Finding`
//! - No `previous_results` in `CheckContext` — pure functional contract
//! - Scene constants are the only valid scene values; unknown → "general"

#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod checker;
pub mod registry;
pub mod scene;
pub mod types;

// Re-exports for convenience.
pub use checker::GateChecker;
pub use registry::CheckerRegistry;
pub use types::{CheckContext, CheckResult, Finding, GateVerdict, Severity};
