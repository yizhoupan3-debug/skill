//! Framework command alias envelopes (`/autopilot`, `deepinterview`, …).

use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

use super::constants::{
    FRAMEWORK_ALIAS_SCHEMA_VERSION, FRAMEWORK_RUNTIME_AUTHORITY, TERMINAL_VERIFICATION_STATUSES,
};
use super::types::FrameworkAliasBuildOptions;
use super::{
    classify_runtime_continuity, is_terminal, load_framework_runtime_view, stable_line_items,
    supervisor_contract, value_string_list, value_text, workspace_name_from_root,
};

fn string_or_null(value: String) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

pub fn build_framework_alias_envelope(
    repo_root: &Path,
    alias_name: &str,
    options: FrameworkAliasBuildOptions<'_>,
) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let alias_record = load_framework_alias_record(repo_root, alias_name)?;
    let host_entrypoint = resolve_alias_host_entrypoint(&alias_record, options.host_id);
    let canonical_owner = alias_record_text(&alias_record, &["canonical_owner"]);
    let lineage = alias_value_at_path(&alias_record, &["lineage"])
        .cloned()
        .unwrap_or(Value::Null);
    let official_workflow = alias_value_at_path(&alias_record, &["official_workflow"])
        .cloned()
        .unwrap_or(Value::Null);
    let skill_path = alias_skill_path(alias_name, &alias_record);
    let implementation_bar = alias_record_list(&alias_record, &["implementation_bar"]);
    let local_adaptations = alias_record_list(&alias_record, &["local_adaptations"]);
    let interaction_invariants = alias_value_at_path(&alias_record, &["interaction_invariants"])
        .cloned()
        .unwrap_or(Value::Null);
    let routing_hints = build_framework_alias_routing_hints(alias_name, &alias_record);
    let entry_contract = build_framework_alias_entry_contract(
        alias_name,
        &alias_record,
        &continuity,
        &contract,
        &skill_path,
        options.max_lines,
        options.compact,
    );
    let state_machine = build_framework_alias_state_machine(
        alias_name,
        &alias_record,
        &continuity,
        &skill_path,
        options.max_lines,
        options.compact,
    );
    let continuity_summary =
        build_framework_alias_continuity_summary(&continuity, options.max_lines);
    let alias_payload = if options.compact {
        json!({
            "ok": true,
            "name": alias_name,
            "host_entrypoint": string_or_null(host_entrypoint),
            "canonical_owner": string_or_null(canonical_owner),
            "routing_hints": routing_hints,
            "interaction_invariants": interaction_invariants,
            "continuity": continuity_summary,
            "state_machine": state_machine,
            "entry_contract": entry_contract,
            "compact": true,
        })
    } else {
        let entry_prompt = render_framework_alias_prompt(&entry_contract);
        json!({
            "ok": true,
            "name": alias_name,
            "workspace": workspace_name_from_root(repo_root),
            "host_entrypoint": string_or_null(host_entrypoint),
            "canonical_owner": string_or_null(canonical_owner),
            "lineage": lineage,
            "official_workflow": official_workflow,
            "implementation_bar": implementation_bar,
            "local_adaptations": local_adaptations,
            "routing_hints": routing_hints,
            "interaction_invariants": interaction_invariants,
            "continuity": continuity_summary,
            "state_machine": state_machine,
            "entry_contract": entry_contract,
            "optimization_hints": [
                "prefer alias.state_machine and alias.entry_contract over opening full SKILL docs",
                "prefer live continuity over long prose restatement",
                "open SKILL.md only when the alias payload is insufficient"
            ],
            "entry_prompt": entry_prompt,
            "entry_prompt_token_estimate": estimate_token_count(&entry_prompt),
            "compact": false,
        })
    };
    Ok(json!({
        "schema_version": FRAMEWORK_ALIAS_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "alias": alias_payload
    }))
}

fn resolve_alias_host_entrypoint(alias_record: &Value, host_id: Option<&str>) -> String {
    let requested_host = host_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("codex-cli");
    let host_entrypoints =
        alias_value_at_path(alias_record, &["host_entrypoints"]).and_then(Value::as_object);
    if let Some(entrypoint) = host_entrypoints
        .and_then(|entrypoints| entrypoints.get(requested_host))
        .and_then(Value::as_str)
    {
        return entrypoint.to_string();
    }
    for fallback_host in ["codex-cli", "cursor"] {
        if let Some(entrypoint) = host_entrypoints
            .and_then(|entrypoints| entrypoints.get(fallback_host))
            .and_then(Value::as_str)
        {
            return entrypoint.to_string();
        }
    }
    String::new()
}

