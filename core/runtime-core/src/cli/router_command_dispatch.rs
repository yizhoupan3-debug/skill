//! 子命令 `dispatch_*` 实现。

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::framework_runtime::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata,
};
use crate::browser_dispatch_hook;
use crate::claude_hooks::run_claude_hook_cli;
use super::args::*;
use crate::framework_runtime::json_io::{parse_json_input, print_json_value};
use crate::closeout_enforcement::{
    CloseoutEvidenceContext, closeout_enforcement_contract, evaluate_closeout_record_value,
    evaluate_closeout_record_value_with_context,
};
#[cfg(feature = "codegraph")]
use crate::codegraph_mcp::run_codegraph_mcp_stdio_loop;
use crate::codex_hooks::{
    InstallMode, build_codex_hook_projection, host_entrypoint_provider,
    install_codex_cli_hooks, resolve_codex_home, run_codex_audit_hook,
};
use crate::eval_route::{eval_route_contract, run_eval_route};
use crate::framework_profile::{
    build_codex_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use crate::framework_runtime::{
    FrameworkAliasBuildOptions, build_framework_alias_envelope,
    build_framework_contract_summary_envelope, build_framework_prompt_compression_envelope,
    build_framework_runtime_snapshot_envelope_with_level, build_framework_statusline,
    framework_hook_evidence_append, resolve_repo_root_arg, run_framework_doctor,
    write_framework_session_artifacts,
};
use crate::harness_contract::{harness_contract, lint_skill_contracts};
use crate::hook_policy::{HookPolicyEvaluateRequest, evaluate_hook_policy, hook_policy_contract};
use crate::host_entrypoint_sync::sync_host_entrypoints;
use crate::host_integration::run_host_integration_from_args;
use crate::review_gate_cli::run_review_gate;
use crate::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, runtime_backend_family_catalog_payload,
    runtime_backend_family_parity_payload, runtime_storage_operation,
};
use crate::step_ledger::handle_step_ledger_operation;
use crate::task_command;
use crate::task_state;
use crate::trace_runtime::{
    TraceCompactRequestPayload, TraceRecordEventRequestPayload, compact_trace_stream,
    record_trace_event,
};
use host_projection::hosts::codex_hooks::dispatcher::CodexHookDispatcher;
use host_projection::hosts::hook_dispatch::{HookEvent, HostHookDispatcher, HookOutput};
use host_projection::hooks::{
    HookObservationHost, attach_router_rs_observation, emit_hook_fired,
    hook_action_from_optional_output, read_stdin_limited,
};

use crate::runtime_storage::RuntimeStorageRequestPayload;

fn codex_hook_output_to_value(output: Option<HookOutput>) -> Value {
    match output {
        None | Some(HookOutput::None) => json!({}),
        Some(HookOutput::Raw(val)) => val,
        Some(HookOutput::AdditionalContext(ctx)) => json!({
            "hookSpecificOutput": { "additionalContext": ctx }
        }),
        Some(HookOutput::Deny { reason }) => json!({
            "decision": "block",
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
        }),
        Some(HookOutput::Block { reason }) => json!({
            "decision": "block",
            "followup_message": reason,
        }),
        Some(HookOutput::Advisory { message }) => json!({
            "followup_message": message,
        }),
        Some(HookOutput::Warn { message }) => json!({
            "warning": message,
        }),
    }
}

/// Resolve host entrypoint provider by `--host-id`.
/// Registry-driven: iterates `host_provider_registry` to find a concrete provider.
/// Falls back to the first registered host when no host_id is specified.
fn resolve_host_entrypoint_provider(
    repo_root: &std::path::Path,
    host_id: Option<&str>,
) -> Result<crate::host_entrypoint_sync::HostEntrypointPayloadProvider, String> {
    let resolved = host_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::hosts::default_host_id());
    // Currently only codex has a concrete entrypoint provider.
    // Future hosts add their own providers here via the registry pattern.
    match crate::hosts::host_provider_for_routing_spelling(resolved) {
        Some(provider) if provider.host_id() == "codex" || provider.install_tool() == "codex" => {
            host_entrypoint_provider(repo_root)
        }
        Some(provider) => Err(format!(
            "host '{}' does not yet have an entrypoint provider; only codex is currently supported",
            provider.host_id()
        )),
        None => Err(format!(
            "unknown host-id '{resolved}' for sync-entrypoints; not found in host_provider_registry"
        )),
    }
}

