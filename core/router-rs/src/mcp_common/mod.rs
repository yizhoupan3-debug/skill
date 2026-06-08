//! Shared MCP tool dispatch surfaces for independent stdio processes (not browser-mcp).

#[cfg(feature = "codegraph")]
pub mod codegraph {
    pub use crate::codegraph_mcp::{
        dispatch_tool_call, run_codegraph_mcp_stdio_loop, tool_definitions,
    };
}
