//! Claude 宿主的 [`HostHook`] trait 实现。
//!
//! PreTool / PostTool / Stop / UserPromptSubmit 逻辑已拆至 `claude_hooks/*` handler 模块；
//! CLI 经默认 `run_cli_hook`（observation 附着 + cursor-stdin 误接守卫）。

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use super::host_hook::{HostHook, HookDecision};
use router_rs::router_rs_observation::HookObservationHost;
use serde_json::Value;
use std::path::Path;

pub struct ClaudeHookHost;

impl HostHook for ClaudeHookHost {
    fn host_id(&self) -> &str {
        "claude"
    }

    fn canonical_event(&self, raw: &str) -> FrameworkResult<&'static str> {
        match raw {
            "PreToolUse" | "pre-tool-use" | "pre_tool_use" => Ok("pre-tool-use"),
            "PostToolUse" | "post-tool-use" | "post_tool_use" => Ok("post-tool-use"),
            "Stop" | "stop" => Ok("stop"),
            "UserPromptSubmit" | "user-prompt-submit" => Ok("user-prompt-submit"),
            other => Err(FrameworkError::validation(format!("unknown claude event: {other}"))),
        }
    }

    fn critical_events(&self) -> &[&str] {
        &["pre-tool-use", "stop"]
    }

    fn hook_observation_host(&self) -> Option<HookObservationHost> {
        Some(HookObservationHost::ClaudeCode)
    }

    fn misrouted_stdin_short_circuit(&self, payload: &Value) -> Option<Value> {
        super::claude_hooks::payload_looks_like_cursor_hook_stdin(payload)
            .then(HookDecision::allow_value)
    }

    /// Legacy Claude path: Claude-specific PreTool guards live in handlers only.
    fn evaluate_pre_tool_path_guard(
        &self,
        _repo_root: &Path,
        _payload: &Value,
    ) -> Option<HookDecision> {
        None
    }

    fn handle_pre_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        match super::claude_hooks::evaluate_claude_pre_tool_use(repo_root, payload) {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }

    fn handle_post_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        match super::claude_hooks::evaluate_claude_post_tool_use(repo_root, payload) {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }

    fn handle_stop(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        match super::claude_hooks::evaluate_claude_stop(repo_root, payload) {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }

    fn handle_user_prompt_submit(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        match super::claude_hooks::evaluate_claude_user_prompt_submit(repo_root, payload) {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_hook_canonical_event_mapping() {
        let host = ClaudeHookHost;
        assert_eq!(host.canonical_event("PreToolUse").unwrap(), "pre-tool-use");
        assert_eq!(host.canonical_event("pre-tool-use").unwrap(), "pre-tool-use");
        assert_eq!(host.canonical_event("Stop").unwrap(), "stop");
        assert_eq!(host.canonical_event("stop").unwrap(), "stop");
        assert!(host.canonical_event("unknown_event").is_err());
    }

    #[test]
    fn claude_hook_critical_events() {
        let host = ClaudeHookHost;
        assert_eq!(host.critical_events(), &["pre-tool-use", "stop"]);
    }

    #[test]
    fn claude_hook_host_id() {
        let host = ClaudeHookHost;
        assert_eq!(host.host_id(), "claude");
    }

    #[test]
    fn claude_dispatch_short_circuits_cursor_stdin() {
        let host = ClaudeHookHost;
        let root = Path::new("/tmp");
        let payload = json!({
            "hook_event_name": "postToolUse",
            "cursor_version": "3.3.30",
            "workspace_roots": ["/tmp"],
        });
        assert_eq!(
            host.dispatch(root, "post-tool-use", &payload),
            HookDecision::allow_value()
        );
    }
}
