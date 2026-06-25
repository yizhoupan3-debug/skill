use serde_json::Value;
use std::path::Path;

fn default_subagent_review_types() -> &'static [&'static str] {
    static TYPES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| {
        // Collect all unique subagent_review_types from registry-backed host providers.
        // This ensures the registry is the single source of truth.
        let mut types: Vec<&'static str> = Vec::new();
        for &host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            if let Some(provider) = crate::hosts::host_provider_for_id(host_id) {
                for t in provider.subagent_review_types() {
                    if !types.contains(t) {
                        types.push(t);
                    }
                }
            }
        }
        if types.is_empty() {
            // Registry unavailable — no review types defined.
        }
        types
    })
}

/// Recognized subagent type names for review gate tracking.
pub fn subagent_review_types() -> &'static [&'static str] {
    default_subagent_review_types()
}

/// Extract and normalize subagent type from tool input fields.
pub fn recognize_subagent_type(tool_input: &Value) -> Option<String> {
    use core_policy::hook_common::normalize_subagent_type;
    let review_types = subagent_review_types();
    let typed_fields = [
        tool_input.get("subagent_type").and_then(Value::as_str),
        tool_input.get("agent_type").and_then(Value::as_str),
        tool_input.get("agentType").and_then(Value::as_str),
        tool_input.get("type").and_then(Value::as_str),
    ];
    typed_fields
        .into_iter()
        .map(|field| normalize_subagent_type(field))
        .find(|normalized| review_types.contains(&normalized.as_str()))
}

/// Compute review lane and parallel lane bits from subagent kind.
pub fn subagent_lane_bits(kind: Option<&str>) -> (bool, bool) {
    let Some(k) = kind else {
        return (false, false);
    };
    let review_types = subagent_review_types();
    let review_lane = review_types.contains(&k);
    let parallel_lane = matches!(k, "general-purpose" | "deep-review-agent");
    (review_lane, parallel_lane)
}

/// Default review types (used by most hosts).
pub fn default_review_types() -> &'static [&'static str] {
    subagent_review_types()
}

/// Extended review types (includes worker/shell variants for hosts that use them).
pub fn extended_review_types() -> &'static [&'static str] {
    static EXTENDED: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    EXTENDED.get_or_init(|| {
        let mut types: Vec<&'static str> = subagent_review_types().to_vec();
        for t in &[
            "generalpurpose", "default", "shell", "worker",
            "browser-use", "browseruse", "ci-investigator", "ciinvestigator",
            "best-of-n-runner", "bestofnrunner", "cursor-guide", "cursorguide",
        ] {
            if !types.contains(t) {
                types.push(t);
            }
        }
        types
    })
}

/// Compute review lane and parallel lane bits using a provided review type set.
pub fn subagent_lane_bits_with_types(kind: Option<&str>, review_types: &[&str]) -> (bool, bool) {
    let Some(k) = kind else { return (false, false); };
    let review_lane = review_types.contains(&k);
    let parallel_types = subagent_review_types();
    let parallel_lane = parallel_types.contains(&k);
    (review_lane, parallel_lane)
}

/// Host-aware subagent lane bits. Uses host-specific review type set.
pub fn subagent_lane_bits_for_host(kind: Option<&str>, host_id: &str) -> (bool, bool) {
    let review_types = crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.subagent_review_types())
        .unwrap_or_else(default_review_types);
    subagent_lane_bits_with_types(kind, review_types)
}

/// Truncate string preserving UTF-8 character boundaries, with optional suffix.
pub fn truncate_bytes(s: &str, max_bytes: usize, suffix: &str) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let suffix_len = suffix.len();
    let target = max_bytes.saturating_sub(suffix_len);
    let mut end = target;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}", &s[..end])
}

/// Compact multiple context parts: dedup + join + truncate with suffix.
pub fn compact_contexts(parts: Vec<String>, max_bytes: usize) -> Option<String> {
    compact_contexts_with_suffix(parts, max_bytes, "...")
}

/// Compact multiple context parts with configurable truncation suffix.
pub fn compact_contexts_with_suffix(
    parts: Vec<String>,
    max_bytes: usize,
    suffix: &str,
) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<&str> = parts
        .iter()
        .filter(|p| !p.is_empty() && seen.insert(p.as_str()))
        .map(|s| s.as_str())
        .collect();
    if deduped.is_empty() {
        return None;
    }
    let combined = deduped.join("\n");
    Some(truncate_bytes(&combined, max_bytes, suffix))
}