fn build_framework_alias_routing_hints(alias_name: &str, alias_record: &Value) -> Value {
    match alias_name {
        "autopilot" => json!({
            "reroute_when_ambiguous": alias_record_text(alias_record, &["reroute_when_ambiguous"]),
            "reroute_when_root_cause_unknown": alias_record_text(alias_record, &["reroute_when_root_cause_unknown"]),
            "entrypoint_modes": alias_value_at_path(alias_record, &["entrypoint_modes"])
                .cloned()
                .unwrap_or(Value::Null),
            "research_contract": alias_value_at_path(alias_record, &["research_contract"])
                .cloned()
                .unwrap_or(Value::Null),
        }),
        "deepinterview" => json!({
            "review_lanes": alias_record_list(alias_record, &["review_lanes"]),
        }),
        _ => Value::Null,
    }
}

fn build_framework_alias_continuity_summary(continuity: &Value, max_lines: usize) -> Value {
    json!({
        "state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "can_resume": continuity.get("can_resume").cloned().unwrap_or(Value::Bool(false)),
        "task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "phase": continuity.get("phase").cloned().unwrap_or(Value::Null),
        "status": continuity.get("status").cloned().unwrap_or(Value::Null),
        "next_actions": compact_alias_next_actions(continuity, max_lines),
    })
}

fn load_framework_alias_record(repo_root: &Path, alias_name: &str) -> Result<Value, String> {
    let registry_path = repo_root
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let raw = fs::read_to_string(&registry_path).map_err(|err| {
        format!(
            "framework alias registry unavailable at {}: {err}",
            registry_path.display()
        )
    })?;
    let payload = serde_json::from_str::<Value>(&raw).map_err(|err| {
        format!(
            "framework alias registry parse failed at {}: {err}",
            registry_path.display()
        )
    })?;
    payload
        .get("framework_commands")
        .and_then(Value::as_object)
        .and_then(|aliases| aliases.get(alias_name))
        .cloned()
        .ok_or_else(|| {
            format!(
                "Unknown framework alias `{alias_name}` in {}",
                registry_path.display()
            )
        })
}

fn alias_value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn alias_record_text(value: &Value, path: &[&str]) -> String {
    value_text(alias_value_at_path(value, path))
}

