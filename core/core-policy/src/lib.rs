//! Hook policy, guards, review signals, and permission exemptions (B0 core-policy).
pub mod crypto_util;
pub mod dev_exempt;
pub mod env_flags;
pub mod error;
pub mod goal_auto_detect;
pub mod hook_common;
pub mod hook_policy;
pub mod hook_review_disk_state;
pub mod lane_normalize;
pub mod registry_review_gate;
pub mod review_context_signals;
pub mod review_gate_engine;
pub mod review_output_lint;
pub mod review_routing_signals;
pub mod session_key;
pub mod subagent;

pub mod doc_registry;

#[cfg(any(test, feature = "test-sync"))]
pub mod test_env_sync;

pub use dev_exempt::{EXEMPT_PATH_PREFIXES, should_dev_exempt};
pub use env_flags::{
    env_enabled_default_false, env_enabled_default_true,
    router_rs_hook_legacy_subtracted_events_enabled,
    router_rs_hook_outbound_context_max_bytes, router_rs_hook_silent_enabled,
    router_rs_hook_state_dir_sync_enabled, router_rs_hook_state_fail_open_enabled,
    router_rs_hook_state_file_sync_enabled,
    router_rs_hook_state_legacy_full_sweep_enabled, router_rs_hook_state_lock_retries,
    router_rs_hook_state_stale_sweep_days, router_rs_cargo_check_sync_enabled,
    router_rs_operator_inject_globally_enabled,
    router_rs_pre_goal_enabled, router_rs_pre_goal_strict_disk_enabled,
    router_rs_subagent_model_inherit_nudge_enabled,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_gate_stop_max_nudges_cap,
    router_rs_review_pending_cycle_max, router_rs_review_spawn_first_nudge_enabled,
    router_rs_task_ledger_flock_enabled,
};
pub use hook_common::{
    COMPLETION_DETECT_EN, COMPLETION_DETECT_ZH_PHRASES, HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    GOAL_CHAT_VERIFY_ZH_PHRASES,
    ToolOrigin, classify_tool_origin,
    completion_claim_keywords_export, contains_completion_claim_token,
    has_delegation_override, has_override, has_review_override, hook_assistant_tail_window,
    is_deep_review_gate_lane_normalized,
    is_framework_non_goal_entrypoint_prompt,
    is_mcp_tool_name,
    is_narrow_review_prompt, is_parallel_delegation_prompt, is_review_prompt,
    is_reviewer_lane_normalized,
    normalize_subagent_type, normalize_tool_name, parse_mcp_tool_fqn,
    review_gate_advisory_only,
    review_gate_hard_block_disabled, review_gate_stop_would_nudge, saw_reject_reason,
    should_inject_spawn_first_review_nudge, should_inject_subagent_model_inherit_nudge,
    strip_quoted_or_codeblock_or_url, tool_input_value_from_map,
};
pub use hook_policy::{
    HOOK_POLICY_AUTHORITY, HOOK_POLICY_SCHEMA_VERSION, HookPolicyEvaluateRequest,
    HookPolicyEvaluateResponse, evaluate_hook_policy, evaluate_hook_policy_value,
    hook_policy_contract,
};
pub use hook_review_disk_state::{
    HOOK_REVIEW_DISK_VERSION, HookReviewDiskCore, HookReviewDiskVersion, HookReviewGateFields,
    apply_hook_review_gate_fields, hook_review_disk_core_from_value,
    hook_review_gate_fields_from_facts, hook_review_gate_fields_from_parts,
    hook_review_gate_fields_from_value, hook_review_subagent_state_basename,
    hook_review_independent_reviewer_seen_from_value, hook_review_stop_advisory_line,
    hook_review_stop_advisory_needed,
    hydrate_hook_review_gate_fields_from_value, migrate_hook_review_disk_core,
    review_stop_blocks_with_reject_escape,
};
pub use registry_review_gate::{
    HookRegistryRepoGuard, check_review_gate_registry_snapshot, clear_hook_registry_repo_root,
    is_reviewer_lane_from_registry, lifecycle_profile_disables_spawn_first_nudge,
    review_spawn_first_enabled, review_spawn_first_nudge_line,
    review_subagent_model_inherit_nudge_line, reviewer_lanes_prompt_lines, reviewer_lanes_sorted,
    runtime_registry_json_path, set_hook_registry_repo_root,
    spawn_first_includes_model_inherit_for_host,
};
pub use review_context_signals::{
    has_github_pr_context, has_paper_context, install_review_context_probes,
};
pub use review_gate_engine::{
    ReviewGateMode, ReviewGateFacts, countable_review_subagent_evidence,
    review_gate_mode, cycle_key_eligible_for_lite, fork_context_from_values,
    fork_context_false_means_independent, maybe_bump_review_phase_for_compact_findings,
    review_gate_armed, review_gate_blocks_stop, review_gate_satisfied, review_independent_fork,
    review_independent_reviewer_evidence,
};
pub use review_output_lint::{
    LintFinding, LintSeverity, assistant_has_substantive_compact_review_finding_line,
    lint_review_output,
};
pub use review_routing_signals::{
    ParallelReviewCandidateMarkers, parallel_review_candidate_markers, review_gate_compiled_regexes,
};
pub use subagent::{SUBAGENT_TOOL_NAMES, is_subagent_tool};
