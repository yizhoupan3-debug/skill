//! Claude Code host MCP stdio agent loop.
//!
//! Provides the `skill_route` / `skill_search` / `skill_read` / `framework_snapshot` /
//! `goal_state_*` / `closeout_*` / `rfv_*` / `record_evidence` / `session_checkpoint`
//! tools via MCP stdio, using `host_id = "claude"`.

use std::io;
use std::path::Path;

use crate::hosts::mcp_stdio_harness::run_mcp_stdio;
use framework_kernel::repo_roots::resolve_repo_root_arg;

/// Run the MCP stdio agent loop for Claude Code host.
pub fn run_claude_agent_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, "claude")
}
