//! Codex 宿主的 [`HostHook`] trait 实现。
//!
//! CLI 经 [`CodexHookHost::run_codex_hook`]（与 Claude `run_cli_hook` 对齐）；
//! `contract-guard` / `lifecycle-context` 保留 audit 子命令路径。

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use super::host_hook::{HostHook, HookDecision};
use router_rs::router_rs_observation::HookObservationHost;
use serde_json::{json, Value};
use std::path::Path;

pub struct CodexHookHost;

impl CodexHookHost {
    /// Codex CLI hook 入口：stdin 读取、分派、observation 与 `{}` allow 形状。
    pub fn run_codex_hook(&self, event: &str, repo_root: &Path) -> FrameworkResult<Value> {
        let _registry_guard =
            router_rs::runtime_registry::HookRegistryRepoGuard::new(repo_root);
        let payload = match self.read_stdin_payload() {
            Ok(payload) => payload,
            Err(err) if Self::codex_fail_closed_on_stdin_error(event) => {
                let mut out = super::codex_hooks::codex_lifecycle_input_error(&format!(
                    "Codex lifecycle hook input JSON invalid: {err}"
                ));
                self.finalize_cli_output(&mut out);
                return Ok(out);
            }
            Err(err) => return Err(err),
        };
        let mut output = if Self::is_audit_only_cli_command(event) {
            Self::dispatch_audit_only_command(event, repo_root, &payload)?
        } else {
            self.dispatch(repo_root, event, &payload)
        };
        self.finalize_cli_output(&mut output);
        Ok(output)
    }

    fn is_audit_only_cli_command(event: &str) -> bool {
        matches!(
            event.trim().to_ascii_lowercase().as_str(),
            "contract-guard" | "lifecycle-context" | "review-subagent-gate"
        )
    }

    fn codex_fail_closed_on_stdin_error(event: &str) -> bool {
        matches!(
            super::codex_hooks::canonical_codex_audit_command(event),
            Ok("lifecycle-context")
        )
    }

    fn dispatch_audit_only_command(
        event: &str,
        repo_root: &Path,
        payload: &Value,
    ) -> FrameworkResult<Value> {
        let canonical = super::codex_hooks::canonical_codex_audit_command(event)?;
        let optional = match canonical {
            "contract-guard" => super::codex_hooks::run_codex_contract_guard(repo_root, payload)?,
            "lifecycle-context" => {
                super::codex_hooks::run_codex_lifecycle_context_hook(repo_root, payload)?
            }
            other => {
                return Err(FrameworkError::unsupported(format!("Unsupported Codex audit command: {event} ({other})")));
            }
        };
        Ok(optional.unwrap_or_else(|| json!({})))
    }

    fn normalize_codex_allow_output(output: &mut Value) {
        if output == &HookDecision::allow_value() {
            *output = json!({});
        }
    }

    fn lifecycle_guard_or(
        &self,
        payload: &Value,
        canonical_event: &str,
        handler: impl FnOnce(&Path, &Value) -> Option<Value>,
        repo_root: &Path,
    ) -> HookDecision {
        if let Some(block) =
            super::codex_hooks::codex_maybe_block_missing_stable_session_key(payload, canonical_event)
        {
            return HookDecision::Custom(block);
        }
        match handler(repo_root, payload) {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }
}

impl HostHook for CodexHookHost {
    fn host_id(&self) -> &str {
        "codex"
    }

