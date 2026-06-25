//! Extracted tool handler business logic.
//!
//! These functions were moved from `host-projection/src/hosts/mcp_stdio_harness/tools.rs`
//! (L0) into runtime-core (L4) to eliminate the L0→L3 dependency on core-state.
//! Each function corresponds to a hook slot in `host_projection::hooks`.
//!
//! Parameter validation and cache invalidation remain in the host-projection layer;
//! this module owns payload construction, domain logic, and core-state interaction.

use serde_json::{Value, json, Map};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

// ── Routing evolution types ──

#[derive(serde::Deserialize)]
struct RouteLogEntry {
    ts: Option<String>,
    kind: Option<String>,
    task: Option<String>,
    skill: Option<String>,
    confidence: Option<f32>,
    reroute: Option<bool>,
    parity_gate: Option<String>,
}

// ── Tool handler functions ──

/// goal_state_manage: payload construction + core_state state_manager call.
pub fn goal_state_manage_dispatch(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation")?;

    // Auto-resolve task_id from TASK_POINTERS.json
    let task_id = match arguments.get("task_id").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(tid) => tid.to_string(),
        None => core_state::state_manager::read_primary_task_id(repo_root)
            .ok_or("No active task_id in TASK_POINTERS.json (start a task first or provide task_id explicitly)")?,
    };

    let repo_root_str = repo_root.to_string_lossy().to_string();
    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
        "task_id": task_id,
    });

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or("start requires 'goal' argument (string)")?;
            payload["goal"] = json!(goal);

            let drive_until_done = arguments
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            payload["drive_until_done"] = json!(drive_until_done);

            // Auto-fill contract fields when drive_until_done=true and not explicitly provided
            if drive_until_done {
                if arguments.get("non_goals").is_none() {
                    payload["non_goals"] = json!(["不处理此 goal 范围外的功能"]);
                }
                if arguments.get("done_when").is_none() {
                    payload["done_when"] = json!([
                        format!("goal 已完成: {goal}"),
                        "cargo check / test 通过".to_string(),
                    ]);
                }
                if arguments.get("validation_commands").is_none() {
                    payload["validation_commands"] = json!(["cargo check --workspace", "cargo test --workspace"]);
                }
            }

            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments.get("validation_commands").and_then(Value::as_array) {
                payload["validation_commands"] = json!(vc);
            }

            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);

            if let Some(gt) = arguments.get("goal_type").and_then(Value::as_str) {
                match gt {
                    "linear" | "loop" => payload["goal_type"] = json!(gt),
                    _ => return Err(format!("Invalid goal_type: {gt}. Must be one of: linear, loop")),
                }
            }
            if let Some(lp) = arguments.get("lifecycle_profile").and_then(Value::as_str) {
                match lp {
                    "task" | "loop-auto" => payload["lifecycle_profile"] = json!(lp),
                    _ => return Err(format!("Invalid lifecycle_profile: {lp}. Must be one of: task, loop-auto")),
                }
            }
            if let Some(ch) = arguments.get("current_horizon").and_then(Value::as_str) {
                payload["current_horizon"] = json!(ch);
            }
            if let Some(cg) = arguments.get("completion_gates") {
                payload["completion_gates"] = cg.clone();
            }
            if let Some(md) = arguments.get("metadata") {
                payload["metadata"] = md.clone();
            }
            if let Some(sf) = arguments.get("set_focus").and_then(Value::as_bool) {
                payload["set_focus"] = json!(sf);
            }
        }
        "checkpoint" => {
            let note = arguments
                .get("note")
                .and_then(Value::as_str)
                .ok_or("checkpoint requires 'note' argument (string)")?;
            payload["note"] = json!(note);
        }
        "block" => {
            let blocker = arguments
                .get("blocker")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or("block requires 'blocker' argument (string)")?;
            payload["blocker"] = json!(blocker);
        }
        "append_round" => {
            return Err("append_round is not a valid goal_state_manage operation. \
                 Use quality_gate_manage with operation=append_round instead."
                .to_string());
        }
        "pause" | "resume" | "complete" | "clear" => {}
        "amend" => {
            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments.get("validation_commands").and_then(Value::as_array) {
                payload["validation_commands"] = json!(vc);
            }
            if let Some(g) = arguments.get("goal").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
                payload["goal"] = json!(g);
            }
            if let Some(kp) = arguments.get("keep_progress").and_then(Value::as_bool) {
                payload["keep_progress"] = json!(kp);
            }
        }
        _ => return Err(format!(
            "Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block, amend"
        )),
    }

    let result = core_state::state_manager::framework_goal_drive(payload)?;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

