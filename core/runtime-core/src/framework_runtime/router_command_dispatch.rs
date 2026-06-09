//! 子命令 `dispatch_*` 实现（Roadmap v5 P7：自 `cli/dispatch.rs` 下沉至 B3）。

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use crate::cli::args::*;
use crate::cli::common::{parse_json_input, print_json_value};
use super::{inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta, write_trace_metadata};
use crate::browser_mcp::{
    resolve_browser_mcp_attach_artifact, run_browser_mcp_stdio_loop, BrowserAttachConfig,
};
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
            let provider = codex_host_entrypoint_provider(&repo_root)?;
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
        CodexSubcommand::Sync(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let provider = codex_host_entrypoint_provider(&repo_root)?;
            print_json_value(&sync_host_entrypoints(&repo_root, true, provider)?)
        }
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
    match command {
        BrowserSubcommand::McpStdio(command) => run_browser_mcp_stdio_loop(
            command.repo_root.as_deref(),
            BrowserAttachConfig::from_cli_and_env(
                command.runtime_attach_descriptor_path,
                command.runtime_attach_artifact_path,
                command.headless,
            ),
        ),
        BrowserSubcommand::ResolveAttachArtifact(command) => {
            let repo_root = resolve_repo_root_arg(command.repo_root.as_deref())?;
            let Some(path) =
                resolve_browser_mcp_attach_artifact(&repo_root, command.search_root.as_deref())
            else {
                return Err("no browser-mcp runtime attach artifact candidates found".to_string());
            };
            println!("{path}");
            Ok(())
        }
    }
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
