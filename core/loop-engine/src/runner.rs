use crate::types::{
    LoopError, LoopPhase, LoopRegistryEntry, LoopRunState, LoopAction,
    LoopCloseoutAggregate, LoopProfileConfig, SafetyLevel,
};
use crate::state::{
    create_initial_state, transition_phase, start_new_run, finish_run,
    generate_run_id, update_heartbeat, read_loop_state, write_loop_state,
    closeout_path,
};
use crate::kill_switch::{self, acquire_lock, release_lock};
use crate::safety::assign_safety_for_action;
use crate::dispatcher::{self, SubagentResult};
use crate::closeout::{
    read_action_record, verify_closeout_with_evidence, build_aggregate,
    AggregateActionResult,
};
use crate::report;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub fn preflight_profile_check(entry: &LoopRegistryEntry) -> Result<(), LoopError> {
    match entry.profile.as_str() {
        "interactive" | "my-light" => Err(LoopError::ProfileMismatch(
            "interactive/my-light profile is not schedulable. \
             Use loop-auto for unattended execution."
                .to_string(),
        )),
        "loop-auto" => Ok(()),
        other => Err(LoopError::UnknownProfile(other.to_string())),
    }
}

pub struct RunContext<'a> {
    pub repo_root: &'a Path,
    pub entry: &'a LoopRegistryEntry,
    pub dry_run: bool,
    pub timeout: Option<std::time::Duration>,
}

