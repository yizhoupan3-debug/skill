//! CodeGraph MCP thin dispatch.
//!
//! Runs as an independent stdio MCP surface — never inside `browser_mcp`.

use crate::framework_runtime::resolve_repo_root_arg;
use std::path::Path;

pub use codegraph_rs::mcp::{dispatch_tool_call, tool_definitions};

pub fn run_codegraph_mcp_stdio_loop(repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root)?;
    codegraph_rs::mcp::run_stdio_mcp(&repo_root).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::tool_definitions;

    #[test]
    fn exposes_codegraph_tool_catalog() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 7);
    }
}