/// Check if review gate is suppressed for this host/prompt combination.
pub fn is_review_gate_suppressed(host_id: &str, repo_root: Option<&Path>, prompt: &str) -> bool {
    core_policy::env_flags::router_rs_review_gate_disabled_for_host(host_id)
        || core_policy::hook_common::review_gate_hard_block_disabled(repo_root, prompt)
}

pub fn is_verification_command(tool_name: &str, command: &str) -> bool {
    let name_lower = tool_name.to_ascii_lowercase();
    if !name_lower.contains("bash")
        && !name_lower.contains("shell")
        && !name_lower.contains("exec")
        && !name_lower.contains("terminal")
    {
        return false;
    }
    let cmd_lower = command.to_ascii_lowercase();
    cmd_lower.contains("cargo test")
        || cmd_lower.contains("cargo check")
        || cmd_lower.contains("cargo build")
        || cmd_lower.contains("cargo clippy")
        || cmd_lower.contains("cargo fmt")
        || cmd_lower.contains("npm test")
        || cmd_lower.contains("npm run test")
        || cmd_lower.contains("pytest")
        || cmd_lower.contains("make test")
        || cmd_lower.contains("make check")
        || cmd_lower.contains("go test")
        || cmd_lower.contains("git diff")
        || cmd_lower.contains("git log")
}

// ────────────────────────────────────────────────────────────────
// Shared Stop decision logic (used by all hosts)
// ────────────────────────────────────────────────────────────────

/// `need=` segment for REVIEW_GATE incomplete stop lines.
/// Shared across all hosts for consistent observation classification.
pub const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n|deep-reviewer fork_context=false";

/// Stable hint suffix for REVIEW_GATE incomplete lines.
pub const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

/// `merge_hook_nudge_paragraph` dedup prefix for REVIEW_GATE detail.
pub const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

/// Check if goal tracking is active for this state.
/// Shared: tracks whether `goal_required` or `goal_drive_entry_active` is set.
pub fn shared_tracks_goal(goal_required: bool, goal_drive_entry_active: bool) -> bool {
    goal_required || goal_drive_entry_active
}

/// Check if the goal gate is satisfied.
/// Shared decision logic: goal is satisfied when:
/// 1. Goal tracking is not active, OR
/// 2. Override is in effect, OR
/// 3. All three signals (contract, progress, verify) are seen.
pub fn shared_goal_is_satisfied(
    goal_required: bool,
    goal_drive_entry_active: bool,
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    review_override: bool,
    delegation_override: bool,
) -> bool {
    if !shared_tracks_goal(goal_required, goal_drive_entry_active) {
        return true;
    }
    if review_override || delegation_override {
        return true;
    }
    goal_contract_seen && goal_progress_seen && goal_verify_or_block_seen
}

/// Check if review output lint should be suppressed during Stop.
/// Shared: skip lint when review gate or goal followup is active.
pub fn shared_stop_review_output_lint_suppressed(
    review_advisory_needed: bool,
    goal_required: bool,
    goal_drive_entry_active: bool,
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    review_override: bool,
    delegation_override: bool,
) -> bool {
    if review_advisory_needed {
        return true;
    }
    if shared_tracks_goal(goal_required, goal_drive_entry_active)
        && !shared_goal_is_satisfied(
            goal_required,
            goal_drive_entry_active,
            goal_contract_seen,
            goal_progress_seen,
            goal_verify_or_block_seen,
            review_override,
            delegation_override,
        )
    {
        return true;
    }
    false
}

/// Unified goal gate update — **single implementation for all 4 hosts**.
///
/// Call this from each host's Stop/PostTool handler. It:
/// 1. Detects goal drive entry from prompt
/// 2. Detects goal signals from response text (contract / progress / verify)
/// 3. Optionally reads disk state via `hooks::evaluate_goal_readiness_from_disk` (more precise)
/// 4. Updates `HookReviewDiskCore` fields in-place
///
/// Hosts should pass their `review_state.core` (or equivalent `HookReviewDiskCore`).
pub fn update_goal_gate(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
) {
    update_goal_gate_with_disk(core, prompt, response_text, goal_drive_entrypoint, None, None)
}

