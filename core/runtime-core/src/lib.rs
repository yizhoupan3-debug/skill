#![recursion_limit = "256"]
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! runtime-core: extracted runtime modules from router-rs.
//!
//! Single source of truth for framework_runtime, session_supervisor, and supporting modules.

// ── original four (flattened from runtime-storage) ──
pub use rt_storage::{runtime_envelope_ids, runtime_storage};
#[cfg(feature = "l5-state")]
pub use rt_storage::background_state;
pub use trace_runtime;

// ── migrated modules (B3) ──
pub use fr_contracts::{closeout_enforcement, execution_contract};
pub mod framework_runtime;
pub use session_supervisor;
pub use framework_kernel::framework_profile;

// ── subdomain module groups ──

// │  backward-compatible re-exports from subdomain groups ─────────────────────
pub use runtime_exit_gate::{quality_gate, schema_drift};
pub use runtime_infra::{
    kernel_bootstrap, stdio_transport, telemetry_emit,
};
pub use fr_exec::router_env_flags::{
    router_rs_subagent_model_inherit_nudge_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_pending_cycle_max,
    router_rs_review_spawn_first_nudge_enabled,
    router_rs_operator_inject_globally_enabled, router_rs_pre_goal_enabled,
    router_rs_hook_silent_enabled, router_rs_hook_outbound_context_max_bytes,
    router_rs_pre_goal_strict_disk_enabled, router_rs_hook_state_fail_open_enabled,
    router_rs_hook_state_lock_retries, router_rs_hook_state_file_sync_enabled,
    router_rs_hook_state_dir_sync_enabled, router_rs_cargo_check_sync_enabled,
    router_rs_hook_state_legacy_full_sweep_enabled, router_rs_hook_state_stale_sweep_days,
    router_rs_hook_legacy_subtracted_events_enabled,
    router_rs_env_enabled_default_true, router_rs_env_enabled_default_false,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_task_ledger_flock_enabled, router_rs_hook_timing_enabled,
    router_rs_session_call_tracker_tool_keys_max,
    router_rs_continuity_post_tool_evidence_enabled,
    router_rs_review_gate_stop_max_nudges_cap, router_rs_qg_max_rounds_cap,
    router_rs_session_supervisor_real_process_smoke_enabled,
};

// ── re-exports from rt_core_contracts (remaining pure contract modules) ──
pub use rt_core_contracts::{
    formal_toolchain, harness_contract, harness_context_signals, hook_event_routing,
    mcp_pre_guard, web_fetch_guard, hook_observation_rules,
};

// ── re-exports from core-state (flattened) ──
pub use core_state::{
    step_ledger, task_state, task_state_aggregate,
    state_manager as goal_drive,
};
// ── local contract modules (remain in runtime-core due to internal coupling) ──
pub mod hook_timing;

pub mod review_gate_cli;

pub mod task_command;

// ── migrated supporting modules ──
// browser_mcp: physically migrated to core/browser-mcp crate (§2.4)
// Use browser-mcp crate directly; dispatch via browser_dispatch_hook.
// cli: migrated to router-rs (ADR §10.3)
#[cfg(feature = "codegraph")]
pub mod codegraph_mcp;
pub use eval_route;
pub use framework_kernel::framework_host_targets;
pub use framework_maint;
pub use host_projection::host_entrypoint_sync;
pub use host_projection::host_integration;
pub use host_projection::hosts;
pub use routing_engine::route;
#[cfg(test)]
mod route_metadata_tests;
pub use framework_kernel::router_self;
pub use framework_kernel::skill_repo;
pub use framework_kernel::stdio_payload_types;
#[cfg(any(test, feature = "test-support"))]
pub mod mcp_stdio_test_support;
#[cfg(test)]
pub mod test_env_sync;

// (removed: hook_posttool_normalize was dead code)

// ── re-exports from core-policy (crate-internal only) ──
pub(crate) use core_policy::hook_policy;

