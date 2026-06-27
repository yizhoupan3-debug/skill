#![deny(clippy::unwrap_used, clippy::expect_used)]
//! framework-extra: orchestration control plane (L4).
//!
//! Extracted from `runtime-core/src/framework_runtime/` per ADR-010 §10.3.
//! Contains modules with zero or shallow `crate::` dependencies that can be
//! cleanly separated from runtime-core's orchestration glue.

#![recursion_limit = "256"]

pub mod alias;
pub mod closeout;
pub mod content_store;
pub mod contract_summary;
pub mod evidence;
pub mod framework_doctor;
pub mod orchestration_controller;
pub mod prompt_compression;
pub mod prompt_resolver;
pub mod route_manifest_fallback;
pub mod session_artifacts;
pub mod snapshot;
pub mod statusline;
pub mod util;
