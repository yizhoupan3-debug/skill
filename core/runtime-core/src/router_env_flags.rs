//! Thin re-export from `framework_runtime::router_env_flags` (single source of truth).
//!
//! This module exists to avoid pulling `framework-runtime` into `runtime-core-contracts`.
//! All env-flag reader functions are defined in `framework-runtime/src/router_env_flags.rs`.
pub use framework_runtime::router_env_flags::*;
