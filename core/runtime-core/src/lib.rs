#![recursion_limit = "256"]
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! runtime-core: extracted runtime modules from router-rs.
//!
//! Single source of truth for framework_runtime and supporting modules.

// ── Re-export group: foundational state & storage primitives ──
// These re-exports provide a unified import surface for router-rs,
// which is the CLI entry point and needs access to multiple crates
// without depending on each one directly. They supply runtime state
// management, envelope ID resolution, and tracing infrastructure.
#[cfg(feature = "l5-state")]
pub use rt_storage::background_state;
pub use rt_storage::{runtime_envelope_ids, runtime_storage};
pub use trace_runtime;

// ── Re-export group: core runtime capabilities (B3 consolidation) ──
// Consolidated from runtime-core-contracts and framework-runtime during
// the B3 phase. These provide goal-driven execution, closeout validation,
// and framework profiling — the essential runtime machinery that router-rs
// orchestrates without importing each sub-crate directly.
pub use core_state::closeout_validation;
pub use fr_runtime::execution_contract;
pub mod framework_runtime;
pub use framework_core::framework_profile;

// ── Re-export group: quality-gate scene dispatch ──
// Scene-dispatched CheckerRegistry bridge. Provides the quality gate
// entry point and route so that host-projection can trigger QG evaluation
// without a direct compile-time dependency on the checker implementations.
mod checkers;
pub mod qg_entry;
pub mod qg_route;

// ── Re-export group: schema drift detection ──
// Migrated from runtime-exit-gate. Provides a single source of truth for
// schema drift detection logic, consolidating it under runtime-core so that
// both host-projection and framework-runtime can share the same implementation.
pub mod schema_drift;

// ── Re-export group: subdomain env flags & infra ──
// Backward-compatible re-exports of router environment flags and
// infrastructure utilities (kernel bootstrap, stdio transport). These
// preserve the existing import surface that hook-based hosts (claude,
// codex, opencode) depend on for configuration and bootstrapping.
pub use fr_runtime::router_env_flags::{
    router_rs_cargo_check_sync_enabled, router_rs_continuity_post_tool_evidence_enabled,
    router_rs_env_enabled_default_false, router_rs_env_enabled_default_true,
    router_rs_hook_legacy_subtracted_events_enabled, router_rs_hook_outbound_context_max_bytes,
    router_rs_hook_silent_enabled, router_rs_hook_state_dir_sync_enabled,
    router_rs_hook_state_fail_open_enabled, router_rs_hook_state_file_sync_enabled,
    router_rs_hook_state_legacy_full_sweep_enabled, router_rs_hook_state_lock_retries,
    router_rs_hook_state_stale_sweep_days, router_rs_hook_timing_enabled,
    router_rs_operator_inject_globally_enabled, router_rs_pre_goal_enabled,
    router_rs_pre_goal_strict_disk_enabled, router_rs_qg_max_rounds_cap,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_gate_stop_max_nudges_cap,
    router_rs_review_pending_cycle_max, router_rs_review_spawn_first_nudge_enabled,
    router_rs_subagent_model_inherit_nudge_enabled, router_rs_task_ledger_flock_enabled,
};
pub use runtime_infra::{kernel_bootstrap, kernel_utils, stdio_transport};

// ── Re-export group: framework contracts (merged into runtime-core) ──
// Consolidated from runtime-core-contracts. These modules define the
// framework-level contracts for tool behavior, hook lifecycle, outbound
// protection, and web fetch guards. Merged here to eliminate a separate
// crate and keep contract definitions co-located with their enforcement.
pub mod harness_context_signals;
pub mod harness_contract;
pub mod hook_event_routing;
pub mod hook_observation_rules;
pub mod hook_outbound_protect;
pub mod mcp_pre_guard;
pub mod web_fetch_guard;
pub use framework_core::formal_toolchain;

// ── Re-export group: state management & step ledger (flattened from core-state) ──
// Provides goal-drive state management, step ledger tracking, and task
// state as a flat import surface. Flattened so that consumers can import
// these directly without navigating the core-state module hierarchy.
pub use core_state::{state_manager as goal_drive, step_ledger, task_state};
// ── local contract modules (remain in runtime-core due to internal coupling) ──
pub mod hook_timing;