// (removed: route_task_with_manifest_fallback re-export removed — callers use framework_extra::route_manifest_fallback directly)

// ── routing-engine hook registration ──
use std::sync::OnceLock;
static ROUTING_HOOKS_INIT: OnceLock<()> = OnceLock::new();

/// Register routing-engine hooks with runtime-core implementations.
/// Safe to call multiple times; only the first call takes effect.
pub fn register_routing_hooks() {
    ROUTING_HOOKS_INIT.get_or_init(|| {
        routing_engine::hooks::register_hooks(
            core_policy::hook_common::is_review_prompt,
            hosts::host_provider::host_provider_routing_aliases,
            touch_test_kernel_bootstrap,
            kernel_bootstrap::ensure_kernel_bootstrap,
            skill_repo::discover_skill_policy_repo_root,
            skill_repo::skill_routing_runtime_json,
            || {
                let m = core_policy::review_routing_signals::parallel_review_candidate_markers();
                routing_engine::hooks::ParallelReviewMarkers {
                    review_markers: m.review_markers,
                    breadth_markers: m.breadth_markers,
                    scope_markers: m.scope_markers,
                }
            },
        )
        .ok(); // ignore Err if already registered
    });
}

// ── merged routing config hooks (mcp-tool-registry + tool-routing-engine) ──
static TOOL_ROUTING_CONFIG_HOOKS_INIT: OnceLock<()> = OnceLock::new();

/// Register routing config hooks with merged implementations for
/// mcp-tool-registry and tool-routing-engine. Safe to call multiple times.
fn register_tool_routing_config_hooks() {
    TOOL_ROUTING_CONFIG_HOOKS_INIT.get_or_init(|| {
        routing_core::config_hooks::register_routing_config_hooks(
            // discover_tool_registry_path: default path
            || {
                let path = std::path::PathBuf::from(framework_kernel::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH);
                Some(path)
            },
            // discover_scoring_weights_path: resolve from FRAMEWORK_ROOT
            || {
                let root = std::env::var("FRAMEWORK_ROOT").ok()?;
                Some(format!("{root}/{}", framework_kernel::constants::TOOL_SCORING_WEIGHTS_RELATIVE_PATH))
            },
        )
        .ok(); // ignore Err if already registered
    });
}

// ── host-projection hooks registration ──
static HOST_PROJECTION_HOOKS_INIT: OnceLock<()> = OnceLock::new();

/// Register host-projection hooks with runtime-core implementations.
/// Safe to call multiple times; only the first call takes effect.
///
/// Decomposed into per-domain helpers for maintainability. Registration
/// order is deterministic — helpers execute sequentially within a single
/// OnceLock guard.
pub fn register_host_projection_hooks() {
    HOST_PROJECTION_HOOKS_INIT.get_or_init(|| {
        register_runtime_contract_hooks_impl();
        register_telemetry_hooks_impl();
        register_web_fetch_hooks_impl();
        register_mcp_hooks_impl();
        register_framework_bootstrap_hooks_impl();
        register_tool_dispatch_hooks_impl();
    });
}

// ── per-domain registration helpers (private) ──

