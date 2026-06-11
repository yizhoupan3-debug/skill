//! 子命令 `dispatch_*` 实现（Roadmap v5 P7：自 `cli/dispatch.rs` 下沉至 B3）。

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use crate::cli::args::*;
use crate::cli::common::{parse_json_input, print_json_value};
use super::{inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata};
use crate::browser_dispatch_hook;
#[cfg(feature = "codegraph")]
use crate::codegraph_mcp::run_codegraph_mcp_stdio_loop;
use crate::mcp_stdio_harness::run_antigravity_mcp_loop;
use crate::claude_code_hooks::run_claude_hook_cli;
use crate::closeout_enforcement::{
    closeout_enforcement_contract, evaluate_closeout_record_value,
    evaluate_closeout_record_value_with_context, CloseoutEvidenceContext,
};
use crate::codex_hooks::{
    build_codex_hook_projection, codex_host_entrypoint_provider, install_codex_cli_hooks,
    resolve_codex_home, run_codex_audit_hook, InstallMode,
};
use crate::eval_route::{eval_route_contract, run_eval_route};
use crate::framework_profile::{
    build_codex_artifact_bundle, build_control_plane_contract_descriptors, build_profile_bundle,
    load_framework_profile,
};
use crate::framework_runtime::{
    build_framework_alias_envelope, build_framework_contract_summary_envelope,
    build_framework_prompt_compression_envelope, build_framework_runtime_snapshot_envelope,
    build_framework_statusline, framework_hook_evidence_append, resolve_repo_root_arg,
    run_framework_doctor, write_framework_session_artifacts, FrameworkAliasBuildOptions,
};
use crate::harness_contract::{harness_contract, lint_skill_contracts};
use crate::hook_policy::{evaluate_hook_policy, hook_policy_contract, HookPolicyEvaluateRequest};
use crate::host_entrypoint_sync::sync_host_entrypoints;
use crate::host_integration::run_host_integration_from_args;
use crate::review_gate::run_review_gate;
use crate::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, runtime_backend_family_catalog_payload,
    runtime_backend_family_parity_payload, runtime_storage_operation,
};
use crate::step_ledger::handle_step_ledger_operation;
use crate::task_command;
use crate::task_state;
use crate::trace_runtime::{
    compact_trace_stream, record_trace_event, TraceCompactRequestPayload,
    TraceRecordEventRequestPayload,
};

use crate::runtime_storage::RuntimeStorageRequestPayload;

fn codex_hook_stdout_payload(payload: Option<Value>) -> Value {
    payload.unwrap_or_else(|| json!({}))
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
            codex_host_entrypoint_provider(repo_root)
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
            print_json_value(&build_framework_runtime_snapshot_envelope(
                &repo_root,
                command.artifact_source_dir.as_deref(),
                command.task_id.as_deref(),
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
            let provider = resolve_host_entrypoint_provider(
                &repo_root,
                command.host_id.as_deref(),
            )?;
            print_json_value(&sync_host_entrypoints(&repo_root, true, provider)?)
        }
        FrameworkCommand::PromptCompression(command) => {
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework prompt compression")?;
            {
                let ctx_size = payload.get("context_window_size")
                    .and_then(serde_json::Value::as_u64)
                    .map(|v| v as usize);
                print_json_value(&build_framework_prompt_compression_envelope(payload, ctx_size)?)
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
            let payload =
                parse_json_input::<Value>(&command.input_json, "framework step ledger")?;
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
    use crate::framework_skills::{refresh_skills, validate_skills, SkillsCommand};
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
    if host_id.is_empty() || !host_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("Invalid host_id: {host_id:?}. Use alphanumeric, dash, or underscore."));
    }

    // Check host doesn't already exist
    let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    let registry: serde_json::Value = fs::read_to_string(&registry_path)
        .and_then(|s| serde_json::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
        .map_err(|e| format!("Read RUNTIME_REGISTRY.json: {e}"))?;

    if let Some(supported) = registry.get("host_targets").and_then(|v| v.get("supported")).and_then(|v| v.as_array()) {
        if supported.iter().any(|v| v.as_str() == Some(host_id)) {
            return Err(format!("Host {host_id:?} already exists in RUNTIME_REGISTRY.json"));
        }
    }

    let host_id_upper = host_id.to_uppercase().replace('-', "_");
    let host_id_camel = host_id.replace('-', "_");
    let host_name = host_id.split('-').map(|w| {
        let mut c = w.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }).collect::<Vec<_>>().join(" ");
    let host_name_camel = host_id.split('-').map(|w| {
        let mut c = w.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }).collect::<Vec<_>>().join("");

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
        "AGENTS_{hiu}.md"
    }}
}}
"#,
                    hc = host_name_camel,
                    hid = host_id,
                    hcc = host_id_camel,
                    hiu = host_id_upper,
                ),
        ),
        // 2. AGENTS file
        (
            format!("AGENTS_{hiu}.md", hiu = host_id_upper),
            format!(
r#"# {hn} Agent Context

> Generated by `router-rs scaffold --host-id {hid}`

## 宿主信息

- **Host ID**: `{hid}`
- **传输模式**: TBD
- **Session Supervisor**: unsupported

## 框架集成

参照 `AGENTS.md` 中的通用指令。本文件为 {hn} 宿主特定的上下文补充。

## 待补充

- [ ] 传输模式配置
- [ ] MCP 工具注册
- [ ] Hook 事件映射
- [ ] 宿主特定行为
"#,
                    hn = host_name,
                    hid = host_id,
                ),
        ),
        // 3. Documentation
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
4. 在 AGENTS_{hiu}.md 中补充宿主特定上下文
5. 配置 MCP 工具注册
6. 测试 install/remove/status 流程
"#,
                    hn = host_name,
                    hid = host_id,
                    hcc = host_id_camel,
                    hiu = host_id_upper,
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
                fs::create_dir_all(parent).map_err(|e| format!("Create dir {}: {e}", parent.display()))?;
            }
            fs::write(&full_path, content).map_err(|e| format!("Write {}: {e}", full_path.display()))?;
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
        "managed_mcp_server_ids": ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"],
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
                format!("Complete AGENTS_{host_id_upper}.md with host-specific context", host_id_upper = host_id.to_uppercase().replace('-', "_")),
                "Register host in host-projection/src/hosts/mod.rs",
                "Run: cargo test --workspace",
            ],
        },
    }))
}

