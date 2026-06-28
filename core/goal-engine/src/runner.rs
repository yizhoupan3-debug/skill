use crate::closeout::{
    AggregateActionResult, build_aggregate, read_action_record, verify_closeout_with_evidence,
};
use crate::dispatcher::{self, SubagentResult};
use crate::kill_switch::{self, acquire_lock, release_lock};
use crate::report;
use crate::safety::assign_safety_for_action;
use crate::state::{
    closeout_path, create_initial_state, finish_run, generate_run_id, read_loop_state,
    start_new_run, transition_phase, update_heartbeat, write_loop_state,
};
use crate::types::{
    LoopAction, LoopCloseoutAggregate, LoopError, LoopPhase, LoopProfileConfig, LoopRegistryEntry,
    LoopRunState, SafetyLevel,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::time::Instant;

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
        let mut state = match read_loop_state(ctx.repo_root, loop_id)? {
            Some(s) => s,
            None => create_initial_state(loop_id, &entry.profile),
        };

        let run_id = generate_run_id(loop_id);
        start_new_run(&mut state, &run_id);
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
            return Ok(aggregate);
        }

        acquire_lock(ctx.repo_root, loop_id, &run_id)?;
        let lock_start = Instant::now();

        let result = run_loop_inner(ctx, &mut state, &run_id, entry);

        match result {
            Ok(agg) => {
                let findings = state
                    .current_run
                    .as_ref()
                    .map(|r| r.unconsumed_findings.clone())
                    .unwrap_or_default();
                let elapsed = Some(lock_start.elapsed().as_secs());
                let report_text = report::render_loop_report(&state, &agg, &findings, elapsed);
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
                if let Err(e) = release_lock(ctx.repo_root) {
                    tracing::error!("failed to release lock on success path: {e}");
                }
                break Ok(agg);
            }
            Err(LoopError::ResearchEscalation(msg)) => {
                // Research completed with candidates: restart the loop to consume them
                tracing::info!("[goal-engine] {msg}");
                state.circuit_breaker.consecutive_failures = 0;
                if let Err(e) = write_loop_state(ctx.repo_root, loop_id, &state) {
                    tracing::error!("failed to write loop state on research escalation: {e}");
                }
                if let Err(e) = release_lock(ctx.repo_root) {
                    tracing::error!("failed to release lock on research escalation: {e}");
                }

                if depth_remaining == 0 {
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
                if let Err(lock_err) = release_lock(ctx.repo_root) {
                    tracing::error!("failed to release lock on error path: {lock_err}");
                }
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
    transition_phase(state, LoopPhase::Discovering);
    let actions = discover_actions(entry, ctx.repo_root)?;

    if let Some(ref mut run) = state.current_run {
        run.discovery = Some(crate::types::DiscoveryResult {
            actions_found: actions.len() as u32,
            actions: actions.clone(),
        });
    }

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
        });
    let safety_map = assign_safety_levels(&actions, entry);
    check_budget_preflight(&profile_config)?;

    transition_phase(state, LoopPhase::Running);
    let mut results: Vec<(String, AggregateActionResult)> = Vec::new();

    for action in &actions {
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
            SafetyLevel::L2AssistedFix | SafetyLevel::L3Unattended => {
                update_heartbeat(state);
                if let Some(ref mut run) = state.current_run {
                    run.dispatch
                        .insert(action.action_id.clone(), "running".to_string());
                }
                let sub_result = dispatcher::run_action_sync(
                    ctx.repo_root,
                    &entry.loop_id,
                    run_id,
                    action,
                    ctx.timeout,
                );
                match sub_result {
                    Ok(output) => {
                        let aggregate_result = evaluate_subagent_output(
                            ctx.repo_root,
                            &entry.loop_id,
                            run_id,
                            action,
                            &output,
                        );
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

    transition_phase(state, LoopPhase::Verifying);
    let mut aggregate = build_aggregate(run_id, &entry.loop_id, &actions, results);

    // For loops with verify_rfv_convergence enabled, additionally verify RFV convergence state.
    // If the aggregate passes but RFV hasn't converged, mark as fail.
    if entry.verify_rfv_convergence.unwrap_or(false) && aggregate.overall_status == "pass" {
        // Derive task_id from action_id (strip "-orchestrator" suffix)
        let task_id = actions
            .first()
            .map(|a| {
                a.action_id
                    .strip_suffix("-orchestrator")
                    .unwrap_or(&a.action_id)
                    .to_string()
            })
            .unwrap_or_default();
        if let Err(violations) = crate::closeout::verify_rfv_convergence(ctx.repo_root, &task_id) {
            tracing::warn!(
                "RFV convergence not met (verify_rfv_convergence=true): {}",
                violations.join(", ")
            );
            aggregate.overall_status = "fail".to_string();
        }
    }

    // Anti-drift check: after each review cycle, increment counter
    // and fire drift check every N cycles (default 3).
    state.anti_drift.review_cycle_count += 1;
    let should_check = crate::drift::should_check_drift(&state.anti_drift);
    let current_goal = read_goal_snapshot(ctx.repo_root, entry);
    if current_goal.is_none() && should_check && state.anti_drift.original_goal_snapshot.is_some() {
        tracing::warn!(
            "[goal-engine] anti-drift check at cycle {} skipped: cannot read current goal (GOAL_STATE.json not found at artifacts/current/{}/)",
            state.anti_drift.review_cycle_count,
            entry.loop_id,
        );
    }
    if should_check && let Some(current_goal_text) = current_goal {
        let result = crate::drift::perform_drift_check(&mut state.anti_drift, &current_goal_text);
        tracing::warn!(
            "[goal-engine] anti-drift check at cycle {}: drift_detected={}, score={:.2}",
            result.review_cycle,
            result.drift_detected,
            result.drift_score
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
                    return Err(LoopError::ActionFailed(
                        "circuit breaker: escalated to research; awaiting human approval."
                            .to_string(),
                    ));
                }
            } else {
                return Err(LoopError::ActionFailed(
                    "circuit breaker: 3 consecutive failures. Loop suspended.".to_string(),
                ));
            }
        }
    }

    Ok(aggregate)
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
    let output = if autoresearch_bin.is_empty() {
        // Slow-path: compile and run via cargo with a timeout
        let mut cmd = std::process::Command::new("cargo");
        cmd.args([
            "run",
            "-p",
            "research-harness",
            "--bin",
            "autoresearch",
            "--",
            "barrier",
            "--problem",
            &problem,
            "--loop-id",
            loop_id,
            "--run-id",
            run_id,
        ])
        .current_dir(repo_root);

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| dispatcher::apply_subprocess_rlimits());
        }

        let child = cmd
            .spawn()
            .map_err(|e| LoopError::SpawnFailed(format!("barrier escalation: {e}")))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        crate::dispatcher::poll_subprocess(
            child,
            repo_root,
            loop_id,
            "barrier-escalation",
            deadline,
            std::time::Duration::from_secs(300),
        )?
    } else {
        let mut cmd = std::process::Command::new(&autoresearch_bin);
        cmd.args([
            "barrier",
            "--problem",
            &problem,
            "--loop-id",
            loop_id,
            "--run-id",
            run_id,
        ])
        .current_dir(repo_root);
        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| dispatcher::apply_subprocess_rlimits());
        }
        cmd.output()
            .map_err(|e| LoopError::ActionFailed(format!("barrier escalation failed: {e}")))?
    };

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
    let candidates = discover_barrier_candidates(repo_root);

    tracing::info!(
        "[goal-engine] barrier escalation to {escalation_target}: {} candidates, auto_resume={auto_resume}",
        candidates.len()
    );

    Ok(BarrierResult {
        candidates,
        will_resume: auto_resume,
    })
}

