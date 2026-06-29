//! Closeout tool handlers (`domain:closeout`).
//! closeout_record_write (file I/O + evaluation) and closeout_gate_evaluate.

use core_errors::FrameworkError;
use serde_json::{Map, Value, json};
use std::path::Path;

/// closeout_record_write: payload construction + file I/O + evaluation.
pub(crate) fn closeout_record_write_dispatch(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: task_id"))?;
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: summary"))?;
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation("Missing required argument: verification_status")
        })?;
    match verification_status {
        "passed" | "failed" | "partial" | "not_run" => {}
        _ => {
            return Err(FrameworkError::validation(format!(
                "Invalid verification_status: {verification_status}. Must be one of: passed, failed, partial, not_run"
            )));
        }
    }

    let mut record = Map::new();
    record.insert(
        "schema_version".to_string(),
        json!(host_projection::hooks::closeout_record_schema_version()),
    );
    record.insert("task_id".to_string(), json!(task_id));
    record.insert(
        "ended_at".to_string(),
        json!(host_projection::hooks::current_local_timestamp()),
    );
    record.insert("summary".to_string(), json!(summary));
    record.insert(
        "verification_status".to_string(),
        json!(verification_status),
    );

    if let Some(files) = arguments.get("changed_files").and_then(Value::as_array) {
        record.insert("changed_files".to_string(), json!(files));
    }
    if let Some(cmds) = arguments.get("commands_run").and_then(Value::as_array) {
        record.insert("commands_run".to_string(), json!(cmds));
    }
    if let Some(blockers) = arguments.get("blockers").and_then(Value::as_array)
        && !blockers.is_empty()
    {
        record.insert("blockers".to_string(), json!(blockers));
    }
    if let Some(risks) = arguments.get("risks").and_then(Value::as_array)
        && !risks.is_empty()
    {
        record.insert("risks".to_string(), json!(risks));
    }
    if let Some(notes) = arguments.get("notes").and_then(Value::as_str)
        && !notes.is_empty()
    {
        record.insert("notes".to_string(), json!(notes));
    }

    let record_value = serde_json::Value::Object(record);

    // Evaluate BEFORE writing to disk (evaluate-then-write pattern)
    let (_, has_success) =
        core_state::state_manager::task_evidence_artifacts_summary_for_task(repo_root, task_id);
    let goal_state = core_state::state_manager::read_goal_state(repo_root, Some(task_id))
        .ok()
        .flatten();
    let goal_prediction = goal_state
        .as_ref()
        .and_then(core_state::goal_prediction::read_goal_prediction);
    let ctx = core_state::closeout_validation::CloseoutEvidenceContext {
        task_id: Some(task_id.to_string()),
        has_successful_verification: has_success,
        goal_prediction,
    };
    let eval_result = core_state::closeout_validation::evaluate_closeout_record_value_with_context(
        record_value.clone(),
        &ctx,
    );

    let eval = match eval_result {
        Ok(v) => v,
        Err(e) => json!({"error": e.to_string()}),
    };

    let closeout_allowed = eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let violations: Vec<String> = eval
        .get("violations")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let rule = v.get("rule").and_then(Value::as_str).unwrap_or("unknown");
                    let detail = v
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("no detail");
                    format!("[{rule}] {detail}")
                })
                .collect()
        })
        .unwrap_or_default();

    let result = json!({
        "closeout_allowed": closeout_allowed,
        "violations": violations,
    });

    // Write to disk after evaluation — only if closeout is allowed
    let record_path = host_projection::hooks::closeout_record_path_for_task(repo_root, task_id)?;
    if !closeout_allowed {
        // Write failed record with .failed suffix for diagnostics, without polluting
        // the normal closeout path.
        let failed_path = {
            let mut p = record_path.clone();
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            p.set_file_name(format!("{}.failed.json", name));
            p
        };
        if let Some(parent) = failed_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(FrameworkError::Io)?;
        }
        core_state_utils::atomic_write::write_atomic_json(&failed_path, &record_value)?;
    } else {
        if let Some(parent) = record_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(FrameworkError::Io)?;
        }
        core_state_utils::atomic_write::write_atomic_json(&record_path, &record_value)?;
    }

    Ok(serde_json::to_string_pretty(&result)
        .map_err(FrameworkError::Json)?)
}

