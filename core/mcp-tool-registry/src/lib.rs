#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # mcp-tool-registry
//!
//! Unified MCP tool registry: discovery and record loading for all MCP tools.
//!
//! This crate provides the Tool Layer — a single source of truth for all MCP tools
//! across dispatch domains (CompositeRegistry, Research, Browser-MCP, External binaries,
//! codegraph-rs).
//!
//! ## Layer boundaries
//!
//! | Concern | Crate |
//! |---|---|
//! | Tool record types, JSON loading | `mcp-tool-registry` (this crate) |
//! | Tool scoring, routing, search | `tool-routing-engine` (routing layer) |
//! | Skill scoring, routing, search | `routing-engine` (routing layer) |
//!
//! ## Browser MCP dispatch
//! The `browser_dispatch` module (ex-browser-mcp-dispatch) provides a thin bridge
//! between `tools/browser-mcp` and `core/host-projection`.

pub mod browser_dispatch;
pub mod hooks;
#[cfg(test)]
pub mod tests;
pub mod tool_registry;
pub mod tool_types;

use std::path::PathBuf;

pub use tool_registry::{
    EXPECTED_SCHEMA, invalidate_tool_cache, invalidate_tool_cache_for_path, load_tool_records,
    load_tool_records_cached,
};
pub use tool_types::{DispatchDomain, McpToolInputSchema, McpToolRecord, ToolLayer, ToolOwner};

/// Resolve the path to MCP_TOOL_REGISTRY.json.
/// Uses the hooks-injected path if available, otherwise returns the default.
pub fn resolve_tool_registry_path() -> Option<PathBuf> {
    hooks::discover_tool_registry_path()
}
