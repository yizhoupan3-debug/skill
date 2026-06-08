//! CodeGraph MCP thin dispatch (Roadmap v5 B10).
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
    use crate::mcp_common::codegraph;

    #[test]
    fn exposes_codegraph_tool_catalog() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn mcp_common_reexports_dispatch_surface() {
        let tools = codegraph::tool_definitions();
        assert_eq!(tools.len(), 6);
        assert!(tools[0].get("name").is_some());
    }
}
