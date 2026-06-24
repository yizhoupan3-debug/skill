//! Hook registry for mcp-tool-registry dependency injection.
//!
//! This crate is a leaf module with zero internal path dependencies.
//! The functions in this module allow runtime-core to register callbacks
//! that the tool registry needs at runtime (registry path, scoring weights path).
//!
//! All hooks are **optional** — unregistered hooks return safe defaults.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Default relative path to the MCP tool registry JSON (from project root).
const DEFAULT_REGISTRY_RELATIVE_PATH: &str = "configs/framework/MCP_TOOL_REGISTRY.json";

type DiscoverToolRegistryPathFn = fn() -> Option<PathBuf>;
type DiscoverScoringWeightsPathFn = fn() -> Option<String>;

struct ToolRegistryHooks {
    discover_tool_registry_path: DiscoverToolRegistryPathFn,
    discover_scoring_weights_path: DiscoverScoringWeightsPathFn,
}

static HOOKS: OnceLock<ToolRegistryHooks> = OnceLock::new();

/// Register all tool registry hooks. Should be called once from runtime-core at startup.
pub fn register_hooks(
    discover_tool_registry_path: DiscoverToolRegistryPathFn,
    discover_scoring_weights_path: DiscoverScoringWeightsPathFn,
) -> Result<(), &'static str> {
    HOOKS
        .set(ToolRegistryHooks {
            discover_tool_registry_path,
            discover_scoring_weights_path,
        })
        .map_err(|_| "tool registry hooks already registered")
}

/// Discover the tool registry JSON path.
/// Default: `configs/framework/MCP_TOOL_REGISTRY.json` (relative to repo root).
pub fn discover_tool_registry_path() -> Option<PathBuf> {
    HOOKS
        .get()
        .and_then(|h| (h.discover_tool_registry_path)())
        .or_else(|| Some(PathBuf::from(DEFAULT_REGISTRY_RELATIVE_PATH)))
}

/// Discover the scoring weights JSON path.
/// Returns the absolute path to `configs/tool_scoring_weights.json` if resolvable.
pub fn discover_scoring_weights_path() -> Option<String> {
    HOOKS
        .get()
        .and_then(|h| (h.discover_scoring_weights_path)())
}
