//! Skill routing: re-export from routing-engine.
//!
//! All route/ logic has been migrated to `routing-engine`. This module
//! re-exports the public API for backward compatibility with `crate::route::*` paths.
//! The test file `metadata_tests.rs` remains here because it depends on
//! `route_task_with_manifest_fallback` which lives in runtime-core.

pub use routing_engine::route::*;

#[cfg(test)]
mod metadata_tests;
