//! Browser MCP：`include!("frag_*")` 只是把 **同一个 `browser_mcp` 模块** 分到多个磁盘文件以降低单文件体量。
//!
//! **维护契约（硬）**：任何 `frag_*.rs` 的增补/删减必须在 **Rust 顶层项边界**完成（例如在完整 `fn`/`impl`/`struct` 首尾），**严禁**按行号在 **函数体半途**断开，否则会生成不可编译的半截括号。
//!
//! | 分段 | 内容梗概 |
//! |------|----------|
//! | `frag_01_through_types.rs` | MCP 常量、stdio transport、请求分发、`BrowserRuntime`/`BrowserAttachConfig` 及会话/页面等内部类型，`struct CdpClient` |
//! | `frag_impl_browser_runtime.rs` | `impl BrowserRuntime` |
//! | `frag_impl_cdp.rs` | `impl CdpClient` |
//! | `frag_rest.rs` | CDP HTTP/Chrome 助手、Attach 候选与 skill 路由、工具 JSON 收尾等自由函数、`decode_base64` 等 |

include!("frag_01_through_types.rs");
include!("frag_impl_browser_runtime.rs");
include!("frag_impl_cdp.rs");
include!("frag_rest.rs");

#[cfg(test)]
mod tests;

/// Dispatch a browser subcommand (CLI entry point for `router-rs browser ...`).
pub fn dispatch_browser_command(command: runtime_core::cli::args::BrowserSubcommand) -> Result<(), String> {
    use runtime_core::cli::args::BrowserSubcommand;
    match command {
        BrowserSubcommand::McpStdio(command) => run_browser_mcp_stdio_loop(
            command.repo_root.as_deref(),
            BrowserAttachConfig::from_cli_and_env(
                command.runtime_attach_descriptor_path,
                command.runtime_attach_artifact_path,
                command.headless,
            ),
        ),
        BrowserSubcommand::ResolveAttachArtifact(command) => {
            let repo_root = runtime_core::framework_runtime::resolve_repo_root_arg(command.repo_root.as_deref())?;
            let Some(path) =
                resolve_browser_mcp_attach_artifact(&repo_root, command.search_root.as_deref())
            else {
                return Err("no browser-mcp runtime attach artifact candidates found".to_string());
            };
            println!("{path}");
            Ok(())
        }
    }
}

/// Register this crate's dispatch function with runtime-core's browser_dispatch_hook.
/// Call once at startup (e.g., in router-rs-cli main or router-rs lib init).
pub fn register_browser_dispatch() {
    runtime_core::browser_dispatch_hook::set_browser_dispatch(dispatch_browser_command);
}
