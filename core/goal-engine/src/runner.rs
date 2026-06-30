use crate::closeout::{
    AggregateActionResult, build_aggregate, read_action_record,
    verify_closeout_with_evidence, write_loop_output,
};
use crate::dispatcher::{self, SubagentResult, };
use crate::kill_switch::{self, acquire_lock_guarded, clear_pause_state, read_pause_state};
use crate::report;
use crate::safety::assign_safety_for_action;
use crate::state::{
    closeout_path, create_initial_state, finish_run, generate_run_id,
    read_loop_state, start_new_run, transition_phase, update_heartbeat, write_loop_state,
};
use crate::types::{
    KillSignalAction, LoopAction, LoopCloseoutAggregate, LoopError, LoopPhase, LoopProfileConfig,
    LoopRegistryEntry, LoopRunState, SafetyLevel,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Check whether a loop entry's profile is schedulable.
/// Returns an error for task profiles which cannot be scheduled for unattended execution.
pub fn preflight_profile_check(entry: &LoopRegistryEntry) -> Result<(), LoopError> {
    match entry.profile.as_str() {
        "task" => Err(LoopError::ProfileMismatch(
            "task profile is not schedulable. \
             Use loop-auto for unattended execution."
                .to_string(),
        )),
        "loop-auto" => Ok(()),
        "interactive" => Ok(()),
        other => Err(LoopError::UnknownProfile(other.to_string())),
    }
}

/// Execution context for a single loop run, including repo root, registry entry, dry-run flag, and timeout.
pub struct RunContext<'a> {
    pub repo_root: &'a Path,
    pub entry: &'a LoopRegistryEntry,
    pub dry_run: bool,
    pub timeout: Option<std::time::Duration>,
    /// Max remaining recursion depth for research-escalation auto-restart.
    /// Decremented on each recursive call; `run_loop` returns `ResearchEscalation`
    /// instead of recursing when this reaches 0.
    pub depth_remaining: u32,
    /// Host identifier for harness capability lookups (used by closeout gate).
    pub host_id: String,
}

impl RunContext<'_> {
    /// Default max recursion depth for research-escalation auto-restart.
    pub fn default_max_depth() -> u32 {
        5
    }
}