/// Extended goal gate update with optional disk-based readiness evaluation.
///
/// When `repo_root` and `task_id` are provided, reads `GOAL_STATE.json` via
/// `hooks::evaluate_goal_readiness_from_disk` for more precise signal detection.
/// Disk signals are merged with regex-based signals (union: either can arm a field).
pub fn update_goal_gate_with_disk(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
    repo_root: Option<&std::path::Path>,
    task_id: Option<&str>,
) {
    // Arm goal drive on entry
    if goal_drive_entrypoint {
        core.goal.goal_drive_entry_active = true;
    }
    // Only scan for signals if goal tracking is active
    if !core.goal.goal_drive_entry_active {
        return;
    }
    // Scan combined signal text for goal signals (regex-based, all hosts)
    let signal = if prompt.is_empty() {
        response_text.to_string()
    } else if response_text.is_empty() {
        prompt.to_string()
    } else {
        format!("{prompt}\n{response_text}")
    };
    if core_policy::hook_common::has_structured_goal_contract(&signal) {
        core.goal.goal_contract_seen = true;
    }
    if core_policy::hook_common::has_goal_progress_signal(&signal) {
        core.goal.goal_progress_seen = true;
    }
    if core_policy::hook_common::has_goal_verify_or_block_signal(&signal) {
        core.goal.goal_verify_or_block_seen = true;
    }
    // Disk-based readiness (more precise: reads GOAL_STATE.json + EVIDENCE_INDEX.json)
    if let (Some(root), Some(tid)) = (repo_root, task_id) {
        let goal_val = serde_json::Value::Null; // placeholder; real evaluator reads disk
        let readiness = crate::hooks::evaluate_goal_readiness_from_disk(root, &goal_val, tid);
        if readiness.contract {
            core.goal.goal_contract_seen = true;
        }
        if readiness.progress {
            core.goal.goal_progress_seen = true;
        }
        if readiness.verification {
            core.goal.goal_verify_or_block_seen = true;
        }
    }
}

/// Check if goal gate is satisfied using shared `HookReviewDiskCore` fields.
pub fn goal_gate_satisfied(core: &core_policy::HookReviewDiskCore) -> bool {
    shared_goal_is_satisfied(
        false, // goal_required is Cursor-specific; shared uses goal_drive_entry_active
        core.goal.goal_drive_entry_active,
        core.goal.goal_contract_seen,
        core.goal.goal_progress_seen,
        core.goal.goal_verify_or_block_seen,
        core.gate.review_override,
        core.goal.delegation_override,
    )
}

/// Generate the goal stop followup line using shared logic.
/// Phase-aware: includes short code for goal drive continuation.
pub fn shared_goal_stop_followup_line(
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    goal_followup_count: u32,
) -> String {
    let missing = {
        let mut m = Vec::new();
        if !goal_contract_seen {
            m.push("contract");
        }
        if !goal_progress_seen {
            m.push("progress");
        }
        if !goal_verify_or_block_seen {
            m.push("verify");
        }
        m.join(",")
    };
    format!(
        "router-rs GOAL_FOLLOWUP missing={} nudge={}",
        missing, goal_followup_count
    )
}

/// Shared advisory for settings changed but not validated.
pub fn shared_settings_validation_advisory() -> String {
    "Validate Claude hook/settings JSON before ending this turn.".to_string()
}

/// Shared advisory for framework source changed but not tested.
pub fn shared_framework_test_advisory() -> String {
    "Framework source files were modified. Consider running tests.".to_string()
}

// ════════════════════════════════════════════════════════════════════
// Shared handler logic (4-host unification)
// ════════════════════════════════════════════════════════════════════

/// Record tool call telemetry + session tracking (PostToolUse).
/// All 4 hosts emit the same 2-line sequence; call once after extracting tool_name + duration.
pub fn record_tool_call_emission(repo_root: &Path, tool_name: &str, duration_ms: u64, succeeded: bool) {
    telemetry_emit::emit_tool_call(tool_name, duration_ms, succeeded);
    if let Err(e) = crate::hooks::record_tool_call(repo_root, tool_name, None) {
        tracing::warn!("session tracker record_tool_call failed (non-fatal): {e}");
    }
}