fn register_runtime_contract_hooks_impl() {
    host_projection::hooks::register_framework_runtime(
        |repo_root| framework_extra::contract_summary::build_framework_contract_summary_envelope(repo_root).map_err(Into::into),
        framework_extra::evidence::try_append_post_tool_shell_evidence,
        framework_extra::closeout::closeout_programmatic_enforcement_enabled,
        |repo_root, task_id| framework_extra::closeout::closeout_record_path_for_task(repo_root, task_id).map_err(Into::into),
        |repo_root, task_id, record_path| framework_extra::closeout::evaluate_closeout_record_file_for_task(repo_root, task_id, record_path).map_err(Into::into),
        framework_extra::closeout::first_task_id_from_registry,
        framework_extra::evidence::framework_hook_evidence_append,
        framework_extra::evidence::extract_post_tool_duration_ms,
        framework_extra::evidence::post_tool_call_succeeded,
        framework_extra::closeout::closeout_stop_followup_for_completion_text,
    );

    host_projection::hooks::register_framework_runtime_extra(
        |repo_root| framework_kernel::repo_roots::resolve_repo_root_arg(repo_root).map_err(Into::into),
        framework_extra::util::current_local_timestamp,
        |payload| framework_extra::session_artifacts::write_framework_session_artifacts(payload).map_err(Into::into),
        |records_json,
         host_id,
         query,
         session_id,
         allow_overlay,
         first_turn| {
            let records: Vec<routing_engine::route::SkillRecord> = records_json.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            Ok(framework_extra::route_manifest_fallback::route_task_with_manifest_fallback(
                &records, host_id, query, session_id, allow_overlay, first_turn,
            )
            .map(|d| host_projection::hooks::RouteDecision {
                selected_skill: d.selected_skill,
                selected_skill_path: d.selected_skill_path,
                reasons: d.reasons,
                score: d.score,
            })?)
        },
        |repo_root, artifact_root, task_id| framework_extra::snapshot::build_framework_runtime_snapshot_envelope(repo_root, artifact_root, task_id).map_err(Into::into),
        framework_runtime::build_automatic_continuity_checkpoint_payload_with_task_id,
        framework_extra::evidence::append_evidence_index_merged_row,
        telemetry_emit::hook_action_from_output,
        || closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION,
        |repo_root| framework_extra::session_call::check_anomalies(repo_root).map_err(Into::into),
    );
    host_projection::hooks::register_build_framework_runtime_snapshot_envelope_with_level(
        |repo_root, artifact_root, task_id, detail_level| framework_extra::snapshot::build_framework_runtime_snapshot_envelope_with_level(repo_root, artifact_root, task_id, detail_level).map_err(Into::into),
    );

    // ── RFV loop full implementation (supports append_round) ──
    host_projection::hooks::register_quality_gate_drive(quality_gate::framework_quality_gate);

    // ── session-supervisor op dispatch (for MCP tools) ──
    host_projection::hooks::register_session_supervisor_op(
        |payload| session_supervisor::handle_session_supervisor_operation(payload).map_err(Into::into),
    );

    // ── framework-runtime-hooks internal hooks (pre_tool_use_guard, closeout, etc.) ──
    framework_runtime_hooks::register(framework_runtime_hooks::RuntimeCoreHooks {
        telemetry: framework_runtime_hooks::TelemetryHooks {
            hook_fired: telemetry_emit::emit_hook_fired,
            tool_call: |tool, count, blocked| {
                telemetry_emit::emit_tool_call(tool, count as u64, blocked);
            },
            route_decision: |_query, _decision, _reroute| {},
            prediction_outcome: |_task_id, _checks_summary, _verification_status, _checks_count| {},
            rfv_round: telemetry_emit::emit_rfv_round,
        },
        host_provider: framework_runtime_hooks::HostProviderHooks {
            for_routing_spelling: |host_id| {
                host_id.and_then(|id| {
                    hosts::host_provider::host_provider_for_routing_spelling(id)
                        .map(|p| p.host_id())
                })
            },
            strict_pre_tool_fallback_hint: hosts::host_provider::host_provider_strict_pre_tool_fallback_hint,
            registry: || {
                hosts::host_provider::host_provider_registry()
                    .iter()
                    .map(|p| (p.host_id(), None))
                    .collect()
            },
        },
        framework_goal_drive: core_state::state_manager::framework_goal_drive,
        framework_quality_gate: |payload| quality_gate::framework_quality_gate(payload).map_err(|e| e.to_string()),
        handle_session_supervisor_operation: session_supervisor::handle_session_supervisor_operation,
        #[cfg(feature = "l5-state")]
        handle_background_state_operation: rt_storage::background_state::handle_background_state_operation,
        #[cfg(not(feature = "l5-state"))]
        handle_background_state_operation: |_: serde_json::Value| -> Result<serde_json::Value, String> {
            Err("background_state requires l5-state feature".to_string())
        },
        runtime_concurrency_defaults_payload: || {
            serde_json::to_value(stdio_transport::runtime_concurrency_defaults_payload())
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "runtime_concurrency_defaults_payload serialization failed");
                    serde_json::json!({})
                })
        },
        eval_route_contract: eval_route::eval_route_contract,
        run_eval_route: |cases_path, runtime| {
            eval_route::run_eval_route(cases_path, runtime)
                .map(|report| serde_json::to_value(report).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "eval_route report serialization failed");
                    serde_json::json!({})
                }))
                .map_err(|e| e.to_string())
        },
        generated_artifacts_status_for_repo: |repo_root| {
            crate::host_integration::generated_artifacts_status_for_repo(repo_root)
                .map(|v| v.to_string())
                .map_err(|e| e.to_string())
        },
        ensure_kernel_bootstrap: kernel_bootstrap::ensure_kernel_bootstrap,
    });
}