/// closeout_gate_evaluate: multi-source closeout readiness evaluation.
pub(crate) fn closeout_gate_evaluate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let task_id_override = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let task_view = core_state::task_state::resolve_task_view(repo_root, task_id_override);
    let mut findings: Vec<String> = Vec::new();

    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "closeout_evidence_hooks",
        )
    {
        findings.push(format!("harness: closeout_evidence_hooks — {rationale}"));
    }
    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "review_gate_router_observation",
        )
    {
        findings.push(format!(
            "harness: review_gate_router_observation — {rationale}"
        ));
    }

    findings.push(format!(
        "review_gate: {host_id} has no hook REVIEW_GATE — reviewer evidence is honor-system / self-attested"
    ));

    let goal_present = task_view.goal_state.is_some();
    if !goal_present {
        findings.push("goal_state: no GOAL_STATE.json".to_string());
    } else {
        findings.push("goal_state: present".to_string());
    }

    let evidence_success = task_view
        .evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);
    let task_id = task_view.task_id.as_deref().unwrap_or("");

    if !evidence_success {
        findings.push("evidence: no successful EVIDENCE_INDEX records".to_string());
    } else {
        findings.push("evidence: successful records present".to_string());
        if !task_id.is_empty()
            && core_state::state_manager::task_evidence_success_only_self_attested(
                repo_root, task_id,
            )
        {
            findings.push("WARN: evidence: only self-attested MCP record_evidence rows — verify independently".to_string());
        }
    }

    let summary_path = repo_root
        .join("artifacts")
        .join("current")
        .join(if task_id.is_empty() { "" } else { task_id })
        .join("SESSION_SUMMARY.md");
    let summary_rel = summary_path
        .strip_prefix(repo_root)
        .unwrap_or_else(|_| Path::new(summary_path.file_name().unwrap_or_default()));
    let has_summary = summary_path.is_file();
    if !has_summary {
        findings.push(format!(
            "checkpoint: missing SESSION_SUMMARY at {}",
            summary_rel.display()
        ));
    } else {
        findings.push("checkpoint: SESSION_SUMMARY.md on disk".to_string());
    }

    let review_goal = task_view
        .goal_state
        .as_ref()
        .is_some_and(check_goal_suggests_review);

    // desktop_review_evidence_attested uses args.reviewer_lane + fork_context
    let has_review_evidence = arguments
        .get("reviewer_lane")
        .and_then(Value::as_str)
        .is_some()
        || arguments.get("fork_context").is_some();

    if review_goal && !has_review_evidence {
        findings.push(
            "WARN: review_gate: GOAL suggests review work but no reviewer evidence — \
             pass reviewer_lane + fork_context in closeout_gate args"
                .to_string(),
        );
    } else if review_goal {
        findings.push("review_gate: GOAL suggests review; reviewer evidence attested".to_string());
    }

    let all_clear = compute_closeout_all_clear(
        goal_present, evidence_success, has_summary,
        review_goal,
        has_review_evidence,
    );
    let checkpoint_only =
        !all_clear && goal_present && evidence_success && (!review_goal || has_review_evidence);

    let verdict_label = if all_clear {
        "PASS: all closeout gates satisfied"
    } else if checkpoint_only {
        "ADVISORY: checkpoint missing — call session_checkpoint before complete"
    } else {
        "ADVISORY: closeout gates not satisfied"
    };

    let formatted = format!("[Closeout Gate] {verdict_label}\n\n{}", findings.join("\n"));
    Ok(serde_json::to_string(&json!({"result": formatted})).map_err(FrameworkError::Json)?)
}

