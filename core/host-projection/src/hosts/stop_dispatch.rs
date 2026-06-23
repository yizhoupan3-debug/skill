//! Unified Stop hook decision pipeline (ADR §2.1).
//!
//! All hosts share the same Stop logic pipeline. Host-specific differences
//! (file paths, env vars, signal text) are injected via the `StopHostOps` trait.
//!
//! ## Pipeline (shared across all 4 hosts)
//! 1. Bootstrap kernel
//! 2. Host-specific pre-stop cleanup
//! 3. Closeout advisory check (non-blocking)
//! 4. Build host-specific context (paths, session key)
//! 5. Load review gate + touch state from disk
//! 6. Extract prompt, response, stop signal
//! 7. Load review state + apply override + reject detection
//! 8. Goal gate evaluation (shared)
//! 9. Review gate evaluation (shared) + followup tracking
//! 10. Goal followup check (shared)
//! 11. Touch state checks (shared advisory)
//! 12. Conditional state cleanup
//! 13. Review output lint (shared)

use crate::hosts::hook_dispatch;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Host-specific operations needed by the Stop pipeline.
///
/// Each host implements this trait to provide its file paths,
/// env vars, and signal text extraction. The unified Stop handler
/// calls these methods instead of using host-specific match arms.
pub trait StopHostOps {
    /// Host identifier (e.g., "claude", "cursor").
    fn host_id(&self) -> &'static str;

    /// Log label for error messages (e.g., "Claude", "Cursor").
    fn log_label(&self) -> &'static str;

    /// Hook-state base directory relative to repo_root.
    /// Default: `.claude/hook-state` (shared across all hosts for cross-host state continuity).
    fn hook_state_base(&self, repo_root: &Path) -> PathBuf {
        repo_root.join(".claude").join("hook-state")
    }

    /// Session key for state file naming.
    fn session_key(&self, repo_root: &Path, payload: &Value) -> String;

    /// Extract stop signal text from the event payload.
    fn stop_signal_text(&self, payload: &Value) -> String;

    /// Host-specific pre-stop cleanup (e.g., clear skill context).
    /// Return None for no-op.
    fn pre_stop_cleanup(&self, _repo_root: &Path) -> Option<()> {
        None
    }

    /// Hydrate goal gate from disk (GOAL_STATE.json + EVIDENCE_INDEX.json).
    /// Default: no-op. Hosts that support goal drive override this.
    fn hydrate_goal_gate_from_disk(
        &self,
        _repo_root: &Path,
        _state: &mut core_policy::hook_review_disk_state::HookReviewDiskCore,
        _goal_drive_entrypoint: bool,
    ) {
        // no-op default
    }
}

