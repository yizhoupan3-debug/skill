#![recursion_limit = "256"]

//! runtime-core: extracted runtime modules from router-rs.
//!
//! Single source of truth for framework_runtime, session_supervisor, and supporting modules.

// ── original four ──
// background_state → extracted to runtime-storage crate
pub(crate) use rt_storage::background_state;
// runtime_envelope_ids, runtime_storage → extracted to runtime-storage crate
pub use rt_storage::runtime_envelope_ids;
pub use rt_storage::runtime_storage;
pub use trace_runtime;

// ── migrated modules (B3) ──
pub use ::framework_runtime::closeout_enforcement;
pub(crate) use ::framework_runtime::execution_contract;
pub mod framework_runtime;
pub(crate) use session_supervisor;
pub use framework_kernel::framework_profile;
pub mod rfv_loop;
pub mod schema_drift;

// ── browser dispatch hook (decouples runtime-core from browser-mcp crate) ──
pub mod browser_dispatch_hook;

// ── re-exports from rt_core_contracts (flattened, no intermediate contracts module) ──
#[allow(unused_imports)]
pub(crate) use rt_core_contracts::formal_toolchain;
pub use rt_core_contracts::harness_contract;
pub(crate) use rt_core_contracts::harness_context_signals;
pub(crate) use rt_core_contracts::harness_operator_nudges;
pub(crate) use rt_core_contracts::hook_event_routing;
pub use rt_core_contracts::kernel_bootstrap;
pub use rt_core_contracts::mcp_pre_guard;
pub use rt_core_contracts::router_env_flags;
pub(crate) use rt_core_contracts::framework_skills;
pub(crate) use rt_core_contracts::router_rs_observation;
pub use rt_core_contracts::session_call_tracker;
pub use rt_core_contracts::web_fetch_guard;

// ── re-exports from core-state (flattened) ──
pub use core_state::utils::atomic_write;
pub use core_state::utils::path_guard;
pub use core_state::step_ledger;
pub(crate) use core_state::task_ledger;
pub use core_state::task_state;
pub use core_state::task_state_aggregate;
pub(crate) use core_state::utils::task_write_lock;
pub use core_state::state_manager as goal_drive;

// ── local contract modules (remain in runtime-core due to internal coupling) ──
pub mod hook_timing;
pub mod review_gate;
pub mod task_command;

// ── migrated supporting modules ──
// browser_mcp: physically migrated to core/browser-mcp crate (§2.4)
// Use browser-mcp crate directly; dispatch via browser_dispatch_hook.
pub mod cli;
#[cfg(feature = "codegraph")]
pub mod codegraph_mcp;
pub mod eval_route;
pub(crate) use framework_kernel::framework_host_targets;
pub mod framework_maint;
pub use host_projection::host_entrypoint_sync;
pub use host_projection::host_integration;
pub use host_projection::hosts;
pub mod paper_adversarial_hook;
mod paper_block_cache;
pub mod paper_prose_hook;
pub mod research_activity_log;
pub use routing_engine::route;
#[cfg(test)]
mod route_metadata_tests;
pub use framework_kernel::router_self;
// runtime_registry: explicit re-exports from framework-kernel (no glob)
// + review gate additions from core-policy (separated by source)
pub mod runtime_registry {
    // ── framework-kernel re-exports ──
    pub use framework_kernel::runtime_registry::{
        ALL_KNOWN_HOST_DIRS, DEFAULT_MANAGED_MCP_SERVER_IDS, HOST_ADAPTER_CONTRACT_PATH,
        HOST_HOME_DIRS, RUNTIME_REGISTRY_PATH, RUNTIME_REGISTRY_SCHEMA_VERSION,
        RuntimeRegistry, RuntimeSkillsDefaults, RuntimeWorkspaceBootstrapDefaults,
        closeout_evidence_hooks_unsupported_on_host, harness_capability_exception_entry,
        harness_capability_exception_rationale, host_projection_object,
        load_runtime_registry, load_runtime_registry_json,
        load_runtime_registry_payload, load_runtime_registry_payload_if_repo_local,
        managed_mcp_server_for_tool, managed_mcp_server_ids,
        parse_host_mcp_tool_fqn, resolves_managed_mcp_tool,
        runtime_registry_path,
    };
    // ── core-policy review gate re-exports ──
    pub use core_policy::registry_review_gate::{
        HookRegistryRepoGuard, check_review_gate_registry_snapshot, clear_hook_registry_repo_root,
        is_reviewer_lane_from_registry, lifecycle_profile_disables_spawn_first_nudge,
        review_spawn_first_enabled, review_spawn_first_nudge_line,
        review_subagent_model_inherit_nudge_line, reviewer_lanes_prompt_lines,
        reviewer_lanes_sorted, set_hook_registry_repo_root,
        spawn_first_includes_model_inherit_for_host,
    };
}
pub use framework_kernel::skill_repo;
pub use framework_kernel::stdio_payload_types;
pub mod mcp_stdio_test_support;
pub mod stdio_transport;
pub mod telemetry_emit;
#[cfg(test)]
pub mod test_env_sync;

// ── path-qualified module ──
#[path = "utils/hook_posttool_normalize.rs"]
pub mod hook_posttool_normalize;

// ── re-exports from core-policy (crate-internal only) ──
pub(crate) use core_policy::hook_common;
pub(crate) use core_policy::hook_policy;
pub(crate) use core_policy::review_gate_engine;

// ── crate-level re-exports for `crate::X` path compat ──
pub use framework_runtime::route_manifest_fallback::route_task_with_manifest_fallback;

// ── host submodule re-exports (for `crate::X` path compat) ──
pub use hosts::claude_hooks;
pub use hosts::codex_hooks;
pub use hosts::cursor_hooks;
pub use hosts::mcp_stdio_harness;

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

        host_projection::hooks::register_paper_hooks(
            |root, prompt, lines, host| {
                paper_prose_hook::maybe_append_paper_prose_context(root, prompt, lines, host)
            },
            |root, output, prompt, followup| {
                paper_prose_hook::maybe_merge_paper_prose_before_submit(
                    root, output, prompt, followup,
                )
            },
            |root, prompt, lines, host| {
                paper_adversarial_hook::maybe_append_paper_adversarial_context(
                    root, prompt, lines, host,
                )
            },
            |root, output, prompt, followup| {
                paper_adversarial_hook::maybe_merge_paper_adversarial_before_submit(
                    root, output, prompt, followup,
                )
            },
        );

        host_projection::hooks::register_research_activity_hook(
            |root, tool, summary| {
                research_activity_log::record_research_activity(root, tool, summary)
            },
        );

        // ── extra hooks (runtime, web fetch, mcp guard, env flags) ──
        host_projection::hooks::register_framework_runtime_extra(
            framework_runtime::resolve_repo_root_arg,
            framework_runtime::current_local_timestamp,
            framework_runtime::write_framework_session_artifacts,
            |records,
             runtime_path,
             manifest_path,
             host_id,
             query,
             session_id,
             allow_overlay,
             first_turn| {
                framework_runtime::route_task_with_manifest_fallback(
                    records,
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

        // ── RFV loop full implementation (supports append_round) ──
        host_projection::hooks::register_quality_gate_drive(rfv_loop::framework_rfv_loop);

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
            framework_quality_gate: rfv_loop::framework_rfv_loop,
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
                host_projection::host_integration::generated_artifacts_status_for_repo(repo_root)
                    .map(|v| v.to_string())
            },
            ensure_kernel_bootstrap: kernel_bootstrap::ensure_kernel_bootstrap,
        });
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