/// Execute a full loop run: profile check, discovery, preflight, dispatch, verification, and closeout.
/// Returns an aggregated closeout result for all actions in the run.
pub fn run_loop(ctx: &RunContext) -> Result<LoopCloseoutAggregate, LoopError> {
    let entry = ctx.entry;
    let loop_id = &entry.loop_id;

    preflight_profile_check(entry)?;
    let mut depth_remaining = ctx.depth_remaining;

    loop {
        // Clean any stale pause state from a prior crash.
        let _ = clear_pause_state(ctx.repo_root, loop_id);

        let mut state = match read_loop_state(ctx.repo_root, loop_id)? {
            Some(s) => s,
            None => create_initial_state(loop_id, &entry.profile),
        };

        let run_id = generate_run_id(loop_id);

        // ── Checkpoint restart: rescue completed dispatch entries ──
        // If the previous run crashed mid-loop (e.g. after action 2/5),
        // LOOP_RUN_STATE.json still has current_run with dispatch entries
        // showing which actions already completed. Rescue those so the new
        // run skips already-completed actions.
        // NOTE: only "done" is rescued; "failed" actions should be retried.
        let rescued_dispatch: std::collections::HashMap<String, String> = state
            .current_run
            .as_ref()
            .map(|run| {
                run.dispatch
                    .iter()
                    .filter(|(_, status)| status.as_str() == "done")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        start_new_run(&mut state, &run_id);

        // Re-insert rescued entries into the new current_run.dispatch
        if !rescued_dispatch.is_empty() {
            tracing::info!(
                rescued_count = rescued_dispatch.len(),
                "checkpoint restart: rescued completed action(s) from previous run"
            );
            if let Some(ref mut run) = state.current_run {
                run.dispatch.extend(rescued_dispatch);
            }
        }

        // Initialize anti-drift original goal snapshot on first run
        if state.anti_drift.original_goal_snapshot.is_none() {
            state.anti_drift.original_goal_snapshot = read_goal_snapshot(ctx.repo_root, entry);
        }
        transition_phase(&mut state, LoopPhase::Pending);

        if ctx.dry_run {
            transition_phase(&mut state, LoopPhase::Discovering);
            let actions = discover_actions(entry, ctx.repo_root)?;
            transition_phase(&mut state, LoopPhase::Preflight);
            let _safety_map = assign_safety_levels(&actions, entry);
            transition_phase(&mut state, LoopPhase::Completed);
            let aggregate = build_aggregate(
                &run_id,
                loop_id,
                &actions,
                actions
                    .iter()
                    .map(|a| (a.action_id.clone(), AggregateActionResult::Skipped))
                    .collect(),
            );
            finish_run(&mut state, "dry-run");
            if let Err(e) = write_loop_state(ctx.repo_root, loop_id, &state) {
                tracing::error!("failed to write loop state on dry-run path: {e}");
            }
            tracing::info!(
                exit_path = "dry_run",
                "run_loop exit"
            );
            return Ok(aggregate);
        }

        let _guard = acquire_lock_guarded(ctx.repo_root, loop_id, &run_id)?;
        let lock_start = Instant::now();

        let result = run_loop_inner(ctx, &mut state, &run_id, entry);

        match result {
            Ok(agg) => {
                let findings = state
                    .current_run
                    .as_ref()
                    .map(|r| r.unconsumed_findings.clone())
                    .unwrap_or_default();
                let lock_ms = lock_start.elapsed().as_millis() as u64;
                let report_text = report::render_loop_report(&state, &agg, &findings, Some(lock_ms / 1000));
                let report_path =
                    report::write_loop_report(ctx.repo_root, loop_id, &run_id, &report_text).ok();
                if let Some(ref mut r) = state.current_run {
                    r.report_path = report_path;
                    r.closeout_aggregate = Some(agg.clone());
                }
                transition_phase(&mut state, LoopPhase::Completed);
                finish_run(&mut state, &agg.overall_status);
                if let Err(e) = write_loop_state(ctx.repo_root, loop_id, &state) {
                    tracing::error!("failed to write loop state on success path: {e}");
                }
                tracing::info!(
                    exit_path = "success",
                    loop_id = %loop_id,
                    run_id = %run_id,
                    duration_ms = lock_ms,
                    overall_status = %agg.overall_status,
                    actions_count = agg.actions.len(),
                    "run_loop exit"
                );
                break Ok(agg);
            }
            Err(LoopError::ResearchEscalation(msg)) => {
                // Research completed with candidates: restart the loop to consume them
                tracing::info!("[goal-engine] {msg}");
                state.circuit_breaker.consecutive_failures = 0;
                if let Err(e) = write_loop_state(ctx.repo_root, loop_id, &state) {
                    tracing::error!("failed to write loop state on research escalation: {e}");
                }
                if depth_remaining == 0 {
                    tracing::info!(
                        exit_path = "research_max_depth",
                        depth_remaining = 0,
                        "run_loop exit"
                    );
                    break Err(LoopError::ResearchEscalation(
                        "max recursion depth reached for research escalation auto-restart"
                            .to_string(),
                    ));
                }

                depth_remaining -= 1;
                tracing::info!(depth_remaining, "research escalation: restarting loop");
                continue;
            }
            Err(e) => {
                transition_phase(&mut state, LoopPhase::Escalated);
                finish_run(&mut state, "escalated");
                if let Err(write_err) = write_loop_state(ctx.repo_root, loop_id, &state) {
                    tracing::error!("failed to write loop state on error path: {write_err}");
                }
                tracing::info!(
                    exit_path = "error",
                    error = %e,
                    "run_loop exit"
                );
                break Err(e);
            }
        } // close match
    } // close loop
}

fn run_loop_inner(
    ctx: &RunContext,
    state: &mut LoopRunState,
    run_id: &str,
    entry: &LoopRegistryEntry,
) -> Result<LoopCloseoutAggregate, LoopError> {
    let mut phase_start = Instant::now();

    transition_phase(state, LoopPhase::Discovering);
    let actions = discover_actions(entry, ctx.repo_root)?;

    if let Some(ref mut run) = state.current_run {
        run.discovery = Some(crate::types::DiscoveryResult {
            actions_found: actions.len() as u32,
            actions: actions.clone(),
        });
    }
    tracing::info!(
        phase = "discovering",
        duration_us = phase_start.elapsed().as_micros() as u64,
        actions_found = actions.len(),
        "phase completed"
    );

    phase_start = Instant::now();
    transition_phase(state, LoopPhase::Preflight);
    let profile_config = LoopProfileConfig::from_runtime_registry(ctx.repo_root, &entry.profile)
        .unwrap_or_else(|| LoopProfileConfig {
            profile: entry.profile.clone(),
            loop_capable: true,
            closeout_mode: "hard-block".to_string(),
            review_gate: "mandatory".to_string(),
            spawn_first_nudge: true,
            cost_budget: entry.cost_budget.clone(),
            escalation: None,
            interactive_capable: false,
            pause_timeout_secs: None,
        });
    let safety_map = assign_safety_levels(&actions, entry);
    check_budget_preflight(&profile_config)?;
    tracing::info!(
        phase = "preflight",
        duration_us = phase_start.elapsed().as_micros() as u64,
        "phase completed"
    );

    phase_start = Instant::now();
    transition_phase(state, LoopPhase::Running);
    let mut results: Vec<(String, AggregateActionResult)> = Vec::new();
    // Action output cache: populated after each committed action, consumed
    // by subsequent actions that reference this action via consumed_action_ids.
    let mut action_outputs: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    for action in &actions {
        // ── Checkpoint restart: skip actions already completed ──
        // This check MUST happen before the unconditional dispatch insert below
        // (line 239), otherwise rescued "done" entries would be overwritten.
        if let Some(ref run) = state.current_run {
            if run
                .dispatch
                .get(&action.action_id)
                .map(|s| s == "done")
                .unwrap_or(false)
            {
                tracing::info!(
                    action_id = %action.action_id,
                    "skipping already-completed action (checkpoint restart)"
                );
                results.push((
                    action.action_id.clone(),
                    AggregateActionResult::Skipped,
                ));
                continue;
            }
        }

        let level = safety_map
            .get(&action.action_id)
            .cloned()
            .unwrap_or(SafetyLevel::L1ReportOnly);

        if let Some(ref mut run) = state.current_run {
            run.dispatch
                .insert(action.action_id.clone(), level.as_str().to_string());
        }

        match level {
            SafetyLevel::L1ReportOnly => {
                results.push((action.action_id.clone(), AggregateActionResult::Skipped));
                if let Some(ref mut run) = state.current_run {
                    run.dispatch
                        .insert(action.action_id.clone(), "skipped".to_string());
                }
            }
            // L2 and L3 are currently handled identically: both dispatch the action
            // to the subagent with the same timeout, kill-signal, and closeout flow.
            // In a future enhancement, L3Unattended should skip the interactive
            // review nudge and proceed with fully unattended execution.
            SafetyLevel::L2AssistedFix | SafetyLevel::L3Unattended => {
                update_heartbeat(state);
                if let Some(ref mut run) = state.current_run {
                    run.dispatch
                        .insert(action.action_id.clone(), "running".to_string());
                }

                // Pre-dispatch: write consumed action outputs as files the subagent can read.
                if !action.consumed_action_ids.is_empty() {
                    let action_outputs_dir = ctx
                        .repo_root
                        .join("artifacts/loop")
                        .join(&entry.loop_id)
                        .join("action_outputs");
                    for consumed_id in &action.consumed_action_ids {
                        if let Some(output) = action_outputs.get(consumed_id) {
                            let out_path = action_outputs_dir.join(format!("{consumed_id}.json"));
                            if let Some(parent) = out_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = core_state_utils::atomic_write::write_atomic_json(
                                &out_path,
                                output,
                            );
                        }
                    }
                }

                let sub_protocol = crate::types::SubagentProtocol::resolve(
                    entry.subagent_protocol.as_deref(),
                );

                // Wrap dispatch in an inner loop to support pause→resume re-dispatch
                let sub_result = 'exec: loop {
                    match dispatcher::run_action_sync(
                        ctx.repo_root,
                        &entry.loop_id,
                        run_id,
                        action,
                        ctx.timeout,
                        sub_protocol,
                    ) {
                        Ok(output) => break 'exec Ok(output),
                        Err(LoopError::PauseSignaled(_)) | Err(LoopError::Paused(_)) => {
                            // Enter pause state
                            transition_phase(state, LoopPhase::Paused);
                            if let Err(e) = write_loop_state(ctx.repo_root, &entry.loop_id, state) {
                                tracing::error!("failed to write loop state on pause: {e}");
                            }

                            match pause_wait_loop(ctx.repo_root, &entry.loop_id) {
                                Ok(PauseCommand::Resume) => {
                                    tracing::info!(action = %action.action_id, "pause-resumed");
                                    transition_phase(state, LoopPhase::Running);
                                    update_heartbeat(state);
                                    if let Err(e) = write_loop_state(ctx.repo_root, &entry.loop_id, state) {
                                        tracing::error!("failed to write loop state on resume: {e}");
                                    }
                                    let _ = clear_pause_state(ctx.repo_root, &entry.loop_id);
                                    // Reset scope paths to clear any partial modifications
                                    // from the killed subagent before re-dispatch.
                                    dispatcher::reset_scope_paths(ctx.repo_root, &action.scope_paths);
                                    continue 'exec; // re-dispatch same action
                                }
                                Ok(PauseCommand::Redirect { .. }) => {
                                    tracing::info!(action = %action.action_id, "pause-redirected");
                                    transition_phase(state, LoopPhase::Running);
                                    update_heartbeat(state);
                                    if let Err(e) = write_loop_state(ctx.repo_root, &entry.loop_id, state) {
                                        tracing::error!("failed to write loop state on redirect: {e}");
                                    }
                                    let _ = clear_pause_state(ctx.repo_root, &entry.loop_id);
                                    break 'exec Err(LoopError::Redirected(
                                        "redirected during pause".into(),
                                    ));
                                }
                                Ok(PauseCommand::Kill) => {
                                    let _ = clear_pause_state(ctx.repo_root, &entry.loop_id);
                                    break 'exec Err(LoopError::KillSignaled(
                                        "killed during pause".into(),
                                    ));
                                }
                                Err(e) => {
                                    let _ = clear_pause_state(ctx.repo_root, &entry.loop_id);
                                    break 'exec Err(e);
                                }
                            }
                        }
                        Err(e) => break 'exec Err(e),
                    }
                };
                match sub_result {
                    Ok(output) => {
                        let aggregate_result = evaluate_subagent_output(
                            ctx.repo_root,
                            &entry.loop_id,
                            run_id,
                            action,
                            &output,
                        );

                        // Post-dispatch: capture action output from closeout record
                        if let AggregateActionResult::Committed { .. } = &aggregate_result {
                            let closeout_dir = ctx
                                .repo_root
                                .join("artifacts/loop")
                                .join(&entry.loop_id)
                                .join("closeout");
                            let closeout_file = closeout_dir.join(format!(
                                "{run_id}-{}.json",
                                action.action_id
                            ));
                            let action_out = std::fs::read_to_string(&closeout_file)
                                .ok()
                                .and_then(|raw| serde_json::from_str(&raw).ok())
                                .unwrap_or_else(|| serde_json::json!({
                                    "action_id": &action.action_id,
                                    "status": "committed"
                                }));
                            action_outputs.insert(action.action_id.clone(), action_out);
                            // ── Checkpoint: persist state after committed action ──
                            // This enables crash recovery: if the process dies between
                            // actions, the next run skips already-"done" actions.
                            if let Err(e) = write_loop_state(ctx.repo_root, &entry.loop_id, state) {
                                tracing::warn!("failed to write action checkpoint: {e}");
                            }
                        }

                        if let Some(ref mut run) = state.current_run {
                            let status = match &aggregate_result {
                                AggregateActionResult::Committed { .. } => "done",
                                AggregateActionResult::Failed { .. } => "failed",
                                AggregateActionResult::Interrupted => "interrupted",
                                AggregateActionResult::Skipped => "skipped",
                            };
                            run.dispatch
                                .insert(action.action_id.clone(), status.to_string());
                        }
                        results.push((action.action_id.clone(), aggregate_result));
                    }
                    Err(LoopError::KillSignaled(msg)) => {
                        results.push((
                            action.action_id.clone(),
                            AggregateActionResult::Failed {
                                reason: format!("killed: {msg}"),
                            },
                        ));
                        return Err(LoopError::KillSignaled(msg));
                    }
                    Err(LoopError::Timeout(secs)) => {
                        results.push((
                            action.action_id.clone(),
                            AggregateActionResult::Failed {
                                reason: format!("timeout after {secs}s"),
                            },
                        ));
                        return Err(LoopError::Timeout(secs));
                    }
                    Err(LoopError::Redirected(msg)) => {
                        results.push((
                            action.action_id.clone(),
                            AggregateActionResult::Interrupted,
                        ));
                        if let Some(ref mut run) = state.current_run {
                            run.dispatch.insert(action.action_id.clone(), "redirected".to_string());
                        }
                        tracing::info!(action = %action.action_id, "redirected: {msg}");
                    }
                    Err(e) => {
                        results.push((
                            action.action_id.clone(),
                            AggregateActionResult::Failed {
                                reason: e.to_string(),
                            },
                        ));
                    }
                }
            }
        }
    }

    tracing::info!(
        phase = "running",
        duration_us = phase_start.elapsed().as_micros() as u64,
        actions_executed = results.len(),
        "phase completed"
    );

    phase_start = Instant::now();
    transition_phase(state, LoopPhase::Verifying);
    let mut aggregate = build_aggregate(run_id, &entry.loop_id, &actions, results);
    // Write LOOP_OUTPUT.json for downstream consumers
    write_loop_output(ctx.repo_root, &entry.loop_id, &aggregate);

    // ── Quality Gate (verify_quality_gate) ──
    // Two-stage exit gate: Stage 1 anti-fraud evidence check + Stage 2 scene-dispatched checker evaluation.
    // If the QG gate blocks, the aggregate is downgraded to "fail".
    if entry.verify_quality_gate.unwrap_or(true) && aggregate.overall_status == "pass" {
        let task_id = actions
            .first()
            .map(|a| {
                a.action_id
                    .strip_suffix("-orchestrator")
                    .unwrap_or(&a.action_id)
                    .to_string()
            })
            .unwrap_or_default();
        if !task_id.is_empty() {
            // Read scene from goal state instead of hardcoding "general" (P2-005)
            let scene = core_state::state_manager::read_goal_state(ctx.repo_root, Some(&task_id))
                .ok()
                .flatten()
                .and_then(|s| s.get("scene").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| "general".to_string());

            if let Some(qg_hooks) = framework_core::runtime_hooks::try_hooks() {
                let qg_payload = serde_json::json!({
                    "repo_root": ctx.repo_root.to_string_lossy().to_string(),
                    "task_id": task_id,
                    "scene": scene,
                    "goal": entry.loop_id,
                    "round": 1,
                });
                let qg_start = Instant::now();
                match qg_hooks.evaluate_quality_gate(qg_payload) {
                    Ok(verdict) => {
                        let passed = verdict
                            .get("passed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let qg_duration_us = qg_start.elapsed().as_micros() as u64;
                        if !passed {
                            let blockers: Vec<String> = verdict
                                .get("blockers")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|b| {
                                            b.get("description")
                                                .and_then(|d| d.as_str())
                                                .map(|s| s.to_string())
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            tracing::warn!(
                                loop_id = %entry.loop_id,
                                qg_passed = false,
                                qg_blocker_count = blockers.len(),
                                qg_duration_us = qg_duration_us,
                                "Quality gate blocked (verify_quality_gate=true)"
                            );
                            aggregate.overall_status = "fail".to_string();
                            aggregate.qg_blockers = blockers;
                        } else {
                            tracing::info!(
                                loop_id = %entry.loop_id,
                                qg_passed = true,
                                qg_duration_us = qg_duration_us,
                                "quality gate passed"
                            );
                        }
                    }
                    Err(e) => {
                        let qg_duration_us = qg_start.elapsed().as_micros() as u64;
                        // P1-007: Fail-closed on QG hook error, consistent with goal_ops.rs
                        // (which returns Err on QG hook failure). In runner context, downgrade
                        // the aggregate to "fail" so the caller is aware of the blocked gate.
                        tracing::error!(
                            loop_id = %entry.loop_id,
                            qg_error = %e,
                            qg_duration_us = qg_duration_us,
                            "Quality gate hook error (verify_quality_gate=true) — downgrading aggregate to fail (fail-closed)"
                        );
                        aggregate.overall_status = "fail".to_string();
                    }
                }
            } else {
                tracing::warn!(
                    loop_id = %entry.loop_id,
                    task_id = %task_id,
                    "verify_quality_gate=true but RuntimeCoreHooks not registered — skipping QG gate"
                );
            }
        }
    }

    // ── Closeout Gate (verify_closeout_gate) ──
    // Readiness check for goal_state, evidence, session_summary.
    // Results are advisory only — does not downgrade aggregate.
    if entry.verify_closeout_gate.unwrap_or(true) && aggregate.overall_status == "pass" {
        let task_id = actions
            .first()
            .map(|a| {
                a.action_id
                    .strip_suffix("-orchestrator")
                    .unwrap_or(&a.action_id)
                    .to_string()
            })
            .unwrap_or_default();
        if let Some(cg_hooks) = framework_core::runtime_hooks::try_hooks() {
            let closeout_payload = serde_json::json!({
                "repo_root": ctx.repo_root.to_string_lossy().to_string(),
                "task_id": task_id,
                "host_id": ctx.host_id,
            });
            let cg_start = Instant::now();
            match cg_hooks.evaluate_closeout_gate(closeout_payload) {
                Ok(result) => {
                    let passed = result
                        .get("passed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let findings: Vec<String> = result
                        .get("findings")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let cg_duration_us = cg_start.elapsed().as_micros() as u64;
                    if !passed {
                        tracing::warn!(
                            loop_id = %entry.loop_id,
                            cg_passed = false,
                            cg_findings_count = findings.len(),
                            cg_duration_us = cg_duration_us,
                            "Closeout gate advisory (verify_closeout_gate=true)"
                        );
                    } else {
                        tracing::info!(
                            loop_id = %entry.loop_id,
                            cg_passed = true,
                            cg_duration_us = cg_duration_us,
                            "closeout gate passed"
                        );
                    }
                }
                Err(e) => {
                    let cg_duration_us = cg_start.elapsed().as_micros() as u64;
                    tracing::warn!(
                        loop_id = %entry.loop_id,
                        cg_error = %e,
                        cg_duration_us = cg_duration_us,
                        "Closeout gate hook error (verify_closeout_gate=true)"
                    );
                }
            }
        }
    }

    // Anti-drift check: after each review cycle, increment counter
    // and fire drift check every N cycles (default 3).
    state.anti_drift.review_cycle_count += 1;
    // Cap at 10000 to prevent u32 overflow under extended execution.
    // Reset to 0 if threshold is reached.
    if state.anti_drift.review_cycle_count >= 10000 {
        state.anti_drift.review_cycle_count = 0;
    }
    let should_check = crate::drift::should_check_drift(&state.anti_drift);
    let current_goal = read_goal_snapshot(ctx.repo_root, entry);
    if current_goal.is_none() && should_check && state.anti_drift.original_goal_snapshot.is_some() {
        tracing::warn!(
            review_cycle = state.anti_drift.review_cycle_count,
            loop_id = %entry.loop_id,
            "anti-drift check skipped: cannot read current goal (GOAL_STATE.json not found)"
        );
    }
    if should_check && let Some(current_goal_text) = current_goal {
        let result = crate::drift::perform_drift_check(&mut state.anti_drift, &current_goal_text);
        tracing::warn!(
            review_cycle = result.review_cycle,
            drift_detected = result.drift_detected,
            drift_score = result.drift_score,
            "anti-drift check result"
        );
        state.anti_drift.last_drift_check = Some(result.clone());
        state.anti_drift.drift_check_history.push(result);
        // Cap history to last 20 entries to prevent state file bloat
        let max_history = 20;
        if state.anti_drift.drift_check_history.len() > max_history {
            let drain_count = state.anti_drift.drift_check_history.len() - max_history;
            state.anti_drift.drift_check_history.drain(..drain_count);
        }
    }

    if aggregate.overall_status == "pass" {
        state.circuit_breaker.consecutive_failures = 0;
    } else if aggregate.overall_status == "fail" || aggregate.overall_status == "partial" {
        state.circuit_breaker.consecutive_failures += 1;
        let threshold = entry
            .research
            .as_ref()
            .map(|r| r.barrier_threshold)
            .unwrap_or(3);
        if state.circuit_breaker.consecutive_failures >= threshold {
            tracing::warn!(
                loop_id = %entry.loop_id,
                consecutive_failures = state.circuit_breaker.consecutive_failures,
                threshold = threshold,
                research_enabled = entry.research_enabled,
                "circuit breaker firing"
            );
            if entry.research_enabled {
                let escalation = barrier_escalation(
                    entry,
                    &entry.loop_id,
                    run_id,
                    state.circuit_breaker.consecutive_failures,
                    ctx.repo_root,
                )?;
                if escalation.should_resume() {
                    return Err(LoopError::ResearchEscalation(format!(
                        "barrier={} candidates={}: research complete, auto-resume loop",
                        escalation
                            .candidates
                            .first()
                            .map(|s| s.as_str())
                            .unwrap_or("?"),
                        escalation.candidates.len()
                    )));
                } else {
                    transition_phase(state, LoopPhase::Escalated);
                    return Err(LoopError::ActionFailed(format!(
                        "circuit breaker: loop={} escalated to research; awaiting human approval.",
                        entry.loop_id,
                    )));
                }
            } else {
                return Err(LoopError::ActionFailed(
                    "circuit breaker: 3 consecutive failures. Loop suspended.".to_string(),
                ));
            }
        }
    }

    // Emit verifying phase timing before the GOAL_STATE epilogue
    // so it's not lost if the sync write propagates an error.
    tracing::info!(
        phase = "verifying",
        duration_us = phase_start.elapsed().as_micros() as u64,
        overall_status = %aggregate.overall_status,
        "phase completed"
    );

    // ── GOAL_STATE heartbeat sync (formerly iteration_count sync) ──
    // Best-effort touch of GOAL_STATE.json to prevent the goal-engine's
    // LOOP_RUN_STATE and core-state's GOAL_STATE from having wildly
    // diverging timestamps. We do NOT sync iteration_count here because
    // LoopCloseoutAggregate doesn't carry it — core-state is authoritative
    // for iteration tracking.
    if aggregate.overall_status == "pass" {
        let sync_task_id = actions
            .first()
            .map(|a| {
                a.action_id
                    .strip_suffix("-orchestrator")
                    .unwrap_or(&a.action_id)
                    .to_string()
            })
            .unwrap_or_default();
        if !sync_task_id.is_empty() {
            if let Ok(path) = core_state::state_manager::goal_state_path_for_task(
                ctx.repo_root,
                &sync_task_id,
            ) {
                if let Ok(Some(mut goal_state)) = core_state::state_manager::read_goal_state_raw(
                    ctx.repo_root,
                    &sync_task_id,
                ) {
                    goal_state["updated_at"] =
                        serde_json::json!(framework_core::time::now_iso());
                    // P1-006: Propagate write errors to prevent silent divergence
                    // between GOAL_STATE and LOOP_RUN_STATE timestamps.
                    core_state_utils::atomic_write::write_atomic_json(
                        &path, &goal_state,
                    )
                    .map_err(|e| LoopError::Io(format!("GOAL_STATE sync write failed: {e}")))?;
                }
            }
        }
    }

    Ok(aggregate)
}

// ── Pause / Resume support ──

/// Command returned by `pause_wait_loop` indicating how to proceed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PauseCommand {
    /// Resume the action with original parameters.
    Resume,
    /// Redirect: skip to next action (human changed the goal).
    Redirect,
    /// Kill: terminate the loop.
    Kill,
}

/// Maximum time a loop can remain paused before timing out (1 hour, matching lock staleness).
const PAUSE_TIMEOUT_SECS: u64 = 3600;

/// Enter a tight polling loop while the loop is paused.
///
/// Polls for signals on `.loop-kill/{loop_id}`. On `Resume`, returns `Ok(Resume)`.
/// On `Redirect`, returns `Ok(Redirect)`. On `Kill`, returns `Ok(Kill)`.
/// If no signal arrives within the timeout, returns `Err(Timeout)`.
///
/// During the wait, the loop lock is refreshed every 60s to prevent stale
/// lock takeover by other processes.
pub(crate) fn pause_wait_loop(
    repo_root: &Path,
    loop_id: &str,
) -> Result<PauseCommand, LoopError> {
    let deadline = Instant::now() + Duration::from_secs(PAUSE_TIMEOUT_SECS);
    let mut last_refresh = Instant::now();

    loop {
        // Refresh lock every 60s to prevent staleness
        if last_refresh.elapsed().as_secs() >= 60 {
            let _ = kill_switch::refresh_lock(repo_root);
            last_refresh = Instant::now();
        }

        match crate::kill_switch::take_signal(repo_root, loop_id) {
            Ok(Some(payload)) => match payload.action {
                KillSignalAction::Resume => {
                    tracing::info!(%loop_id, "pause-wait: resume signal received");
                    return Ok(PauseCommand::Resume);
                }
                KillSignalAction::Redirect { .. } => {
                    tracing::info!(%loop_id, "pause-wait: redirect signal received");
                    return Ok(PauseCommand::Redirect);
                }
                KillSignalAction::Kill => {
                    tracing::info!(%loop_id, "pause-wait: kill signal received");
                    return Ok(PauseCommand::Kill);
                }
                KillSignalAction::PauseWithFeedback { feedback } => {
                    // Update the existing PauseState's feedback field
                    if let Ok(Some(mut pause_state)) = read_pause_state(repo_root, loop_id) {
                        pause_state.feedback = Some(feedback);
                        let _ = kill_switch::write_pause_state(repo_root, &pause_state);
                        tracing::info!(%loop_id, "pause-wait: feedback updated");
                    }
                }
                KillSignalAction::Pause => {
                    // Already paused — no-op
                }
            },
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(%loop_id, error = %e, "pause-wait: signal read error");
            }
        }

        if Instant::now() > deadline {
            // Pause timeout — clear state and return error
            let _ = clear_pause_state(repo_root, loop_id);
            return Err(LoopError::Timeout(PAUSE_TIMEOUT_SECS));
        }

        thread::sleep(Duration::from_secs(dispatcher::KILL_POLL_INTERVAL_SECS));
    }
}

