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
pub(crate) use core_policy::lane_normalize;
pub(crate) use core_policy::review_gate_engine;
pub(crate) use core_policy::review_output_lint;
pub(crate) use core_policy::review_routing_signals;

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
static HOST_PROJECTION_HOOKS_INIT: OnceLock<()> = OnceLock::new();

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

/// Register host-projection hooks with runtime-core implementations.
/// Safe to call multiple times; only the first call takes effect.
pub fn register_host_projection_hooks() {
    HOST_PROJECTION_HOOKS_INIT.get_or_init(|| {
        let hooks = host_projection::hooks::build_hooks(
            // Framework Runtime
            framework_runtime::append_evidence_index_merged_row,
            || false, // closeout_programmatic_enforcement_enabled
            framework_runtime::closeout_record_path_for_task,
            framework_runtime::closeout_stop_followup_for_completion_text,
            || closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION,
            framework_runtime::current_local_timestamp,
            framework_runtime::evaluate_closeout_record_file_for_task,
            framework_runtime::extract_post_tool_duration_ms,
            |_repo_root| None, // first_task_id_from_registry
            |v| Ok(v), // framework_hook_evidence_append (passthrough)
            framework_runtime::post_tool_call_succeeded,
            framework_runtime::resolve_repo_root_arg,
            framework_runtime::write_framework_session_artifacts,
            framework_runtime::build_framework_runtime_snapshot_envelope,
            framework_runtime::build_automatic_continuity_checkpoint_payload_with_task_id,
            framework_runtime::route_manifest_fallback::route_task_with_manifest_fallback,
            // Hook timing
            hook_timing::mark_hook_start,
            hook_timing::add_lock_wait_ms,
            hook_timing::add_cargo_check_ms,
            hook_timing::emit_hook_timing_line,
            // Telemetry
            telemetry_emit::emit_hook_fired,
            telemetry_emit::emit_tool_call,
            telemetry_emit::hook_action_from_optional_output,
            telemetry_emit::hook_action_from_output,
            // Session call tracker
            session_call_tracker::init_tracker,
            session_call_tracker::read_tracker_state,
            |root, name, stats| {
                let cs = stats.map(|s| session_call_tracker::CacheStats {
                    cache_read_input_tokens: s.cache_read_input_tokens,
                    cache_creation_input_tokens: s.cache_creation_input_tokens,
                    input_tokens: s.input_tokens,
                    output_tokens: s.output_tokens,
                });
                session_call_tracker::record_tool_call(root, name, cs)
            },
            session_call_tracker::check_anomalies,
            // Router env flags
            router_env_flags::router_rs_skip_pre_tool_use_guard,
            router_env_flags::router_rs_task_ledger_flock_enabled,
            router_env_flags::router_rs_hook_timing_enabled,
            router_env_flags::router_rs_continuity_post_tool_evidence_enabled,
            router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes,
            router_env_flags::router_rs_session_call_tracker_tool_keys_max,
            router_env_flags::router_rs_rfv_max_rounds_cap,
            router_env_flags::router_rs_cursor_hook_state_lock_retries,
            router_env_flags::router_rs_cursor_hook_state_stale_sweep_days,
            router_env_flags::router_rs_cursor_review_gate_stop_max_nudges_cap,
            router_env_flags::router_rs_operator_inject_globally_enabled,
            router_env_flags::router_rs_cursor_hook_silent_enabled,
            router_env_flags::router_rs_cursor_sessionstart_context_max_bytes,
            // Paper hooks
            |root, prompt, lines, host| {
                let h = match host {
                    host_projection::hooks::PaperProseHookHostType::Claude => paper_prose_hook::PaperProseHookHost::Claude,
                    host_projection::hooks::PaperProseHookHostType::Cursor => paper_prose_hook::PaperProseHookHost::Cursor,
                    host_projection::hooks::PaperProseHookHostType::Codex => paper_prose_hook::PaperProseHookHost::Codex,
                };
                paper_prose_hook::maybe_append_paper_prose_context(root, prompt, lines, h);
            },
            |root, prompt, lines, host| {
                let h = match host {
                    host_projection::hooks::PaperProseHookHostType::Claude => paper_prose_hook::PaperProseHookHost::Claude,
                    host_projection::hooks::PaperProseHookHostType::Cursor => paper_prose_hook::PaperProseHookHost::Cursor,
                    host_projection::hooks::PaperProseHookHostType::Codex => paper_prose_hook::PaperProseHookHost::Codex,
                };
                paper_adversarial_hook::maybe_append_paper_adversarial_context(root, prompt, lines, h);
            },
            // Observation
            |output, host| {
                let h = match host {
                    host_projection::hooks::HookObservationHostType::ClaudeCode => router_rs_observation::HookObservationHost::ClaudeCode,
                    host_projection::hooks::HookObservationHostType::Cursor => router_rs_observation::HookObservationHost::Cursor,
                    host_projection::hooks::HookObservationHostType::Codex => router_rs_observation::HookObservationHost::Codex,
                };
                router_rs_observation::attach_router_rs_observation(output, h);
            },
            // Kernel
            kernel_bootstrap::ensure_kernel_bootstrap,
            // MCP pre-guard
            |tool, args, root| {
                let v = mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool, args, root);
                host_projection::hooks::McpPreGuardVerdictType { blocked: v.blocked, reason: v.reason }
            },
            // Web fetch guard
            web_fetch_guard::validate_and_resolve_web_fetch_url_as_strings,
            web_fetch_guard::resolve_web_fetch_redirect_as_string,
            web_fetch_guard::resolve_web_fetch_addresses_as_strings,
            // RFV loop
            rfv_loop::framework_rfv_loop,
            // Autopilot goal
            autopilot_goal::framework_goal_drive,
            |root| autopilot_goal::read_goal_state(root, None).map(|opt| opt.unwrap_or(serde_json::Value::Null)),
        );
        host_projection::hooks::register_hooks(hooks).ok();
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