/// quality_gate_manage: payload construction + registered quality gate hook call.
pub fn quality_gate_manage_dispatch(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation (string)")?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or("start requires 'goal' argument (string)")?;
            payload["goal"] = json!(goal);
            if let Some(mr) = arguments.get("max_rounds").and_then(Value::as_u64) {
                payload["max_rounds"] = json!(mr);
            }
            if let Some(er) = arguments.get("allow_external_research").and_then(Value::as_bool) {
                payload["allow_external_research"] = json!(er);
            }
            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);
        }
        "append_round" => {
            let round = arguments
                .get("round")
                .and_then(Value::as_u64)
                .ok_or("append_round requires 'round' argument (integer)")?;
            payload["round"] = json!(round);

            let review_summary = arguments
                .get("review_summary")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'review_summary' argument (string)")?;
            payload["review_summary"] = json!(review_summary);

            let fix_summary = arguments
                .get("fix_summary")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'fix_summary' argument (string)")?;
            payload["fix_summary"] = json!(fix_summary);

            let verify_result = arguments
                .get("verify_result")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'verify_result' argument (string)")?;
            if !matches!(verify_result, "PASS" | "FAIL" | "SKIPPED" | "UNKNOWN") {
                return Err(format!("verify_result must be one of PASS/FAIL/SKIPPED/UNKNOWN, got: {verify_result}"));
            }
            payload["verify_result"] = json!(verify_result);
            payload["supervisor_decision"] = json!(arguments
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'supervisor_decision' argument (string)")?);
            payload["reason"] = json!(arguments
                .get("reason")
                .and_then(Value::as_str)
                .ok_or("append_round requires 'reason' argument (string)")?);
        }
        _ => return Err(format!(
            "Unknown quality gate operation: {operation}. Valid operations: start, append_round"
        )),
    }

    // Delegate to the registered quality gate hook (runtime_exit_gate)
    let result = match host_projection::hooks::quality_gate_drive_registered() {
        Some(f) => f(payload)?,
        None => return Err("framework_quality_gate runtime-core hook not registered; \
                             runtime-core::boot() must be called before quality gate operations"
            .to_string()),
    };

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