pub mod task_command;

// ── Re-export group: host integration, routing, and support modules ──
// Provides the primary integration surface for router-rs: host projection
// (entrypoint sync, host integration, host providers), routing engine,
// eval route, and supporting modules. These re-exports let router-rs
// orchestrate the full runtime without direct dependencies on each crate.
// browser_mcp: physically migrated to core/browser-mcp crate (§2.4)
// Use browser-mcp crate directly; dispatch via browser_dispatch_hook.
// cli: migrated to router-rs (ADR §10.3)
#[cfg(feature = "codegraph")]
pub mod codegraph_mcp;
pub use eval_route;
// ── Re-export group: framework-core primitives ──
// Core framework utilities needed by router-rs: host target resolution,
// skill repository discovery, and stdio payload type definitions. These
// bridge the framework-core API surface to the CLI entry point.
pub use framework_core::framework_host_targets;
pub use framework_maint;
pub use host_projection::host_entrypoint_sync;
pub use host_projection::host_integration;
pub use host_projection::hosts;
pub use routing_engine::route;
#[cfg(test)]
mod route_metadata_tests;
pub use framework_core::router_self;
pub use framework_core::skill_repo;
pub use framework_core::stdio_payload_types;
#[cfg(any(test, feature = "test-support"))]
pub mod mcp_stdio_test_support;
#[cfg(test)]
pub mod test_env_sync;

// (removed: hook_posttool_normalize was dead code)

// ── Re-export group: crate-internal policy (not public) ──
// Hook policy is crate-internal only — consumers inside runtime-core
// use it directly, but external crates must not depend on it.
pub(crate) use framework_core::hook_policy;

// (removed: route_task_with_manifest_fallback re-export removed — callers use framework_extra::route_manifest_fallback directly)

// ── Consolidated runtime init (single RUNTIME_INIT) ──
// Combines routing-engine hooks, routing config hooks, and host-projection hooks
// under a single init guard (replaces ROUTING_HOOKS_INIT / TOOL_ROUTING_CONFIG_HOOKS_INIT / HOST_PROJECTION_HOOKS_INIT).
use std::sync::OnceLock;
static RUNTIME_INIT: OnceLock<()> = OnceLock::new();

/// Combined orchestrator handler: background control operations → framework-extra,
/// team/agent/worker/session operations → session-supervisor.
fn combined_orchestrator_handler(
    payload: serde_json::Value,
) -> Result<serde_json::Value, core_errors::FrameworkError> {
    let operation = payload.get("operation").and_then(serde_json::Value::as_str);
    match operation {
        Some(
            "batch-plan" | "enqueue" | "interrupt" | "claim" | "complete" | "completion-race"
            | "retry-claim" | "interrupt-finalize" | "retry" | "session-release",
        ) => framework_extra::orchestration_controller::handle_orchestrator_operation(payload),
        Some(_) => session_supervisor::handle_session_supervisor_operation(payload),
        None => Err(core_errors::FrameworkError::validation(
            "orchestrator: missing 'operation' field",
        )),
    }
}

#[cfg(not(feature = "l5-state"))]
fn background_state_stub(
    _: serde_json::Value,
) -> Result<serde_json::Value, core_errors::FrameworkError> {
    tracing::warn!(
        target: "runtime_core",
        "l5-state feature is disabled — background_state operation is a no-op. \
         Enable the 'l5-state' feature in Cargo.toml to activate real background state handling."
    );
    Err(core_errors::FrameworkError::hook(
        "background_state requires l5-state feature",
    ))
}