/// Unified Stop hook entry point.
///
/// All hosts call this function with their `StopHostOps` implementation.
pub fn run_unified_stop(
    repo_root: &Path,
    payload: &Value,
    host: &dyn StopHostOps,
) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();

    // ── 1. Host-specific pre-stop cleanup ──
    host.pre_stop_cleanup(repo_root);

    // ── 2. Closeout advisory (non-blocking) ──
    let response_text = hook_dispatch::extract_response_text(payload);
    if let Some(msg) = crate::hooks::closeout_stop_followup_for_completion_text(
        repo_root,
        &response_text,
    ) {
        return add_context("Stop", &format!("[advisory] {msg}"));
    }

    // ── 3. Build context ──
    let key = host.session_key(repo_root, payload);
    let base = host.hook_state_base(repo_root);
    let review_path = base.join(core_policy::hook_review_disk_state::hook_review_subagent_state_basename(&key));
    let touch_path = base.join(format!("hook_state_{key}.json"));

    // ── 4. Load state from disk ──
    let review_load = load_review_gate_disk(&review_path);
    let touch_load = load_touch_state_disk(&touch_path);

    if matches!(review_load, DiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} review_gate state unreadable on Stop: {}",
            host.log_label(),
            review_path.display()
        );
        return add_context(
            "Stop",
            "[advisory] review_gate state unreadable; data will be overwritten on next UserPromptSubmit.",
        );
    }
    if matches!(touch_load, DiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} hook_state unreadable on Stop: {}",
            host.log_label(),
            touch_path.display()
        );
        clear_file(&touch_path);
        return add_context(
            "Stop",
            "[advisory] hook-state unreadable; cleared stale files.",
        );
    }

    // ── 5. Extract prompt, response, stop signal ──
    let stop_signal = host.stop_signal_text(payload);
    let prompt = hook_dispatch::extract_prompt_text(payload);
    let response_for_lint = core_policy::hook_common::hook_assistant_tail_window(
        &response_text,
        core_policy::hook_common::HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    );

    // ── 6. Load review state ──
    let mut review_state = match review_load {
        DiskState::Absent => core_policy::hook_review_disk_state::HookReviewDiskCore::default(),
        DiskState::Ok(s) => s,
        DiskState::Unreadable => unreachable!(),
    };

    // ── 7. Apply override + reject detection (shared) ──
    hook_dispatch::apply_override_and_reject(&mut review_state, &prompt, &stop_signal);

    // ── 8. Goal gate evaluation (shared) ──
    let goal_drive_entrypoint =
        core_policy::hook_common::is_framework_goal_entry_prompt(&prompt);
    hook_dispatch::update_goal_gate(
        &mut review_state,
        &prompt,
        &hook_dispatch::extract_response_text(payload),
        goal_drive_entrypoint,
    );

    // Hydrate goal gate from disk (host-specific)
    host.hydrate_goal_gate_from_disk(repo_root, &mut review_state, goal_drive_entrypoint);

    // ── 9. Review gate check (shared) + followup tracking ──
    let review_suppressed = hook_dispatch::is_review_gate_suppressed(
        host.host_id(),
        Some(repo_root),
        &prompt,
    );
    let gate_fields = review_state.gate_fields();
    let review_advisory_needed = if review_suppressed {
        None
    } else {
        core_policy::hook_review_disk_state::hook_review_stop_advisory_needed(
            &gate_fields,
            &format!("{}_REVIEW_GATE", host.host_id().to_uppercase()),
        )
    };

    if let Some(reason) = &review_advisory_needed {
        review_state.followup_count += 1;
        review_state.review_followup_count += 1;
        let _ = write_review_state(&review_path, &review_state);
        return add_context("Stop", reason);
    }

    // ── 10. Goal followup check (shared) + disk-driven done_when validation ──
    let goal_is_satisfied = review_state.goal_is_satisfied();

    // Try to read GOAL_STATE from disk for done_when validation
    let done_when_coverage = goal_is_satisfied.then(|| {
        // Only run disk validation if the base signals are satisfied
        core_state::state_manager::read_goal_state(repo_root, None).ok().flatten().and_then(|goal| {
            let done_when = goal.get("done_when").and_then(Value::as_array)?;
            if done_when.is_empty() { return None; }
            let total = done_when.len();
            let covered = done_when.iter().filter(|item| {
                item.as_str().map(|s| response_text.contains(s)).unwrap_or(false)
            }).count();
            Some((covered, total, done_when.clone()))
        })
    }).flatten();

    if review_state.tracks_goal() && !goal_is_satisfied {
        review_state.followup_count += 1;
        review_state.goal_followup_count += 1;
        let _ = write_review_state(&review_path, &review_state);
        let message = if let Some((covered, total, items)) = &done_when_coverage {
            let mut msg = format!("Goal not yet satisfied (contract={}, progress={}, verify={}). done_when: {covered}/{total} covered, continue working.",
                review_state.goal_contract_seen,
                review_state.goal_progress_seen,
                review_state.goal_verify_or_block_seen,
            );
            // List uncovered done_when items (up to 3)
            let uncovered: Vec<&str> = items.iter()
                .filter_map(|item| item.as_str())
                .filter(|s| !response_text.contains(s))
                .take(3)
                .collect();
            if !uncovered.is_empty() {
                msg.push_str(&format!(" Still missing: {}", uncovered.join("; ")));
            }
            msg
        } else {
            format!("Goal not yet satisfied (contract={}, progress={}, verify={}). Continue working.",
                review_state.goal_contract_seen,
                review_state.goal_progress_seen,
                review_state.goal_verify_or_block_seen,
            )
        };
        return add_context("Stop", &message);
    }

    // Advisory: even if signals are satisfied, check done_when coverage ≥50%
    if goal_is_satisfied
        && let Some((covered, total, _)) = &done_when_coverage
            && *total > 0 && (*covered as f64 / *total as f64) < 0.5 {
                let message = format!(
                    "Goal signals satisfied but done_when coverage is low ({covered}/{total}). \
                     Verify all completion conditions before completing. Still missing items may remain.",
                );
                return add_context("Stop", &message);
            }

    // ── 10b. Auto-complete detection ──
    // When goal signals are satisfied and all done_when (if any) are 100% covered,
    // suggest completion and optionally probe for next goal.
    if goal_is_satisfied && review_state.tracks_goal() {
        let all_done = done_when_coverage
            .as_ref()
            .map(|(c, t, _)| *t == 0 || *c >= *t)
            .unwrap_or(true);
        if all_done {
            // Resolve task_id via pointer (same strategy used in goal ops)
            let task_id = core_state::state_manager::read_primary_task_id(repo_root)
                .unwrap_or_default();
            if !task_id.is_empty() {
                let complete_payload = serde_json::json!({
                    "repo_root": repo_root.to_string_lossy().to_string(),
                    "operation": "complete",
                    "task_id": task_id,
                });
                if let Err(e) = core_state::state_manager::framework_goal_drive(complete_payload) {
                    tracing::warn!("auto-complete failed: {e}");
                }
            }
            // After completion, check if the conversation describes a new complex task
            let next_goal = core_policy::goal_auto_detect::analyze_complexity(&prompt);
            if next_goal.is_complex {
                return add_context("Stop", "[Goal Suggestion] 前一个任务已完成。检测到新的复杂任务，是否创建新 Goal？");
            }
        }
    }

    // ── 11. Touch state checks (shared advisory) ──
    let touch_state = match touch_load {
        DiskState::Absent => TouchState::default(),
        DiskState::Ok(s) => serde_json::from_value(s).unwrap_or_default(),
        DiskState::Unreadable => unreachable!(),
    };
    if touch_state.settings && !touch_state.settings_validated {
        clear_file(&touch_path);
        return add_context("Stop", "[advisory] settings changed but not validated — run a verification command.");
    }
    if touch_state.framework && !touch_state.framework_tested {
        clear_file(&touch_path);
        return add_context("Stop", "[advisory] framework changes detected but not tested — run cargo test.");
    }

    // ── 12. Conditional state cleanup ──
    let should_clear = !review_state.review_required
        && !review_state.tracks_goal()
        && !review_state.reject_reason_seen;
    if should_clear {
        clear_file(&review_path);
    } else {
        let _ = write_review_state(&review_path, &review_state);
    }
    clear_file(&touch_path);

    // ── 13. Review output lint (shared) ──
    let mut output = json!({});
    if !response_for_lint.trim().is_empty() && response_for_lint.contains("[P") {
        let lint_findings = core_policy::review_output_lint::lint_review_output(&response_for_lint);
        let warning_count = lint_findings
            .iter()
            .filter(|f| f.severity == core_policy::review_output_lint::LintSeverity::Warning)
            .count();
        if warning_count > 0 {
            let msg = format!(
                "review-output-lint: {} compact envelope warning(s)",
                warning_count
            );
            output["additional_context"] = Value::String(msg);
        }
    }

    if output.as_object().is_some_and(|o| !o.is_empty()) {
        Some(output)
    } else {
        None
    }
}

// ── Helper types and functions ──

#[derive(Default, serde::Deserialize)]
struct TouchState {
    settings: bool,
    framework: bool,
    settings_validated: bool,
    framework_tested: bool,
}

enum DiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
}

fn load_review_gate_disk(path: &Path) -> DiskState<core_policy::hook_review_disk_state::HookReviewDiskCore> {
    if !path.is_file() {
        return DiskState::Absent;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(state) => DiskState::Ok(state),
            Err(_) => DiskState::Unreadable,
        },
        Err(_) => DiskState::Unreadable,
    }
}

fn load_touch_state_disk(path: &Path) -> DiskState<Value> {
    if !path.is_file() {
        return DiskState::Absent;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(state) => DiskState::Ok(state),
            Err(_) => DiskState::Unreadable,
        },
        Err(_) => DiskState::Unreadable,
    }
}

fn write_review_state(path: &Path, state: &core_policy::hook_review_disk_state::HookReviewDiskCore) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| format!("serialize review state: {e}"))?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, text)
        .map_err(|e| format!("write review state: {e}"))
}

fn clear_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn add_context(event: &str, msg: &str) -> Option<Value> {
    Some(json!({
        "context_append": format!("[{event}] {msg}")
    }))
}