/// closeout_record_write: payload construction + file I/O + evaluation.
pub fn closeout_record_write_dispatch(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: task_id")?;
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: summary")?;
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: verification_status")?;
    match verification_status {
        "passed" | "failed" | "partial" | "not_run" => {}
        _ => return Err(format!(
            "Invalid verification_status: {verification_status}. Must be one of: passed, failed, partial, not_run"
        )),
    }

    let mut record = Map::new();
    record.insert("schema_version".to_string(), json!(host_projection::hooks::closeout_record_schema_version()));
    record.insert("task_id".to_string(), json!(task_id));
    record.insert("ended_at".to_string(), json!(host_projection::hooks::current_local_timestamp()));
    record.insert("summary".to_string(), json!(summary));
    record.insert("verification_status".to_string(), json!(verification_status));

    if let Some(files) = arguments.get("changed_files").and_then(Value::as_array) {
        record.insert("changed_files".to_string(), json!(files));
    }
    if let Some(cmds) = arguments.get("commands_run").and_then(Value::as_array) {
        record.insert("commands_run".to_string(), json!(cmds));
    }
    if let Some(blockers) = arguments.get("blockers").and_then(Value::as_array)
        && !blockers.is_empty() {
            record.insert("blockers".to_string(), json!(blockers));
        }
    if let Some(risks) = arguments.get("risks").and_then(Value::as_array)
        && !risks.is_empty() {
            record.insert("risks".to_string(), json!(risks));
        }
    if let Some(notes) = arguments.get("notes").and_then(Value::as_str)
        && !notes.is_empty() {
            record.insert("notes".to_string(), json!(notes));
        }

    let record_path = host_projection::hooks::closeout_record_path_for_task(repo_root, task_id)
        .map_err(|e| format!("invalid task_id: {e}"))?;
    if let Some(parent) = record_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create closeout directory failed: {e}"))?;
    }

    let record_value = serde_json::Value::Object(record);
    core_state_utils::atomic_write::write_atomic_json(&record_path, &record_value)
        .map_err(|e| format!("write closeout record failed: {e}"))?;

    // Evaluate the record
    let eval_result =
        host_projection::hooks::evaluate_closeout_record_file_for_task(repo_root, task_id, &record_path);
    let eval = match eval_result {
        Ok(v) => v,
        Err(e) => json!({"error": e}),
    };

    let closeout_allowed = eval.get("closeout_allowed").and_then(Value::as_bool).unwrap_or(false);
    let violations: Vec<String> = eval.get("violations").and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let rule = v.get("rule").and_then(Value::as_str).unwrap_or("unknown");
                    let detail = v.get("detail").and_then(Value::as_str).unwrap_or("no detail");
                    format!("[{rule}] {detail}")
                })
                .collect()
        })
        .unwrap_or_default();

    let result = json!({
        "closeout_allowed": closeout_allowed,
        "violations": violations,
    });

    serde_json::to_string_pretty(&result).map_err(|e| format!("serialize closeout result failed: {e}"))
}

/// closeout_gate_evaluate: multi-source closeout readiness evaluation.
pub fn closeout_gate_evaluate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, String> {
    let task_id_override = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let task_view = core_state::task_state::resolve_task_view(repo_root, task_id_override);
    let mut findings: Vec<String> = Vec::new();

    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root, host_id, "closeout_evidence_hooks",
        ) {
        findings.push(format!("harness: closeout_evidence_hooks — {rationale}"));
    }
    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root, host_id, "review_gate_router_observation",
        ) {
        findings.push(format!("harness: review_gate_router_observation — {rationale}"));
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

    let evidence_success = task_view.evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);
    let task_id = task_view.task_id.as_deref().unwrap_or("");

    if !evidence_success {
        findings.push("evidence: no successful EVIDENCE_INDEX records".to_string());
    } else {
        findings.push("evidence: successful records present".to_string());
        if !task_id.is_empty()
            && core_state::state_manager::task_evidence_success_only_self_attested(repo_root, task_id)
        {
            findings.push("WARN: evidence: only self-attested MCP record_evidence rows — verify independently".to_string());
        }
    }

    let summary_path = repo_root.join("artifacts").join("current")
        .join(if task_id.is_empty() { "" } else { task_id })
        .join("SESSION_SUMMARY.md");
    let has_summary = summary_path.is_file();
    if !has_summary {
        findings.push(format!("checkpoint: missing SESSION_SUMMARY at {}", summary_path.display()));
    } else {
        findings.push("checkpoint: SESSION_SUMMARY.md on disk".to_string());
    }

    let review_goal = task_view.goal_state
        .as_ref()
        .is_some_and(check_goal_suggests_review);

    // desktop_review_evidence_attested uses args.reviewer_lane + fork_context
    let has_review_evidence = arguments.get("reviewer_lane").and_then(Value::as_str).is_some()
        || arguments.get("fork_context").is_some();

    if review_goal && !has_review_evidence {
        findings.push("WARN: review_gate: GOAL suggests review work but no reviewer evidence — \
             pass reviewer_lane + fork_context in closeout_gate args".to_string());
    } else if review_goal {
        findings.push("review_gate: GOAL suggests review; reviewer evidence attested".to_string());
    }

    let all_clear = goal_present && evidence_success && has_summary
        && (!review_goal || has_review_evidence);
    let checkpoint_only = !all_clear && goal_present && evidence_success
        && (!review_goal || has_review_evidence);

    let verdict_label = if all_clear {
        "PASS: all closeout gates satisfied"
    } else if checkpoint_only {
        "ADVISORY: checkpoint missing — call session_checkpoint before complete"
    } else {
        "ADVISORY: closeout gates not satisfied"
    };

    let formatted = format!("[Closeout Gate] {verdict_label}\n\n{}", findings.join("\n"));
    serde_json::to_string(&json!({"result": formatted})).map_err(|e| e.to_string())
}