fn alias_record_list(value: &Value, path: &[&str]) -> Vec<String> {
    alias_value_at_path(value, path)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn alias_skill_path(alias_name: &str, alias_record: &Value) -> String {
    let explicit_path = alias_record_text(alias_record, &["skill_path"]);
    if !explicit_path.is_empty() {
        return explicit_path;
    }
    let upstream_path =
        alias_record_text(alias_record, &["upstream_source", "official_skill_path"]);
    if !upstream_path.is_empty() {
        return upstream_path;
    }
    let _ = alias_name;
    String::new()
}

fn compact_alias_next_actions(continuity: &Value, max_lines: usize) -> Vec<String> {
    continuity
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|items| {
            stable_line_items(
                items
                    .iter()
                    .map(|item| value_text(Some(item)))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
        .into_iter()
        .take(max_lines.clamp(1, 3))
        .collect()
}

fn compact_alias_route_rules(route_rules: Vec<String>, compact: bool) -> Vec<String> {
    let limit = if compact { 3 } else { route_rules.len() };
    route_rules.into_iter().take(limit).collect()
}

fn compact_alias_guardrails(guardrails: Vec<String>, compact: bool) -> Vec<String> {
    let limit = if compact { 2 } else { guardrails.len() };
    guardrails.into_iter().take(limit).collect()
}

fn build_framework_alias_entry_contract(
    alias_name: &str,
    alias_record: &Value,
    continuity: &Value,
    contract: &Map<String, Value>,
    skill_path: &str,
    max_lines: usize,
    compact: bool,
) -> Value {
    let task = value_text(continuity.get("task"));
    let phase = value_text(continuity.get("phase"));
    let status = value_text(continuity.get("status"));
    let continuity_state = value_text(continuity.get("state"));
    let next_actions = compact_alias_next_actions(continuity, max_lines);
    let acceptance = value_string_list(contract.get("acceptance_criteria"))
        .into_iter()
        .take(max_lines.clamp(1, 2))
        .collect::<Vec<_>>();
    let implementation_bar = alias_record_list(alias_record, &["implementation_bar"]);
    let decision_contract = if compact {
        Value::Null
    } else {
        alias_value_at_path(alias_record, &["decision_contract"])
            .cloned()
            .unwrap_or(Value::Null)
    };
    let blockers = value_string_list(continuity.get("blockers"));
    let verification_status = value_text(continuity.get("verification_status"));
    let evidence_missing = continuity
        .get("evidence_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let missing_recovery_anchors = value_string_list(continuity.get("missing_recovery_anchors"));
    let execution_ready = alias_name == "autopilot"
        && continuity_state == "active"
        && !task.is_empty()
        && !next_actions.is_empty()
        && missing_recovery_anchors.is_empty();
    let needs_recovery =
        alias_name == "autopilot" && matches!(continuity_state.as_str(), "stale" | "inconsistent");
    let needs_verification = alias_name == "autopilot"
        && evidence_missing
        && !is_terminal(&verification_status, TERMINAL_VERIFICATION_STATUSES);
    let needs_debugging = alias_name == "autopilot"
        && !blockers.is_empty()
        && blockers.iter().any(|item| {
            let lowered = item.to_ascii_lowercase();
            lowered.contains("unknown")
                || lowered.contains("root cause")
                || lowered.contains("根因")
                || lowered.contains("重复")
        });
    let needs_clarification = alias_name == "autopilot"
        && continuity_state == "missing"
        && task.is_empty()
        && next_actions.is_empty();
    let execution_readiness = if alias_name == "autopilot" {
        if needs_recovery {
            "needs_recovery"
        } else if needs_verification {
            "needs_verification"
        } else if needs_debugging {
            "needs_debugging"
        } else if needs_clarification {
            "needs_clarification"
        } else if execution_ready {
            "ready_to_execute"
        } else {
            "continue_autopilot"
        }
    } else {
        "use-alias-default"
    };
    let mut route_rules = Vec::new();
    let summary = match alias_name {
        "autopilot" => {
            let ambiguous = alias_record_text(alias_record, &["reroute_when_ambiguous"]);
            let root_cause = alias_record_text(alias_record, &["reroute_when_root_cause_unknown"]);
            let owner = alias_record_text(alias_record, &["canonical_owner"]);
            route_rules.push(format!("模糊需求 -> `{ambiguous}`"));
            route_rules.push(format!("根因未知 -> `{root_cause}`"));
            route_rules.push(format!("其他情况 -> `{owner}`"));
            if evidence_missing {
                route_rules
                    .push("缺少验证证据 -> 先补 QA / Validation，再决定是否 closeout".to_string());
            }
            if !missing_recovery_anchors.is_empty() {
                route_rules.push(format!(
                    "恢复锚点缺失 -> 先补 {}",
                    missing_recovery_anchors.join(", ")
                ));
            }
            "进入 autopilot。本仓原生执行流启动，状态、恢复和续跑都走本地 Rust/continuity。"
                .to_string()
        }
        "deepinterview" => {
            let owner = alias_record_text(alias_record, &["canonical_owner"]);
            let review_lanes = alias_record_list(alias_record, &["review_lanes"]);
            route_rules.push(format!("主 owner -> `{owner}`"));
            route_rules.push("每轮只问一个问题".to_string());
            route_rules.push("先查仓库证据，再问用户".to_string());
            route_rules.push("清晰度过线后 handoff 到 `autopilot`".to_string());
            if !review_lanes.is_empty() {
                route_rules.push(format!("review lanes -> {}", review_lanes.join(", ")));
            }
            "进入 deepinterview。本仓原生澄清流启动，访谈状态与 handoff 都走本地 Rust/continuity。"
                .to_string()
        }
        _ => format!(
            "进入 {alias_name}。优先使用本地 Rust/continuity alias 载荷，不要回退成长文说明。"
        ),
    };

    let guardrails = compact_alias_guardrails(
        implementation_bar
            .into_iter()
            .take(max_lines.clamp(1, 3))
            .collect::<Vec<_>>(),
        compact,
    );
    let route_rules = compact_alias_route_rules(route_rules, compact);
    json!({
        "summary": summary,
        "context": {
            "continuity_state": continuity_state,
            "task": if task.is_empty() { Value::Null } else { Value::String(task) },
            "phase": if phase.is_empty() { Value::Null } else { Value::String(phase) },
            "status": if status.is_empty() { Value::Null } else { Value::String(status) },
            "verification_status": if verification_status.is_empty() { Value::Null } else { Value::String(verification_status) },
            "execution_readiness": Value::String(execution_readiness.to_string()),
        },
        "route_rules": route_rules,
        "guardrails": guardrails,
        "decision_contract": decision_contract,
        "acceptance": acceptance,
        "next_actions": next_actions,
        "skill_fallback_path": if skill_path.is_empty() { Value::Null } else { Value::String(skill_path.to_string()) },
    })
}

fn build_framework_alias_state_machine(
    alias_name: &str,
    alias_record: &Value,
    continuity: &Value,
    skill_path: &str,
    max_lines: usize,
    compact: bool,
) -> Value {
    let state = value_text(continuity.get("state"));
    let task = value_text(continuity.get("task"));
    let phase = value_text(continuity.get("phase"));
    let status = value_text(continuity.get("status"));
    let can_resume = continuity
        .get("can_resume")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let evidence_missing = continuity
        .get("evidence_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let verification_status = value_text(continuity.get("verification_status"));
    let missing_recovery_anchors = value_string_list(continuity.get("missing_recovery_anchors"));
    let next_steps = compact_alias_next_actions(continuity, max_lines);
    let recovery_hints = value_string_list(continuity.get("recovery_hints"))
        .into_iter()
        .take(max_lines.clamp(1, 2))
        .collect::<Vec<_>>();
    let required_anchors = continuity
        .get("paths")
        .and_then(Value::as_object)
        .map(|paths| {
            if compact {
                stable_line_items(vec![
                    path_anchor_label(paths.get("session_summary")),
                    path_anchor_label(paths.get("next_actions")),
                    path_anchor_label(paths.get("trace_metadata")),
                    path_anchor_label(paths.get("supervisor_state")),
                ])
            } else {
                stable_line_items(vec![
                    value_text(paths.get("session_summary")),
                    value_text(paths.get("next_actions")),
                    value_text(paths.get("trace_metadata")),
                    value_text(paths.get("supervisor_state")),
                ])
            }
        })
        .unwrap_or_default();
    let (current_state, recommended_action, resume_mode, resume_reason) =
        if alias_name == "autopilot" {
            match state.as_str() {
                "active"
                    if evidence_missing
                        && !is_terminal(&verification_status, TERMINAL_VERIFICATION_STATUSES) =>
                {
                    (
                        "resume_active_needs_verification".to_string(),
                        "verify_before_done".to_string(),
                        "continue-current-task".to_string(),
                        "implementation is active but verification evidence is still missing"
                            .to_string(),
                    )
                }
                "active" if !missing_recovery_anchors.is_empty() => (
                    "resume_active_missing_anchors".to_string(),
                    "repair_recovery_anchors_then_resume".to_string(),
                    "repair-continuity".to_string(),
                    "active continuity is missing required recovery anchors".to_string(),
                ),
                "active" => (
                    "resume_active".to_string(),
                    "resume_current_task".to_string(),
                    "continue-current-task".to_string(),
                    "live continuity is active".to_string(),
                ),
                "completed" => (
                    "resume_blocked_completed".to_string(),
                    "start_new_task".to_string(),
                    "start-new-task".to_string(),
                    "completed work should stay historical; start a new bounded task".to_string(),
                ),
                "stale" => (
                    "resume_requires_refresh".to_string(),
                    "refresh_continuity_then_resume".to_string(),
                    "refresh-continuity".to_string(),
                    "stale continuity cannot be resumed directly".to_string(),
                ),
                "inconsistent" => (
                    "resume_requires_repair".to_string(),
                    "repair_continuity_then_resume".to_string(),
                    "repair-continuity".to_string(),
                    "continuity artifacts disagree and must be repaired first".to_string(),
                ),
                _ => (
                    "fresh_entry".to_string(),
                    "start_execution".to_string(),
                    "fresh-start".to_string(),
                    "no active continuity is available; enter as a fresh task".to_string(),
                ),
            }
        } else {
            match state.as_str() {
                "active" => (
                    "resume_active".to_string(),
                    if alias_name == "deepinterview" {
                        "resume_interview".to_string()
                    } else {
                        "resume_current_task".to_string()
                    },
                    "continue-current-task".to_string(),
                    "live continuity is active".to_string(),
                ),
                "completed" => (
                    "resume_blocked_completed".to_string(),
                    "start_new_task".to_string(),
                    "start-new-task".to_string(),
                    "completed work should stay historical; start a new bounded task".to_string(),
                ),
                "stale" => (
                    "resume_requires_refresh".to_string(),
                    "refresh_continuity_then_resume".to_string(),
                    "refresh-continuity".to_string(),
                    "stale continuity cannot be resumed directly".to_string(),
                ),
                "inconsistent" => (
                    "resume_requires_repair".to_string(),
                    "repair_continuity_then_resume".to_string(),
                    "repair-continuity".to_string(),
                    "continuity artifacts disagree and must be repaired first".to_string(),
                ),
                _ => (
                    "fresh_entry".to_string(),
                    if alias_name == "deepinterview" {
                        "start_interview".to_string()
                    } else {
                        "start_execution".to_string()
                    },
                    "fresh-start".to_string(),
                    "no active continuity is available; enter as a fresh task".to_string(),
                ),
            }
        };
    let handoff = match alias_name {
        "autopilot" => json!({
            "default_mode": "stay-in-autopilot",
            "rules": [
                {
                    "when": "task is still ambiguous",
                    "target": alias_record_text(alias_record, &["reroute_when_ambiguous"]),
                    "action": "handoff_for_clarification",
                },
                {
                    "when": "root cause is still unknown",
                    "target": alias_record_text(alias_record, &["reroute_when_root_cause_unknown"]),
                    "action": "handoff_for_debugging",
                }
            ]
        }),
        "deepinterview" => json!({
            "default_mode": "clarify-in-deepinterview",
            "rules": [
                {
                    "when": "clarity is still below threshold",
                    "target": "deepinterview",
                    "action": "stay_and_ask_next_question",
                },
                {
                    "when": "clarity is high enough to execute",
                    "target": "autopilot",
                    "action": "handoff_to_execution",
                }
            ]
        }),
        _ => json!({
            "default_mode": "stay-in-alias",
            "rules": []
        }),
    };
    let mut resume = Map::new();
    resume.insert("allowed".to_string(), Value::Bool(can_resume));
    resume.insert("mode".to_string(), Value::String(resume_mode.clone()));
    if alias_name == "autopilot" {
        resume.insert(
            "missing_recovery_anchors".to_string(),
            Value::Array(
                missing_recovery_anchors
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    resume.insert("reason".to_string(), Value::String(resume_reason.clone()));
    if !compact {
        resume.insert(
            "task".to_string(),
            if task.is_empty() {
                Value::Null
            } else {
                Value::String(task)
            },
        );
        resume.insert(
            "phase".to_string(),
            if phase.is_empty() {
                Value::Null
            } else {
                Value::String(phase)
            },
        );
        resume.insert(
            "status".to_string(),
            if status.is_empty() {
                Value::Null
            } else {
                Value::String(status)
            },
        );
    }
    json!({
        "schema_version": "framework-alias-state-machine-v1",
        "current_state": current_state,
        "recommended_action": recommended_action,
        "verification_status": if verification_status.is_empty() { Value::Null } else { Value::String(verification_status) },
        "evidence_missing": evidence_missing,
        "resume": Value::Object(resume),
        "handoff": handoff,
        "next_steps": if state == "active" { next_steps } else { recovery_hints },
        "required_anchors": required_anchors,
        "skill_fallback_path": if skill_path.is_empty() { Value::Null } else { Value::String(skill_path.to_string()) },
    })
}

fn path_anchor_label(path: Option<&Value>) -> String {
    let text = value_text(path);
    Path::new(&text)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.trim_start_matches('.').to_ascii_uppercase())
        .unwrap_or_default()
}

fn render_framework_alias_prompt(entry_contract: &Value) -> String {
    let summary = value_text(entry_contract.get("summary"));
    let context = entry_contract
        .get("context")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let route_rules = value_string_list(entry_contract.get("route_rules"));
    let guardrails = value_string_list(entry_contract.get("guardrails"));
    let acceptance = value_string_list(entry_contract.get("acceptance"));
    let next_actions = value_string_list(entry_contract.get("next_actions"));
    let skill_path = value_text(entry_contract.get("skill_fallback_path"));
    let mut lines = Vec::new();
    if !summary.is_empty() {
        lines.push(summary);
    }
    let task = value_text(context.get("task"));
    let phase = value_text(context.get("phase"));
    let status = value_text(context.get("status"));
    if !task.is_empty() || !phase.is_empty() || !status.is_empty() {
        lines.push(format!(
            "当前：{} / {} / {}",
            if task.is_empty() {
                "未记录"
            } else {
                task.as_str()
            },
            if phase.is_empty() {
                "未记录"
            } else {
                phase.as_str()
            },
            if status.is_empty() {
                "未记录"
            } else {
                status.as_str()
            },
        ));
    }
    if !route_rules.is_empty() {
        lines.push(format!("路由：{}", route_rules.join("；")));
    }
    if !guardrails.is_empty() {
        lines.push(format!("硬约束：{}", guardrails.join("；")));
    }
    if !acceptance.is_empty() {
        lines.push(format!("验收：{}", acceptance.join("；")));
    }
    if !next_actions.is_empty() {
        lines.push(format!("下一步：{}", next_actions.join("；")));
    }
    if !skill_path.is_empty() {
        lines.push(format!("不够再开 `{skill_path}`。"));
    }
    lines.join("\n")
}

pub(super) fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        0
    } else {
        (trimmed.chars().count() / 4).max(1)
    }
}