fn register_telemetry_hooks_impl() {
    host_projection::hooks::register_hook_timing(
        hook_timing::mark_hook_start,
        hook_timing::add_lock_wait_ms,
        hook_timing::add_cargo_check_ms,
        hook_timing::emit_hook_timing_line,
    );

    host_projection::hooks::register_session_call_tracker(
        |repo_root| framework_extra::session_call::init_tracker(repo_root).map_err(Into::into),
        |root, name, stats_json| {
            let stats = stats_json.and_then(|v| {
                serde_json::from_value::<framework_extra::session_call::CacheStats>(v.clone()).ok()
            });
            Ok(framework_extra::session_call::record_tool_call(root, name, stats)?)
        },
        |repo_root| framework_extra::session_call::read_tracker_state(repo_root).map_err(Into::into),
    );

    host_projection::hooks::register_router_rs_observation(
        |_output, _host| {},
        |_output| {},
    );

    // ── Runtime trace transport proxies (for L3 browser-mcp) ──
    host_projection::hooks::register_attach_runtime_event_transport(
        |payload| fr_exec::trace_attach::attach_runtime_event_transport(payload).map_err(Into::into),
    );
    host_projection::hooks::register_inspect_trace_stream(
        |payload| fr_exec::trace_stream_io::inspect_trace_stream(payload).map_err(Into::into),
    );
}

fn register_web_fetch_hooks_impl() {
    host_projection::hooks::register_web_fetch_guard_extra(
        |url| {
            web_fetch_guard::validate_and_resolve_web_fetch_url(url).map(|(u, addrs)| {
                (u.to_string(), addrs.iter().map(|a| a.to_string()).collect())
            })
        },
        |base, location| {
            let base_url = reqwest::Url::parse(base)
                .map_err(|e| format!("web_fetch redirect base URL parse error: {e}"))?;
            web_fetch_guard::resolve_web_fetch_redirect(&base_url, location)
                .map(|u| u.to_string())
        },
        |host, port| {
            web_fetch_guard::resolve_web_fetch_addresses(host, port)
                .map(|addrs| addrs.iter().map(|a| a.to_string()).collect())
        },
    );
}

