//! HostHook 参考实现示例。
//!
//! 此模块演示如何为一个 hook 宿主实现 [`router_rs::hosts::host_hook::HostHook`] trait。
//! 实际宿主（claude, cursor, codex）的迁移应参考此模式。

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use super::host_hook::{HostHook, HookDecision};
use serde_json::Value;
use std::path::Path;

#[cfg_attr(not(test), allow(dead_code))]
/// 示例 hook 宿主：最小化实现，所有事件返回 Allow。
pub struct ExampleHookHost;

impl HostHook for ExampleHookHost {
    fn host_id(&self) -> &str {
        "example"
    }

    fn canonical_event(&self, raw: &str) -> FrameworkResult<&'static str> {
        match raw {
            "PreToolUse" | "pre-tool-use" | "pre_tool_use" => Ok("pre-tool-use"),
            "PostToolUse" | "post-tool-use" | "post_tool_use" => Ok("post-tool-use"),
            "Stop" | "stop" => Ok("stop"),
            "UserPromptSubmit" | "user-prompt-submit" => Ok("user-prompt-submit"),
            other => Err(FrameworkError::validation(format!("unknown event: {other}"))),
        }
    }

    fn critical_events(&self) -> &[&str] {
        &["pre-tool-use", "stop"]
    }

    fn handle_pre_tool_use(&self, _repo_root: &Path, _payload: &Value) -> HookDecision {
        HookDecision::Allow
    }

    fn handle_post_tool_use(&self, _repo_root: &Path, _payload: &Value) -> HookDecision {
        HookDecision::Allow
    }

    fn handle_stop(&self, _repo_root: &Path, _payload: &Value) -> HookDecision {
        HookDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn example_hook_canonical_event_mapping() {
        let host = ExampleHookHost;
        assert_eq!(host.canonical_event("PreToolUse").unwrap(), "pre-tool-use");
        assert_eq!(host.canonical_event("stop").unwrap(), "stop");
        assert!(host.canonical_event("unknown_event").is_err());
    }

    #[test]
    fn example_hook_critical_events() {
        let host = ExampleHookHost;
        assert_eq!(host.critical_events(), &["pre-tool-use", "stop"]);
    }

    #[test]
    fn example_hook_allows_all_events() {
        let host = ExampleHookHost;
        let root = Path::new("/tmp");
        let payload = json!({});
        assert!(matches!(host.handle_pre_tool_use(root, &payload), HookDecision::Allow));
        assert!(matches!(host.handle_post_tool_use(root, &payload), HookDecision::Allow));
        assert!(matches!(host.handle_stop(root, &payload), HookDecision::Allow));
    }

    #[test]
    fn example_hook_dispatch_returns_allow() {
        let host = ExampleHookHost;
        let root = Path::new("/tmp");
        let result = host.dispatch(root, "PreToolUse", &json!({}));
        assert_eq!(result, HookDecision::allow_value());
    }

    #[test]
    fn example_hook_dispatch_unknown_event_silent_success() {
        let host = ExampleHookHost;
        let root = Path::new("/tmp");
        let result = host.dispatch(root, "unknown_event", &json!({}));
        assert_eq!(result, host.silent_success());
    }
}
