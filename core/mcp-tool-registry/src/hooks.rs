//! Direct path resolution (was a fn-ptr hook — simplified down).

use std::path::PathBuf;

const MCP_TOOL_REGISTRY_RELATIVE_PATH: &str = "configs/framework/MCP_TOOL_REGISTRY.json";

/// Resolve the path to the MCP tool registry JSON file.
pub fn discover_tool_registry_path() -> Option<PathBuf> {
    Some(PathBuf::from(MCP_TOOL_REGISTRY_RELATIVE_PATH))
}
