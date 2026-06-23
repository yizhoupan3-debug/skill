//! Minimal L1 env flag helpers — only flags needed by L2 contracts.
//! The canonical SSoT for most env flags is `core_policy::env_flags`.

/// Check if the pre-tool-use guard should be skipped.
/// `ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD` (default OFF; `1`|`true`|`on`|`yes` enables).
pub fn router_rs_skip_pre_tool_use_guard() -> bool {
    matches!(
        std::env::var("ROUTER_RS_SKIP_PRE_TOOL_USE_GUARD")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "on" | "yes"
    )
}