// ── Public API ──

/// Send a pause signal to a running loop, optionally injecting human feedback.
///
/// The running subprocess is terminated, the action context is persisted to
/// `.loop-pause/{loop_id}`, and the loop enters a pause-wait cycle.
/// Call `run_loop_resume`, `run_loop_redirect`, or `run_loop_kill` to continue.
///
/// Returns an error if the signal file could not be written.
pub fn run_loop_pause(
    repo_root: &Path,
    loop_id: &str,
    feedback: Option<&str>,
) -> Result<(), LoopError> {
    let action_id = read_pause_state(repo_root, loop_id)
        .ok()
        .flatten()
        .map(|s| s.action_id)
        .unwrap_or_else(|| "unknown".to_string());
    crate::kill_switch::write_pause_signal(repo_root, loop_id, &action_id, feedback)
}

/// Send a resume signal to a paused loop.
/// The paused action is re-dispatched with its original parameters (and any
/// injected feedback from a prior `run_loop_pause` call).
pub fn run_loop_resume(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    crate::kill_switch::write_resume_signal(repo_root, loop_id)
}

/// Send a redirect signal to a paused loop.
/// The current action is skipped and the loop continues with the next action.
pub fn run_loop_redirect(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    crate::kill_switch::write_redirect_signal(repo_root, loop_id, "redirected")
}

