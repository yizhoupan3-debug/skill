//! # mcp-tool-registry
//!
//! Unified MCP tool registry: discovery, routing, and search for all MCP tools.
//!
//! This crate provides the Tool Layer — a single source of truth for all MCP tools
//! across dispatch domains (CompositeRegistry, Research, Browser-MCP, External binaries,
//! codegraph-rs).

pub mod fuzzy;
pub mod hooks;
pub(crate) mod scoring_config;
pub mod tool_registry;
pub mod tool_routing;
pub mod tool_search;
pub mod tool_types;

use std::path::PathBuf;

pub use tool_types::{McpToolDecision, McpToolInputSchema, McpToolRecord};
pub use tool_routing::route_tool;
pub use tool_search::{search_tools, ToolSearchResult};
pub use tool_registry::{load_tool_records, load_tool_records_cached, invalidate_tool_cache};

/// Resolve the path to MCP_TOOL_REGISTRY.json.
/// Uses the hooks-injected path if available, otherwise returns the default.
pub fn resolve_tool_registry_path() -> Option<PathBuf> {
    hooks::discover_tool_registry_path()
}

#[cfg(test)]
mod tests;
