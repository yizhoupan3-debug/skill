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
