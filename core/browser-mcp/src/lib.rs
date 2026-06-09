//! B5: Browser MCP — 浏览器自动化 MCP 服务
//!
//! CDP 协议浏览器自动化，提供 browser_open/click/fill/screenshot 等工具。
//!
//! ## 架构说明
//! 当前为 stub crate — 源码仍在 router-rs/src/browser_mcp/ 中。
//! 后续 wave 将逐步将物理文件迁移到此 crate。
//!
//! 依赖方向：browser-mcp → {core-state, routing-engine, framework-kernel}

pub use framework_kernel;
pub use routing_engine;

/// Browser MCP version
pub const BROWSER_MCP_VERSION: &str = "0.1.0";
