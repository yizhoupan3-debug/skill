//! Opencode MCP agent: `router-rs opencode agent --repo-root …`。
//!
//! MCP 服务器（stdio transport），暴露框架工具（snapshot / skill_route / goal_state_manage 等）。
//! Opencode 为无 hook 宿主，门控通过 opencode.json 的 permission 规则实现。
//! 默认 lifecycle_profile: my-light（advisory closeout）。
//! 非 my-light 时 closeout_gate / goal_state_manage 在 MCP 工具层阻拦。

use crate::framework_runtime::resolve_repo_root_arg;
use crate::mcp_stdio_harness::run_mcp_stdio;
use std::io;
use std::path::Path;

/// Run the MCP stdio agent loop for Opencode host.
pub fn run_opencode_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, "opencode")
}
