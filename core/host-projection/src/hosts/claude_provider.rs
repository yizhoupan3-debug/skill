//! Claude host: `HostProvider` skeleton (stdio hook metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};
use serde_json::Value;

crate::impl_host_provider! {
    ClaudeHostProvider for "claude";
    capabilities { mcp_key: ""; transport: "anthropic-claude"; config: ".claude/settings.json"; }
}

impl HostLifecycle for ClaudeHostProvider {
    fn profile_id(&self) -> &'static str { "claude_profile" }
    fn session_supervisor_driver(&self) -> &'static str { "mcp_bridge" }
    fn context_file(&self) -> &'static str { "AGENTS_CLAUDE.md" }
    fn driver_binary(&self) -> &'static str { "claude" }
    fn driver_supports_resume(&self) -> bool { true }

    fn build_driver_args(
        &self,
        _cwd: &str,
        prompt: Option<&str>,
        resume_target: Option<&str>,
        _resume_mode: &str,
        resume_only: bool,
    ) -> Option<(Vec<String>, String)> {
        let mut args = vec!["--print".to_string()];
        if resume_only {
            if let Some(target) = resume_target {
                args.push("--resume".to_string());
                args.push(target.to_string());
            }
        } else if let Some(p) = prompt {
            args.push("-p".to_string());
            args.push(p.to_string());
        }
        let shell_cmd = format!("claude {}", args.join(" "));
        Some((args, shell_cmd))
    }
}

impl HostTelemetry for ClaudeHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str { "anthropic-claude" }
    fn observation_host_id(&self) -> Option<&'static str> { Some("claude") }

    fn extract_observation_surfaces(&self, output: &Value) -> (Option<String>, Option<String>) {
        let followup = output
            .get("stopReason")
            .or_else(|| output.get("systemMessage"))
            .or_else(|| output.get("followup_message"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .or_else(|| {
                output
                    .get("message")
                    .or_else(|| output.get("reason"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
            });
        let additional = output
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        (followup, additional)
    }
}