/// Merge review gate state on UserPromptSubmit (pure logic, no I/O).
///
/// All 4 hosts share this core sequence:
/// 1. interactive / goal_drive / narrow → suppress review (clear `review_required` + `independent_reviewer_seen`)
/// 2. review_arms && !override_now → clear `independent_reviewer_seen` (fresh cycle)
/// 3. Accumulate: `review_required = prev || review_arms`, `review_override = prev || override_now`
///
/// Returns the updated `HookReviewDiskCore` and flags for the caller:
/// - `review_required` (post-merge): whether review gate is armed
/// - `review_override` (post-merge): whether override is active
/// - `fresh_cycle`: whether a new review cycle was armed this call
pub struct ReviewGateMergeResult {
    pub core: core_policy::HookReviewDiskCore,
    pub review_arms: bool,
    pub override_now: bool,
    pub fresh_cycle: bool,
    pub suppressed: bool,
}

pub fn merge_review_gate_on_user_prompt(
    prev: &core_policy::HookReviewDiskCore,
    prompt: &str,
    repo_root: &Path,
    host_id: &str,
) -> ReviewGateMergeResult {
    let suppressed = is_review_gate_suppressed(host_id, Some(repo_root), prompt);
    if suppressed {
        return ReviewGateMergeResult {
            core: prev.clone(),
            review_arms: false,
            override_now: false,
            fresh_cycle: false,
            suppressed: true,
        };
    }

    let task_profile = core_policy::hook_common::is_task_profile(Some(repo_root), prompt);
    let goal_drive = false;
    let narrow = core_policy::hook_common::is_narrow_review_prompt(prompt);
    let review_arms = core_policy::hook_common::is_review_prompt(prompt);
    let override_now = core_policy::hook_common::has_override(prompt);

    let mut core = prev.clone();

    if task_profile || goal_drive || narrow {
        core.gate.review_required = false;
        core.gate.independent_reviewer_seen = false;
    } else {
        if review_arms && !override_now {
            core.gate.independent_reviewer_seen = false;
        }
        core.gate.review_required = core.gate.review_required || review_arms;
    }
    core.gate.review_override = core.gate.review_override || override_now;

    let fresh_cycle = review_arms && !override_now && !task_profile && !goal_drive && !narrow;

    ReviewGateMergeResult {
        core,
        review_arms,
        override_now,
        fresh_cycle,
        suppressed: false,
    }
}

/// Apply override + reject detection to review gate state (Stop event).
/// All 4 hosts run this sequence. Call before gate evaluation.
pub fn apply_override_and_reject(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    stop_signal: &str,
) {
    if core_policy::hook_common::has_override(prompt) {
        core.gate.review_override = true;
        core.goal.delegation_override = true;
    }
    if core_policy::hook_common::saw_reject_reason(stop_signal, prompt)
        || core_policy::hook_common::saw_reject_reason(prompt, stop_signal)
    {
        core.gate.reject_reason_seen = true;
        core.goal.followup_count = 0;
        core.goal.review_followup_count = 0;
    }
}

/// Stop decision enum — returned by `evaluate_stop_decision`.
pub enum StopDecision {
    /// Closeout advisory (unmet closeout evidence).
    Closeout { message: String },
    /// Review gate nudge (unmet review requirements).
    ReviewGateNudge { message: String },
    /// Goal followup nudge (unmet goal contract/progress/verify).
    GoalFollowup { message: String },
    /// All gates satisfied — safe to stop.
    Clean,
}

/// Evaluate the full stop decision sequence (pure logic, no I/O).
///
/// All 4 hosts run the same pipeline:
/// 1. Closeout check → advisory
/// 2. Override + reject detection
/// 3. Goal gate update (via `update_goal_gate`)
/// 4. Review gate check → advisory nudge
/// 5. Goal followup check → advisory nudge
/// 6. Clean
///
/// Callers must pass mutable `HookReviewDiskCore` (override/reject mutations applied in-place).
pub fn evaluate_stop_decision(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    stop_signal: &str,
    completion_text: &str,
    repo_root: &Path,
    host_id: &str,
) -> StopDecision {
    // 1. Closeout
    if let Some(msg) = crate::hooks::closeout_stop_followup_for_completion_text(repo_root, completion_text) {
        return StopDecision::Closeout { message: msg };
    }

    // 2. Override + reject
    apply_override_and_reject(core, prompt, stop_signal);

    // 3. Goal gate update
    let goal_entry = false;
    update_goal_gate(core, prompt, response_text, goal_entry);

    // 4. Review gate
    if let Some(nudge) = core_policy::hook_review_stop_advisory_needed(
        &core.gate,
        &format!("{}_REVIEW_GATE", host_id.to_ascii_uppercase()),
    ) {
        return StopDecision::ReviewGateNudge { message: nudge };
    }

    // 5. Goal followup
    if !goal_gate_satisfied(core) {
        let followup = shared_goal_stop_followup_line(
            core.goal.goal_contract_seen,
            core.goal.goal_progress_seen,
            core.goal.goal_verify_or_block_seen,
            core.goal.goal_followup_count,
        );
        return StopDecision::GoalFollowup { message: followup };
    }

    StopDecision::Clean
}