/// Hook-compatible closeout gate evaluation wrapper.
///
/// Parses a Value payload, calls `closeout_gate_evaluate()`, returns structured
/// Value with passed/findings/result. Registered as `evaluate_closeout_gate`
/// in RuntimeCoreHooks.
///
/// Payload: { repo_root: String, task_id: String, host_id: String }
/// Returns: { passed: bool, findings: Vec<String>, result: String }
pub(crate) fn evaluate_closeout_gate_hook(
    payload: serde_json::Value,
) -> Result<serde_json::Value, core_errors::FrameworkError> {
    use std::path::Path;

    let repo_root_str = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let repo_root = Path::new(repo_root_str);
    let host_id = payload
        .get("host_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let task_id = payload.get("task_id").and_then(|v| v.as_str());
    let task_view = core_state::task_state::resolve_task_view(repo_root, task_id);

    let mut findings: Vec<String> = Vec::new();

    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "closeout_evidence_hooks",
        )
    {
        findings.push(format!("harness: closeout_evidence_hooks — {rationale}"));
    }
    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "review_gate_router_observation",
        )
    {
        findings.push(format!(
            "harness: review_gate_router_observation — {rationale}"
        ));
    }

    findings.push(format!(
        "review_gate: {host_id} has no hook REVIEW_GATE — reviewer evidence is honor-system / self-attested"
    ));

    let goal_present = task_view.goal_state.is_some();
    if !goal_present {
        findings.push("goal_state: no GOAL_STATE.json".to_string());
    } else {
        findings.push("goal_state: present".to_string());
    }

    let evidence_success = task_view
        .evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);
    let tid = task_view.task_id.as_deref().unwrap_or("");

    if !evidence_success {
        findings.push("evidence: no successful EVIDENCE_INDEX records".to_string());
    } else {
        findings.push("evidence: successful records present".to_string());
        if !tid.is_empty()
            && core_state::state_manager::task_evidence_success_only_self_attested(repo_root, tid)
        {
            findings.push(
                "WARN: evidence: only self-attested MCP record_evidence rows — verify independently"
                    .to_string(),
            );
        }
    }

    let summary_path = repo_root
        .join("artifacts")
        .join("current")
        .join(if tid.is_empty() { "" } else { tid })
        .join("SESSION_SUMMARY.md");
    let summary_rel = summary_path
        .strip_prefix(repo_root)
        .unwrap_or_else(|_| Path::new(summary_path.file_name().unwrap_or_default()));
    let has_summary = summary_path.is_file();
    if !has_summary {
        findings.push(format!(
            "checkpoint: missing SESSION_SUMMARY at {}",
            summary_rel.display()
        ));
    } else {
        findings.push("checkpoint: SESSION_SUMMARY.md on disk".to_string());
    }

    let review_goal = task_view
        .goal_state
        .as_ref()
        .is_some_and(check_goal_suggests_review);
    // P2-008: Optional reviewer_lane/fork_context from hook payload
    let has_review_evidence = payload
        .get("reviewer_lane")
        .and_then(|v| v.as_str())
        .is_some()
        || payload.get("fork_context").is_some();

    if review_goal {
        if !has_review_evidence {
            findings.push(
                "WARN: review_gate: GOAL suggests review work but hook path has no reviewer evidence \
                 — pass reviewer_lane + fork_context in payload"
                    .to_string(),
            );
        } else {
            findings.push("review_gate: GOAL suggests review; reviewer evidence attested".to_string());
        }
    }

    let all_clear = compute_closeout_all_clear(
        goal_present, evidence_success, has_summary,
        review_goal,
        false, // hook path has no reviewer evidence args
    );
    let passed = all_clear;

    // P2-012: Three-level verdict matching the MCP tool path.
    // checkpoint-only means only SESSION_SUMMARY.md is missing.
    let checkpoint_only =
        !all_clear && goal_present && evidence_success && (!review_goal || false);

    let verdict_label = if all_clear {
        "PASS: all closeout gates satisfied"
    } else if checkpoint_only {
        "ADVISORY: checkpoint missing — call session_checkpoint before complete"
    } else {
        "ADVISORY: closeout gates not satisfied"
    };

    let result = format!("[Closeout Gate] {verdict_label}");
    Ok(serde_json::json!({
        "passed": passed,
        "result": result,
        "findings": findings,
    }))
}

/// Shared all-clear computation for closeout gate evaluation.
/// Used by both the MCP tool path and the hook path.
fn compute_closeout_all_clear(
    goal_present: bool,
    evidence_success: bool,
    has_summary: bool,
    needs_review_evidence: bool,
    has_review_evidence: bool,
) -> bool {
    if !goal_present || !evidence_success || !has_summary {
        return false;
    }
    if needs_review_evidence && !has_review_evidence {
        return false;
    }
    true
}

/// Minimal check: does the goal mention review-related work?
fn check_goal_suggests_review(goal_state: &Value) -> bool {
    let goal_text = goal_state.get("goal").and_then(Value::as_str).unwrap_or("");
    let review_markers = ["review", "审计", "审稿", "check", "verify", "验证"];
    review_markers.iter().any(|m| goal_text.contains(m))
}