/// Read the current pause state for a loop, if any.
/// Returns `Ok(Some(PauseState))` when the loop is paused, `Ok(None)` otherwise.
pub fn run_loop_pause_status(
    repo_root: &Path,
    loop_id: &str,
) -> Result<Option<crate::types::PauseState>, LoopError> {
    read_pause_state(repo_root, loop_id)
}

/// Result of a barrier escalation attempt.
struct BarrierResult {
    candidates: Vec<String>,
    will_resume: bool,
}

impl BarrierResult {
    fn should_resume(&self) -> bool {
        self.will_resume && !self.candidates.is_empty()
    }
}

/// Resolve the autoresearch binary path.
/// Prefers `ROUTER_RS_AUTORESEARCH_BIN` env var; falls back to `cargo run` slow-path.
fn resolve_autoresearch_binary() -> Result<String, LoopError> {
    Ok(crate::env_flags::autoresearch_binary())
}

/// Spawn an autoresearch barrier subprocess, unifying the cargo-run slow-path and
/// binary fast-path into a single code path. Returns the subprocess output.
fn spawn_autoresearch_barrier(
    autoresearch_bin: &str,
    repo_root: &Path,
    loop_id: &str,
    run_id: &str,
    problem: &str,
    timeout_secs: u64,
) -> Result<std::process::Output, LoopError> {
    use std::process::Command;

    let mut cmd;
    if autoresearch_bin.is_empty() {
        // Cargo run slow-path: compile and run via cargo with a timeout
        cmd = Command::new("cargo");
        cmd.args([
            "run",
            "-p",
            "research-harness",
            "--bin",
            "autoresearch",
            "--",
        ]);
    } else {
        // Binary fast-path: use pre-compiled binary
        cmd = Command::new(autoresearch_bin);
    }

    // Common args and working directory
    cmd.args(["barrier", "--problem", problem, "--loop-id", loop_id, "--run-id", run_id])
        .current_dir(repo_root);

    // SAFETY: setrlimit is async-signal-safe; pre_exec runs in single-threaded forked child.
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| dispatcher::apply_subprocess_rlimits());
    }

    let label = if autoresearch_bin.is_empty() {
        "barrier-escalation (cargo)"
    } else {
        "barrier-escalation (binary)"
    };

    let child = cmd
        .spawn()
        .map_err(|e| LoopError::SpawnFailed(format!("{label}: {e}")))?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    crate::dispatcher::poll_subprocess(
        child,
        repo_root,
        loop_id,
        label,
        deadline,
        std::time::Duration::from_secs(timeout_secs),
        None, // no pause support for barrier escalation
    )
}