pub fn dispatch_framework_command(command: FrameworkCommand) -> Result<(), String> {
    match command {
        FrameworkCommand::Snapshot(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let detail_level = command.detail_level.as_deref().unwrap_or("summary");
            print_json_value(&build_framework_runtime_snapshot_envelope_with_level(
                &repo_root,
                command.artifact_source_dir.as_deref(),
                command.task_id.as_deref(),
                detail_level,
            )?)
        }
        FrameworkCommand::Doctor(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let result = run_framework_doctor(&repo_root)?;
            if result.warn_count > 0 {
                std::process::exit(1);
            }
            Ok(())
        }
        FrameworkCommand::SyncEntrypoints(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let provider =
                resolve_host_entrypoint_provider(&repo_root, command.host_id.as_deref())?;
            print_json_value(&sync_host_entrypoints(&repo_root, true, provider)?)
        }
        FrameworkCommand::PromptCompression(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework prompt compression")?;
            {
                let ctx_size = payload
                    .get("context_window_size")
                    .and_then(serde_json::Value::as_u64)
                    .map(|v| v as usize);
                print_json_value(&build_framework_prompt_compression_envelope(
                    payload, ctx_size,
                )?)
            }
        }
        FrameworkCommand::Statusline(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            println!("{}", build_framework_statusline(&repo_root)?);
            Ok(())
        }
        FrameworkCommand::SessionArtifactWrite(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework session artifact write")?;
            print_json_value(&write_framework_session_artifacts(payload)?)
        }
        FrameworkCommand::HookEvidenceAppend(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework hook evidence append")?;
            print_json_value(&framework_hook_evidence_append(payload)?)
        }
        FrameworkCommand::Alias(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            print_json_value(&build_framework_alias_envelope(
                &repo_root,
                &command.alias,
                FrameworkAliasBuildOptions {
                    max_lines: command.max_lines,
                    compact: command.compact,
                    host_id: command.host_id.as_deref(),
                },
            )?)
        }
        FrameworkCommand::TaskStateResolve(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let view = task_state::resolve_task_view(&repo_root, command.task_id.as_deref());
            print_json_value(&view)
        }
        FrameworkCommand::TaskLedgerDispatch(command) => {
            let envelope =
                parse_json_input::<Value>(&command.input_json, "framework task ledger dispatch")?;
            print_json_value(&task_command::dispatch_task_ledger_command_envelope(
                envelope,
            )?)
        }
        FrameworkCommand::TaskStateAggregateSync(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let task_id = command
                .task_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    "framework task-state-aggregate-sync requires --task-id (pointer fallback removed)"
                        .to_string()
                })?;
            crate::task_state_aggregate::sync_task_state_aggregate(&repo_root, &task_id)?;
            print_json_value(&json!({
                "ok": true,
                "task_id": task_id,
                "task_state_path": crate::task_state_aggregate::task_state_aggregate_path(&repo_root, &task_id).display().to_string(),
            }))
        }
        FrameworkCommand::StepLedger(command) => {
            let payload = parse_json_input::<Value>(&command.input_json, "framework step ledger")?;
            print_json_value(&handle_step_ledger_operation(payload)?)
        }
        FrameworkCommand::Maint { command } => crate::framework_maint::dispatch(command),
        FrameworkCommand::Skills { command } => dispatch_framework_skills(command),
        FrameworkCommand::HostIntegration(command) => {
            let payload = run_host_integration_from_args(&command.args)?;
            print_json_value(&payload)
        }
        FrameworkCommand::NlRouteSignalRegistryContract => {
            println!("{}", crate::route::nl_route_signal_registry_names_json());
            Ok(())
        }
        FrameworkCommand::Contracts(command) => {
            if command.summary {
                let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
                print_json_value(&build_framework_contract_summary_envelope(&repo_root)?)
            } else {
                print_json_value(&build_control_plane_contract_descriptors())
            }
        }
        FrameworkCommand::Scaffold(command) => {
            let repo_root = resolve_repo_root_arg(command.framework_root.as_deref())?;
            let result = scaffold_host_integration(&repo_root, &command.host_id, command.dry_run)?;
            print_json_value(&result)
        }
    }
}

pub fn dispatch_framework_skills(command: SkillsSubcommand) -> Result<(), String> {
    use crate::framework_skills::{SkillsCommand, refresh_skills, validate_skills};
    match command {
        SkillsSubcommand::Validate(args) => {
            let repo_root = resolve_repo_root_arg(args.framework_root.as_deref())?;
            validate_skills(&repo_root)
        }
        SkillsSubcommand::Refresh {
            repo_root,
            write,
            write_companions,
        } => {
            let repo_root = resolve_repo_root_arg(repo_root.as_deref())?;
            refresh_skills(&SkillsCommand {
                repo_root,
                write,
                write_companions,
            })
        }
    }
}

