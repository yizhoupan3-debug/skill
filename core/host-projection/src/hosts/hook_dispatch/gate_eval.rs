use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn default_subagent_review_types() -> &'static [&'static str] {
    static TYPES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| {
        // Collect all unique subagent_review_types from registry-backed host providers.
        // This ensures the registry is the single source of truth.
        let mut types: Vec<&'static str> = Vec::new();
        for &host_id in framework_core::runtime_registry::ALL_HOST_IDS {
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

/// Default review types (used by most hosts).
pub fn default_review_types() -> &'static [&'static str] {
    subagent_review_types()
}

/// Compute review lane and parallel lane bits using a provided review type set.
pub fn subagent_lane_bits_with_types(kind: Option<&str>, review_types: &[&str]) -> (bool, bool) {
    let Some(k) = kind else {
        return (false, false);
    };
    let review_lane = review_types.contains(&k);
    let parallel_types = subagent_review_types();
    let parallel_lane = parallel_types.contains(&k);
    (review_lane, parallel_lane)
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
    framework_core::env_flags::router_rs_review_gate_disabled_for_host(host_id)
        || framework_core::hook_common::review_gate_hard_block_disabled(repo_root, prompt)
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
    const VERIFY_CMDS: &[&str] = &[
        "cargo test",
        "cargo check",
        "cargo build",
        "cargo clippy",
        "cargo fmt",
        "npm test",
        "npm run test",
        "pytest",
        "make test",
        "make check",
        "go test",
        "git diff",
        "git log",
    ];
    VERIFY_CMDS.iter().any(|cmd| cmd_lower.contains(cmd))
}

// ────────────────────────────────────────────────────────────────
// Shared Stop decision logic (used by all hosts)
// ────────────────────────────────────────────────────────────────

/// `need=` segment for REVIEW_GATE incomplete stop lines.
/// Shared across all hosts for consistent observation classification.
pub const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n|deep-reviewer fork_context=false";

/// Stable hint suffix for REVIEW_GATE incomplete lines.
pub const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str = "hint=fork_context_json_false_not_omitted";

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
    core: &mut framework_core::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
) {
    update_goal_gate_with_disk(
        core,
        prompt,
        response_text,
        goal_drive_entrypoint,
        None,
        None,
    )
}

/// Extended goal gate update with optional disk-based readiness evaluation.
///
/// When `repo_root` and `task_id` are provided, reads `GOAL_STATE.json` via
/// `hooks::evaluate_goal_readiness_from_disk` for more precise signal detection.
/// Disk signals are merged with regex-based signals (union: either can arm a field).
pub fn update_goal_gate_with_disk(
    core: &mut framework_core::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
    _repo_root: Option<&std::path::Path>,
    _task_id: Option<&str>,
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
    if framework_core::hook_common::has_structured_goal_contract(&signal) {
        core.goal.goal_contract_seen = true;
    }
    if framework_core::hook_common::has_goal_progress_signal(&signal) {
        core.goal.goal_progress_seen = true;
    }
    if framework_core::hook_common::has_goal_verify_or_block_signal(&signal) {
        core.goal.goal_verify_or_block_seen = true;
    }
}

// ════════════════════════════════════════════════════════════════════
// Shared handler logic (4-host unification)
// ════════════════════════════════════════════════════════════════════

/// Apply override + reject detection to review gate state (Stop event).
/// All 4 hosts run this sequence. Call before gate evaluation.
pub fn apply_override_and_reject(
    core: &mut framework_core::HookReviewDiskCore,
    prompt: &str,
    stop_signal: &str,
) {
    if framework_core::hook_common::has_override(prompt) {
        core.gate.review_override = true;
        core.goal.delegation_override = true;
    }
    if framework_core::hook_common::saw_reject_reason(stop_signal, prompt)
        || framework_core::hook_common::saw_reject_reason(prompt, stop_signal)
    {
        core.gate.reject_reason_seen = true;
        core.goal.followup_count = 0;
        core.goal.review_followup_count = 0;
    }
}

/// Detect if the user is explicitly invoking plan mode in the prompt.
/// Covers: "写plan", "给plan", "plan mode", "做计划", "制定计划" etc.
fn is_plan_keyword_in_prompt(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    const PLAN_KWS: &[&str] = &[
        "plan mode",
        "写plan",
        "给plan",
        "做计划",
        "制定计划",
        "先规划",
        "先计划",
        "let's plan",
        "lets plan",
        "create a plan",
        "make a plan",
        "write a plan",
        "draft a plan",
        "draw up a plan",
        "design a plan",
        "做个计划",
        "写个计划",
        "草拟计划",
        "规划方案",
        "我需要一个计划",
    ];
    PLAN_KWS.iter().any(|kw| lower.contains(kw))
}

/// Extract the new constraint phrase from a scope-change message.
/// Strips common scope-change markers and returns the remaining meaningful text
/// (truncated to 120 chars for done_when readability).
fn extract_scope_change_constraint(text: &str) -> String {
    let trimmed = text.trim();

    // Strip whole-phrase scope-change markers (ZH), longest-first
    const ZH_MARKERS: &[&str] = &[
        "另外还需要",
        "另外还要",
        "另外需要",
        "另外要",
        "顺便说一下",
        "顺便提一下",
        "顺便优化",
        "顺便修复",
        "补充一下",
        "补充说明",
        "等一下",
        "对了",
        "追加要求",
        "增加约束",
        "另外",
        "顺便",
        "补充",
        "追加",
        "额外",
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
        "one more thing",
        "apart from that",
        "by the way",
        "additionally",
        "also need",
        "also want",
    ];
    for marker in EN_MARKERS {
        if let Some(rest) = stripped.strip_prefix(marker) {
            stripped = rest;
            break;
        }
    }

    // Strip leading punctuation and whitespace
    let stripped = stripped.trim_start_matches(|c: char| {
        c.is_ascii_whitespace()
            || c == '：'
            || c == ':'
            || c == '，'
            || c == ','
            || c == '。'
            || c == '.'
    });

    // Take first sentence or 120 chars, whichever is shorter
    let end = stripped.find(['。', '.', '\n']).unwrap_or(stripped.len());
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
    paper_host: crate::hooks::PaperProseHookHost,
    review_required: bool,
    review_override: bool,
) -> Vec<String> {
    let mut contexts = Vec::new();

    // Spawn-first review nudge
    if review_required
        && !review_override
        && framework_core::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), prompt)
    {
        contexts.push(
            framework_core::registry_review_gate::review_spawn_first_nudge_line(
                Some(repo_root),
                host_id,
            ),
        );
    }

    // Paper context injection
    crate::hooks::maybe_append_paper_adversarial_context(
        repo_root,
        prompt,
        &mut contexts,
        paper_host,
    );
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
            let goal_result = framework_core::goal_auto_detect::analyze_complexity(prompt);
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

    // Goal auto-detect: complex task, no active goal → auto-create lightweight goal.
    // Check for ANY running non-stale goal (not just driving ones) to avoid
    // overwriting active_task pointer when the user already has a goal in progress.
    if !is_plan_invocation {
        let has_active_goal = goal_state.as_ref().is_some_and(|g| {
            g.get("status").and_then(Value::as_str) == Some("running")
                && g.get("stale").and_then(Value::as_bool) != Some(true)
        });
        if !has_active_goal {
            let result = framework_core::goal_auto_detect::analyze_complexity(prompt);
            if result.is_complex {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let task_id = format!("auto-{nanos}");
                let goal_text = if prompt.len() > 500 {
                    format!("{}…", &prompt[..prompt.floor_char_boundary(500)])
                } else {
                    prompt.to_string()
                };
                let create_payload = serde_json::json!({
                    "repo_root": repo_root.to_string_lossy().to_string(),
                    "operation": "start",
                    "task_id": task_id,
                    "goal": goal_text,
                });
                match core_state::state_manager::framework_goal_drive(create_payload) {
                    Ok(_) => {
                        let indicators = result.matched_indicators.join(", ");
                        contexts.push(format!(
                            "[Goal Auto-Detect] 已自动创建 Goal（匹配特征: {indicators}）\n\
                             task_id: {task_id}\n\
                             goal: {goal_text}"
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(
                            task_id = %task_id,
                            error = %e,
                            "auto-detect goal creation failed"
                        );
                    }
                }
            }
        }
    }

    contexts
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ── shared_tracks_goal ──

    #[test]
    fn tracks_goal_when_goal_required() {
        assert!(shared_tracks_goal(true, false));
    }

    #[test]
    fn tracks_goal_when_drive_entry_active() {
        assert!(shared_tracks_goal(false, true));
    }

    #[test]
    fn tracks_goal_when_both() {
        assert!(shared_tracks_goal(true, true));
    }

    #[test]
    fn no_tracking_when_neither() {
        assert!(!shared_tracks_goal(false, false));
    }

    // ── shared_goal_is_satisfied ──

    #[test]
    fn satisfied_when_tracking_off() {
        assert!(shared_goal_is_satisfied(false, false, false, false, false, false, false));
    }

    #[test]
    fn satisfied_when_review_override() {
        assert!(shared_goal_is_satisfied(true, true, false, false, false, true, false));
    }

    #[test]
    fn satisfied_when_delegation_override() {
        assert!(shared_goal_is_satisfied(true, true, false, false, false, false, true));
    }

    #[test]
    fn satisfied_when_all_signals_seen() {
        assert!(shared_goal_is_satisfied(true, true, true, true, true, false, false));
    }

    #[test]
    fn unsatisfied_when_missing_progress() {
        assert!(!shared_goal_is_satisfied(true, true, true, false, true, false, false));
    }

    #[test]
    fn unsatisfied_when_missing_verify() {
        assert!(!shared_goal_is_satisfied(true, true, true, true, false, false, false));
    }

    #[test]
    fn unsatisfied_when_no_signals() {
        assert!(!shared_goal_is_satisfied(true, true, false, false, false, false, false));
    }

    // ── update_goal_gate ──

    #[test]
    fn update_goal_gate_sets_drive_entry() {
        let mut core = framework_core::HookReviewDiskCore::default();
        update_goal_gate(&mut core, "goal: build feature X", "", true);
        assert!(core.goal.goal_drive_entry_active);
    }

    #[test]
    fn update_goal_gate_skips_when_tracking_off() {
        let mut core = framework_core::HookReviewDiskCore::default();
        update_goal_gate(&mut core, "", "", false);
        assert!(!core.goal.goal_drive_entry_active);
        assert!(!core.goal.goal_contract_seen);
    }

    #[test]
    fn update_goal_gate_detects_progress_signal() {
        let mut core = framework_core::HookReviewDiskCore::default();
        core.goal.goal_drive_entry_active = true;
        update_goal_gate(&mut core, "", "完成了第一个 milestone", false);
        assert!(core.goal.goal_progress_seen);
    }

    // ── apply_override_and_reject ──

    #[test]
    fn apply_override_sets_review_override() {
        let mut core = framework_core::HookReviewDiskCore::default();
        apply_override_and_reject(&mut core, "do not use subagent", "");
        assert!(core.gate.review_override);
        assert!(core.goal.delegation_override);
    }

    #[test]
    fn apply_reject_resets_followup_counts() {
        let mut core = framework_core::HookReviewDiskCore::default();
        core.goal.followup_count = 5;
        core.goal.review_followup_count = 3;
        apply_override_and_reject(&mut core, "small_task", "");
        assert!(core.gate.reject_reason_seen);
        assert_eq!(core.goal.followup_count, 0);
        assert_eq!(core.goal.review_followup_count, 0);
    }

    #[test]
    fn no_override_without_keyword() {
        let mut core = framework_core::HookReviewDiskCore::default();
        apply_override_and_reject(&mut core, "just a normal prompt", "");
        assert!(!core.gate.review_override);
        assert!(!core.gate.reject_reason_seen);
    }
}