/// Execute barrier escalation: shell out to `autoresearch barrier --problem <desc>`.
fn barrier_escalation(
    entry: &LoopRegistryEntry,
    loop_id: &str,
    run_id: &str,
    consecutive_failures: u32,
    repo_root: &Path,
) -> Result<BarrierResult, LoopError> {
    let problem = format!(
        "loop={} run={} consecutive_failures={} skill={}",
        loop_id,
        run_id,
        consecutive_failures,
        entry.skill.as_deref().unwrap_or("none"),
    );

    // Use pre-compiled binary if available (via ROUTER_RS_AUTORESEARCH_BIN),
    // otherwise fall back to cargo run slow-path.
    let autoresearch_bin = resolve_autoresearch_binary()?;

    // Acquire SubagentPermit for barrier escalation subprocess (consistent with dispatch path).
    let _permit = dispatcher::SubagentPermit::acquire(dispatcher::subagent_semaphore());

    // Use max_research_time_min for subprocess timeout instead of hardcoded 300s.
    let max_time_secs = u64::from(
        entry
            .research
            .as_ref()
            .map(|r| r.max_research_time_min)
            .unwrap_or(30),
    ) * 60;

    let output = spawn_autoresearch_barrier(
        &autoresearch_bin,
        repo_root,
        loop_id,
        run_id,
        &problem,
        max_time_secs,
    )?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("[goal-engine] autoresearch barrier failed: {stderr}");
        return Ok(BarrierResult {
            candidates: vec![],
            will_resume: false,
        });
    }

    let escalation_target = entry
        .research
        .as_ref()
        .map(|r| r.escalation_target.as_str())
        .unwrap_or("autoresearch");
    let auto_resume = entry
        .research
        .as_ref()
        .map(|r| r.auto_resume)
        .unwrap_or(true);
    let require_human_approval = entry
        .research
        .as_ref()
        .map(|r| r.require_human_approval)
        .unwrap_or(false);
    let freshness_window_secs = u64::from(
        entry
            .research
            .as_ref()
            .map(|r| r.freshness_window_min)
            .unwrap_or(60),
    ) * 60;
    let candidates = discover_barrier_candidates(repo_root, freshness_window_secs);

    tracing::info!(
        "[goal-engine] barrier escalation to {escalation_target}: {} candidates, \
         auto_resume={auto_resume}, require_human_approval={require_human_approval}",
        candidates.len()
    );

    Ok(BarrierResult {
        candidates,
        will_resume: auto_resume && !require_human_approval,
    })
}

/// Scan artifacts/research-barrier/ for the most recent BARRIER_REPORT.json.
fn discover_barrier_candidates(
    repo_root: &Path,
    freshness_window_secs: u64,
) -> Vec<String> {
    let barrier_dir = repo_root.join("artifacts").join("research-barrier");
    if !barrier_dir.exists() {
        return vec![];
    }
    // Find most recent barrier directory.
    // NOTE: This uses lexicographic path sorting, which works correctly when
    // directory names contain ISO-like timestamps (e.g. "2026-06-20T12-00-00Z")
    // but is only an approximation for truly chronological ordering. If barrier
    // directories use non-sortable names, this may select the wrong report.
    let mut entries: Vec<_> = match std::fs::read_dir(&barrier_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            // Validate directory names as approximate timestamps before sorting.
            // Lexicographic sort only works correctly for sortable timestamp names
            // (e.g., "2026-06-20T12-00-00Z"). Filter out invalid names to prevent
            // non-timestamp directories from polluting the ordering.
            .filter(|e| {
                let fname = e.file_name();
                let name = fname.to_string_lossy();
                // Accept ISO-like names (YYYY-MM-DD prefix) or all-numeric epoch timestamps
                name.len() >= 10
                    && (chrono::NaiveDate::parse_from_str(&name[..10], "%Y-%m-%d").is_ok()
                        || name.chars().all(|c| c.is_ascii_digit()))
            })
            .collect(),
        Err(_) => return vec![],
    };
    entries.sort_by_key(|e| e.path());
    if let Some(latest) = entries.last() {
        let report_path = latest.path().join("BARRIER_REPORT.json");
        if report_path.exists()
            // Freshness check: skip barrier reports older than configured window
            && let Ok(metadata) = std::fs::metadata(&report_path)
            && let Ok(modified) = metadata.modified()
            && let Ok(age) = modified.elapsed()
            && age < std::time::Duration::from_secs(freshness_window_secs)
            && let Ok(content) = std::fs::read_to_string(&report_path)
            && let Ok(report) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(candidates) = report.get("candidates").and_then(|c| c.as_array())
        {
            return candidates
                .iter()
                .filter_map(|c| c.as_str().map(|s| s.to_string()))
                .collect();
        }
    }
    vec![]
}