fn register_mcp_hooks_impl() {
    host_projection::hooks::register_mcp_pre_guard_extra(|tool, args, repo_root| {
        let v = mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool, args, repo_root);
        host_projection::hooks::McpPreGuardVerdict {
            blocked: v.blocked,
            reason: v.reason,
        }
    });

    // ── MCP routing: decouple L0→L1 DAG (ADR-010 §11.2) ──
    host_projection::hooks::register_mcp_tool_skill_route(
        |query: &str, host_id: &str, first_turn: bool, repo_root: &str| {
            let repo_root = std::path::Path::new(repo_root);
            let runtime_path = framework_kernel::skill_repo::skill_routing_runtime_json(repo_root);
            let records = routing_engine::route::load_records_cached_for_stdio(
                Some(&runtime_path),
            )?;
            let records = routing_engine::route::filter_records_for_host(
                records.as_ref(), Some(host_id),
            )?;
            let records_json: Vec<serde_json::Value> = records.iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect();
            let decision = host_projection::hooks::route_task_with_manifest_fallback(
                &records_json,
                Some(host_id),
                query,
                "session",
                true,
                first_turn,
            )?;
            if decision.selected_skill.is_empty() || decision.selected_skill == "none" {
                Ok(serde_json::to_string(&serde_json::json!({
                    "routed": false, "skill_slug": null,
                    "skill_path": null, "match_reason": "no match",
                })).map_err(|e| e.to_string())?)
            } else {
                Ok(serde_json::to_string(&serde_json::json!({
                    "routed": true,
                    "skill_slug": decision.selected_skill,
                    "skill_path": decision.selected_skill_path,
                    "match_reason": decision.reasons.first().cloned().unwrap_or_default(),
                })).map_err(|e| e.to_string())?)
            }
        },
    );
    host_projection::hooks::register_mcp_tool_search_skills(
        |query: &str, limit: usize, effective_host: &str, repo_root: &str| {
            let repo_root = std::path::Path::new(repo_root);
            let runtime_path = framework_kernel::skill_repo::skill_routing_runtime_json(repo_root);
            let records = routing_engine::route::load_records_cached_for_stdio(
                Some(&runtime_path),
            )?;
            let host_indices = routing_engine::route::filter_record_indices_for_host(
                records.as_ref(), Some(effective_host),
            )?;
            let rows = routing_engine::route::search_skills_subset(
                records.as_ref(), Some(&host_indices), query, limit,
            );
            let results = routing_engine::route::build_search_results_payload(query, rows);
            Ok(serde_json::to_string(&results).map_err(|e| e.to_string())?)
        },
    );
}

fn register_framework_bootstrap_hooks_impl() {
    host_projection::hooks::register_kernel_bootstrap(
        kernel_bootstrap::ensure_kernel_bootstrap,
    );

    // ── stdio transport dispatch (decouples runtime-infra from cli/) ──
    runtime_infra::stdio_transport::register_stdio_dispatch(
        crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request_payload,
        |key| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
        },
    );
}

fn register_tool_dispatch_hooks_impl() {
    host_projection::hooks::register_tool_goal_state_manage_dispatch(
        framework_runtime::tool_handlers::goal_state_manage_dispatch,
    );
    host_projection::hooks::register_tool_quality_gate_manage_dispatch(
        framework_runtime::tool_handlers::quality_gate_manage_dispatch,
    );
    host_projection::hooks::register_tool_closeout_record_write_dispatch(
        framework_runtime::tool_handlers::closeout_record_write_dispatch,
    );
    host_projection::hooks::register_tool_closeout_gate_evaluate(
        framework_runtime::tool_handlers::closeout_gate_evaluate,
    );
    host_projection::hooks::register_tool_routing_evolution_dispatch(
        framework_runtime::tool_handlers::routing_evolution_dispatch,
    );
}

/// Explicitly initialize all runtime-core hooks.
///
/// Call this at the top of `main()`. Explicit init gives you
/// deterministic ordering, easier testing, and avoids undefined behavior
/// around static initialization ordering across dynamic libraries.
///
/// Safe to call multiple times — internal `OnceLock` guards make repeated
/// calls no-ops.
pub fn init_hooks() {
    register_routing_hooks();
    register_tool_routing_config_hooks();
    register_host_projection_hooks();
}

// ── test helpers ──
#[cfg(test)]
pub fn touch_test_kernel_bootstrap() {
    kernel_bootstrap::ensure_kernel_bootstrap();
}

#[cfg(not(test))]
pub fn touch_test_kernel_bootstrap() {}
pub use fr_utils::constants::FRAMEWORK_RUNTIME_AUTHORITY;
