//! Cursor 宿主的 [`HostHook`] trait 实现。
//!
//! Review gate / subagent 门控逻辑在 `cursor_hooks::handlers`；CLI 入口经
//! [`router_rs::hosts::cursor_hooks::execute_cursor_hook`]（`dispatch` → handler → post-process）。

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use super::host_hook::{HostHook, HookDecision};
use router_rs::hosts::cursor_hooks::dispatch_cursor_hook_event;
use serde_json::Value;
use std::path::Path;

pub struct CursorHookHost;

impl HostHook for CursorHookHost {
    fn host_id(&self) -> &str {
        "cursor"
    }

    fn canonical_event(&self, raw: &str) -> FrameworkResult<&'static str> {
        // Cursor 事件名格式：camelCase（hooks.json 注册格式）或 kebab-case。
        // dispatch_cursor_hook_event 使用 normalize_cursor_dispatch_event 处理，这里保持一致。
        match raw.trim().to_ascii_lowercase().as_str() {
            "beforesubmitprompt" | "before-submit-prompt" | "before_submit_prompt"
            | "userpromptsubmit" | "user-prompt-submit" | "user_prompt_submit" => {
                Ok("user-prompt-submit")
            }
            "posttooluse" | "post-tool-use" | "post_tool_use" => Ok("post-tool-use"),
            "stop" => Ok("stop"),
            "sessionstart" | "session-start" | "session_start" => Ok("session-start"),
            "sessionend" | "session-end" | "session_end" => Ok("session-end"),
            "subagentstart" | "subagent-start" | "subagent_start" => Ok("subagent-start"),
            "subagentstop" | "subagent-stop" | "subagent_stop" => Ok("subagent-stop"),
            "beforeshellexecution" | "before-shell-execution" | "before_shell_execution" => {
                Ok("before-shell-execution")
            }
            "aftershellexecution" | "after-shell-execution" | "after_shell_execution" => {
                Ok("after-shell-execution")
            }
            "afteragentresponse" | "after-agent-response" | "after_agent_response" => {
                Ok("after-agent-response")
            }
            "afterfileedit" | "after-file-edit" | "after_file_edit" => Ok("after-file-edit"),
            "precompact" | "pre-compact" | "pre_compact" => Ok("pre-compact"),
            other => Err(FrameworkError::validation(format!("unknown cursor event: {other}"))),
        }
    }

    fn critical_events(&self) -> &[&str] {
        // Cursor 的 critical 事件：stop 和 post-tool-use（与 review_gate.rs 对齐）。
        &["stop", "post-tool-use"]
    }

    fn finalize_cli_output(&self, output: &mut Value) {
        router_rs::goal_state::scrub_followup_fields_in_hook_output(output);
        router_rs::hosts::cursor_hooks::apply_cursor_hook_output_policy(output);
        router_rs::hosts::cursor_hooks::apply_cursor_hook_silent_policy(output);
    }

    fn handle_pre_tool_use(&self, _repo_root: &Path, _payload: &Value) -> HookDecision {
        // Cursor 没有 PreToolUse 事件（cursor hooks 不注册此事件）。
        HookDecision::Allow
    }

    fn handle_post_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        HookDecision::Custom(dispatch_cursor_hook_event(repo_root, "postToolUse", payload))
    }

    fn handle_stop(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        HookDecision::Custom(dispatch_cursor_hook_event(repo_root, "stop", payload))
    }

    fn handle_user_prompt_submit(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        HookDecision::Custom(dispatch_cursor_hook_event(
            repo_root,
            "beforeSubmitPrompt",
            payload,
        ))
    }

    fn handle_custom_event(
        &self,
        event: &str,
        repo_root: &Path,
        payload: &Value,
    ) -> HookDecision {
        HookDecision::Custom(dispatch_cursor_hook_event(repo_root, event, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_hook_canonical_event_mapping_camel_case() {
        let host = CursorHookHost;
        assert_eq!(
            host.canonical_event("beforeSubmitPrompt").unwrap(),
            "user-prompt-submit"
        );
        assert_eq!(
            host.canonical_event("postToolUse").unwrap(),
            "post-tool-use"
        );
        assert_eq!(host.canonical_event("stop").unwrap(), "stop");
        assert_eq!(
            host.canonical_event("sessionStart").unwrap(),
            "session-start"
        );
        assert_eq!(
            host.canonical_event("sessionEnd").unwrap(),
            "session-end"
        );
        assert_eq!(
            host.canonical_event("subagentStart").unwrap(),
            "subagent-start"
        );
        assert_eq!(
            host.canonical_event("subagentStop").unwrap(),
            "subagent-stop"
        );
        assert_eq!(
            host.canonical_event("beforeShellExecution").unwrap(),
            "before-shell-execution"
        );
        assert_eq!(
            host.canonical_event("afterShellExecution").unwrap(),
            "after-shell-execution"
        );
        assert_eq!(
            host.canonical_event("afterAgentResponse").unwrap(),
            "after-agent-response"
        );
        assert_eq!(
            host.canonical_event("afterFileEdit").unwrap(),
            "after-file-edit"
        );
        assert_eq!(
            host.canonical_event("preCompact").unwrap(),
            "pre-compact"
        );
    }

    #[test]
    fn cursor_hook_canonical_event_mapping_kebab_case() {
        let host = CursorHookHost;
        assert_eq!(
            host.canonical_event("before-submit-prompt").unwrap(),
            "user-prompt-submit"
        );
        assert_eq!(
            host.canonical_event("post-tool-use").unwrap(),
            "post-tool-use"
        );
        assert_eq!(host.canonical_event("stop").unwrap(), "stop");
        assert_eq!(
            host.canonical_event("session-start").unwrap(),
            "session-start"
        );
        assert_eq!(
            host.canonical_event("subagent-start").unwrap(),
            "subagent-start"
        );
    }

    #[test]
    fn cursor_hook_canonical_event_mapping_snake_case() {
        let host = CursorHookHost;
        assert_eq!(
            host.canonical_event("before_submit_prompt").unwrap(),
            "user-prompt-submit"
        );
        assert_eq!(
            host.canonical_event("post_tool_use").unwrap(),
            "post-tool-use"
        );
        assert_eq!(
            host.canonical_event("session_start").unwrap(),
            "session-start"
        );
    }

    #[test]
    fn cursor_hook_canonical_event_unknown() {
        let host = CursorHookHost;
        assert!(host.canonical_event("unknown_event").is_err());
        assert!(host.canonical_event("PreToolUse").is_err());
    }

    #[test]
    fn cursor_hook_critical_events() {
        let host = CursorHookHost;
        assert_eq!(host.critical_events(), &["stop", "post-tool-use"]);
    }

    #[test]
    fn cursor_hook_host_id() {
        let host = CursorHookHost;
        assert_eq!(host.host_id(), "cursor");
    }
}
