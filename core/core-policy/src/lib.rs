//! Hook policy, guards, review signals, and permission exemptions (B0 core-policy).
pub mod crypto_util;
pub mod dev_exempt;
pub mod session_key;
pub mod subagent;
pub mod env_flags;
pub mod hook_common;
pub mod hook_policy;
pub mod hook_review_disk_state;
pub mod lane_normalize;
pub mod registry_review_gate;
pub mod review_context_signals;
pub mod review_gate_engine;
pub mod review_output_lint;
pub mod review_routing_signals;

#[cfg(any(test, feature = "test-sync"))]
pub mod test_env_sync;

pub use dev_exempt::{should_dev_exempt, EXEMPT_PATH_PREFIXES};
pub use env_flags::{
    env_enabled_default_false, env_enabled_default_true,
    router_rs_cursor_subagent_model_inherit_nudge_enabled,
    router_rs_review_fork_context_missing_infer_false_enabled,
    router_rs_review_gate_disabled_for_host, router_rs_review_gate_stop_max_nudges_cap,
    router_rs_review_pending_cycle_max, router_rs_review_spawn_first_nudge_enabled,
};
pub use hook_common::{
    completion_claim_keywords_export, contains_completion_claim_token, has_delegation_override,
    tool_input_value_from_map,
    has_override, has_review_override, hook_assistant_tail_window,
    is_deep_review_gate_lane_normalized, is_reviewer_lane_normalized,
    is_framework_goal_entry_prompt, is_framework_implement_entry_prompt,
    is_framework_non_goal_entrypoint_prompt, is_my_lifecycle_entry_prompt,
    is_my_pre_execution_entry_prompt, is_my_verify_entry_prompt, is_narrow_review_prompt,
    is_parallel_delegation_prompt, is_review_prompt, my_goal_drive_hook_nudge_for_prompt,
    my_light_profile_active, normalize_subagent_type, normalize_tool_name, review_gate_advisory_only,
    review_gate_hard_block_disabled, review_gate_stop_would_nudge, saw_reject_reason,
    should_inject_spawn_first_review_nudge,
    should_inject_subagent_model_inherit_nudge, strip_quoted_or_codeblock_or_url,
    COMPLETION_DETECT_EN, COMPLETION_DETECT_ZH_PHRASES, CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    GOAL_CHAT_VERIFY_ZH_PHRASES, MY_GOAL_DRIVE_HOOK_NUDGE, MY_IMPLEMENT_GOAL_DRIVE_HOOK_NUDGE,
    MY_PRE_EXECUTION_HOOK_NUDGE,
};
pub use hook_review_disk_state::{
    apply_hook_review_gate_fields, hook_review_disk_core_from_value,
    hook_review_gate_fields_from_facts, hook_review_gate_fields_from_parts,
    hook_review_gate_fields_from_value, hook_review_gate_legacy_state_basename,
    hook_review_subagent_state_basename,
    hook_review_independent_reviewer_seen_from_value,
    hydrate_hook_review_gate_fields_from_value, migrate_hook_review_disk_core,
    hook_review_stop_advisory_line, hook_review_stop_advisory_needed,
    review_stop_blocks_with_reject_escape, HookReviewDiskCore, HookReviewDiskVersion,
    HookReviewGateFields, HOOK_REVIEW_DISK_VERSION,
};
pub use hook_policy::{
    evaluate_hook_policy, evaluate_hook_policy_value, hook_policy_contract,
    HookPolicyEvaluateRequest, HookPolicyEvaluateResponse, HOOK_POLICY_AUTHORITY,
    HOOK_POLICY_SCHEMA_VERSION,
};
pub use registry_review_gate::{
    check_review_gate_registry_snapshot, clear_hook_registry_repo_root,
    runtime_registry_json_path, set_hook_registry_repo_root, HookRegistryRepoGuard,
    is_reviewer_lane_from_registry, lifecycle_profile_disables_spawn_first_nudge,
    review_spawn_first_enabled, review_spawn_first_nudge_line,
    review_subagent_model_inherit_nudge_line, reviewer_lanes_prompt_lines, reviewer_lanes_sorted,
    spawn_first_includes_model_inherit_for_host,
};
pub use review_context_signals::{has_github_pr_context, has_paper_context, install_review_context_probes};
pub use review_gate_engine::{
    codex_countable_review_subagent_evidence, cursor_review_gate_mode,
    cycle_key_eligible_for_lite, fork_context_from_values, independent_context_fork,
    maybe_bump_codex_review_phase_for_compact_findings, review_gate_armed, review_gate_blocks_stop,
    review_gate_satisfied, review_independent_fork, review_independent_reviewer_evidence,
    CursorReviewGateMode, ReviewGateFacts,
};
pub use review_output_lint::{
    assistant_has_substantive_compact_review_finding_line, lint_review_output, LintFinding,
    LintSeverity,
};
pub use subagent::{is_subagent_tool, SUBAGENT_TOOL_NAMES};
pub use review_routing_signals::{
    parallel_review_candidate_markers, review_gate_compiled_regexes,
    ParallelReviewCandidateMarkers,
};