/// Detect if the user is explicitly invoking plan mode in the prompt.
/// Covers: "写plan", "给plan", "plan mode", "做计划", "制定计划" etc.
fn is_plan_keyword_in_prompt(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("plan mode")
        || lower.contains("写plan")
        || lower.contains("给plan")
        || lower.contains("做计划")
        || lower.contains("制定计划")
        || lower.contains("先规划")
        || lower.contains("先计划")
        || lower.contains("let's plan")
        || lower.contains("lets plan")
        // Additional English plan invocations
        || lower.contains("create a plan")
        || lower.contains("make a plan")
        || lower.contains("write a plan")
        || lower.contains("draft a plan")
        || lower.contains("draw up a plan")
        || lower.contains("design a plan")
        // Additional Chinese plan invocations
        || lower.contains("做个计划")
        || lower.contains("写个计划")
        || lower.contains("草拟计划")
        || lower.contains("规划方案")
        || lower.contains("我需要一个计划")
}

/// Extract the new constraint phrase from a scope-change message.
/// Strips common scope-change markers and returns the remaining meaningful text
/// (truncated to 120 chars for done_when readability).
fn extract_scope_change_constraint(text: &str) -> String {
    let trimmed = text.trim();

    // Strip whole-phrase scope-change markers (ZH), longest-first
    const ZH_MARKERS: &[&str] = &[
        "另外还需要", "另外还要", "另外需要", "另外要",
        "顺便说一下", "顺便提一下", "顺便优化", "顺便修复",
        "补充一下", "补充说明",
        "等一下", "对了",
        "追加要求", "增加约束",
        "另外", "顺便", "补充", "追加", "额外",
    ];
    let mut stripped = trimmed;
    for marker in ZH_MARKERS {
        if let Some(rest) = stripped.strip_prefix(marker) {
            stripped = rest;
            break;
        }
    }

    // Strip English markers, longest-first
    const EN_MARKERS: &[&str] = &[
        "one more thing", "apart from that", "by the way",
        "additionally", "also need", "also want",
    ];
    for marker in EN_MARKERS {
        if let Some(rest) = stripped.strip_prefix(marker) {
            stripped = rest;
            break;
        }
    }

    // Strip leading punctuation and whitespace
    let stripped = stripped
        .trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '：' || c == ':' || c == '，' || c == ',' || c == '。' || c == '.');

    // Take first sentence or 120 chars, whichever is shorter
    let end = stripped
        .find(['。', '.', '\n'])
        .unwrap_or(stripped.len());
    let end = end.min(120);
    // Use floor_char_boundary to avoid splitting UTF-8 multi-byte characters
    let safe_end = stripped.floor_char_boundary(end);
    let result = stripped[..safe_end].trim();
    result.to_string()
}

