//! 子命令 `dispatch_*` 实现。

use serde_json::{Value, json};
use serde::Serialize;
use std::fs;
use std::path::Path;

use fr_exec::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata,
};
use host_projection::hooks;
use host_projection::host_entrypoint_sync::sync_host_entrypoints;
use super::args::*;
use fr_utils::json_io::{parse_json_input, print_json_value as raw_print_json_value};
use runtime_core::closeout_enforcement::{
    CloseoutEvidenceContext, closeout_enforcement_contract, evaluate_closeout_record_value,
    evaluate_closeout_record_value_with_context,
};
#[cfg(feature = "codegraph")]
use runtime_core::codegraph_mcp::run_codegraph_mcp_stdio_loop;
use runtime_core::eval_route::{eval_route_contract, run_eval_route};
use runtime_core::framework_profile::{
    build_profile_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use fr_utils::types::FrameworkAliasBuildOptions;
use framework_extra::alias::build_framework_alias_envelope;
use framework_extra::contract_summary::build_framework_contract_summary_envelope;
use framework_extra::prompt_compression::build_framework_prompt_compression_envelope;
use framework_extra::snapshot::build_framework_runtime_snapshot_envelope_with_level;
use framework_extra::statusline::build_framework_statusline;
use framework_extra::evidence::framework_hook_evidence_append;
use framework_kernel::repo_roots::resolve_repo_root_arg;
use framework_extra::framework_doctor::run_framework_doctor;
use framework_extra::session_artifacts::write_framework_session_artifacts;
use runtime_core::harness_contract::{harness_contract, lint_skill_contracts};
use core_policy::hook_policy::{HookPolicyEvaluateRequest, evaluate_hook_policy, hook_policy_contract};
use runtime_core::host_integration::run_host_integration_from_args;
use runtime_core::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, runtime_backend_family_catalog_payload,
    runtime_backend_family_parity_payload, runtime_storage_operation,
};
use runtime_core::step_ledger::handle_step_ledger_operation;
use runtime_core::task_command;
use runtime_core::task_state;
use runtime_core::trace_runtime::{
    TraceCompactRequestPayload, TraceRecordEventRequestPayload, compact_trace_stream,
    record_trace_event,
};
use host_projection::hosts::hook_dispatch::{HookEvent, HookOutput};
use host_projection::hooks::read_stdin_limited;

use runtime_core::runtime_storage::RuntimeStorageRequestPayload;

/// Thin wrapper: print_json_value shim that bridges FrameworkError → String.
fn print_json_value<T: Serialize>(v: &T) -> Result<(), String> {
    raw_print_json_value(v).map_err(|e| e.to_string())
}

fn hook_output_to_value(output: Option<HookOutput>, event_name: &str) -> Value {
    match output {
        None | Some(HookOutput::None) => json!({}),
        Some(HookOutput::Raw(val)) => val,
        Some(HookOutput::AdditionalContext(ctx)) => json!({
            "hookSpecificOutput": {
                "hookEventName": event_name,
                "additionalContext": ctx,
            }
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
            "hookSpecificOutput": {
                "hookEventName": event_name,
                "additionalContext": reason,
            },
        }),
        Some(HookOutput::Advisory { message }) => json!({
            "hookSpecificOutput": {
                "hookEventName": event_name,
                "additionalContext": message,
            },
        }),
        Some(HookOutput::Warn { message }) => json!({
            "hookSpecificOutput": {
                "hookEventName": event_name,
                "additionalContext": message,
            },
        }),
    }
}

