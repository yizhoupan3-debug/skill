//! Direct path resolution (was a fn-ptr hook — simplified down).

const TOOL_SCORING_WEIGHTS_RELATIVE_PATH: &str = "configs/tool_scoring_weights.json";

/// Resolve the path to the tool scoring weights JSON.
/// Uses `SKILL_FRAMEWORK_ROOT` env var if set, otherwise returns None.
pub fn discover_scoring_weights_path() -> Option<String> {
    let root = std::env::var("SKILL_FRAMEWORK_ROOT").ok()?;
    Some(format!("{root}/{TOOL_SCORING_WEIGHTS_RELATIVE_PATH}"))
}