/// Scan artifacts/research-barrier/ for the most recent BARRIER_REPORT.json.
fn discover_barrier_candidates(repo_root: &Path) -> Vec<String> {
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
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return vec![],
    };
    entries.sort_by_key(|e| e.path());
    if let Some(latest) = entries.last() {
        let report_path = latest.path().join("BARRIER_REPORT.json");
        if report_path.exists()
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
        }]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actions = parse_discovery_output(&stdout, entry, default_safety);

    if actions.is_empty() {
        tracing::info!("discovery returned no actions for loop {}", entry.loop_id);
    }

    Ok(actions)
}

fn parse_discovery_output(
    output: &str,
    _entry: &LoopRegistryEntry,
    default_safety: &str,
) -> Vec<LoopAction> {
    let json_start = output.find('[');
    let json_end = output.rfind(']');
    let json_str = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => &output[start..=end],
        _ => return Vec::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse discovery output as JSON: {e}");
            return Vec::new();
        }
    };

    let arr = match parsed.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
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
            })
        })
        .collect()
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
    let goal_path = repo_root
        .join("artifacts")
        .join("current")
        .join(&entry.loop_id)
        .join("GOAL_STATE.json");
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
            verify_rfv_convergence: None,
            static_actions: None,
        }
    }

    #[test]
    fn test_accepts_loop_auto() {
        let entry = make_entry("loop-auto");
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
        }]);
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = discover_actions(&entry, tmp.path()).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].safety, "L1");
    }
}
