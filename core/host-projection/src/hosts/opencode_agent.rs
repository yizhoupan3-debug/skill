//! Opencode MCP agent: `router-rs opencode agent --repo-root ...`。
//!
//! MCP 服务器（stdio transport），暴露框架工具（snapshot / skill_route / goal_state_manage 等）。
//! Hook 处理通过 `router-rs opencode hook`（native Rust hook，与 cursor/claude/codex 统一）。
//! 默认 lifecycle_profile: my-light（advisory closeout）。
//! 非 my-light 时 closeout_gate / goal_state_manage 在 MCP 工具层阻拦。

use std::path::Path;

use crate::hosts::run_agent_mcp_loop;

/// Run the MCP stdio agent loop for Opencode host.
pub fn run_opencode_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    run_agent_mcp_loop(repo_root_arg, "opencode")
}
