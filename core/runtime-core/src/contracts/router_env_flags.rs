//! Re-export from `framework-runtime` (single source of truth).
//!
//! B3 extraction: the canonical implementation lives in the leaf crate
//! `framework-runtime` to avoid physical duplication. This module re-exports
//! the entire public API so `runtime_core::router_env_flags::*` continues
//! to work for all downstream consumers.
//!
//! See `framework-runtime/src/router_env_flags.rs` for documentation, flag
//! inventory, and unit tests.

pub use framework_runtime::router_env_flags::*;
