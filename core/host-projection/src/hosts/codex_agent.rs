//! Codex host MCP stdio agent loop.
//!
//! Provides the `skill_route` / `skill_search` / `skill_read` / `framework_snapshot` /
//! `goal_state_*` / `closeout_*` / `quality_gate_*` / `record_evidence` / `session_checkpoint`
//! tools via MCP stdio, using `host_id = "codex"`.

use std::path::Path;

use crate::hosts::run_agent_mcp_loop;

/// Run the MCP stdio agent loop for Codex host.
pub fn run_codex_agent_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    run_agent_mcp_loop(repo_root_arg, "codex")
}