/// Minimal check: does the goal mention review-related work?
fn check_goal_suggests_review(goal_state: &Value) -> bool {
    let goal_text = goal_state.get("goal").and_then(Value::as_str).unwrap_or("");
    let review_markers = ["review", "审计", "审稿", "check", "verify", "验证"];
    review_markers.iter().any(|m| goal_text.contains(m))
}

/// routing_evolution: read telemetry journal, aggregate, and report.
pub fn routing_evolution_dispatch(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation (stats|analyze|extract|calibrate)")?;
    let skill_filter = arguments.get("skill").and_then(Value::as_str);
    let lookback_days = arguments.get("days").and_then(Value::as_u64).unwrap_or(0);

    let journal_path = repo_root.join("artifacts/telemetry/events.jsonl");
    if !journal_path.exists() {
        return Err(format!("Telemetry journal not found at {}", journal_path.display()));
    }

    let file = std::fs::File::open(&journal_path)
        .map_err(|e| format!("open journal: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = if lookback_days > 0 {
        now.saturating_sub(lookback_days * 86400)
    } else {
        0
    };

    let mut entries: Vec<RouteLogEntry> = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("[routing_evolution] read journal line failed: {e}");
                continue;
            }
        };
        if line.trim().is_empty() { continue; }
        let entry: RouteLogEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.kind.as_deref() != Some("route_decision") { continue; }
        if cutoff > 0
            && let Some(ts) = &entry.ts
            && let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(ts)
            && parsed.timestamp() < cutoff as i64 {
                continue;
            }
        if let Some(filter) = skill_filter
            && entry.skill.as_deref() != Some(filter) {
                continue;
            }
        entries.push(entry);
    }

    match operation {
        "stats" => Ok(routing_stats(&entries)),
        "analyze" => Ok(routing_analyze(&entries)),
        "extract" => Ok(routing_extract(&entries)),
        "calibrate" => Ok(routing_calibrate(&entries)),
        _ => Err(format!("Unknown operation: {operation}. Use stats|analyze|extract|calibrate")),
    }
}