pub fn run_loop(ctx: &RunContext) -> Result<LoopCloseoutAggregate, LoopError> {
    let entry = ctx.entry;
    let loop_id = &entry.loop_id;

    preflight_profile_check(entry)?;

    let mut state = match read_loop_state(ctx.repo_root, loop_id)? {
        Some(s) => s,
        None => create_initial_state(loop_id, &entry.profile),
    };

    let run_id = generate_run_id(loop_id);
    start_new_run(&mut state, &run_id);
    transition_phase(&mut state, LoopPhase::Pending);

    if ctx.dry_run {
        transition_phase(&mut state, LoopPhase::Discovering);
        let actions = discover_actions(entry, ctx.repo_root)?;
        transition_phase(&mut state, LoopPhase::Preflight);
        let _ = assign_safety_levels(&actions, entry);
        transition_phase(&mut state, LoopPhase::Completed);
        let aggregate = build_aggregate(
            &run_id, loop_id, &actions,
            actions.iter().map(|a| (a.action_id.clone(), AggregateActionResult::Skipped)).collect(),
        );
        finish_run(&mut state, "dry-run");
        let _ = write_loop_state(ctx.repo_root, loop_id, &state);
        return Ok(aggregate);
    }

    acquire_lock(ctx.repo_root, loop_id, &run_id)?;
    let lock_start = Instant::now();

    let result = run_loop_inner(ctx, &mut state, &run_id, entry);

    let lock_secs = lock_start.elapsed().as_secs();
    let _ = release_lock(ctx.repo_root);

    match result {
        Ok(agg) => {
            let report_text = report::render_loop_report(&state, &agg, &state.current_run.as_ref().map(|r| r.unconsumed_findings.clone()).unwrap_or_default(), Some(lock_secs));
            let report_path = report::write_loop_report(ctx.repo_root, loop_id, &run_id, &report_text)
                .ok();
            if let Some(ref mut r) = state.current_run {
                r.report_path = report_path;
                r.closeout_aggregate = Some(agg.clone());
            }
            transition_phase(&mut state, LoopPhase::Completed);
            finish_run(&mut state, &agg.overall_status);
            let _ = write_loop_state(ctx.repo_root, loop_id, &state);
            Ok(agg)
        }
        Err(e) => {
            transition_phase(&mut state, LoopPhase::Escalated);
            finish_run(&mut state, "escalated");
            let _ = write_loop_state(ctx.repo_root, loop_id, &state);
            Err(e)
        }
    }
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
            closeout_enforcement: "hard-block".to_string(),
            review_gate: "mandatory".to_string(),
            spawn_first_nudge: true,
            cost_budget: entry.cost_budget.clone(),
            escalation: None,
        });
    let safety_map = assign_safety_levels(&actions, entry);
    check_budget_preflight(&profile_config)?;

    transition_phase(state, LoopPhase::Dispatching);
    let mut results: Vec<(String, AggregateActionResult)> = Vec::new();

    for action in &actions {
        let level = safety_map.get(&action.action_id).cloned()
            .unwrap_or(SafetyLevel::L1ReportOnly);

        if let Some(ref mut run) = state.current_run {
            run.dispatch.insert(action.action_id.clone(), level.as_str().to_string());
        }

        match level {
            SafetyLevel::L1ReportOnly => {
                results.push((action.action_id.clone(), AggregateActionResult::Skipped));
                if let Some(ref mut run) = state.current_run {
                    run.dispatch.insert(action.action_id.clone(), "skipped".to_string());
                }
            }
            SafetyLevel::L2AssistedFix | SafetyLevel::L3Unattended => {
                update_heartbeat(state);
                if let Some(ref mut run) = state.current_run {
                    run.dispatch.insert(action.action_id.clone(), "running".to_string());
                }
                let sub_result = dispatcher::run_action_sync(ctx.repo_root, &entry.loop_id, run_id, action, ctx.timeout);
                match sub_result {
                    Ok(output) => {
                        let aggregate_result = evaluate_subagent_output(ctx.repo_root, &entry.loop_id, run_id, action, &output);
                        if let Some(ref mut run) = state.current_run {
                            let status = match &aggregate_result {
                                AggregateActionResult::Committed { .. } => "done",
                                AggregateActionResult::Failed { .. } => "failed",
                                AggregateActionResult::Interrupted => "interrupted",
                                AggregateActionResult::Skipped => "skipped",
                            };
                            run.dispatch.insert(action.action_id.clone(), status.to_string());
                        }
                        results.push((action.action_id.clone(), aggregate_result));
                    }
                    Err(LoopError::KillSignaled(msg)) => {
                        results.push((action.action_id.clone(), AggregateActionResult::Failed {
                            reason: format!("killed: {msg}"),
                        }));
                        return Err(LoopError::KillSignaled(msg));
                    }
                    Err(LoopError::Timeout(secs)) => {
                        results.push((action.action_id.clone(), AggregateActionResult::Failed {
                            reason: format!("timeout after {secs}s"),
                        }));
                        return Err(LoopError::Timeout(secs));
                    }
                    Err(e) => {
                        results.push((action.action_id.clone(), AggregateActionResult::Failed {
                            reason: e.to_string(),
                        }));
                    }
                }
            }
        }
    }

    transition_phase(state, LoopPhase::Running);

    transition_phase(state, LoopPhase::Verifying);
    let aggregate = build_aggregate(run_id, &entry.loop_id, &actions, results);

    if aggregate.overall_status == "pass" {
        state.circuit_breaker.consecutive_failures = 0;
    } else if aggregate.overall_status == "fail" || aggregate.overall_status == "partial" {
        state.circuit_breaker.consecutive_failures += 1;
        if state.circuit_breaker.consecutive_failures >= 3 {
            return Err(LoopError::ActionFailed(
                "circuit breaker: 3 consecutive failures. Loop suspended.".to_string(),
            ));
        }
    }

    Ok(aggregate)
}

