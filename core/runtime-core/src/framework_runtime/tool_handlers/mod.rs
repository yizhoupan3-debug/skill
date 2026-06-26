//! Tool handler dispatch module — split by dispatch sub-domain.
//!
//! Each sub-module corresponds to a `dispatch_domain` value
//! in MCP_TOOL_REGISTRY.json (`domain:goal`, `domain:quality-gate`,
//! `domain:closeout`, `domain:framework`).
//!
//! Framework-tool dispatch handlers live directly in host-projection's
//! `tools.rs` (MCP-level handlers), not in runtime-core — so there is
//! no `framework_handler.rs` here.

pub mod closeout_handler;
pub mod goal_handler;
pub mod quality_gate_handler;

pub use closeout_handler::{closeout_gate_evaluate, closeout_record_write_dispatch};
pub use goal_handler::goal_state_manage_dispatch;
pub use quality_gate_handler::quality_gate_manage_dispatch;
