//! Claude Code host MCP stdio agent loop.
//!
//! Provides the `skill_route` / `skill_search` / `skill_read` / `framework_snapshot` /
//! `goal_state_*` / `closeout_*` / `quality_gate_*` / `record_evidence` / `session_checkpoint`
//! tools via MCP stdio, using `host_id = "claude"`.

use std::path::Path;

use crate::hosts::run_agent_mcp_loop;

/// Run the MCP stdio agent loop for Claude Code host.
pub fn run_claude_agent_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    run_agent_mcp_loop(repo_root_arg, "claude")
}
