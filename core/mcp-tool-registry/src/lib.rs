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

pub mod error;
pub mod hooks;
pub mod tool_registry;
pub mod tool_types;

use std::path::PathBuf;

pub use tool_types::{McpToolInputSchema, McpToolRecord};
pub use tool_registry::{load_tool_records, load_tool_records_cached, invalidate_tool_cache, EXPECTED_SCHEMA};

/// Resolve the path to MCP_TOOL_REGISTRY.json.
/// Uses the hooks-injected path if available, otherwise returns the default.
pub fn resolve_tool_registry_path() -> Option<PathBuf> {
    hooks::discover_tool_registry_path()
}
