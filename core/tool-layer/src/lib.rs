//! tool-layer: 工具注册表抽象 (ToolRegistry trait) 与统一工具发现。
//!
//! 提供 MCP 工具、Native 工具、插件工具的注册/发现/调用抽象。
//! 各宿主 (host-projection) 的 MCP stdio 适配器消费此层的 registry。

pub mod tool_registry;