/// §5.4: 生成新宿主接入所需的全部脚手架文件。
fn scaffold_host_integration(
    repo_root: &std::path::Path,
    host_id: &str,
    dry_run: bool,
) -> Result<serde_json::Value, String> {
    use std::fs;

    // Validate host_id
    if host_id.is_empty()
        || !host_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "Invalid host_id: {host_id:?}. Use alphanumeric, dash, or underscore."
        ));
    }

    // Check host doesn't already exist
    let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    let registry: serde_json::Value = fs::read_to_string(&registry_path)
        .and_then(|s| {
            serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
        .map_err(|e| format!("Read RUNTIME_REGISTRY.json: {e}"))?;

    if let Some(supported) = registry
        .get("host_targets")
        .and_then(|v| v.get("supported"))
        .and_then(|v| v.as_array())
        && supported.iter().any(|v| v.as_str() == Some(host_id)) {
            return Err(format!(
                "Host {host_id:?} already exists in RUNTIME_REGISTRY.json"
            ));
        }

    let host_id_camel = host_id.replace('-', "_");
    let host_name = host_id
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let host_name_camel = host_id
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let files = vec![
        // 1. Host provider
        (
            format!("core/host-projection/src/hosts/{host_id_camel}_provider.rs"),
            format!(
                r#"use crate::hosts::host_provider::HostProvider;

pub struct {hc}HostProvider;

impl HostProvider for {hc}HostProvider {{
    fn host_id(&self) -> &'static str {{
        "{hid}"
    }}

    fn profile_name(&self) -> &'static str {{
        "{hcc}_profile"
    }}

    fn session_supervisor_driver(&self) -> &'static str {{
        "unsupported"
    }}

    fn harness_capabilities(&self) -> &'static [&'static str] {{
        &["hot_runtime_routing", "l2_continuity_contract"]
    }}

    fn context_file(&self) -> &'static str {{
        "AGENTS.md"
    }}
}}
"#,
                hc = host_name_camel,
                hid = host_id,
                hcc = host_id_camel,
            ),
        ),
        // 2. Documentation
        (
            format!("docs/hosts/{hid}.md", hid = host_id),
            format!(
                r#"---
status: scaffold
host_id: {hid}
---

# {hn} 宿主接入

> Generated by `router-rs scaffold --host-id {hid}`

## 接入状态

🟡 脚手架已生成，待实现。

## 需要完成的工作

1. 实现 `{hcc}_provider.rs` 中的 HostProvider trait 方法
2. 添加 Cargo.toml feature: `host-{hid}`
3. 在 RUNTIME_REGISTRY.json 中注册 host_targets
4. 在 AGENTS.md 中补充宿主特定上下文
5. 配置 MCP 工具注册
6. 测试 install/remove/status 流程
"#,
                hn = host_name,
                hid = host_id,
                hcc = host_id_camel,
            ),
        ),
    ];

    let mut generated = Vec::new();

    for (path, content) in &files {
        let full_path = repo_root.join(path);
        if dry_run {
            generated.push(serde_json::json!({
                "path": path,
                "action": "create",
                "size": content.len(),
            }));
        } else {
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Create dir {}: {e}", parent.display()))?;
            }
            fs::write(&full_path, content)
                .map_err(|e| format!("Write {}: {e}", full_path.display()))?;
            generated.push(serde_json::json!({
                "path": path,
                "action": "created",
                "size": content.len(),
            }));
        }
    }

    // Generate Cargo.toml feature suggestion
    let cargo_feature = format!("host-{host_id} = []");

    // Generate RUNTIME_REGISTRY.json entry suggestion
    let registry_entry = serde_json::json!({
        "host_id": host_id,
        "session_supervisor_driver": "unsupported",
        "harness_capabilities": ["hot_runtime_routing", "l2_continuity_contract"],
        "capabilities": [],
        "managed_mcp_server_ids": framework_kernel::runtime_registry::DEFAULT_MANAGED_MCP_SERVER_IDS,
    });

    Ok(serde_json::json!({
        "ok": true,
        "host_id": host_id,
        "dry_run": dry_run,
        "generated_files": generated,
        "manual_steps": {
            "cargo_feature": cargo_feature,
            "runtime_registry_entry": registry_entry,
            "instructions": [
                format!("Add `host-{host_id} = []` to runtime-core/Cargo.toml [features]"),
                format!("Add `host-{host_id} = [\"runtime-core/host-{host_id}\"]` to router-rs/Cargo.toml [features]"),
                "Add host entry to configs/framework/RUNTIME_REGISTRY.json host_targets.supported",
                format!("Implement HostProvider trait in {host_id_camel}_provider.rs"),
                "Add host context to AGENTS.md",
                "Register host in host-projection/src/hosts/mod.rs",
                "Run: cargo test --workspace",
            ],
        },
    }))
}