/// Build UserPromptSubmit additional context (spawn-first nudge + paper context).
/// All 4 hosts inject the same context sequence. Returns empty vec if nothing to inject.
pub fn build_user_prompt_context_injection(
    repo_root: &Path,
    prompt: &str,
    host_id: &str,
    paper_host: crate::hooks::PaperProseHookHostType,
    review_required: bool,
    review_override: bool,
) -> Vec<String> {
    let mut contexts = Vec::new();

    // Spawn-first review nudge
    if review_required && !review_override
        && core_policy::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), prompt) {
            contexts.push(core_policy::registry_review_gate::review_spawn_first_nudge_line(
                Some(repo_root),
                host_id,
            ));
        }

    // Paper context injection
    crate::hooks::maybe_append_paper_adversarial_context(repo_root, prompt, &mut contexts, paper_host);
    crate::hooks::maybe_append_paper_prose_context(repo_root, prompt, &mut contexts, paper_host);

    // Auto-amend: when scope change is detected and there's an active goal,
    // append the new constraint to done_when so the model incorporates it.
    // SKIP if user is invoking plan mode (plan and goal are mutually exclusive).
    let is_plan_invocation = is_plan_keyword_in_prompt(prompt);

    // Read goal state once, share between auto-amend and auto-detect blocks.
    // Err on I/O is non-fatal — treat as "no state" to avoid a transient error
    // suppressing auto-detect for all prompts (worst case: harmless extra context).
    let goal_state: Option<Value> = if !is_plan_invocation {
        match core_state::state_manager::read_goal_state(repo_root, None) {
            Ok(state) => state,
            Err(_) => None,
        }
    } else {
        None
    };

    if let Some(ref goal) = goal_state {
        let goal_running = goal.get("status").and_then(Value::as_str) == Some("running");
        let goal_driving = goal.get("drive_until_done").and_then(Value::as_bool) == Some(true);
        let not_stale = goal.get("stale").and_then(Value::as_bool) != Some(true);
        if goal_running && goal_driving && not_stale {
            let goal_result = core_policy::goal_auto_detect::analyze_complexity(prompt);
            if goal_result.is_scope_change {
                let constraint = extract_scope_change_constraint(prompt);
                if !constraint.is_empty() {
                    let task_id = goal.get("task_id").and_then(Value::as_str).unwrap_or("");
                    if !task_id.is_empty() {
                        let mut done_when: Vec<String> = goal
                            .get("done_when")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !done_when.iter().any(|d| d == &constraint) {
                            done_when.push(constraint.clone());
                            let done_when_values: Vec<Value> =
                                done_when.iter().map(|s| Value::String(s.clone())).collect();
                            let amend_payload = serde_json::json!({
                                "repo_root": repo_root.to_string_lossy().to_string(),
                                "operation": "amend",
                                "task_id": task_id,
                                "done_when": done_when_values,
                            });
                            let _ = core_state::state_manager::framework_goal_drive(amend_payload);
                            contexts.push(format!(
                                "[Goal Amendment] 已自动追加完成条件: 「{constraint}」"
                            ));
                        }
                    }
                }
            }
        }
    }

    // Goal auto-detect: complex task, no active goal → inject set_goal context.
    if !is_plan_invocation {
        let has_active_goal = goal_state.as_ref().is_some_and(|g| {
            g.get("status").and_then(Value::as_str) == Some("running")
                && g.get("drive_until_done").and_then(Value::as_bool) == Some(true)
                && g.get("stale").and_then(Value::as_bool) != Some(true)
        });
        if !has_active_goal {
            let result = core_policy::goal_auto_detect::analyze_complexity(prompt);
            if result.is_complex {
                let indicators = result.matched_indicators.join(", ");
                contexts.push(format!(
                    "[Goal Auto-Detect] 检测到复杂任务（匹配特征: {indicators}），当前无活跃 Goal 契约。\n\
                     请执行 set_goal 流程：\n\
                     ① 调研分析任务范围与约束（允许搜索相关代码、文档或外部信息）\n\
                     ② 提炼结构化 Goal 契约：\n\
                     - Goal：不是复述原话，而是分析后提取的核心目标\n\
                     - Non-goals：明确不做什么\n\
                     - Done when：可验证的完成条件列表\n\
                     - Validation commands：验证命令\n\
                     ③ 调用 goal_state_manage(operation=start, task_id=<task_id>, \
                     goal=<goal>, done_when=[...], non_goals=[...], \
                     validation_commands=[...])\n\
                     （请将 <task_id> 等替换为实际值）\n\
                     创建后回复用户当前 Goal 状态。"
                ));
            }
        }
    }

    contexts
}

/// Detect reviewer evidence from PostToolUse (fork_context + review lane).
/// Returns true if independent_reviewer_seen should be armed.
/// All 4 hosts run this same detection after subagent type recognition.
pub fn detect_reviewer_evidence(
    tool_input: &Value,
    reviewer_lane: bool,
) -> bool {
    if !reviewer_lane {
        return false;
    }
    let fork = extract_fork_context(tool_input);
    core_policy::review_gate_engine::review_independent_reviewer_evidence(fork, reviewer_lane)
}

/// Extract fork_context from tool input (tries multiple field names).
/// Delegates to `fork_context_from_values` from core-policy (single source of truth).
/// Returns `None` if field is absent or unparseable (Claude semantics: absent ≠ false).
fn extract_fork_context(tool_input: &Value) -> Option<bool> {
    core_policy::review_gate_engine::fork_context_from_values(tool_input, None)
}