fn discover_actions(entry: &LoopRegistryEntry, repo_root: &std::path::Path) -> Result<Vec<LoopAction>, LoopError> {
    let skill_name = entry.skill.as_deref().unwrap_or(&entry.loop_id);
    let default_safety = entry.default_safety.as_deref().unwrap_or("L1");
    let _schedule_info = entry.trigger.schedule.as_deref().unwrap_or("manual");

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
    let mut child = std::process::Command::new(&binary)
        .args(["-p", &handoff])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| LoopError::SpawnFailed(format!("discovery {binary}: {e}")))?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let poll_interval = std::time::Duration::from_secs(dispatcher::KILL_POLL_INTERVAL_SECS);

    let output = loop {
        match child.try_wait().map_err(|e| LoopError::Io(format!("discovery try_wait: {e}")))? {
            Some(_status) => {
                break child.wait_with_output()
                    .map_err(|e| LoopError::Io(format!("discovery collect: {e}")))?;
            }
            None => {
                if crate::kill_switch::is_kill_signal_active(
                    repo_root,
                    &entry.loop_id,
                ) {
                    child.kill().map_err(|e| LoopError::Io(format!("discovery kill: {e}")))?;
                    child.wait().map_err(|e| LoopError::Io(format!("discovery wait: {e}")))?;
                    return Err(LoopError::KillSignaled(format!(
                        "discovery for loop {} killed by signal",
                        entry.loop_id,
                    )));
                }
                if std::time::Instant::now() > deadline {
                    child.kill().map_err(|e| LoopError::Io(format!("discovery kill timeout: {e}")))?;
                    child.wait().map_err(|e| LoopError::Io(format!("discovery wait timeout: {e}")))?;
                    return Err(LoopError::Timeout(300));
                }
                std::thread::sleep(poll_interval);
            }
        }
    };

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
            let scope_paths: Vec<String> = item.get("scope_paths")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let safety = item.get("safety")
                .and_then(|v| v.as_str())
                .unwrap_or(default_safety);
            let description = item.get("description").and_then(|v| v.as_str()).map(String::from);

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
        && let Some(max_tokens) = budget.tokens_per_run {
            tracing::info!(
                "budget preflight: tokens_per_run={max_tokens} (soft limit, enforcement={})",
                profile.closeout_enforcement,
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
    if record_path.is_file()
        && let Ok(Some(record)) = read_action_record(repo_root, loop_id, run_id, &action.action_id) {
            let verification = verify_closeout_with_evidence(&record.closeout, repo_root, &action.action_id);

            let scope_violations = if !action.scope_paths.is_empty() {
                dispatcher::check_scope_compliance(repo_root, &action.scope_paths)
            } else {
                Vec::new()
            };

            if !scope_violations.is_empty() {
                tracing::warn!(
                    "scope violation in action {}: {:?}",
                    action.action_id, scope_violations
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

pub fn run_loop_status(repo_root: &Path, loop_id: &str) -> Result<Option<LoopRunState>, LoopError> {
    read_loop_state(repo_root, loop_id)
}

pub fn run_loop_kill(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    kill_switch::write_kill_signal(repo_root, loop_id)
}

pub fn run_loop_kill_all(repo_root: &Path) -> Result<(), LoopError> {
    let registry_path = repo_root.join("configs").join("framework").join("LOOP_REGISTRY.json");
    let raw = fs::read_to_string(&registry_path)
        .map_err(|e| LoopError::Io(format!("read LOOP_REGISTRY.json: {e}")))?;
    let registry: crate::LoopRegistryRoot = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse LOOP_REGISTRY.json: {e}")))?;
    for entry in &registry.loops {
        kill_switch::write_kill_signal(repo_root, &entry.loop_id)?;
    }
    Ok(())
}

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
        }
    }

    #[test]
    fn test_accepts_loop_auto() {
        let entry = make_entry("loop-auto");
        assert!(preflight_profile_check(&entry).is_ok());
    }

    #[test]
    fn test_rejects_interactive() {
        let entry = make_entry("interactive");
        let result = preflight_profile_check(&entry);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoopError::ProfileMismatch(_)));
    }

    #[test]
    fn test_rejects_my_light() {
        let entry = make_entry("my-light");
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
        let entry = make_entry("loop-auto");
        let tmp = tempfile::TempDir::new().unwrap();
        let actions = discover_actions(&entry, tmp.path()).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].safety, "L1");
    }
}