/// Generic hook dispatch routed by host_id via dispatch table.
#[tracing::instrument(level = "info", skip_all, fields(host_id))]
pub fn dispatch_hook_command(host_id: &str, command: GenericHookCommand) -> Result<(), String> {
    type HookDispatchFn = fn(&GenericHookCommand) -> Result<(), String>;

    fn dispatch_cursor(cmd: &GenericHookCommand) -> Result<(), String> {
        run_review_gate(&cmd.event, cmd.repo_root.as_deref())
    }
    fn dispatch_claude(cmd: &GenericHookCommand) -> Result<(), String> {
        run_claude_hook_cli(&cmd.event, cmd.repo_root.as_deref())
    }
    fn dispatch_opencode(cmd: &GenericHookCommand) -> Result<(), String> {
        run_opencode_hook_cli(&cmd.event, cmd.repo_root.as_deref())
    }
    fn dispatch_codex(cmd: &GenericHookCommand) -> Result<(), String> {
        dispatch_codex_hook(cmd.event.as_str(), cmd.repo_root.as_deref())
    }

    // Registry-driven dispatch table: add new hosts here.
    const DISPATCH_TABLE: &[(&str, HookDispatchFn)] = &[
        ("cursor", dispatch_cursor),
        ("claude", dispatch_claude),
        ("opencode", dispatch_opencode),
        ("codex", dispatch_codex),
    ];

    DISPATCH_TABLE
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, f)| f(&command))
        .unwrap_or_else(|| Err(format!("hook dispatch not implemented for host `{host_id}`")))
}

/// Hook dispatch for Codex via `CodexHookDispatcher` (unified trait dispatch).
///
/// Contract-guard is not a lifecycle event — keep existing `run_codex_audit_hook` path.
/// All lifecycle events (pretooluse, userpromptsubmit, posttooluse, stop, sessionstart,
/// subagentstart, subagentstop) are routed via `CodexHookDispatcher.dispatch()`.
fn dispatch_codex_hook(event: &str, repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root)?;

    // Contract-guard is not a lifecycle event; keep existing path.
    if event.trim().eq_ignore_ascii_case("contract-guard") {
        let payload = run_codex_audit_hook(event, &repo_root)?;
        print_json_value(&payload.unwrap_or_else(|| json!({})))?;
        std::process::exit(0);
    }

    // Bootstrap for lifecycle events
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    crate::hook_timing::mark_hook_start();
    let _registry_guard = crate::runtime_registry::HookRegistryRepoGuard::new(&repo_root);

    // Read stdin payload (shared 4 MiB limited reader)
    let mut stdin = std::io::stdin().lock();
    let input = read_stdin_limited(&mut stdin).unwrap_or_default();
    let payload: Value = if input.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(input.trim())
            .map_err(|err| format!("stdin_json_invalid: {err}"))?
    };

    // Dispatch via CodexHookDispatcher (unified trait dispatch replaces hand-written match)
    let hook_event = HookEvent {
        repo_root: &repo_root,
        event_name: event,
        payload: &payload,
    };
    let output = CodexHookDispatcher.dispatch(&hook_event);

    // Convert HookOutput → JSON value
    let mut json_output = codex_hook_output_to_value(output);

    // Attach router-rs observation (matches attach_codex_hook_observation in handlers.rs)
    attach_router_rs_observation(&mut json_output, HookObservationHost::Codex);

    // Emit telemetry
    let telemetry_event = event.to_ascii_lowercase();
    emit_hook_fired(&telemetry_event, hook_action_from_optional_output(Some(&json_output)));
    crate::hook_timing::emit_hook_timing_line(&telemetry_event);

    // Print JSON output
    print_json_value(&json_output)?;
    std::process::exit(0);
}

