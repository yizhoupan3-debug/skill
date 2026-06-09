//! B5: Browser MCP — 浏览器自动化 MCP 服务
//!
//! CDP 协议浏览器自动化，提供 browser_open/click/fill/screenshot 等工具。
//! 从 router-rs 物理迁移到独立 crate。
//!
//! 依赖方向：browser-mcp → router-rs（临时，后续解耦）

include!("frag_01_through_types.rs");
include!("frag_impl_browser_runtime.rs");
include!("frag_impl_cdp.rs");
include!("frag_rest.rs");

#[cfg(test)]
mod tests;
