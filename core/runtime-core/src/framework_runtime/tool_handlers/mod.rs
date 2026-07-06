//! Tool handler dispatch module — split by dispatch sub-domain.
//!
//! Each sub-module corresponds to a `dispatch_domain` value
//! in MCP_TOOL_REGISTRY.json (`domain:goal` is handled directly in
//! host-projection's tools.rs, `domain:closeout` here).
//!
//! Framework-tool dispatch handlers live directly in host-projection's
//! `tools.rs` (MCP-level handlers), not in runtime-core — so there is
//! no `framework_handler.rs` here.
//!
//! `domain:goal` (goal_state_manage) was previously in `goal_handler.rs`
//! but has been inlined into host-projection/tools.rs to eliminate the
//! intermediate JSON payload construction layer.

pub mod closeout_handler;

pub(crate) use closeout_handler::{closeout_gate_evaluate, closeout_record_write_dispatch};
