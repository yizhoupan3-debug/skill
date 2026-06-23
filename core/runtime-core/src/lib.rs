#![recursion_limit = "256"]

//! runtime-core: extracted runtime modules from router-rs.
//!
//! Single source of truth for framework_runtime, session_supervisor, and supporting modules.

// ── original four (flattened from runtime-storage) ──
pub use rt_storage::{background_state, runtime_envelope_ids, runtime_storage};
pub use trace_runtime;

// ── migrated modules (B3) ──
pub use ::framework_runtime::{closeout_enforcement, execution_contract};
pub mod framework_runtime;
pub use session_supervisor;
pub use framework_kernel::framework_profile;

// ── subdomain module groups ──
pub use runtime_infra as infrastructure;
pub use runtime_exit_gate as exit_gate;

// │  backward-compatible re-exports from subdomain groups ─────────────────────
pub use exit_gate::{quality_gate, schema_drift, harness_ops as harness_operator_nudges};
pub use framework_extra::session_call as session_call_tracker;
pub use infrastructure::{
    kernel_bootstrap, framework_skills, stdio_transport, telemetry_emit,
};
pub use ::framework_runtime::router_env_flags::*;

// ── re-exports from rt_core_contracts (remaining pure contract modules) ──
pub use rt_core_contracts::{
    formal_toolchain, harness_contract, harness_context_signals, hook_event_routing,
    mcp_pre_guard, web_fetch_guard, hook_observation_rules,
};