/// Generic agent dispatch routed by host_id via dispatch table.
#[tracing::instrument(level = "info", skip_all, fields(host_id))]
pub fn dispatch_agent_command(host_id: &str, command: GenericAgentCommand) -> Result<(), String> {
    type AgentDispatchFn = fn(&GenericAgentCommand) -> Result<(), String>;

    fn dispatch_claude_agent(cmd: &GenericAgentCommand) -> Result<(), String> {
        let root = resolve_repo_root_arg(cmd.repo_root.as_deref())?;
        crate::hosts::claude_agent::run_claude_agent_mcp_loop(Some(&root))
    }
    fn dispatch_opencode_agent(cmd: &GenericAgentCommand) -> Result<(), String> {
        let root = resolve_repo_root_arg(cmd.repo_root.as_deref())?;
        crate::hosts::opencode_agent::run_opencode_mcp_loop(Some(&root))
    }

    // Registry-driven dispatch table: add new hosts here.
    const DISPATCH_TABLE: &[(&str, AgentDispatchFn)] = &[
        ("claude", dispatch_claude_agent),
        ("opencode", dispatch_opencode_agent),
    ];

    DISPATCH_TABLE
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, f)| f(&command))
        .unwrap_or_else(|| {
            Err(format!(
                "agent dispatch not implemented for host `{host_id}`"
            ))
        })
}

/// Unified host command dispatcher (registry-driven: Codex / Hook / Agent).
#[tracing::instrument(level = "info", skip_all)]
pub fn dispatch_host_command(command: HostCommand) -> Result<(), String> {
    match command {
        HostCommand::Codex { command } => dispatch_codex_command(command),
        HostCommand::Hook { host_id, command } => dispatch_hook_command(&host_id, command),
        HostCommand::Agent { host_id, command } => dispatch_agent_command(&host_id, command),
    }
}

/// Unified diagnostic command dispatcher (merged: profile, browser)
pub fn dispatch_diagnose_command(command: DiagnoseCommand) -> Result<(), String> {
    match command {
        DiagnoseCommand::Profile { command } => dispatch_profile_command(command),
        DiagnoseCommand::Browser { command } => dispatch_browser_command(command),
    }
}