/// Initialize all runtime-core subsystems. Safe to call multiple times.
pub fn init_hooks() {
    RUNTIME_INIT.get_or_init(|| {
        // 1. Routing-engine hooks
        routing_engine::hooks::register_hooks(
            framework_core::hook_common::is_review_prompt,
            hosts::host_provider::host_provider_routing_aliases,
            touch_test_kernel_bootstrap,
            kernel_bootstrap::ensure_kernel_bootstrap,
            skill_repo::discover_skill_policy_repo_root,
            skill_repo::skill_routing_runtime_json,
            || {
                let m = framework_core::review_routing_signals::parallel_review_candidate_markers();
                routing_engine::hooks::ParallelReviewMarkers {
                    review_markers: m.review_markers,
                    breadth_markers: m.breadth_markers,
                    scope_markers: m.scope_markers,
                }
            },
        )
        .ok(); // ignore Err if already registered

        // 2. Routing config hooks
        routing_core::config_hooks::register_routing_config_hooks(
            || {
                let path = std::path::PathBuf::from(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH);
                Some(path)
            },
            || {
                let root = std::env::var("SKILL_FRAMEWORK_ROOT").ok()?;
                Some(format!("{root}/{}", framework_core::constants::TOOL_SCORING_WEIGHTS_RELATIVE_PATH))
            },
        )
        .ok();

        // 3. Host-projection hooks (RuntimeHooks struct, direct construction)
        host_projection::hooks::set_runtime_hooks(
            host_projection::hooks::RuntimeHooks {
                // framework_runtime (5 fields)
                closeout_record_path_for_task: framework_extra::closeout::closeout_record_path_for_task,
                evaluate_closeout_record_file_for_task: framework_extra::closeout::evaluate_closeout_record_file_for_task,
                extract_post_tool_duration_ms: framework_extra::evidence::extract_post_tool_duration_ms,
                post_tool_call_succeeded: framework_extra::evidence::post_tool_call_succeeded,
                closeout_stop_followup_for_completion_text: framework_extra::closeout::closeout_stop_followup_for_completion_text,
                // paper hooks (4 fields) — defaults; research-harness overrides via individual OnceLock
                maybe_append_paper_prose_context: |_, _, _, _| {},
                maybe_merge_paper_prose_before_submit: |_, _, _, _, _| {},
                maybe_append_paper_adversarial_context: |_, _, _, _| {},
                maybe_merge_paper_adversarial_before_submit: |_, _, _, _, _| {},
                // research activity (1 field) — default; research-harness overrides via OnceLock
                maybe_record_research_activity: |_, _, _| {},
                // kernel bootstrap (1 field)
                ensure_kernel_bootstrap: kernel_bootstrap::ensure_kernel_bootstrap,
                // framework_runtime_extra (7 fields)
                current_local_timestamp: framework_extra::util::current_local_timestamp,
                write_framework_session_artifacts: framework_extra::session_artifacts::write_framework_session_artifacts,
                build_automatic_continuity_checkpoint_payload: crate::framework_runtime::build_automatic_continuity_checkpoint_payload_with_task_id,
                append_evidence_index: framework_extra::evidence::append_evidence_index_merged_row,
                closeout_record_schema_version: || closeout_validation::CLOSEOUT_RECORD_SCHEMA_VERSION,
                // web_fetch_guard (3 fields)
                validate_and_resolve_web_fetch_url: |url| {
                    web_fetch_guard::validate_and_resolve_web_fetch_url(url).map(|(u, addrs)| {
                        (u.to_string(), addrs.iter().map(|a| a.to_string()).collect())
                    })
                },
                resolve_web_fetch_redirect: |base, location| {
                    let base_url = reqwest::Url::parse(base)
                        .map_err(|e| format!("web_fetch redirect base URL parse error: {e}"))?;
                    web_fetch_guard::resolve_web_fetch_redirect(&base_url, location)
                        .map(|(u, addrs)| (u.to_string(), addrs.iter().map(|a| a.to_string()).collect()))
                },
                resolve_web_fetch_addresses: |host, port| {
                    web_fetch_guard::resolve_web_fetch_addresses(host, port)
                        .map(|addrs| addrs.iter().map(|a| a.to_string()).collect())
                },
                // mcp_pre_guard (1 field)
                evaluate_mcp_pre_guard_safe: |tool, args, repo_root| {
                    let v = mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool, args, repo_root);
                    host_projection::hooks::McpPreGuardVerdict {
                        blocked: v.blocked,
                        reason: v.reason,
                    }
                },
                // research_tool_dispatch (1 field) — default; research-harness overrides via OnceLock
                research_tool_dispatch: |_, _| Err(core_errors::FrameworkError::validation("research_tool_dispatch not registered")),
                // mcp_tool_routing (2 fields)
                mcp_tool_skill_route: |query: &str, host_id: &str, first_turn: bool, repo_root: &str| {
                    let repo_root = std::path::Path::new(repo_root);
                    let runtime_path = framework_core::skill_repo::skill_routing_runtime_json(repo_root);
                    let records = routing_engine::route::load_records_cached_for_stdio(
                        Some(&runtime_path),
                    )?;
                    // Direct call to route_task_with_manifest_fallback avoids:
                    // 1. Double filter_records_for_host (done once inside the function)
                    // 2. SkillRecord → serde_json::Value → SkillRecord round-trip
                    // 3. Hook function pointer indirection
                    let decision = framework_extra::route_manifest_fallback::route_task_with_manifest_fallback(
                        records.as_ref(),
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
                // tool_dispatch (4 fields)
                tool_goal_state_manage_dispatch: crate::framework_runtime::tool_handlers::goal_state_manage_dispatch,
                tool_closeout_record_write_dispatch: crate::framework_runtime::tool_handlers::closeout_record_write_dispatch,
                tool_closeout_gate_evaluate: crate::framework_runtime::tool_handlers::closeout_gate_evaluate,
                // browser_dispatch (1 field) — default; set_browser_dispatch overrides via OnceLock
                browser_dispatch: |_| Err(core_errors::FrameworkError::validation("browser-mcp dispatch not registered")),
                // runtime_trace_transport (2 fields)
                attach_runtime_event_transport:
                    fr_runtime::trace_attach::attach_runtime_event_transport,
                inspect_trace_stream: fr_runtime::trace_stream_io::inspect_trace_stream,
            },
        );

        // ── framework_core::runtime_hooks (pre_tool_use_guard, closeout, etc.) ──
        framework_core::runtime_hooks::register(framework_core::runtime_hooks::RuntimeCoreHooks {
            host_provider: framework_core::runtime_hooks::HostProviderHooks {
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
            handle_orchestrator_operation: combined_orchestrator_handler,
            #[cfg(feature = "l5-state")]
            handle_background_state_operation: rt_storage::background_state::handle_background_state_operation,
            #[cfg(not(feature = "l5-state"))]
            handle_background_state_operation: background_state_stub,
            runtime_concurrency_defaults_payload: || {
                serde_json::to_value(stdio_transport::runtime_concurrency_defaults_payload())
                    .map_err(|e| {
                        tracing::warn!(error = %e, "runtime_concurrency_defaults_payload serialization failed");
                        core_errors::FrameworkError::validation(format!("runtime_concurrency_defaults_payload serialization: {e}"))
                    })
            },
            eval_route_contract: eval_route::eval_route_contract,
            run_eval_route: |cases_path, runtime| {
                let report = eval_route::run_eval_route(cases_path, runtime)?;
                serde_json::to_value(report).map_err(|e| {
                    tracing::warn!(error = %e, "eval_route report serialization failed");
                    core_errors::FrameworkError::validation(format!("eval_route report serialization: {e}"))
                })
            },
            generated_artifacts_status_for_repo: |repo_root| {
                crate::host_integration::generated_artifacts_status_for_repo(repo_root)
                    .map(|v| v.to_string())
            },
            ensure_kernel_bootstrap: kernel_bootstrap::ensure_kernel_bootstrap,
            evaluate_quality_gate: crate::qg_entry::evaluate_quality_gate_hook,
            evaluate_closeout_gate: crate::framework_runtime::tool_handlers::closeout_handler::evaluate_closeout_gate_hook,
        });

        // 4. QG Route: scene-dispatched CheckerRegistry
        qg_route::init_qg_route();

        // 5. Stdio transport dispatch (decouples runtime-infra from cli/)
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

// ── test helpers ──
#[cfg(test)]
pub fn touch_test_kernel_bootstrap() {
    kernel_bootstrap::ensure_kernel_bootstrap();
}

#[cfg(not(test))]
pub fn touch_test_kernel_bootstrap() {}
pub use fr_runtime::constants::FRAMEWORK_RUNTIME_AUTHORITY;