// ── re-exports from core-state (flattened) ──
pub use core_state::{
    step_ledger, task_state, task_state_aggregate,
    state_manager as goal_drive, utils::{atomic_write, path_guard},
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
pub mod eval_route;
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

// ── crate-level re-exports for `crate::X` path compat ──
pub use framework_extra::route_manifest_fallback::route_task_with_manifest_fallback;

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

// ── host-projection hooks registration ──
static HOST_PROJECTION_HOOKS_INIT: OnceLock<()> = OnceLock::new();

/// Register host-projection hooks with runtime-core implementations.
/// Safe to call multiple times; only the first call takes effect.
///
/// NOTE: This function is ~209 lines. Consider splitting into per-domain
/// registration helpers (e.g. `register_runtime_hooks`, `register_paper_hooks`,
/// `register_web_fetch_hooks`) to keep each registration scope focused.
pub fn register_host_projection_hooks() {
    HOST_PROJECTION_HOOKS_INIT.get_or_init(|| {
        // ── per-module registration ──
        host_projection::hooks::register_framework_runtime(
            framework_runtime::build_framework_contract_summary_envelope,
            framework_runtime::try_append_post_tool_shell_evidence,
            framework_runtime::closeout_programmatic_enforcement_enabled,
            framework_runtime::closeout_record_path_for_task,
            framework_runtime::evaluate_closeout_record_file_for_task,
            framework_runtime::first_task_id_from_registry,
            framework_runtime::framework_hook_evidence_append,
            framework_runtime::extract_post_tool_duration_ms,
            framework_runtime::post_tool_call_succeeded,
            framework_runtime::closeout_stop_followup_for_completion_text,
        );

        host_projection::hooks::register_hook_timing(
            hook_timing::mark_hook_start,
            hook_timing::add_lock_wait_ms,
            hook_timing::add_cargo_check_ms,
            hook_timing::emit_hook_timing_line,
        );

        host_projection::hooks::register_telemetry(
            telemetry_emit::emit_hook_fired,
            telemetry_emit::emit_tool_call,
            telemetry_emit::hook_action_from_optional_output,
        );

        host_projection::hooks::register_session_call_tracker(
            session_call_tracker::init_tracker,
            |root, name, stats_json| {
                let stats = stats_json.and_then(|v| {
                    serde_json::from_value::<session_call_tracker::CacheStats>(v.clone()).ok()
                });
                session_call_tracker::record_tool_call(root, name, stats)
            },
            session_call_tracker::read_tracker_state,
        );

        host_projection::hooks::register_router_rs_observation(
            |_output, _host| {}, // attach: no-op (runtime-core handles directly)
            |_output| {},        // strip: no-op
        );

        host_projection::hooks::register_kernel_bootstrap(
            kernel_bootstrap::ensure_kernel_bootstrap,
        );

        // ── extra hooks (runtime, web fetch, mcp guard, env flags) ──
        host_projection::hooks::register_framework_runtime_extra(
            framework_runtime::resolve_repo_root_arg,
            framework_runtime::current_local_timestamp,
            framework_runtime::write_framework_session_artifacts,
            |records_json,
             runtime_path,
             manifest_path,
             host_id,
             query,
             session_id,
             allow_overlay,
             first_turn| {
                // Deserialize from JSON to avoid L5→L1 dep on routing_engine::SkillRecord
                let records: Vec<routing_engine::route::SkillRecord> = records_json.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                framework_runtime::route_task_with_manifest_fallback(
                    &records,
                    runtime_path,
                    manifest_path,
                    host_id,
                    query,
                    session_id,
                    allow_overlay,
                    first_turn,
                )
                .map(|d| host_projection::hooks::RouteDecision {
                    selected_skill: d.selected_skill,
                    selected_skill_path: d.selected_skill_path,
                    reasons: d.reasons,
                    score: d.score,
                })
            },
            framework_runtime::build_framework_runtime_snapshot_envelope,
            framework_runtime::build_automatic_continuity_checkpoint_payload_with_task_id,
            framework_runtime::append_evidence_index_merged_row,
            telemetry_emit::hook_action_from_output,
            || closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION,
            session_call_tracker::check_anomalies,
        );
        host_projection::hooks::register_build_framework_runtime_snapshot_envelope_with_level(
            framework_runtime::build_framework_runtime_snapshot_envelope_with_level,
        );

        // web_fetch_guard: convert (Url, Vec<SocketAddr>) → (String, Vec<String>)
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

        host_projection::hooks::register_mcp_pre_guard_extra(|tool, args, repo_root| {
            let v = mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool, args, repo_root);
            host_projection::hooks::McpPreGuardVerdict {
                blocked: v.blocked,
                reason: v.reason,
            }
        });

        // ── MCP routing: decouple L0→L1 DAG (ADR-010 §11.2) ──
        // host-projection (L0) calls these fn ptrs instead of depending on
        // routing-engine (L1) at compile time. The actual routing-engine
        // types never cross the fn ptr boundary — only JSON strings do.
        host_projection::hooks::register_mcp_tool_skill_route(
            |query: &str, host_id: &str, first_turn: bool, repo_root: &str| {
                let repo_root = std::path::Path::new(repo_root);
                let runtime_path = framework_kernel::skill_repo::skill_routing_runtime_json(repo_root);
                let records = routing_engine::route::load_records_cached_for_stdio(
                    Some(&runtime_path), None,
                )?;
                let records = routing_engine::route::filter_records_for_host(
                    records.as_ref(), Some(host_id),
                )?;
                let records_json: Vec<serde_json::Value> = records.iter()
                    .filter_map(|r| serde_json::to_value(r).ok())
                    .collect();
                let decision = host_projection::hooks::route_task_with_manifest_fallback(
                    &records_json,
                    Some(&runtime_path),
                    None,
                    Some(host_id),
                    query,
                    "session",
                    true,
                    first_turn,
                )?;
                if decision.selected_skill.is_empty() || decision.selected_skill == "none" {
                    serde_json::to_string(&serde_json::json!({
                        "routed": false, "skill_slug": null,
                        "skill_path": null, "match_reason": "no match",
                    })).map_err(|e| e.to_string())
                } else {
                    serde_json::to_string(&serde_json::json!({
                        "routed": true,
                        "skill_slug": decision.selected_skill,
                        "skill_path": decision.selected_skill_path,
                        "match_reason": decision.reasons.first().cloned().unwrap_or_default(),
                    })).map_err(|e| e.to_string())
                }
            },
        );
        host_projection::hooks::register_mcp_tool_search_skills(
            |query: &str, limit: usize, effective_host: &str, repo_root: &str| {
                let repo_root = std::path::Path::new(repo_root);
                let runtime_path = framework_kernel::skill_repo::skill_routing_runtime_json(repo_root);
                let records = routing_engine::route::load_records_cached_for_stdio(
                    Some(&runtime_path), None,
                )?;
                let host_indices = routing_engine::route::filter_record_indices_for_host(
                    records.as_ref(), Some(effective_host),
                )?;
                let rows = routing_engine::route::search_skills_subset(
                    records.as_ref(), Some(&host_indices), query, limit,
                );
                let results = routing_engine::route::build_search_results_payload(query, rows);
                serde_json::to_string(&results).map_err(|e| e.to_string())
            },
        );

        // ── RFV loop full implementation (supports append_round) ──
        host_projection::hooks::register_quality_gate_drive(quality_gate::framework_quality_gate);

        // ── session-supervisor op dispatch (for MCP tools) ──
        host_projection::hooks::register_session_supervisor_op(
            session_supervisor::handle_session_supervisor_operation,
        );

        // ── framework-runtime internal hooks (pre_tool_use_guard, closeout, etc.) ──
        ::framework_runtime::hooks::register(::framework_runtime::hooks::RuntimeCoreHooks {
            telemetry: ::framework_runtime::hooks::TelemetryHooks {
                hook_fired: telemetry_emit::emit_hook_fired,
                tool_call: |tool, count, blocked| {
                    telemetry_emit::emit_tool_call(tool, count as u64, blocked);
                },
                route_decision: |_query, _decision, _reroute| {},
                prediction_outcome: |_task_id, _checks_summary, _verification_status, _checks_count| {},
                rfv_round: telemetry_emit::emit_rfv_round,
            },
            host_provider: ::framework_runtime::hooks::HostProviderHooks {
                for_routing_spelling: |host_id| {
                    host_id.and_then(|id| {
                        hosts::host_provider::host_provider_for_routing_spelling(id)
                            .map(|p| p.host_id())
                    })
                },
                default_id: hosts::host_provider::default_host_id,
                strict_pre_tool_fallback_hint: hosts::host_provider::host_provider_strict_pre_tool_fallback_hint,
                registry: || {
                    hosts::host_provider::host_provider_registry()
                        .iter()
                        .map(|p| (p.host_id(), None))
                        .collect()
                },
            },
            framework_goal_drive: core_state::state_manager::framework_goal_drive,
            framework_quality_gate: quality_gate::framework_quality_gate,
            handle_session_supervisor_operation: session_supervisor::handle_session_supervisor_operation,
            handle_background_state_operation: rt_storage::background_state::handle_background_state_operation,
            runtime_concurrency_defaults_payload: || {
                serde_json::to_value(stdio_transport::runtime_concurrency_defaults_payload())
                    .unwrap_or(serde_json::json!({}))
            },
            eval_route_contract: eval_route::eval_route_contract,
            run_eval_route: |cases_path, runtime, manifest| {
                eval_route::run_eval_route(cases_path, runtime, manifest)
                    .map(|report| serde_json::to_value(report).unwrap_or(serde_json::json!({})))
                    .map_err(|e| e.to_string())
            },
            generated_artifacts_status_for_repo: |repo_root| {
                crate::host_integration::generated_artifacts_status_for_repo(repo_root)
                    .map(|v| v.to_string())
            },
            ensure_kernel_bootstrap: kernel_bootstrap::ensure_kernel_bootstrap,
        });

        // ── Runtime trace transport proxies (for L3 browser-mcp) ──
        host_projection::hooks::register_attach_runtime_event_transport(
            |payload| ::framework_runtime::trace_attach::attach_runtime_event_transport(payload),
        );
        host_projection::hooks::register_inspect_trace_stream(
            ::framework_runtime::trace_stream_io::inspect_trace_stream,
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
    });
}

/// Explicitly initialize all runtime-core hooks.
///
/// **Prefer calling this at the top of `main()`** instead of relying on the
/// `#[ctor::ctor]` auto-initialization below.  Explicit init gives you
/// deterministic ordering, easier testing, and avoids undefined behavior
/// around static initialization ordering across dynamic libraries.
///
/// Safe to call multiple times — internal `OnceLock` guards make repeated
/// calls no-ops.
pub fn init_hooks() {
    register_routing_hooks();
    register_host_projection_hooks();
}

/// Auto-initialize routing hooks at library load time.
///
/// **SAFETY / CAVEAT**: `#[ctor::ctor]` runs before `main()` with no
/// guaranteed ordering relative to other static initializers.  This is
/// acceptable for the router-rs CLI binary (single crate, no dynamic
/// loading), but **not safe** for:
/// - Embedding runtime-core as a dynamic library
/// - Test harnesses that need deterministic init ordering
///
/// For those cases, call [`init_hooks()`] explicitly and compile with
/// `--no-default-features` to disable ctor.
#[cfg(not(test))]
#[ctor::ctor]
fn auto_init_routing_hooks() {
    init_hooks();
}

// ── test helpers ──
#[cfg(test)]
pub fn touch_test_kernel_bootstrap() {
    kernel_bootstrap::ensure_kernel_bootstrap();
}

#[cfg(not(test))]
pub fn touch_test_kernel_bootstrap() {}
pub use framework_runtime::FRAMEWORK_RUNTIME_AUTHORITY;
