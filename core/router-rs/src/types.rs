//! Re-export of typed data structures from runtime-core.
//!
//! Canonical definitions live in `runtime_core::types`. This module preserves
//! the `router_rs::types` import path for downstream callers.

#[allow(unused_imports)] // re-export available for future use
pub use runtime_core::types::*;
