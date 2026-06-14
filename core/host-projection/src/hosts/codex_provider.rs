//! Codex host: `HostProvider` skeleton (native hook + TOML projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct CodexHostProvider;

impl HostLifecycle for CodexHostProvider {
    fn profile_id(&self) -> &'static str {
        "codex_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "codex_driver"
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_CODEX.md"
    }

    fn hooks_manifest_path(&self) -> Option<&'static str> {
        Some(crate::hosts::codex_hooks::CODEX_HOOKS_PATH)
    }

    fn registered_hook_events(&self) -> &'static [&'static str] {
        &crate::hosts::codex_hooks::INSTALL_LIFECYCLE_EVENTS
    }

    fn driver_binary(&self) -> &'static str {
        "codex"
    }

    fn driver_supports_resume(&self) -> bool {
        true
    }

    fn build_driver_args(
        &self,
        cwd: &str,
        prompt: Option<&str>,
        resume_target: Option<&str>,
        resume_mode: &str,
        resume_only: bool,
    ) -> Option<(Vec<String>, String)> {
        let mut args = vec!["-C".to_string(), cwd.to_string()];
        if resume_only {
            args.push("resume".to_string());
            if let Some(target) = resume_target {
                if target == "last" || resume_mode == "last" {
                    args.push("--last".to_string());
                } else {
                    args.push(target.to_string());
                }
            } else {
                args.push("--last".to_string());
            }
        } else if let Some(p) = prompt {
            args.push(p.to_string());
        }
        let shell_cmd = format!("codex {}", args.join(" "));
        Some((args, shell_cmd))
    }
}

impl HostToolExecutor for CodexHostProvider {}

impl HostTelemetry for CodexHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str {
        "native-codex"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("codex")
    }
}

impl HostProvider for CodexHostProvider {
    fn host_id(&self) -> &'static str {
        "codex"
    }

    fn install_tool(&self) -> &'static str {
        "codex"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["codex-cli", "codex-app"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            mcp_config_key: "mcp_servers",
            transport_type: "native-codex",
            config_path: ".codex/config.toml",
            batch_execution: true,
            cron_execution: true,
            ci_runner: true,
            non_interactive_entrypoint: true,
            external_session_supervisor: true,
            rate_limit_auto_resume: true,
            ..Default::default()
        }
    }
}
