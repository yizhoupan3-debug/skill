use crate::state::loop_reports_dir;
use crate::types::{AggregateActionEntry, LoopCloseoutAggregate, LoopRunState, UnconsumedFinding};
use std::fs;
use std::path::Path;

/// Render a Markdown loop report from the run state, closeout aggregate, unconsumed findings, and lock duration.
pub fn render_loop_report(
    state: &LoopRunState,
    aggregate: &LoopCloseoutAggregate,
    unconsumed: &[UnconsumedFinding],
    lock_duration_secs: Option<u64>,
) -> String {
    let mut md = String::new();

    md.push_str(&format!(
        "# Loop Report: {} | {}\n\n",
        state.loop_id, state.last_refreshed_at,
    ));

    let total_actions = aggregate.actions.len();
    let dispatched = aggregate
        .actions
        .iter()
        .filter(|a| a.execution != "skipped")
        .count();
    let skipped = total_actions - dispatched;
    md.push_str(&format!(
        "## Summary\n\
         - {total} actions, {dispatched} dispatched, {skipped} L1 report-only. Overall: {status}\n\n",
        total = total_actions,
        dispatched = dispatched,
        skipped = skipped,
        status = aggregate.overall_status.to_uppercase(),
    ));

    md.push_str("## Actions\n");
    for action in &aggregate.actions {
        md.push_str(&render_action_section(action));
    }

    if !unconsumed.is_empty() {
        md.push_str("\n## Unconsumed Findings\n");
        for finding in unconsumed {
            md.push_str(&format!(
                "- {} (from action {})\n",
                finding.finding, finding.source_action,
            ));
        }
    }

    if let Some(secs) = lock_duration_secs {
        let minutes = secs / 60;
        let remaining = secs % 60;
        md.push_str(&format!(
            "\n## Lock\n\
             - .loop-active: acquired → released. Duration: {}m{}s. No conflicts.\n",
            minutes, remaining,
        ));
    }

    // Anti-drift checks section
    if !state.anti_drift.drift_check_history.is_empty() {
        md.push_str("\n## Anti-Drift Checks\n\n");
        md.push_str(&format!(
            "Review cycle interval: {}\n\n",
            state.anti_drift.check_interval
        ));
        for check in &state.anti_drift.drift_check_history {
            md.push_str(&format!(
                "- Cycle {}: drift_detected={}, score={:.2}, type={}\n",
                check.review_cycle, check.drift_detected, check.drift_score, check.drift_type
            ));
        }
    }

    md
}

fn render_action_section(action: &AggregateActionEntry) -> String {
    let mut section = String::new();
    let level_display = action.safety_level.as_str();

    let execution_detail = match action.execution.as_str() {
        "committed" => "committed",
        "skipped" => "report only",
        "failed" => "FAILED",
        "interrupted" => "INTERRUPTED",
        other => other,
    };

    section.push_str(&format!(
        "### {}: {} ({})\n",
        action.action_id, level_display, execution_detail,
    ));

    if action.execution == "skipped" {
        section.push_str("- Report only.\n");
    }

    if let Some(ref cp) = action.closeout_path {
        section.push_str(&format!("- Closeout: {}\n", cp));
    }
    if let Some(ref sha) = action.commit_sha {
        section.push_str(&format!("- Commit: {} (not merged)\n", sha));
    }
    if let Some(ref v) = action.verification {
        section.push_str(&format!("- Verification: {}\n", v));
    }

    section.push('\n');
    section
}

/// Write a loop report Markdown string to `artifacts/loop/{loop_id}/reports/{run_id}.md`.
pub fn write_loop_report(
    repo_root: &Path,
    loop_id: &str,
    run_id: &str,
    report: &str,
) -> Result<String, String> {
    let dir = loop_reports_dir(repo_root, loop_id);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}.md", run_id));
    fs::write(&path, report).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_aggregate(status: &str) -> LoopCloseoutAggregate {
        LoopCloseoutAggregate {
            schema_version: "loop-closeout-aggregate-v1".into(),
            run_id: "run-test".into(),
            loop_id: "test-loop".into(),
            overall_status: status.into(),
            actions: vec![
                AggregateActionEntry {
                    action_id: "a1".into(),
                    safety_level: "L1".into(),
                    execution: "skipped".into(),
                    closeout_path: None,
                    verification: None,
                    commit_sha: None,
                    merged: None,
                },
                AggregateActionEntry {
                    action_id: "a2".into(),
                    safety_level: "L2".into(),
                    execution: "committed".into(),
                    closeout_path: Some("artifacts/closeout/a2.json".into()),
                    verification: Some("pass".into()),
                    commit_sha: Some("abc123".into()),
                    merged: Some(false),
                },
            ],
            escalated: false,
            partial: false,
        }
    }

    fn make_state() -> LoopRunState {
        LoopRunState {
            schema_version: "loop-run-state-v1".into(),
            loop_id: "test-loop".into(),
            profile: "loop-auto".into(),
            phase: "completed".into(),
            last_heartbeat: "2026-06-16T06:00:05Z".into(),
            current_run: None,
            history: vec![],
            circuit_breaker: CircuitBreaker::default(),
            anti_drift: AntiDriftState::default(),
            last_refreshed_at: "2026-06-16T06:00:05Z".into(),
        }
    }

    #[test]
    fn test_render_report_basic() {
        let state = make_state();
        let agg = make_aggregate("pass");
        let report = render_loop_report(&state, &agg, &[], Some(300));
        assert!(report.contains("Loop Report: test-loop"));
        assert!(report.contains("2 actions"));
        assert!(report.contains("Overall: PASS"));
        assert!(report.contains("a1"));
        assert!(report.contains("a2"));
        assert!(report.contains("5m0s"));
    }

    #[test]
    fn test_render_report_with_findings() {
        let state = make_state();
        let agg = make_aggregate("pass");
        let findings = vec![UnconsumedFinding {
            finding_hash: "sha256:abc".into(),
            source_action: "a2".into(),
            finding: "also in cli_utils.rs".into(),
        }];
        let report = render_loop_report(&state, &agg, &findings, None);
        assert!(report.contains("Unconsumed Findings"));
        assert!(report.contains("also in cli_utils.rs"));
    }

    #[test]
    fn test_render_report_partial() {
        let state = make_state();
        let agg = make_aggregate("partial");
        let report = render_loop_report(&state, &agg, &[], None);
        assert!(report.contains("Overall: PARTIAL"));
    }
}