pub fn dispatch_codex_command(command: CodexSubcommand) -> Result<(), String> {
    match command {
        CodexSubcommand::HookProjection => print_json_value(&build_codex_hook_projection()),
        CodexSubcommand::Check(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let provider = host_entrypoint_provider(&repo_root)?;
            print_json_value(&sync_host_entrypoints(&repo_root, false, provider)?)
        }
        CodexSubcommand::Hook(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let event_name = command
                .event
                .or(command.name)
                .ok_or("hook event required")?;
            let payload = run_codex_audit_hook(&event_name, &repo_root)?;
            print_json_value(&payload.unwrap_or_else(|| json!({})))?;
            std::process::exit(0);
        }
        CodexSubcommand::HostIntegration(command) => {
            let payload = run_host_integration_from_args(&command.args)?;
            print_json_value(&payload)
        }
        CodexSubcommand::InstallHooks(command) => {
            let resolved_codex_home = resolve_codex_home(command.codex_home.as_deref())?;
            let mode = if command.check {
                InstallMode::Check
            } else {
                InstallMode::Apply
            };
            let repo_root = command
                .repo_root
                .unwrap_or_else(|| std::env::current_dir().unwrap_or(PathBuf::from(".")));
            let payload = install_codex_cli_hooks(&resolved_codex_home, &repo_root, mode)?;
            print_json_value(&payload)
        }
    }
}
/// Run opencode hook event dispatch via stdin JSON payload.
fn run_opencode_hook_cli(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    crate::hook_timing::mark_hook_start();
    let _result = (|| -> Result<(), String> {
        // Read stdin JSON payload (same pattern as cursor/claude/codex)
        let payload = crate::cursor_hooks::read_cursor_hook_stdin_json()
            .unwrap_or_else(|_| serde_json::json!({}));
        let repo_root = cli_repo_root
            .map(|p| p.to_path_buf())
            .or_else(|| {
                payload
                    .get("cwd")
                    .and_then(serde_json::Value::as_str)
                    .map(std::path::PathBuf::from)
            })
            .or_else(|| std::env::current_dir().ok())
            .ok_or("repo_root required")?;
        let _registry_guard = crate::runtime_registry::HookRegistryRepoGuard::new(&repo_root);

        // Dispatch via HostHookDispatcher trait
        use host_projection::hosts::hook_dispatch::{HookEvent, HostHookDispatcher};
        use host_projection::hosts::opencode_hooks::OpencodeHookDispatcher;

        let hook_event = HookEvent {
            repo_root: &repo_root,
            event_name: event,
            payload: &payload,
        };
        let dispatcher = OpencodeHookDispatcher;
        let output = dispatcher.dispatch(&hook_event);

        // Serialize output
        let json_output = match output {
            Some(hook_output) => {
                use host_projection::hosts::hook_dispatch::HookOutput;
                match hook_output {
                    HookOutput::AdditionalContext(ctx) => serde_json::json!({
                        "hookSpecificOutput": { "additionalContext": ctx }
                    }),
                    HookOutput::Deny { reason } => serde_json::json!({
                        "decision": "deny",
                        "reason": reason,
                    }),
                    HookOutput::Warn { message } => serde_json::json!({
                        "decision": "allow",
                        "warning": message,
                    }),
                    HookOutput::Block { reason } => serde_json::json!({
                        "decision": "block",
                        "reason": reason,
                    }),
                    HookOutput::Advisory { message } => serde_json::json!({
                        "decision": "allow",
                        "advisory": message,
                    }),
                    HookOutput::Raw(value) => value,
                    HookOutput::None => serde_json::json!({}),
                }
            }
            None => serde_json::json!({}),
        };

        let mut stdout = std::io::stdout();
        let serialized = serde_json::to_string(&json_output).map_err(|e| e.to_string())?;
        stdout
            .write_all(format!("{serialized}\n").as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    crate::hook_timing::emit_hook_timing_line(event);
    // Force immediate exit — skip background thread cleanup (file watcher, telemetry).
    // Hook processes are short-lived fire-and-forget; background threads are not needed.
    std::process::exit(0);
}

pub fn dispatch_trace_command(command: TraceCommand) -> Result<(), String> {
    match command {
        TraceCommand::RecordEvent(command) => {
            let payload = parse_json_input::<TraceRecordEventRequestPayload>(
                &command.input_json,
                "trace record event",
            )?;
            print_json_value(&record_trace_event(payload)?)
        }
        TraceCommand::StreamReplay(command) => {
            let payload = parse_json_input::<TraceStreamReplayRequestPayload>(
                &command.input_json,
                "trace stream replay",
            )?;
            print_json_value(&replay_trace_stream(payload)?)
        }
        TraceCommand::StreamInspect(command) => {
            let payload = parse_json_input::<TraceStreamInspectRequestPayload>(
                &command.input_json,
                "trace stream inspect",
            )?;
            print_json_value(&inspect_trace_stream(payload)?)
        }
        TraceCommand::Compact(command) => {
            let payload = parse_json_input::<TraceCompactRequestPayload>(
                &command.input_json,
                "trace compact",
            )?;
            print_json_value(&compact_trace_stream(payload)?)
        }
        TraceCommand::WriteCompactionDelta(command) => {
            let payload = parse_json_input::<TraceCompactionDeltaWriteRequestPayload>(
                &command.input_json,
                "trace compaction delta write",
            )?;
            print_json_value(&write_trace_compaction_delta(payload)?)
        }
        TraceCommand::WriteMetadata(command) => {
            let payload = parse_json_input::<TraceMetadataWriteRequestPayload>(
                &command.input_json,
                "trace metadata write",
            )?;
            print_json_value(&write_trace_metadata(payload)?)
        }
    }
}

pub fn dispatch_storage_command(command: StorageCommand) -> Result<(), String> {
    match command {
        StorageCommand::Runtime(command) => {
            let payload = parse_json_input::<RuntimeStorageRequestPayload>(
                &command.input_json,
                "runtime storage",
            )?;
            print_json_value(&runtime_storage_operation(payload)?)
        }
        StorageCommand::CheckpointControlPlane(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "runtime checkpoint control plane")?;
            print_json_value(&build_checkpoint_control_plane_compiler_payload(payload)?)
        }
        StorageCommand::BackendCatalog => {
            print_json_value(&runtime_backend_family_catalog_payload())
        }
        StorageCommand::BackendParity(command) => {
            print_json_value(&runtime_backend_family_parity_payload(
                command.store.as_deref(),
                command.checkpointer.as_deref(),
                command.trace.as_deref(),
                command.state.as_deref(),
            )?)
        }
    }
}

#[cfg(feature = "codegraph")]
pub fn dispatch_codegraph_command(command: CodegraphSubcommand) -> Result<(), String> {
    match command {
        CodegraphSubcommand::McpStdio(command) => {
            run_codegraph_mcp_stdio_loop(command.repo_root.as_deref())
        }
    }
}

pub fn dispatch_browser_command(command: BrowserSubcommand) -> Result<(), String> {
    browser_dispatch_hook::dispatch_browser_command(command)
}

pub fn dispatch_profile_command(command: ProfileSubcommand) -> Result<(), String> {
    match command {
        ProfileSubcommand::Emit(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_profile_bundle(profile)?)
        }
        ProfileSubcommand::Artifacts(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_codex_artifact_bundle(profile, command.full)?)
        }
    }
}

