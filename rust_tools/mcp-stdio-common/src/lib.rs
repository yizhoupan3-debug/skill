//! Shared utilities for MCP stdio tool crates.

pub mod stdio_server;
pub mod util;

/// Generate a `fn main()` entry point for an MCP stdio server binary.
///
/// # Usage
/// ```ignore
/// mcp_stdio_common::mcp_stdio_main!("mcp-pdf", pdf_tool_rs);
/// ```
///
/// Expands to a `main` function that resolves `repo_root` from the current
/// working directory and delegates to `stdio_server::run_stdio_mcp` with
/// the given crate's `mcp::tool_definitions` and `mcp::dispatch` functions.
#[macro_export]
macro_rules! mcp_stdio_main {
    ($server_name:expr, $crate_name:ident) => {
        fn main() -> anyhow::Result<()> {
            let repo_root =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            mcp_stdio_common::stdio_server::run_stdio_mcp(
                &repo_root,
                $server_name,
                $crate_name::mcp::tool_definitions,
                $crate_name::mcp::dispatch,
            )
        }
    };
}