// ── Routing evolution helper functions ──
fn routing_stats(entries: &[RouteLogEntry]) -> String {
    #[derive(serde::Serialize)]
    struct RouteStats {
        total: usize,
        per_skill: Vec<serde_json::Value>,
        gate_distribution: Vec<serde_json::Value>,
        total_reroute: u64,
    }

    let total = entries.len();
    let mut per_skill: HashMap<&str, (u64, f64, u64)> = HashMap::new();
    let mut gate_counts: HashMap<&str, u64> = HashMap::new();
    let mut total_reroute = 0u64;

    // For brevity, just use the struct field names
    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        let (count, sum, reroute) = per_skill.entry(skill).or_insert((0, 0.0, 0));
        *count += 1;
        *sum += e.confidence.unwrap_or(0.0) as f64;
        if e.reroute.unwrap_or(false) { *reroute += 1; total_reroute += 1; }
        let gate = e.parity_gate.as_deref().unwrap_or("unknown");
        *gate_counts.entry(gate).or_insert(0) += 1;
    }

    let skills: Vec<serde_json::Value> = per_skill
        .iter()
        .map(|(slug, (count, sum, reroute))| {
            json!({
                "slug": slug,
                "count": count,
                "avg_confidence": if *count > 0 { format!("{:.2}", sum / *count as f64) } else { "0.00".to_string() },
                "reroute_count": reroute,
            })
        })
        .collect();

    let gate_distribution: Vec<serde_json::Value> = gate_counts
        .iter()
        .map(|(gate, count)| json!({"gate": gate, "count": count}))
        .collect();

    serde_json::to_string_pretty(&RouteStats { total, per_skill: skills, gate_distribution, total_reroute })
        .unwrap_or_else(|_| "{}".to_string())
}

fn routing_analyze(entries: &[RouteLogEntry]) -> String {
    let mut low_conf: Vec<(&str, f64)> = Vec::new();
    let mut high_reroute: Vec<(&str, u64, u64)> = Vec::new();

    let mut per_skill: HashMap<&str, (u64, f64, u64)> = HashMap::new();
    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        let (count, sum, reroute) = per_skill.entry(skill).or_insert((0, 0.0, 0));
        *count += 1;
        *sum += e.confidence.unwrap_or(0.0) as f64;
        if e.reroute.unwrap_or(false) { *reroute += 1; }
    }

    for (slug, (count, sum, reroute)) in &per_skill {
        let avg_conf = if *count > 0 { sum / *count as f64 } else { 0.0 };
        if avg_conf < 60.0 { low_conf.push((slug, avg_conf)); }
        if *reroute > 0 && *count > 0 { high_reroute.push((slug, *reroute, *count)); }
    }

    low_conf.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    high_reroute.sort_by(|a, b| b.0.cmp(a.0));

    let analysis = json!({
        "total_entries": entries.len(),
        "low_confidence_skills": low_conf.iter().take(10).map(|(s, c)| json!({"slug": s, "avg_confidence": format!("{:.2}", c)})).collect::<Vec<_>>(),
        "reroute_analysis": high_reroute.iter().take(10).map(|(s, r, c)| json!({"slug": s, "reroute_count": r, "total_count": c})).collect::<Vec<_>>(),
    });

    serde_json::to_string_pretty(&analysis).unwrap_or_else(|_| "{}".to_string())
}

fn routing_extract(entries: &[RouteLogEntry]) -> String {
    let extracts: Vec<serde_json::Value> = entries.iter().map(|e| {
        json!({
            "ts": e.ts,
            "task": e.task,
            "skill": e.skill,
            "confidence": e.confidence,
            "reroute": e.reroute,
        })
    }).collect();

    serde_json::to_string_pretty(&extracts).unwrap_or_else(|_| "[]".to_string())
}

fn routing_calibrate(entries: &[RouteLogEntry]) -> String {
    let mut total_conf = 0.0f64;
    let mut conf_count = 0u64;
    for e in entries {
        if let Some(c) = e.confidence {
            total_conf += c as f64;
            conf_count += 1;
        }
    }
    let baseline = if conf_count > 0 { total_conf / conf_count as f64 } else { 70.0 };

    let calibration = json!({
        "baseline_confidence": format!("{:.2}", baseline),
        "suggestion": if baseline < 60.0 {
            "增加 NL 调整规则以提高路由准确性"
        } else if baseline < 75.0 {
            "微调 trigger_hints 和 keyword 权重"
        } else {
            "当前路由表现良好，无需调整"
        },
    });

    serde_json::to_string_pretty(&calibration).unwrap_or_else(|_| "{}".to_string())
}