/// Unified host command dispatcher (merged: codex, cursor, claude, antigravity, opencode)
pub fn dispatch_host_command(command: HostCommand) -> Result<(), String> {
    match command {
        HostCommand::Codex { command } => dispatch_codex_command(command),
        HostCommand::Cursor { command } => dispatch_cursor_command(command),
        HostCommand::Claude { command } => dispatch_claude_command(command),
        HostCommand::Antigravity { command } => dispatch_antigravity_command(command),
        HostCommand::AntigravityAppHost { command } => dispatch_antigravity_command(command),
        HostCommand::Opencode { command } => dispatch_opencode_command(command),
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
            let provider = codex_host_entrypoint_provider(&repo_root)?;
            print_json_value(&sync_host_entrypoints(&repo_root, false, provider)?)
        }
        CodexSubcommand::Hook(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let event_name = command
                .event
                .or(command.name)
                .ok_or("hook event required")?;
            let payload = run_codex_audit_hook(&event_name, &repo_root)?;
            print_json_value(&codex_hook_stdout_payload(payload))?;
            Ok(())
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

pub fn dispatch_cursor_command(command: CursorSubcommand) -> Result<(), String> {
    match command {
        CursorSubcommand::Hook(command) => {
            run_review_gate(&command.event, command.repo_root.as_deref())
        }
    }
}

pub fn dispatch_claude_command(command: ClaudeSubcommand) -> Result<(), String> {
    match command {
        ClaudeSubcommand::Hook(command) => {
            run_claude_hook_cli(&command.event, command.repo_root.as_deref())
        }
    }
}

pub fn dispatch_antigravity_command(command: AntigravitySubcommand) -> Result<(), String> {
    match command {
        AntigravitySubcommand::Agent(command) => {
            eprintln!(
                "[router-rs] deprecate: bare `antigravity agent` → prefer `host antigravity agent` (antigravity-app alias accepted)"
            );
            let root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            run_antigravity_mcp_loop(Some(&root))
        }
    }
}

pub fn dispatch_opencode_command(command: OpenCodeSubcommand) -> Result<(), String> {
    match command {
        OpenCodeSubcommand::Agent(command) => {
            let root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            crate::hosts::opencode_agent::run_opencode_mcp_loop(Some(&root))
        }
    }
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
            print_json_value(&build_profile_bundle(&profile)?)
        }
        ProfileSubcommand::Artifacts(command) => {
            let profile = load_framework_profile(&command.framework_profile)?;
            print_json_value(&build_codex_artifact_bundle(&profile, command.full)?)
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
                        )
                    }
                };
            let record_value: Value = serde_json::from_str(&raw)
                .map_err(|err| format!("closeout evaluate: invalid JSON: {err}"))?;
            let response =
                match (args.repo_root.as_deref(), args.task_id.as_deref(), args.record_path.as_deref()) {
                    (Some(repo_root), Some(task_id), Some(record_path)) => {
                        crate::framework_runtime::evaluate_closeout_record_file_for_task(
                            repo_root,
                            task_id,
                            record_path,
                        )?
                    }
                    (Some(repo_root), Some(task_id), None) => {
                        let (rows_non_empty, has_success) =
                            crate::autopilot_goal::task_evidence_artifacts_summary_for_task(
                                repo_root,
                                task_id,
                            );
                        let goal_state = crate::autopilot_goal::read_goal_state(
                            repo_root,
                            Some(task_id),
                        )
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