/// Resolve host entrypoint provider by `--host-id`.
/// Registry-driven: iterates `host_provider_registry` to find a concrete provider,
/// reads the host's context_file (policy entrypoint) and builds the sync provider.
fn resolve_host_entrypoint_provider(
    repo_root: &std::path::Path,
    host_id: Option<&str>,
) -> Result<runtime_core::host_entrypoint_sync::HostEntrypointPayloadProvider, String> {
    let resolved = match host_id {
        Some(id) if !id.trim().is_empty() => id.trim(),
        _ => return Err("host-id is required for sync-entrypoints; pass --host-id <host_name>".to_string()),
    };
    let host_provider = runtime_core::hosts::host_provider_for_routing_spelling(resolved)
        .ok_or_else(|| format!(
            "unknown host-id '{resolved}' for sync-entrypoints; not found in host_provider_registry"
        ))?;
    let context_file = host_provider.context_file();
    let mut files = std::collections::BTreeMap::new();
    let context_path = repo_root.join(context_file);
    if let Ok(contents) = std::fs::read(&context_path) {
        files.insert(context_file.to_string(), contents);
    }
    Ok(runtime_core::host_entrypoint_sync::HostEntrypointPayloadProvider {
        files,
        json_relative_paths: vec![],
        manifest_relative_path: format!(".host_entrypoints_sync_manifest_{resolved}.json"),
        agent_policy_entrypoint: context_file.to_string(),
        after_apply: None,
    })
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
                return Err(format!(
                    "doctor found {} warning(s)",
                    result.warn_count,
                ));
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
        FrameworkCommand::StepLedger(command) => {
            let payload = parse_json_input::<Value>(&command.input_json, "framework step ledger")?;
            print_json_value(&handle_step_ledger_operation(payload)?)
        }
        FrameworkCommand::Maint { command } => runtime_core::framework_maint::dispatch(command),
        FrameworkCommand::Skills { command } => dispatch_framework_skills(command),
        FrameworkCommand::HostIntegration(command) => {
            let payload = run_host_integration_from_args(&command.args)?;
            print_json_value(&payload)
        }
        FrameworkCommand::NlRouteSignalRegistryContract => {
            println!("{}", runtime_core::route::nl_route_signal_registry_names_json());
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
    use skill_layer::refresh::{SkillsCommand, refresh_skills, validate_skills};
    match command {
        SkillsSubcommand::Validate(args) => {
            let repo_root = resolve_repo_root_arg(args.framework_root.as_deref())?;
            validate_skills(&repo_root).map_err(|e| e.to_string())
        }
        SkillsSubcommand::Refresh {
            repo_root,
            write,
            backfill,
            dry_run,
            generate,
            ..
        } => {
            let repo_root = resolve_repo_root_arg(repo_root.as_deref())?;
            refresh_skills(&SkillsCommand {
                repo_root,
                write,
                backfill,
                dry_run,
                generate,
            }).map_err(|e| e.to_string())
        }
    }
}

/// 拆分连字符分隔字符串，每段首字母大写，用 separator 拼接。
fn split_and_capitalize(s: &str, separator: &str) -> String {
    s.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
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
    let host_name = split_and_capitalize(host_id, " ");
    let host_name_camel = split_and_capitalize(host_id, "");

    let files = vec![
        // 1. Host provider
        (
            format!("core/host-projection/src/hosts/{host_id_camel}_provider.rs"),
            format!(
                r#"use runtime_core::hosts::host_provider::HostProvider;

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

/// Generic hook dispatch routed by host_id via dispatch registry.
/// Dispatchers are registered once at first call from the host layer types.
#[tracing::instrument(level = "info", skip_all, fields(host_id))]
pub fn dispatch_hook_command(host_id: &str, command: GenericHookCommand) -> Result<(), String> {
    let f = host_projection::hosts::find_hook_dispatch(host_id).ok_or_else(|| {
        format!(
            "hook dispatch not implemented for host `{host_id}`; \
             add a dispatch entry in the registration init"
        )
    })?;
    f(host_id, &command.event, command.repo_root.as_deref())
}

/// Register all hook and agent dispatch functions into the host-projection registry.
/// Hook dispatchers are registered for ALL hosts from ALL_HOST_IDS (registry-driven).
/// Agent dispatchers are registered for all hosts (delegates to generic run_agent_mcp_loop).
pub fn ensure_host_dispatchers_registered() {
    // Hook dispatchers — all hosts route to dispatch_host_hook
    use host_projection::hosts::HookDispatchFn;
    let hook_entries: Vec<(&'static str, HookDispatchFn)> =
        framework_kernel::runtime_registry::ALL_HOST_IDS
            .iter()
            .map(|&host_id| (host_id, (|hid, ev, repo| dispatch_host_hook(hid, ev, repo)) as HookDispatchFn))
            .collect();
    host_projection::hosts::register_hook_dispatchers(hook_entries);

    // Agent dispatchers — all hosts delegate to generic run_agent_mcp_loop
    use host_projection::hosts::AgentDispatchFn;
    let agent_entries: Vec<(&'static str, AgentDispatchFn)> =
        framework_kernel::runtime_registry::ALL_HOST_IDS
            .iter()
            .map(|&host_id| (host_id, (|host_id, repo| {
                let root = resolve_repo_root_arg(repo)?;
                runtime_core::hosts::run_agent_mcp_loop(Some(&root), host_id)
            }) as AgentDispatchFn))
            .collect();
    host_projection::hosts::register_agent_dispatchers(agent_entries);
}

/// Generic host hook dispatch via `HostProvider::dispatcher()`.
///
/// All lifecycle events (pretooluse, userpromptsubmit, posttooluse, stop, sessionstart,
/// subagentstart, subagentstop) are routed via the host's `HostHookDispatcher::dispatch()`.
/// Each host returns its registered dispatcher from the host provider registry.
fn dispatch_host_hook(host_id: &str, event: &str, repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root)?;

    // Bootstrap for lifecycle events
    runtime_core::kernel_bootstrap::ensure_kernel_bootstrap();
    runtime_core::hook_timing::mark_hook_start();
    let _registry_guard = core_policy::registry_review_gate::HookRegistryRepoGuard::new(&repo_root);

    // Read stdin payload (shared 4 MiB limited reader)
    let mut stdin = std::io::stdin().lock();
    let input = read_stdin_limited(&mut stdin).map_err(|e| format!("read_stdin: {e}"))?;
    let payload: Value = if input.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(input.trim())
            .map_err(|err| format!("stdin_json_invalid: {err}"))?
    };

    // Dispatch via host provider's registered HostHookDispatcher
    let provider = host_projection::hosts::host_provider_for_routing_spelling(host_id)
        .ok_or_else(|| format!("unknown host: {host_id}"))?;
    let dispatcher = provider.dispatcher();
    let hook_event = HookEvent {
        repo_root: &repo_root,
        event_name: event,
        payload: &payload,
    };
    let output = dispatcher.dispatch(&hook_event);

    // Convert HookOutput -> JSON value
    let mut json_output = hook_output_to_value(output, event);

    // Merge paper hooks into output (userpromptsubmit only — modify output before submit)
    // PostToolUse is intentionally excluded: it fires per-tool-call, not per-user-turn,
    // so merging into its output would repeat the nudge paragraph N times per prompt.
    if host_projection::hosts::hook_dispatch::normalize_event_name(event).as_ref() == "userpromptsubmit" {
        let prompt_text = host_projection::hosts::hook_dispatch::extract_prompt_text(&payload);
        let use_followup_message = provider.host_id() == "cursor";
        host_projection::hooks::maybe_merge_paper_prose_before_submit(
            &repo_root, &mut json_output, &prompt_text, use_followup_message, provider.host_id(),
        );
        host_projection::hooks::maybe_merge_paper_adversarial_before_submit(
            &repo_root, &mut json_output, &prompt_text, use_followup_message, provider.host_id(),
        );
    }

    // Emit hook timing telemetry + tracing::debug! to stderr
    runtime_core::hook_timing::emit_hook_timing_line(event);

    print_json_value(&json_output)?;
    Ok(())
}

/// Unified host command dispatcher (registry-driven: Hook / Agent).
pub fn dispatch_host_command(command: HostCommand) -> Result<(), String> {
    match command {
        HostCommand::Hook { host_id, command } => dispatch_hook_command(&host_id, command),
        HostCommand::Agent { host_id, command } => dispatch_agent_command(&host_id, command),
    }
}

/// Agent dispatch routed by host_id via dispatch registry.
pub fn dispatch_agent_command(host_id: &str, command: GenericAgentCommand) -> Result<(), String> {
    let f = host_projection::hosts::find_agent_dispatch(host_id).ok_or_else(|| {
        format!(
            "agent dispatch not implemented for host `{host_id}`; \
             add an agent dispatch entry in ensure_host_dispatchers_registered()"
        )
    })?;
    f(host_id, command.repo_root.as_deref())
}

/// Unified diagnostic command dispatcher (merged: profile, browser)
pub fn dispatch_diagnose_command(command: DiagnoseCommand) -> Result<(), String> {
    match command {
        DiagnoseCommand::Profile { command } => dispatch_profile_command(command),
        DiagnoseCommand::Browser { command } => dispatch_browser_command(command),
    }
}

pub fn dispatch_trace_command(command: TraceCommand) -> Result<(), String> {
    match command {
        TraceCommand::RecordEvent(command) => {
            let payload = parse_json_input::<TraceRecordEventRequestPayload>(
                &command.input_json,
                "trace record event",
            )?;
            print_json_value(&record_trace_event(payload).map_err(|e| e.to_string())?)
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
            print_json_value(&compact_trace_stream(payload).map_err(|e| e.to_string())?)
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
    Ok(hooks::dispatch_browser_command(command)?)
}

pub fn dispatch_profile_command(command: ProfileSubcommand) -> Result<(), String> {
    match command {
        ProfileSubcommand::Emit(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_profile_bundle(profile)?)
        }
        ProfileSubcommand::Artifacts(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_profile_artifact_bundle(profile, command.full)?)
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
                    framework_extra::closeout::evaluate_closeout_record_file_for_task(
                        repo_root,
                        task_id,
                        record_path,
                    )?
                }
                (Some(repo_root), Some(task_id), None) => {
                    let (_rows_non_empty, has_success) =
                        runtime_core::goal_drive::task_evidence_artifacts_summary_for_task(
                            repo_root, task_id,
                        );
                    let goal_state =
                        runtime_core::goal_drive::read_goal_state(repo_root, Some(task_id))
                            .ok()
                            .flatten();
                    let goal_prediction = goal_state
                        .as_ref()
                        .and_then(core_state::goal_prediction::read_goal_prediction);
                    let ctx = CloseoutEvidenceContext {
                        task_id: Some(task_id.trim().to_string()),
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
    use runtime_core::schema_drift::{
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
            let repo_root = resolve_repo_root_arg(None)?;
            let registry_path = repo_root.join("configs/framework/LOOP_REGISTRY.json");
            let raw = fs::read_to_string(&registry_path)
                .map_err(|e| format!("read LOOP_REGISTRY.json: {e}"))?;
            let registry: goal_engine::LoopRegistryRoot = serde_json::from_str(&raw)
                .map_err(|e| format!("parse LOOP_REGISTRY.json: {e}"))?;
            let entry = registry.loops.iter()
                .find(|e| e.loop_id == args.loop_id)
                .ok_or_else(|| format!("loop '{}' not found in LOOP_REGISTRY.json", args.loop_id))?;
            let timeout = std::time::Duration::from_secs(args.timeout);
            let ctx = goal_engine::runner::RunContext {
                repo_root: &repo_root,
                entry,
                dry_run: args.dry_run,
                timeout: Some(timeout),
                depth_remaining: goal_engine::runner::RunContext::default_max_depth(),
            };
            let aggregate = goal_engine::runner::run_loop(&ctx)
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
            let repo_root = resolve_repo_root_arg(None)?;
            match goal_engine::runner::run_loop_status(&repo_root, &args.loop_id)
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
            let repo_root = resolve_repo_root_arg(None)?;
            if args.all {
                goal_engine::runner::run_loop_kill_all(&repo_root)
                    .map_err(|e| format!("loop kill --all failed: {e}"))?;
                print_json_value(&serde_json::json!({
                    "ok": true,
                    "message": "All kill signals sent",
                }))
            } else {
                goal_engine::runner::run_loop_kill(&repo_root, &args.loop_id)
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
