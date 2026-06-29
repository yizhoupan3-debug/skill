/// Standard subagent tool names used across all hosts.
pub const SUBAGENT_TOOL_NAMES: &[&str] = &[
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
];

/// Check if a normalized tool name indicates a subagent operation.
pub fn is_subagent_tool(normalized: &str) -> bool {
    if SUBAGENT_TOOL_NAMES.contains(&normalized) {
        return true;
    }
    // Segment-boundary matching: ".subagent", "_subagent", ".spawn_agent", "_spawn_agent"
    if normalized.ends_with("_subagent")
        || normalized.ends_with("_spawn_agent")
        || normalized.ends_with(".subagent")
        || normalized.ends_with(".spawn_agent")
    {
        return true;
    }
    // Dot-segment matching: "xxx.subagent.yyy"
    normalized
        .split('.')
        .any(|seg| seg == "subagent" || seg == "spawn_agent")
}
