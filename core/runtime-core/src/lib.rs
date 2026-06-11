#![recursion_limit = "256"]
#![allow(unused_variables, unused_mut)]

//! runtime-core: extracted runtime modules from router-rs.
//!
//! Single source of truth for framework_runtime, session_supervisor, and supporting modules.

// ── original four ──
pub mod background_state;
pub mod runtime_envelope_ids;
pub mod runtime_storage;
pub mod trace_runtime;

// ── migrated modules (B3) ──
pub mod framework_runtime;
pub mod session_supervisor;
pub mod closeout_enforcement;
pub mod execution_contract;
pub use framework_profile;
pub mod rfv_loop;
pub mod schema_drift;

// ── browser dispatch hook (decouples runtime-core from browser-mcp crate) ──
pub mod browser_dispatch_hook;

// ── proxy / re-export modules ──
pub mod autopilot_goal;
pub mod atomic_write;
pub mod formal_toolchain;
pub mod kernel_bootstrap;
pub mod path_guard;
pub mod task_state;
pub mod task_state_aggregate;
pub mod task_ledger;
pub mod step_ledger;
pub mod task_write_lock;

// ── migrated supporting modules ──
// browser_mcp: physically migrated to core/browser-mcp crate (§2.4)
// Use browser-mcp crate directly; dispatch via browser_dispatch_hook.
#[cfg(feature = "codegraph")]
pub mod codegraph_mcp;
#[cfg(feature = "codegraph")]
pub mod codegraph_fts_bridge;
pub mod cli;
pub mod types;
pub mod eval_route;
pub use framework_kernel::framework_host_targets;
pub mod framework_maint;
pub mod framework_skills;
pub mod harness_context_signals;
pub mod harness_contract;
pub mod hook_event_routing;
pub mod hook_outbound_protect;
pub mod hook_timing;
pub use host_projection::host_entrypoint_sync;
pub use host_projection::host_integration;
pub mod hosts;
pub mod mcp_pre_guard;
pub mod paper_adversarial_hook;
pub mod paper_prose_hook;
pub mod review_gate;
pub mod route;
pub use framework_kernel::router_self;
// runtime_registry: re-export from framework-kernel + review gate additions from core-policy
pub mod runtime_registry {
    pub use framework_kernel::runtime_registry::*;
    // Review gate re-exports that were previously in this module
    pub use core_policy::registry_review_gate::{
        check_review_gate_registry_snapshot, clear_hook_registry_repo_root,
        is_reviewer_lane_from_registry, lifecycle_profile_disables_spawn_first_nudge,
        review_spawn_first_enabled, review_spawn_first_nudge_line,
        review_subagent_model_inherit_nudge_line, reviewer_lanes_prompt_lines,
        reviewer_lanes_sorted, set_hook_registry_repo_root,
        spawn_first_includes_model_inherit_for_host, HookRegistryRepoGuard,
    };
}
pub mod session_call_tracker;
pub mod ship_readiness;
pub use framework_kernel::skill_repo;
pub use framework_kernel::stdio_payload_types;
pub mod stdio_transport;
pub mod task_command;
pub mod telemetry_emit;
pub mod web_fetch_guard;
#[cfg(test)]
pub mod test_env_sync;
pub mod mcp_stdio_test_support;
#[allow(unused_imports)]
pub mod integration_test_prelude;

// ── modules with transitive deps ──
pub mod harness_operator_nudges;
pub mod hook_observation_rules;
pub mod router_env_flags;
pub mod router_rs_observation;

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
pub use hosts::cursor_hooks;
pub use hosts::codex_hooks;
pub use hosts::claude_code_hooks;
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
            |_root, _name, _stats_json| Ok(()), // record_tool_call: callers always pass None
            session_call_tracker::read_tracker_state,
        );

        host_projection::hooks::register_router_rs_observation(
            |_output, _host| {},  // attach: no-op (runtime-core handles directly)
            |_output| {},          // strip: no-op
        );

        host_projection::hooks::register_kernel_bootstrap(
            kernel_bootstrap::ensure_kernel_bootstrap,
        );

        host_projection::hooks::register_paper_hooks(
            |root, prompt, lines, host| {
                paper_prose_hook::maybe_append_paper_prose_context(root, prompt, lines, host)
            },
            |root, output, prompt, followup| {
                paper_prose_hook::maybe_merge_paper_prose_before_submit(root, output, prompt, followup)
            },
            |root, prompt, lines, host| {
                paper_adversarial_hook::maybe_append_paper_adversarial_context(root, prompt, lines, host)
            },
            |root, output, prompt, followup| {
                paper_adversarial_hook::maybe_merge_paper_adversarial_before_submit(root, output, prompt, followup)
            },
        );

        // ── extra hooks (runtime, web fetch, mcp guard, env flags) ──
        host_projection::hooks::register_framework_runtime_extra(
            framework_runtime::resolve_repo_root_arg,
            framework_runtime::current_local_timestamp,
            framework_runtime::write_framework_session_artifacts,
            |records, runtime_path, manifest_path, host_id, query, session_id, allow_overlay, first_turn| {
                framework_runtime::route_task_with_manifest_fallback(records, runtime_path, manifest_path, host_id, query, session_id, allow_overlay, first_turn)
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
                web_fetch_guard::validate_and_resolve_web_fetch_url(url)
                    .map(|(u, addrs)| (u.to_string(), addrs.iter().map(|a| a.to_string()).collect()))
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

        host_projection::hooks::register_mcp_pre_guard_extra(
            |tool, args, repo_root| {
                let v = mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool, args, repo_root);
                host_projection::hooks::McpPreGuardVerdict {
                    blocked: v.blocked,
                    reason: v.reason,
                }
            },
        );
    });
}

/// Auto-initialize routing hooks at library load time.
#[ctor::ctor]
fn auto_init_routing_hooks() {
    register_routing_hooks();
    register_host_projection_hooks();
}

// ── test helpers ──
#[cfg(test)]
pub fn touch_test_kernel_bootstrap() {
    kernel_bootstrap::ensure_kernel_bootstrap();
}

#[cfg(not(test))]
pub fn touch_test_kernel_bootstrap() {}