    fn canonical_event(&self, raw: &str) -> FrameworkResult<&'static str> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "pretooluse" | "pre-tool-use" | "pre_tool_use" => Ok("pre-tool-use"),
            "posttooluse" | "post-tool-use" | "post_tool_use" => Ok("post-tool-use"),
            "stop" => Ok("stop"),
            "userpromptsubmit" | "user-prompt-submit" => Ok("user-prompt-submit"),
            "sessionstart" | "session-start" => Ok("session-start"),
            "subagentstart" | "subagent-start" => Ok("subagent-start"),
            "subagentstop" | "subagent-stop" => Ok("subagent-stop"),
            "contract-guard" => Ok("contract-guard"),
            "lifecycle-context" | "review-subagent-gate" => Ok("lifecycle-context"),
            other => Err(FrameworkError::validation(format!("unknown codex event: {other}"))),
        }
    }

    fn critical_events(&self) -> &[&str] {
        &["pre-tool-use", "stop"]
    }

    fn hook_observation_host(&self) -> Option<HookObservationHost> {
        Some(HookObservationHost::Codex)
    }

    fn finalize_cli_output(&self, output: &mut Value) {
        router_rs::goal_state::scrub_followup_fields_in_hook_output(output);
        Self::normalize_codex_allow_output(output);
        if let Some(host) = self.hook_observation_host() {
            router_rs::router_rs_observation::attach_router_rs_observation(output, host);
        }
    }

    fn handle_pre_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        match super::codex_hooks::run_codex_pre_tool_use(repo_root, payload) {
            Ok(Some(value)) => HookDecision::Custom(value),
            Ok(None) => HookDecision::Allow,
            Err(reason) => HookDecision::Block { reason: reason.to_string() },
        }
    }

    fn handle_post_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        self.lifecycle_guard_or(
            payload,
            "post-tool-use",
            super::codex_hooks::evaluate_codex_post_tool_use,
            repo_root,
        )
    }

    fn handle_stop(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        self.lifecycle_guard_or(
            payload,
            "stop",
            super::codex_hooks::evaluate_codex_stop,
            repo_root,
        )
    }

    fn handle_user_prompt_submit(&self, repo_root: &Path, payload: &Value) -> HookDecision {
        self.lifecycle_guard_or(
            payload,
            "user-prompt-submit",
            super::codex_hooks::handle_codex_userpromptsubmit,
            repo_root,
        )
    }

    fn handle_custom_event(
        &self,
        event: &str,
        repo_root: &Path,
        payload: &Value,
    ) -> HookDecision {
        let result = match event {
            "session-start" => super::codex_hooks::handle_codex_session_start(repo_root, payload),
            "subagent-start" => super::codex_hooks::handle_codex_subagent_start(repo_root, payload),
            "subagent-stop" => super::codex_hooks::handle_codex_subagent_stop(repo_root, payload),
            "contract-guard" => {
                return match super::codex_hooks::run_codex_contract_guard(repo_root, payload) {
                    Ok(Some(v)) => HookDecision::Custom(v),
                    Ok(None) => HookDecision::Allow,
                    Err(reason) => HookDecision::Block { reason: reason.to_string() },
                };
            }
            "lifecycle-context" => {
                return match super::codex_hooks::run_codex_lifecycle_context_hook(repo_root, payload)
                {
                    Ok(Some(v)) => HookDecision::Custom(v),
                    Ok(None) => HookDecision::Allow,
                    Err(reason) => HookDecision::Block { reason: reason.to_string() },
                };
            }
            _ => None,
        };
        match result {
            Some(v) => HookDecision::Custom(v),
            None => HookDecision::Allow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_hook_canonical_event_mapping() {
        let host = CodexHookHost;
        assert_eq!(host.canonical_event("PreToolUse").unwrap(), "pre-tool-use");
        assert_eq!(host.canonical_event("pre-tool-use").unwrap(), "pre-tool-use");
        assert_eq!(host.canonical_event("stop").unwrap(), "stop");
        assert_eq!(host.canonical_event("Stop").unwrap(), "stop");
        assert_eq!(host.canonical_event("contract-guard").unwrap(), "contract-guard");
        assert_eq!(
            host.canonical_event("lifecycle-context").unwrap(),
            "lifecycle-context"
        );
        assert_eq!(
            host.canonical_event("review-subagent-gate").unwrap(),
            "lifecycle-context"
        );
        assert!(host.canonical_event("unknown_event").is_err());
    }

    #[test]
    fn codex_hook_critical_events() {
        let host = CodexHookHost;
        assert_eq!(host.critical_events(), &["pre-tool-use", "stop"]);
    }

    #[test]
    fn codex_hook_host_id() {
        let host = CodexHookHost;
        assert_eq!(host.host_id(), "codex");
    }

    #[test]
    fn codex_normalize_allow_output_maps_suppress_to_empty_object() {
        let mut out = HookDecision::allow_value();
        CodexHookHost::normalize_codex_allow_output(&mut out);
        assert_eq!(out, json!({}));
    }

    #[test]
    fn codex_audit_only_commands_detected() {
        assert!(CodexHookHost::is_audit_only_cli_command("contract-guard"));
        assert!(CodexHookHost::is_audit_only_cli_command("lifecycle-context"));
        assert!(CodexHookHost::is_audit_only_cli_command("review-subagent-gate"));
        assert!(!CodexHookHost::is_audit_only_cli_command("Stop"));
        assert!(!CodexHookHost::is_audit_only_cli_command("PreToolUse"));
    }

    #[test]
    fn codex_stdin_fail_closed_matches_lifecycle_context_audit_path() {
        assert!(CodexHookHost::codex_fail_closed_on_stdin_error("Stop"));
        assert!(CodexHookHost::codex_fail_closed_on_stdin_error("SessionStart"));
        assert!(!CodexHookHost::codex_fail_closed_on_stdin_error("PreToolUse"));
        assert!(!CodexHookHost::codex_fail_closed_on_stdin_error("contract-guard"));
    }
}
