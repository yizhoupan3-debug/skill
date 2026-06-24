//! Hook registry for tool-routing-engine dependency injection.
//!
//! All hooks are **optional** — unregistered hooks return safe defaults.

use std::sync::OnceLock;

type DiscoverScoringWeightsPathFn = fn() -> Option<String>;

struct ToolRoutingHooks {
    discover_scoring_weights_path: DiscoverScoringWeightsPathFn,
}

static HOOKS: OnceLock<ToolRoutingHooks> = OnceLock::new();

/// Register tool routing hooks. Should be called once from runtime-core at startup.
pub fn register_hooks(
    discover_scoring_weights_path: DiscoverScoringWeightsPathFn,
) -> Result<(), &'static str> {
    HOOKS
        .set(ToolRoutingHooks {
            discover_scoring_weights_path,
        })
        .map_err(|_| "tool routing hooks already registered")
}

/// Discover the scoring weights JSON path.
/// Returns the absolute path to `configs/tool_scoring_weights.json` if resolvable.
pub fn discover_scoring_weights_path() -> Option<String> {
    HOOKS
        .get()
        .and_then(|h| (h.discover_scoring_weights_path)())
}