pub fn dispatch_migrate_command(command: MigrateCommand) -> Result<(), String> {
    match command {
        MigrateCommand::CurrentArtifactClutter(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let payload = run_host_integration_from_args(&[
                "migrate-current-artifact-clutter".to_string(),
                "--repo-root".to_string(),
                repo_root.display().to_string(),
                "--active-task-id".to_string(),
                command.active_task_id,
            ])?;
            print_json_value(&payload)
        }
    }
}

pub fn dispatch_hook_policy_command(command: HookPolicyCommand) -> Result<(), String> {
    match command {
        HookPolicyCommand::Evaluate(command) => {
            let payload = parse_json_input::<HookPolicyEvaluateRequest>(
                &command.input_json,
                "hook policy evaluate",
            )?;
            print_json_value(&evaluate_hook_policy(payload)?)
        }
        HookPolicyCommand::Contract => print_json_value(&hook_policy_contract()),
    }
}

pub fn dispatch_closeout_command(command: CloseoutCommand) -> Result<(), String> {
    match command {
        CloseoutCommand::Evaluate(args) => {
            let raw =
                match (args.input_json.as_deref(), args.record_path.as_deref()) {
                    (Some(_), Some(_)) => return Err(
                        "closeout evaluate: --input-json and --record-path are mutually exclusive"
                            .to_string(),
                    ),
                    (Some(text), None) => text.to_string(),
                    (None, Some(path)) => fs::read_to_string(path).map_err(|err| {
                        format!(
                            "closeout evaluate: failed to read record file {}: {err}",
                            path.display()
                        )
                    })?,
                    (None, None) => {
                        return Err(
                            "closeout evaluate: provide --input-json or --record-path".to_string()
                        );
                    }
                };
            let record_value: Value = serde_json::from_str(&raw)
                .map_err(|err| format!("closeout evaluate: invalid JSON: {err}"))?;
            let response = match (
                args.repo_root.as_deref(),
                args.task_id.as_deref(),
                args.record_path.as_deref(),
            ) {
                (Some(repo_root), Some(task_id), Some(record_path)) => {
                    crate::framework_runtime::evaluate_closeout_record_file_for_task(
                        repo_root,
                        task_id,
                        record_path,
                    )?
                }
                (Some(repo_root), Some(task_id), None) => {
                    let (rows_non_empty, has_success) =
                        crate::goal_drive::task_evidence_artifacts_summary_for_task(
                            repo_root, task_id,
                        );
                    let goal_state =
                        crate::goal_drive::read_goal_state(repo_root, Some(task_id))
                            .ok()
                            .flatten();
                    let goal_prediction = goal_state
                        .as_ref()
                        .and_then(core_state::goal_prediction::read_goal_prediction);
                    let ctx = CloseoutEvidenceContext {
                        task_id: Some(task_id.trim().to_string()),
                        evidence_rows_non_empty: rows_non_empty,
                        has_successful_verification: has_success,
                        goal_prediction,
                    };
                    evaluate_closeout_record_value_with_context(record_value, &ctx)?
                }
                _ => evaluate_closeout_record_value(record_value)?,
            };
            print_json_value(&response)
        }
        CloseoutCommand::Contract => print_json_value(&closeout_enforcement_contract()),
    }
}

pub fn dispatch_eval_command(command: EvalCommand) -> Result<(), String> {
    match command {
        EvalCommand::Route(args) => {
            let report = run_eval_route(
                &args.cases,
                args.runtime.as_deref(),
                args.manifest.as_deref(),
            )?;
            print_json_value(&report)
        }
        EvalCommand::RouteContract => print_json_value(&eval_route_contract()),
        EvalCommand::HarnessContract => print_json_value(&harness_contract()),
        EvalCommand::SkillContractLint(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "eval skill contract lint")?;
            print_json_value(&lint_skill_contracts(payload)?)
        }
    }
}