fn discover_actions(
    entry: &LoopRegistryEntry,
    repo_root: &std::path::Path,
) -> Result<Vec<LoopAction>, LoopError> {
    let skill_name = entry.skill.as_deref().unwrap_or(&entry.loop_id);
    let default_safety = entry.default_safety.as_deref().unwrap_or("L1");

    // Use static actions if configured in the registry entry.
    // Static actions allow loops to define their action set declaratively
    // instead of spawning a subagent for discovery.
    if let Some(ref static_actions) = entry.static_actions
        && !static_actions.is_empty()
    {
        tracing::info!(
            "loop {}: using {} static action(s) from registry config",
            entry.loop_id,
            static_actions.len()
        );
        return Ok(static_actions.clone());
    }

    let handoff = format!(
        "## Objective\n\
         Discovery for loop: {loop_id}\n\n\
         ## Action\n\
         分析代码库，生成需要执行的 action 列表。\n\n\
         ## Output\n\
         输出 JSON 数组，每个元素包含 action_id, type, scope_paths, safety, description。",
        loop_id = entry.loop_id,
    );

    let binary = dispatcher::resolve_subagent_binary()?;
    let mut cmd = std::process::Command::new(&binary);
    cmd.args(["-p", &handoff])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Acquire SubagentPermit for discovery subprocess (consistent with dispatch path).
    let _permit = dispatcher::SubagentPermit::acquire(dispatcher::subagent_semaphore());

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| dispatcher::apply_subprocess_rlimits());
    }
    let child = cmd
        .spawn()
        .map_err(|e| LoopError::SpawnFailed(format!("discovery {binary}: {e}")))?;

    // Discovery uses a hardcoded 300s timeout rather than `ctx.timeout` because
    // discovery is a pre-dispatch phase that should finish quickly regardless of
    // the per-action timeout. If a configurable discovery timeout is needed,
    // add a `discovery_timeout` field to `RunContext` / `LoopRegistryEntry`.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let timeout_duration = std::time::Duration::from_secs(300);

    let output = crate::dispatcher::poll_subprocess(
        child,
        repo_root,
        &entry.loop_id,
        "discovery",
        deadline,
        timeout_duration,
        None, // no pause support for discovery phase
    )?;

    if !output.status.success() {
        tracing::warn!(
            "discovery subprocess failed (exit={}), falling back to default action",
            output.status.code().unwrap_or(-1)
        );
        return Ok(vec![LoopAction {
            action_id: format!("{}-discovery", entry.loop_id),
            action_type: "discovery".to_string(),
            scope_paths: Vec::new(),
            safety: default_safety.to_string(),
            description: Some(format!("Discovery fallback for skill: {skill_name}")),
            consumed_action_ids: Vec::new(),
        }]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actions = parse_discovery_output(&stdout, entry, default_safety)
        .unwrap_or_else(|e| {
            tracing::info!("discovery output: {e}");
            Vec::new()
        });

    if actions.is_empty() {
        tracing::info!("discovery returned no actions for loop {}", entry.loop_id);
    }

    Ok(actions)
}

fn parse_discovery_output(
    output: &str,
    _entry: &LoopRegistryEntry,
    default_safety: &str,
) -> Result<Vec<LoopAction>, LoopError> {
    let json_start = output.find('[');
    let json_end = output.rfind(']');
    let json_str = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => &output[start..=end],
        _ => {
            return Err(LoopError::Serde(
                "discovery output: no valid JSON array ('[...]') found".to_string(),
            ));
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse discovery output as JSON: {e}");
            return Ok(Vec::new());
        }
    };

    let arr = match parsed.as_array() {
        Some(a) => a,
        None => return Ok(Vec::new()),
    };

    Ok(arr
        .iter()
        .filter_map(|item| {
            let action_id = item.get("action_id")?.as_str()?;
            let action_type = item.get("type")?.as_str()?;
            let scope_paths: Vec<String> = item
                .get("scope_paths")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let safety = item
                .get("safety")
                .and_then(|v| v.as_str())
                .unwrap_or(default_safety);
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);

            Some(LoopAction {
                action_id: action_id.to_string(),
                action_type: action_type.to_string(),
                scope_paths,
                safety: safety.to_string(),
                description,
                consumed_action_ids: Vec::new(),
            })
        })
        .collect())
}

fn assign_safety_levels(
    actions: &[LoopAction],
    entry: &LoopRegistryEntry,
) -> std::collections::HashMap<String, SafetyLevel> {
    let mut map = std::collections::HashMap::new();
    for action in actions {
        let level = assign_safety_for_action(action, entry);
        map.insert(action.action_id.clone(), level);
    }
    map
}

fn check_budget_preflight(profile: &LoopProfileConfig) -> Result<(), LoopError> {
    if let Some(ref budget) = profile.cost_budget
        && let Some(max_tokens) = budget.tokens_per_run
    {
        let hard_limit = crate::env_flags::max_tokens_per_run_hard_limit();

        tracing::info!("budget preflight: tokens_per_run={max_tokens}, hard_limit={hard_limit}",);

        if max_tokens > hard_limit {
            return Err(LoopError::BudgetExceeded(format!(
                "tokens_per_run ({max_tokens}) exceeds hard upper limit ({hard_limit}). \
                     Set ROUTER_RS_LOOP_MAX_TOKENS_PER_RUN to raise the limit.",
            )));
        }
    } else {
        tracing::warn!(
            "budget preflight: no cost_budget or tokens_per_run configured — \
                 budget enforcement disabled; set cost_budget.tokens_per_run to enable limits"
        );
    }
    Ok(())
}

fn evaluate_subagent_output(
    repo_root: &Path,
    loop_id: &str,
    run_id: &str,
    action: &LoopAction,
    output: &SubagentResult,
) -> AggregateActionResult {
    let _eval_start = std::time::Instant::now();

    // V1 protocol: check inline closeout from parsed SubagentOutput first.
    if let Some(ref parsed) = output.parsed_output {
        if let Some(ref closeout) = parsed.closeout {
            // Inline closeout: verify via closeout+evidence rules (reduced file IO —
            // reads EVIDENCE_INDEX.json but skips the closeout JSON file read).
            let verification =
                verify_closeout_with_evidence(closeout, repo_root, &action.action_id);

            let scope_violations = if !action.scope_paths.is_empty() {
                dispatcher::check_scope_compliance(repo_root, &action.scope_paths)
            } else {
                Vec::new()
            };

            if !scope_violations.is_empty() {
                tracing::warn!(
                    "scope violation in action {} (V1 inline): {:?}",
                    action.action_id,
                    scope_violations
                );
                let mut violations = verification.violations;
                violations.push(format!("scope_violation: {:?}", scope_violations));
                return AggregateActionResult::Failed {
                    reason: violations.join("; "),
                };
            }

            if verification.closeout_allowed {
                let record_path = closeout_path(repo_root, loop_id, run_id, &action.action_id);
                // Write inline closeout to traditional closeout file for downstream
                // consumers (report module, closeout gate) that read closeout_path.
                if let Some(ref closeout_val) = parsed.closeout {
                    let record = crate::types::LoopActionRecord {
                        schema_version: crate::closeout::LOOP_CLOSEOUT_AGGREGATE_SCHEMA_VERSION
                            .to_string(),
                        loop_id: loop_id.to_string(),
                        run_id: run_id.to_string(),
                        action_id: action.action_id.clone(),
                        safety_level: action.safety.clone(),
                        closeout: closeout_val.clone(),
                    };
                    if let Ok(json) = serde_json::to_value(&record) {
                        if let Some(parent) = record_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = core_state_utils::atomic_write::write_atomic_json(
                            &record_path, &json,
                        );
                    }
                }
                return AggregateActionResult::Committed {
                    closeout_path: Some(record_path.display().to_string()),
                    commit_sha: None,
                };
            } else {
                return AggregateActionResult::Failed {
                    reason: verification.violations.join("; "),
                };
            }
        }
        // Inline closeout absent: fall through to success/error from parsed output.
        if !parsed.success {
            let err = parsed
                .error
                .as_deref()
                .unwrap_or("V1 subagent reported failure (no detail)");
            return AggregateActionResult::Failed {
                reason: err.to_string(),
            };
        }
        // parsed.success=true but no closeout → continue to file-based check below.
    }

    // V0 / file-based path: read closeout record from disk.
    let record_path = closeout_path(repo_root, loop_id, run_id, &action.action_id);
    if let Ok(Some(record)) = read_action_record(repo_root, loop_id, run_id, &action.action_id) {
        let verification =
            verify_closeout_with_evidence(&record.closeout, repo_root, &action.action_id);

        let scope_violations = if !action.scope_paths.is_empty() {
            dispatcher::check_scope_compliance(repo_root, &action.scope_paths)
        } else {
            Vec::new()
        };

        if !scope_violations.is_empty() {
            tracing::warn!(
                "scope violation in action {}: {:?}",
                action.action_id,
                scope_violations
            );
            let mut violations = verification.violations;
            violations.push(format!("scope_violation: {:?}", scope_violations));
            return AggregateActionResult::Failed {
                reason: violations.join("; "),
            };
        }

        if verification.closeout_allowed {
            return AggregateActionResult::Committed {
                closeout_path: Some(record_path.display().to_string()),
                commit_sha: None,
            };
        } else {
            return AggregateActionResult::Failed {
                reason: verification.violations.join("; "),
            };
        }
    }

    if output.success {
        AggregateActionResult::Committed {
            closeout_path: None,
            commit_sha: None,
        }
    } else {
        AggregateActionResult::Failed {
            reason: output.stderr.chars().take(200).collect(),
        }
    }
}