pub fn dispatch_schema_drift_command(command: SchemaDriftCommand) -> Result<(), String> {
    use crate::schema_drift::{
        check_against_baseline, resolve_task_id_for_schema_drift, schema_drift_contract,
        write_baseline,
    };
    match command {
        SchemaDriftCommand::Contract => print_json_value(&schema_drift_contract()),
        SchemaDriftCommand::Baseline(args) => {
            let repo_root = resolve_repo_root_arg(args.repo_root.as_deref())?;
            let task_id = resolve_task_id_for_schema_drift(&repo_root, args.task_id.as_deref())?;
            let (baseline, path) = write_baseline(&repo_root, &task_id)?;
            print_json_value(&serde_json::json!({
                "schema_version": "schema-drift-baseline-write-response-v1",
                "task_id": task_id,
                "baseline_path": path.display().to_string(),
                "baseline": baseline,
            }))
        }
        SchemaDriftCommand::Check(args) => {
            let repo_root = resolve_repo_root_arg(args.repo_root.as_deref())?;
            let task_id = resolve_task_id_for_schema_drift(&repo_root, args.task_id.as_deref())?;
            let response = check_against_baseline(&repo_root, &task_id);
            print_json_value(&response)?;
            if !response.ok {
                return Err(format!(
                    "schema-drift check failed for task {} ({} drift items); re-run `router-rs schema-drift baseline` after intentional changes",
                    task_id,
                    response.drift.len()
                ));
            }
            Ok(())
        }
    }
}

pub fn dispatch_loop_command(command: LoopCommand) -> Result<(), String> {
    match command {
        LoopCommand::Run(args) => {
            let repo_root = std::env::current_dir()
                .map_err(|e| format!("get current dir: {e}"))?;
            let registry_path = repo_root.join("configs/framework/LOOP_REGISTRY.json");
            let raw = fs::read_to_string(&registry_path)
                .map_err(|e| format!("read LOOP_REGISTRY.json: {e}"))?;
            let registry: loop_engine::LoopRegistryRoot = serde_json::from_str(&raw)
                .map_err(|e| format!("parse LOOP_REGISTRY.json: {e}"))?;
            let entry = registry.loops.iter()
                .find(|e| e.loop_id == args.loop_id)
                .ok_or_else(|| format!("loop '{}' not found in LOOP_REGISTRY.json", args.loop_id))?;
            let timeout = std::time::Duration::from_secs(args.timeout);
            let ctx = loop_engine::runner::RunContext {
                repo_root: &repo_root,
                entry,
                dry_run: args.dry_run,
                timeout: Some(timeout),
            };
            let aggregate = loop_engine::runner::run_loop(&ctx)
                .map_err(|e| format!("loop run failed: {e}"))?;
            print_json_value(&serde_json::json!({
                "ok": true,
                "loop_id": args.loop_id,
                "overall_status": aggregate.overall_status,
                "actions": aggregate.actions.len(),
                "partial": aggregate.partial,
            }))
        }
        LoopCommand::Status(args) => {
            let repo_root = std::env::current_dir()
                .map_err(|e| format!("get current dir: {e}"))?;
            match loop_engine::runner::run_loop_status(&repo_root, &args.loop_id)
                .map_err(|e| format!("loop status failed: {e}"))? {
                Some(state) => print_json_value(&serde_json::json!({
                    "ok": true,
                    "loop_id": args.loop_id,
                    "phase": state.phase,
                    "profile": state.profile,
                    "last_heartbeat": state.last_heartbeat,
                    "circuit_breaker": state.circuit_breaker,
                    "history_count": state.history.len(),
                })),
                None => print_json_value(&serde_json::json!({
                    "ok": true,
                    "loop_id": args.loop_id,
                    "phase": "no_state",
                    "message": "No LOOP_RUN_STATE.json found for this loop",
                })),
            }
        }
        LoopCommand::Kill(args) => {
            let repo_root = std::env::current_dir()
                .map_err(|e| format!("get current dir: {e}"))?;
            if args.all {
                loop_engine::runner::run_loop_kill_all(&repo_root)
                    .map_err(|e| format!("loop kill --all failed: {e}"))?;
                print_json_value(&serde_json::json!({
                    "ok": true,
                    "message": "All kill signals sent",
                }))
            } else {
                loop_engine::runner::run_loop_kill(&repo_root, &args.loop_id)
                    .map_err(|e| format!("loop kill failed: {e}"))?;
                print_json_value(&serde_json::json!({
                    "ok": true,
                    "loop_id": args.loop_id,
                    "message": "Kill signal sent",
                }))
            }
        }
    }
}