/// Read the current loop run state from disk for the given loop ID.
/// Returns `Ok(None)` if no state file exists.
pub fn run_loop_status(repo_root: &Path, loop_id: &str) -> Result<Option<LoopRunState>, LoopError> {
    read_loop_state(repo_root, loop_id)
}

/// Send a kill signal to gracefully terminate a running loop by writing its kill signal file.
pub fn run_loop_kill(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    kill_switch::write_kill_signal(repo_root, loop_id)
}

/// Send a kill signal to every loop registered in LOOP_REGISTRY.json.
/// Iterates all entries and writes individual kill signal files.
pub fn run_loop_kill_all(repo_root: &Path) -> Result<(), LoopError> {
    let registry_path = repo_root
        .join("configs")
        .join("framework")
        .join("LOOP_REGISTRY.json");
    let raw = fs::read_to_string(&registry_path)
        .map_err(|e| LoopError::Io(format!("read LOOP_REGISTRY.json: {e}")))?;
    let registry: crate::LoopRegistryRoot = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse LOOP_REGISTRY.json: {e}")))?;
    for entry in &registry.loops {
        kill_switch::write_kill_signal(repo_root, &entry.loop_id)?;
    }
    Ok(())
}

/// Read the current goal text from GOAL_STATE.json for drift comparison.
fn read_goal_snapshot(repo_root: &Path, entry: &LoopRegistryEntry) -> Option<String> {
    let goal_path = core_state::state_manager::goal_state_path_for_task(
        repo_root,
        &entry.loop_id,
    ).ok()?;
    if !goal_path.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(&goal_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    val.get("goal").and_then(|g| g.as_str()).map(String::from)
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopTriggerConfig;

    fn make_entry(profile: &str) -> LoopRegistryEntry {
        LoopRegistryEntry {
            loop_id: "test".into(),
            profile: profile.into(),
            trigger: LoopTriggerConfig {
                trigger_type: "manual".into(),
                schedule: None,
                timezone: None,
            },
            skill: None,
            scope_based_safety: None,
            default_safety: None,
            scope_conflict_resolution: None,
            cost_budget: None,
            notification: None,
            research_enabled: false,
            research: None,
            verify_quality_gate: None,
            verify_closeout_gate: None,
            static_actions: None,
            subagent_protocol: None,
        }
    }

    #[test]
    fn test_accepts_loop_auto() {
        let entry = make_entry("loop-auto");
        assert!(preflight_profile_check(&entry).is_ok());
    }

    #[test]
    fn test_accepts_interactive() {
        let entry = make_entry("interactive");
        assert!(preflight_profile_check(&entry).is_ok());
    }

    #[test]
    fn test_rejects_task() {
        let entry = make_entry("task");
        let result = preflight_profile_check(&entry);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoopError::ProfileMismatch(_)));
    }

    #[test]
    fn test_unknown_profile() {
        let entry = make_entry("unknown");
        let result = preflight_profile_check(&entry);
        assert!(matches!(result.unwrap_err(), LoopError::UnknownProfile(_)));
    }

    #[test]
    fn test_discover_actions() {
        let mut entry = make_entry("loop-auto");
        entry.static_actions = Some(vec![LoopAction {
            action_id: "test-action".into(),
            action_type: "test".into(),
            scope_paths: Vec::new(),
            safety: "L1".into(),
            description: None,
            consumed_action_ids: Vec::new(),
        }]);
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = discover_actions(&entry, tmp.path()).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].safety, "L1");
    }

    // ── parse_discovery_output ──────────────────────────────────────────

    #[test]
    fn parse_discovery_valid_json_array() {
        let entry = make_entry("loop-auto");
        let output = r#"some preamble text
[
  {"action_id": "fix-bug-1", "type": "fix", "scope_paths": ["src/a.rs"], "safety": "L2", "description": "fix bug"},
  {"action_id": "refactor-2", "type": "refactor", "scope_paths": [], "description": "refactor module"}
]
some trailing text"#;
        let actions = parse_discovery_output(output, &entry, "L1").unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_id, "fix-bug-1");
        assert_eq!(actions[0].action_type, "fix");
        assert_eq!(actions[0].scope_paths, vec!["src/a.rs"]);
        assert_eq!(actions[0].safety, "L2");
        assert_eq!(actions[1].action_id, "refactor-2");
        assert_eq!(actions[1].safety, "L1"); // default safety
    }

    #[test]
    fn parse_discovery_empty_array() {
        let entry = make_entry("loop-auto");
        let actions = parse_discovery_output("[]", &entry, "L1").unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn parse_discovery_no_json_array() {
        let entry = make_entry("loop-auto");
        let result = parse_discovery_output("no json here", &entry, "L1");
        assert!(result.is_err());
    }

    #[test]
    fn parse_discovery_invalid_json() {
        let entry = make_entry("loop-auto");
        // finds [ and ] but content is not valid JSON
        let actions = parse_discovery_output("[not valid json]", &entry, "L1").unwrap();
        assert!(actions.is_empty()); // falls back to empty
    }

    #[test]
    fn parse_discovery_skips_items_missing_required_fields() {
        let entry = make_entry("loop-auto");
        let output = r#"[
          {"action_id": "a1", "type": "fix"},
          {"action_id": "a2"},
          {"type": "fix"},
          {"action_id": "a3", "type": "fix", "safety": "L3"}
        ]"#;
        let actions = parse_discovery_output(output, &entry, "L1").unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_id, "a1");
        assert_eq!(actions[1].action_id, "a3");
        assert_eq!(actions[1].safety, "L3");
    }

    // ── check_budget_preflight ──────────────────────────────────────────

    #[test]
    fn budget_preflight_no_budget() {
        let profile = LoopProfileConfig {
            profile: "loop-auto".into(),
            loop_capable: true,
            closeout_mode: "hard-block".into(),
            review_gate: "mandatory".into(),
            spawn_first_nudge: true,
            cost_budget: None,
            escalation: None,
            interactive_capable: false,
            pause_timeout_secs: None,
        };
        assert!(check_budget_preflight(&profile).is_ok());
    }

    #[test]
    fn budget_preflight_within_limit() {
        let profile = LoopProfileConfig {
            profile: "loop-auto".into(),
            loop_capable: true,
            closeout_mode: "hard-block".into(),
            review_gate: "mandatory".into(),
            spawn_first_nudge: true,
            cost_budget: Some(crate::types::CostBudgetConfig {
                tokens_per_run: Some(100_000),
                daily_tokens: None,
            }),
            escalation: None,
            interactive_capable: false,
            pause_timeout_secs: None,
        };
        assert!(check_budget_preflight(&profile).is_ok());
    }

    #[test]
    fn budget_preflight_exceeds_hard_limit() {
        let profile = LoopProfileConfig {
            profile: "loop-auto".into(),
            loop_capable: true,
            closeout_mode: "hard-block".into(),
            review_gate: "mandatory".into(),
            spawn_first_nudge: true,
            cost_budget: Some(crate::types::CostBudgetConfig {
                tokens_per_run: Some(u64::MAX),
                daily_tokens: None,
            }),
            escalation: None,
            interactive_capable: false,
            pause_timeout_secs: None,
        };
        let result = check_budget_preflight(&profile);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoopError::BudgetExceeded(_)));
    }

    // ── BarrierResult ──────────────────────────────────────────────────

    #[test]
    fn barrier_result_should_resume() {
        let br = BarrierResult { candidates: vec!["a".into()], will_resume: true };
        assert!(br.should_resume());
    }

    #[test]
    fn barrier_result_no_candidates() {
        let br = BarrierResult { candidates: vec![], will_resume: true };
        assert!(!br.should_resume());
    }

    #[test]
    fn barrier_result_will_not_resume() {
        let br = BarrierResult { candidates: vec!["a".into()], will_resume: false };
        assert!(!br.should_resume());
    }

    // ── discover_barrier_candidates ─────────────────────────────────────

    #[test]
    fn barrier_candidates_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let candidates = discover_barrier_candidates(tmp.path(), 3600);
        assert!(candidates.is_empty());
    }

    #[test]
    fn barrier_candidates_no_barrier_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        // no artifacts/research-barrier/ directory at all
        let candidates = discover_barrier_candidates(tmp.path(), 3600);
        assert!(candidates.is_empty());
    }

    #[test]
    fn barrier_candidates_with_report() {
        let tmp = tempfile::TempDir::new().unwrap();
        let barrier_dir = tmp.path().join("artifacts/research-barrier/2026-06-29T12-00-00Z");
        std::fs::create_dir_all(&barrier_dir).unwrap();
        let report = serde_json::json!({
            "candidates": ["candidate-1", "candidate-2"]
        });
        std::fs::write(
            barrier_dir.join("BARRIER_REPORT.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        ).unwrap();
        let candidates = discover_barrier_candidates(tmp.path(), 3600);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&"candidate-1".to_string()));
        assert!(candidates.contains(&"candidate-2".to_string()));
    }

    // ── read_goal_snapshot ──────────────────────────────────────────────

    #[test]
    fn read_goal_snapshot_no_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entry = make_entry("loop-auto");
        let snapshot = read_goal_snapshot(tmp.path(), &entry);
        assert!(snapshot.is_none());
    }

    #[test]
    fn read_goal_snapshot_valid_goal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entry = make_entry("loop-auto");
        let goal_dir = tmp.path().join("artifacts/current/test");
        std::fs::create_dir_all(&goal_dir).unwrap();
        let goal = serde_json::json!({
            "goal": "Implement feature X",
            "status": "active"
        });
        let goal_path = goal_dir.join("GOAL_STATE.json");
        std::fs::write(&goal_path, serde_json::to_string_pretty(&goal).unwrap()).unwrap();
        let snapshot = read_goal_snapshot(tmp.path(), &entry);
        assert_eq!(snapshot.as_deref(), Some("Implement feature X"));
    }

    #[test]
    fn read_goal_snapshot_no_goal_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entry = make_entry("loop-auto");
        let goal_dir = tmp.path().join("artifacts/current/test");
        std::fs::create_dir_all(&goal_dir).unwrap();
        let goal = serde_json::json!({ "status": "active" });
        std::fs::write(
            goal_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&goal).unwrap(),
        ).unwrap();
        let snapshot = read_goal_snapshot(tmp.path(), &entry);
        assert!(snapshot.is_none());
    }

    // ── default_max_depth ───────────────────────────────────────────────

    #[test]
    fn default_max_depth_is_five() {
        assert_eq!(RunContext::default_max_depth(), 5);
    }

    // ── Issue 7: Checkpoint rescue ────────────────────────────────────────

    #[test]
    fn checkpoint_rescue_skips_done() {
        // Simulate a crashed run with "done" in dispatch
        let mut state = create_initial_state("checkpoint-test", "loop-auto");
        let run_id = generate_run_id("checkpoint-test");
        start_new_run(&mut state, &run_id);
        if let Some(ref mut run) = state.current_run {
            run.dispatch.insert("a1".to_string(), "done".to_string());
        }

        // Rescue logic (as in run_loop)
        let rescued: std::collections::HashMap<String, String> = state
            .current_run
            .as_ref()
            .map(|run| {
                run.dispatch
                    .iter()
                    .filter(|(_, status)| status.as_str() == "done")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(rescued.len(), 1);
        assert_eq!(rescued.get("a1").map(|s| s.as_str()), Some("done"));
    }

    #[test]
    fn checkpoint_rescue_does_not_skip_failed() {
        // "failed" actions should NOT be rescued (research escalation should retry them)
        let mut state = create_initial_state("checkpoint-test-2", "loop-auto");
        let run_id = generate_run_id("checkpoint-test-2");
        start_new_run(&mut state, &run_id);
        if let Some(ref mut run) = state.current_run {
            run.dispatch.insert("fail-1".to_string(), "failed".to_string());
            run.dispatch.insert("ok-1".to_string(), "done".to_string());
        }

        let rescued: std::collections::HashMap<String, String> = state
            .current_run
            .as_ref()
            .map(|run| {
                run.dispatch
                    .iter()
                    .filter(|(_, status)| status.as_str() == "done")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(rescued.len(), 1, "only 'done' should be rescued");
        assert!(rescued.contains_key("ok-1"), "completed action should be rescued");
        assert!(!rescued.contains_key("fail-1"), "failed actions must NOT be rescued");
    }

    #[test]
    fn checkpoint_rescue_insert_ordering() {
        // Verify rescued entries are inserted into new current_run BEFORE level insert
        // (see runner.rs line ~240 for the unconditional dispatch insert).
        // This test verifies that after rescue + start_new_run, the dispatch
        // has the rescued "done" entry, which the action loop skip check relies on.
        let mut state = create_initial_state("checkpoint-ordering", "loop-auto");
        let old_run_id = generate_run_id("checkpoint-ordering");
        start_new_run(&mut state, &old_run_id);
        if let Some(ref mut run) = state.current_run {
            run.dispatch.insert("a1".to_string(), "done".to_string());
        }

        // Simulate rescue
        let rescued: std::collections::HashMap<String, String> = state
            .current_run
            .as_ref()
            .map(|run| {
                run.dispatch
                    .iter()
                    .filter(|(_, status)| status.as_str() == "done")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Start new run and insert rescued entries
        let new_run_id = generate_run_id("checkpoint-ordering");
        start_new_run(&mut state, &new_run_id);
        if !rescued.is_empty() {
            if let Some(ref mut run) = state.current_run {
                run.dispatch.extend(rescued);
            }
        }

        // Verify rescued entry exists in new run's dispatch
        let dispatch = state.current_run.as_ref().map(|r| &r.dispatch);
        assert!(dispatch.is_some());
        let entry = dispatch.unwrap().get("a1");
        assert_eq!(entry.map(|s| s.as_str()), Some("done"),
            "rescued 'done' must survive into new current_run.dispatch for skip check");
    }

    // ── Issue 7: Checkpoint write ─────────────────────────────────────────

    #[test]
    fn checkpoint_writes_state_to_disk() {
        // Verify write_loop_state persists state that can be re-read
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let loop_id = "checkpoint-write-test";

        let mut state = create_initial_state(loop_id, "loop-auto");
        let run_id = generate_run_id(loop_id);
        start_new_run(&mut state, &run_id);
        if let Some(ref mut run) = state.current_run {
            run.dispatch.insert("a1".to_string(), "done".to_string());
        }

        // Write checkpoint (as done in the action loop)
        write_loop_state(root, loop_id, &state).unwrap();

        // Read back and verify
        let loaded = read_loop_state(root, loop_id).unwrap().unwrap();
        let dispatch = loaded
            .current_run
            .as_ref()
            .map(|r| r.dispatch.clone())
            .unwrap_or_default();
        assert_eq!(dispatch.get("a1").map(|s| s.as_str()), Some("done"),
            "checkpoint must persist dispatch state");
    }

    // ── Issue 9: require_human_approval ───────────────────────────────────
    // Note: BarrierResult unit tests already exist above.
    // These tests verify the logic that barrier_escalation uses internally.

    #[test]
    fn require_human_approval_blocks_resume() {
        // When require_human_approval=true, will_resume must be false
        // even when auto_resume=true and candidates exist.
        let br = BarrierResult {
            candidates: vec!["candidate-1".into()],
            will_resume: false, // simulate require_human_approval suppression
        };
        assert!(!br.should_resume(),
            "with require_human_approval=true, should_resume must be false");
    }

    #[test]
    fn require_human_approval_no_candidates() {
        // Even without require_human_approval, no candidates = no resume
        let br = BarrierResult {
            candidates: vec![],
            will_resume: true,
        };
        assert!(!br.should_resume());
    }

    // ── Issue 9: Freshness window ─────────────────────────────────────────

    #[test]
    fn barrier_candidates_stale_report() {
        // Report older than freshness window should be ignored.
        // Use freshness_window_secs=0 so age < 0 is always false.
        let tmp = tempfile::TempDir::new().unwrap();
        let barrier_dir = tmp.path().join("artifacts/research-barrier/2026-01-01T00-00-00Z");
        std::fs::create_dir_all(&barrier_dir).unwrap();
        let report = serde_json::json!({"candidates": ["old-candidate"]});
        std::fs::write(
            barrier_dir.join("BARRIER_REPORT.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        ).unwrap();

        // Use freshness_window_secs=0 so no report passes the check
        let candidates = discover_barrier_candidates(tmp.path(), 0);
        assert!(candidates.is_empty(), "stale report must be ignored");
    }
}
