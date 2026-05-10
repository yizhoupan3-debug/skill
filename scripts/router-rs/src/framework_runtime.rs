use crate::closeout_enforcement::{
    evaluate_closeout_record_value, evaluate_closeout_record_value_with_context,
    CloseoutEvidenceContext,
};
use crate::router_env_flags::{router_rs_env_enabled_default_true, router_rs_goal_prompt_verbose};
use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::ops::Not;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Status values that signal the caller is claiming the task is finished.
/// When the status matches one of these **and** programmatic closeout enforcement
/// is active, `write_framework_session_artifacts` requires a `closeout_record`
/// and refuses the write if evaluation fails. When enforcement is off (see
/// `closeout_enforcement_disabled_by_env`), completion writes proceed without
/// that gate. Non-completion statuses skip parsing `closeout_record` on this
/// path so in-progress checkpoints are not blocked by draft records.
const CLOSEOUT_COMPLETION_STATUSES: &[&str] = &[
    "completed",
    "complete",
    "done",
    "finished",
    "succeeded",
    "passed",
];

pub const FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION: &str =
    "router-rs-framework-runtime-snapshot-v1";
pub const FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION: &str =
    "router-rs-framework-contract-summary-v1";
pub const FRAMEWORK_ALIAS_SCHEMA_VERSION: &str = "router-rs-framework-alias-v1";
pub const FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION: &str =
    "router-rs-framework-session-artifact-write-v1";
pub const FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION: &str =
    "router-rs-framework-prompt-compression-v1";
pub const FRAMEWORK_RUNTIME_AUTHORITY: &str = "rust-framework-runtime-read-model";
pub const FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY: &str =
    "rust-framework-session-artifact-writer";
pub const FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY: &str = "rust-framework-prompt-policy";

const CURRENT_ARTIFACT_DIR: &str = "current";
const ACTIVE_TASK_POINTER_NAME: &str = "active_task.json";
const FOCUS_TASK_POINTER_NAME: &str = "focus_task.json";
const TASK_REGISTRY_NAME: &str = "task_registry.json";
const SESSION_SUMMARY_FILENAME: &str = "SESSION_SUMMARY.md";
const NEXT_ACTIONS_FILENAME: &str = "NEXT_ACTIONS.json";
const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
const TRACE_METADATA_FILENAME: &str = "TRACE_METADATA.json";
const CONTINUITY_JOURNAL_FILENAME: &str = "CONTINUITY_JOURNAL.json";
const SUPERVISOR_STATE_FILENAME: &str = ".supervisor_state.json";
const NEXT_ACTIONS_SCHEMA_VERSION: &str = "next-actions-v2";
const EVIDENCE_INDEX_SCHEMA_VERSION: &str = "evidence-index-v2";
const TRACE_METADATA_SCHEMA_VERSION: &str = "trace-metadata-v2";
const CONTINUITY_JOURNAL_SCHEMA_VERSION: &str = "continuity-journal-v1";
const SUPERVISOR_STATE_SCHEMA_VERSION: &str = "supervisor-state-v2";
const TASK_REGISTRY_SCHEMA_VERSION: &str = "task-registry-v1";
const TERMINAL_STORY_STATES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
const TERMINAL_PHASES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
const TERMINAL_VERIFICATION_STATUSES: &[&str] = &[
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];
const STALE_STORY_STATES: &[&str] = &["stale", "expired", "invalid"];

#[derive(Debug, Clone, Copy)]
struct ArtifactPaths<'a> {
    summary: &'a Path,
    next_actions: &'a Path,
    evidence: &'a Path,
    trace_metadata: Option<&'a Path>,
    journal: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy)]
struct ArtifactPayloads<'a> {
    summary_text: &'a str,
    next_actions: &'a Value,
    evidence: &'a Value,
    trace_metadata: &'a Value,
    journal: Option<&'a Value>,
}

#[derive(Debug, Clone, Copy)]
struct SupervisorStateInput<'a> {
    task_id: &'a str,
    task: &'a str,
    phase: &'a str,
    status: &'a str,
    summary: &'a str,
    next_actions_payload: &'a Value,
    evidence_payload: &'a Value,
    trace_metadata_payload: &'a Value,
    artifact_dir: &'a Path,
    supervisor_state: Option<&'a Value>,
    execution_contract: Option<&'a Value>,
    blockers: Option<&'a Value>,
    continuity: Option<&'a Value>,
}

#[derive(Debug, Clone)]
struct ContinuityJournalInput<'a> {
    task_id: &'a str,
    task: &'a str,
    phase: &'a str,
    status: &'a str,
    artifact_dir: &'a Path,
    summary_text: &'a str,
    next_actions_payload: &'a Value,
    evidence_payload: &'a Value,
    trace_metadata_payload: &'a Value,
    supervisor_state_payload: &'a Value,
    existing_journal: Value,
}

#[derive(Debug, Clone, Copy)]
struct TaskRegistryEntry<'a> {
    task_id: &'a str,
    task: &'a str,
    phase: &'a str,
    status: &'a str,
    resume_allowed: Option<bool>,
    updated_at: &'a str,
    focus_task_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct SessionArtifactWritePlan {
    task: String,
    phase: String,
    status: String,
    summary: String,
    task_id: String,
    focus: bool,
    repo_root: Option<PathBuf>,
    mirror_output_dir: Option<PathBuf>,
    summary_path: PathBuf,
    next_actions_path: PathBuf,
    evidence_path: PathBuf,
    trace_metadata_path: PathBuf,
    journal_path: PathBuf,
    next_actions_payload: Value,
    evidence_payload: Value,
    trace_metadata_payload: Value,
    supervisor_state_payload: Value,
    journal_payload: Value,
    expected_active_task_hash: Option<String>,
    expected_focus_task_hash: Option<String>,
    expected_supervisor_state_hash: Option<String>,
    changed_paths: Vec<String>,
}

#[derive(Debug, Clone)]
struct FrameworkRuntimeView {
    session_summary_text: String,
    next_actions: Value,
    evidence_index: Value,
    trace_metadata: Value,
    supervisor_state: Map<String, Value>,
    routing_runtime_version: u64,
    repo_root: PathBuf,
    artifact_base: PathBuf,
    current_root: PathBuf,
    mirror_root: PathBuf,
    task_root: PathBuf,
    active_task_pointer_present: bool,
    focus_task_pointer_present: bool,
    task_registry_present: bool,
    active_task_id: Option<String>,
    focus_task_id: Option<String>,
    control_plane_inconsistency_reasons: Vec<String>,
    known_task_ids: Vec<String>,
    recoverable_task_ids: Vec<String>,
    registered_tasks: Value,
    collected_at: String,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameworkAliasBuildOptions<'a> {
    pub max_lines: usize,
    pub compact: bool,
    pub host_id: Option<&'a str>,
}

impl<'a> Default for FrameworkAliasBuildOptions<'a> {
    fn default() -> Self {
        Self {
            max_lines: 4,
            compact: false,
            host_id: None,
        }
    }
}

/// Skill framework 仓库根：`RUNTIME_REGISTRY` + `router-rs` 清单同时存在。
/// 与 host 投影解析共用此判定，避免双份漂移。
pub fn is_framework_root(path: &Path) -> bool {
    path.join("configs/framework/RUNTIME_REGISTRY.json")
        .is_file()
        && path.join("scripts/router-rs/Cargo.toml").is_file()
}

/// CLI / 若干调用方常在 `scripts/router-rs/` 等子目录执行；continuity、`RUNTIME_REGISTRY`
/// 与 `artifacts/current` 均以仓库根为真源，因此从 cwd 或传入路径向上探测 framework root。
pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    let base = if let Some(path) = repo_root {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|err| format!("resolve current directory failed: {err}"))?
    };
    let normalized = base.canonicalize().unwrap_or(base);
    if is_framework_root(&normalized) {
        return Ok(normalized);
    }
    for ancestor in normalized.ancestors() {
        if is_framework_root(ancestor) {
            return Ok(ancestor.to_path_buf());
        }
    }
    Ok(normalized)
}

pub fn build_framework_runtime_snapshot_envelope(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, artifact_root_override, task_id_override);
    let continuity = classify_runtime_continuity(&snapshot);
    let continuity_route = continuity
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity_route.first().map(|item| value_text(Some(item)))
        } else {
            Some(direct)
        }
    };
    let verification_status = snapshot
        .supervisor_state
        .get("verification")
        .and_then(Value::as_object)
        .and_then(|verification| nonempty_string(verification.get("verification_status")));
    Ok(json!({
        "schema_version": FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "runtime_snapshot": {
            "ok": true,
            "workspace": workspace_name_from_root(repo_root),
            "artifact_base": snapshot.artifact_base.display().to_string(),
            "current_root": snapshot.current_root.display().to_string(),
            "mirror_root": snapshot.mirror_root.display().to_string(),
            "task_root": snapshot.task_root.display().to_string(),
            "control_plane_present": snapshot.active_task_pointer_present
                && snapshot.focus_task_pointer_present
                && snapshot.task_registry_present
                && !snapshot.supervisor_state.is_empty(),
            "control_plane_missing": missing_control_plane_anchors(&snapshot),
            "control_plane_inconsistency_reasons": snapshot.control_plane_inconsistency_reasons,
            "active_task_id": snapshot.active_task_id,
            "focus_task_id": snapshot.focus_task_id,
            "known_task_ids": snapshot.known_task_ids,
            "recoverable_task_ids": snapshot.recoverable_task_ids,
            "parallel_task_count": snapshot.known_task_ids.len(),
            "registered_tasks": snapshot.registered_tasks,
            "collected_at": snapshot.collected_at,
            "session_summary_present": !snapshot.session_summary_text.trim().is_empty(),
            "next_action_count": continuity
                .get("next_actions")
                .and_then(Value::as_array)
                .map(|rows| rows.len())
                .unwrap_or(0),
            "evidence_count": normalize_evidence_index(&snapshot.evidence_index).len(),
            "trace_skill_count": continuity_route.len(),
            "continuity": continuity,
            "supervisor_state": {
                "task_id": nonempty_string(snapshot.supervisor_state.get("task_id")),
                "task_summary": nonempty_string(snapshot.supervisor_state.get("task_summary")),
                "active_phase": nonempty_string(snapshot.supervisor_state.get("active_phase")),
                "primary_owner": primary_owner,
                "verification_status": verification_status,
            },
            "paths": {
                "session_summary": snapshot.current_root.join(SESSION_SUMMARY_FILENAME).display().to_string(),
                "next_actions": snapshot.current_root.join(NEXT_ACTIONS_FILENAME).display().to_string(),
                "evidence_index": snapshot.current_root.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
                "trace_metadata": snapshot.current_root.join(TRACE_METADATA_FILENAME).display().to_string(),
                "current_pointer_root": snapshot.mirror_root.display().to_string(),
                "supervisor_state": repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
            },
        }
    }))
}

pub fn build_framework_contract_summary_envelope(repo_root: &Path) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let workspace = workspace_name_from_root(repo_root);
    let continuity_route = continuity
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_owner = {
        let direct = value_text(snapshot.supervisor_state.get("primary_owner"));
        if direct.is_empty() {
            continuity_route.first().map(|item| value_text(Some(item)))
        } else {
            Some(direct)
        }
    };
    let blocker_list = snapshot
        .supervisor_state
        .get("blockers")
        .and_then(Value::as_object)
        .and_then(|blockers| blockers.get("open_blockers"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let is_active = continuity.get("state").and_then(Value::as_str) == Some("active")
        && continuity.get("can_resume").and_then(Value::as_bool) == Some(true);
    let goal = if is_active {
        contract.get("goal").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let scope = if is_active {
        value_string_list(contract.get("scope"))
    } else {
        Vec::<String>::new()
    };
    let forbidden_scope = if is_active {
        value_string_list(contract.get("forbidden_scope"))
    } else {
        Vec::<String>::new()
    };
    let acceptance_criteria = if is_active {
        value_string_list(contract.get("acceptance_criteria"))
    } else {
        Vec::<String>::new()
    };
    let evidence_required = if is_active {
        value_string_list(contract.get("evidence_required"))
    } else {
        Vec::<String>::new()
    };
    let active_phase = if is_active {
        nonempty_string(snapshot.supervisor_state.get("active_phase"))
    } else {
        Option::<String>::None
    };
    let next_actions = if is_active {
        continuity
            .get("next_actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::<Value>::new()
    };
    let open_blockers = if is_active {
        blocker_list
    } else {
        Vec::<String>::new()
    };
    let session_summary: Map<String, Value> = parse_session_summary(&snapshot.session_summary_text);
    let evidence_count = normalize_evidence_index(&snapshot.evidence_index).len();
    let contract_digest_input = json!({
        "workspace": workspace.clone(),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "goal": goal,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
        "active_phase": active_phase,
        "primary_owner": primary_owner.clone(),
        "next_actions": next_actions,
        "open_blockers": open_blockers,
        "trace_skills": continuity_route.clone(),
        "evidence_count": evidence_count,
    });
    let contract_digest = stable_json_sha256(&contract_digest_input)?;
    let session_summary_value = Value::Object(session_summary.clone());
    let prompt_lines = build_contract_guard_prompt_lines(
        &contract_digest,
        &continuity,
        &contract_digest_input,
        &session_summary_value,
        snapshot.current_root.as_path(),
    );
    Ok(json!({
        "schema_version": FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
        "authority": FRAMEWORK_RUNTIME_AUTHORITY,
        "contract_summary": {
            "ok": true,
            "workspace": workspace,
            "contract_digest": contract_digest,
            "contract_digest_algorithm": "sha256",
            "contract_guard": {
                "contract_active": is_active,
                "drift_classes": ["scope_drift", "owner_drift", "evidence_drift", "contract_digest_drift"],
                "fail_closed_when": [
                    "expected contract_digest differs from live contract_digest",
                    "proposed owner differs from primary_owner without explicit contract update intent",
                    "proposed goal/task changes while continuity is active",
                    "verification/evidence requirements are dropped before completion"
                ],
                "update_requires_explicit_user_intent": true
            },
            "prompt_lines": prompt_lines,
            "continuity": continuity,
            "goal": contract_digest_input.get("goal").cloned().unwrap_or(Value::Null),
            "scope": contract_digest_input.get("scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "forbidden_scope": contract_digest_input.get("forbidden_scope").cloned().unwrap_or(Value::Array(Vec::new())),
            "acceptance_criteria": contract_digest_input.get("acceptance_criteria").cloned().unwrap_or(Value::Array(Vec::new())),
            "evidence_required": contract_digest_input.get("evidence_required").cloned().unwrap_or(Value::Array(Vec::new())),
            "active_phase": contract_digest_input.get("active_phase").cloned().unwrap_or(Value::Null),
            "primary_owner": primary_owner,
            "next_actions": contract_digest_input.get("next_actions").cloned().unwrap_or(Value::Array(Vec::new())),
            "open_blockers": contract_digest_input.get("open_blockers").cloned().unwrap_or(Value::Array(Vec::new())),
            "trace_skills": continuity_route,
            "session_summary": session_summary,
            "evidence_count": evidence_count,
            "artifacts_root": snapshot.current_root.display().to_string(),
            "recent_completed_execution": continuity.get("recent_completed_execution").cloned().unwrap_or(Value::Null),
            "recovery_hints": continuity.get("recovery_hints").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        }
    }))
}

fn stable_json_sha256(value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|err| format!("serialize contract digest input failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn build_contract_guard_prompt_lines(
    contract_digest: &str,
    continuity: &Value,
    digest_input: &Value,
    session_summary: &Value,
    artifact_root: &Path,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("contract_digest: sha256:{contract_digest}"));
    lines.push(format!(
        "continuity: state={} can_resume={}",
        value_text(continuity.get("state")),
        continuity
            .get("can_resume")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    let task = value_text(continuity.get("task"));
    if !task.is_empty() {
        lines.push(format!("task: {task}"));
    } else if let Some(task) = nonempty_string(session_summary.get("task")) {
        lines.push(format!("task: {task}"));
    }
    if let Some(owner) = nonempty_string(digest_input.get("primary_owner")) {
        lines.push(format!("owner: {owner}"));
    }
    if let Some(phase) = nonempty_string(digest_input.get("active_phase")) {
        lines.push(format!("phase: {phase}"));
    }
    for (label, key) in [
        ("goal", "goal"),
        ("scope", "scope"),
        ("forbidden_scope", "forbidden_scope"),
        ("acceptance", "acceptance_criteria"),
        ("evidence", "evidence_required"),
        ("blockers", "open_blockers"),
    ] {
        let line = compact_contract_value_line(label, digest_input.get(key));
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.push(format!("artifacts: {}", artifact_root.display()));
    lines.truncate(12);
    lines
}

fn compact_contract_value_line(label: &str, value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match value {
        Value::Null => String::new(),
        Value::String(text) if text.trim().is_empty() => String::new(),
        Value::String(text) => format!("{label}: {}", compact_contract_text(text, 140)),
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Array(items) => {
            let joined = items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ");
            if joined.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&joined, 180))
            }
        }
        _ => {
            let text = value_text(Some(value));
            if text.is_empty() {
                String::new()
            } else {
                format!("{label}: {}", compact_contract_text(&text, 140))
            }
        }
    }
}

fn compact_contract_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut compact = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    compact.push_str("...");
    compact
}

pub fn build_framework_refresh_payload(
    repo_root: &Path,
    max_lines: usize,
    verbose: bool,
) -> Result<Value, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let mut prompt = render_framework_refresh_prompt(&continuity, &contract, max_lines);
    let goal_state = crate::autopilot_goal::read_goal_state(repo_root, None)
        .ok()
        .flatten();
    if let Some(ref g) = goal_state {
        prompt.push_str("\n\n");
        prompt.push_str(&format_goal_state_refresh_section(g));
    }
    let debug = if verbose {
        json!({
            "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
            "verification_status": continuity.get("verification_status").cloned().unwrap_or(Value::Null),
            "missing_recovery_anchors": continuity
                .get("missing_recovery_anchors")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
            "recovery_hints": continuity
                .get("recovery_hints")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
            "paths": continuity.get("paths").cloned().unwrap_or(Value::Null),
        })
    } else {
        Value::Null
    };
    Ok(json!({
        "ok": true,
        "workspace": workspace_name_from_root(repo_root),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "phase": continuity.get("phase").cloned().unwrap_or(Value::Null),
        "status": continuity.get("status").cloned().unwrap_or(Value::Null),
        "prompt": prompt,
        "goal_state": goal_state.clone().unwrap_or(Value::Null),
        "debug": debug,
    }))
}

/// 把 `GOAL_STATE.json` 嵌进 `framework refresh` 提示，使 `$refresh` 与 Codex SessionStart digest 可见「可执行目标」而非仅有连续性摘要。
/// 默认紧凑；`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 使用冗长 checklist（完整字段始终在 JSON `goal_state`）。
fn format_goal_state_refresh_section(goal: &Value) -> String {
    if router_rs_goal_prompt_verbose() {
        format_goal_state_refresh_section_verbose(goal)
    } else {
        format_goal_state_refresh_section_compact(goal)
    }
}

fn format_goal_state_refresh_section_verbose(goal: &Value) -> String {
    let g = value_text(goal.get("goal"));
    let st = value_text(goal.get("status"));
    let drive = goal
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let done = value_string_list(goal.get("done_when"));
    let val = value_string_list(goal.get("validation_commands"));
    let non = value_string_list(goal.get("non_goals"));
    let horizon = value_text(goal.get("current_horizon"));
    let mut lines: Vec<String> = Vec::new();
    lines.push(
        "## GOAL_STATE（router-rs 目标机；须据此推进直至 complete 或显式 pause/block）".to_string(),
    );
    lines.push(format!(
        "- 目标: {}",
        if g.is_empty() {
            "（未填写）".to_string()
        } else {
            g
        }
    ));
    lines.push(format!("- 状态: {} | drive_until_done: {}", st, drive));
    if !horizon.is_empty() {
        lines.push(format!("- 当前地平线: {}", horizon));
    }
    if !non.is_empty() {
        lines.push(format!("- 非目标: {}", non.join("；")));
    }
    if !done.is_empty() {
        lines.push(format!("- 验收 done_when: {}", done.join("；")));
    }
    if !val.is_empty() {
        lines.push(format!("- 验证命令: {}", val.join("；")));
    }
    lines.push(
        "- 下一跳: 实现 → 跑验证命令 → 更新 SESSION_SUMMARY/NEXT_ACTIONS；满足验收后 `stdio` op `framework_autopilot_goal` operation=complete。"
            .to_string(),
    );
    lines.join("\n")
}

fn format_goal_state_refresh_section_compact(goal: &Value) -> String {
    let g = value_text(goal.get("goal"));
    let st = value_text(goal.get("status"));
    let drive = goal
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let done = value_string_list(goal.get("done_when"));
    let val = value_string_list(goal.get("validation_commands"));
    let horizon = value_text(goal.get("current_horizon"));
    let goal_line = if g.is_empty() {
        "（未填写）".to_string()
    } else {
        compact_contract_text(&g, 200)
    };
    let mut lines: Vec<String> = Vec::new();
    lines.push("## Active goal（全文见 JSON `goal_state`）".to_string());
    lines.push(format!(
        "- {} · drive={} · {}",
        if st.is_empty() {
            "（状态未填）".to_string()
        } else {
            st
        },
        drive,
        goal_line
    ));
    let mut bits: Vec<String> = Vec::new();
    if !horizon.is_empty() {
        bits.push(format!("horizon: {}", compact_contract_text(&horizon, 100)));
    }
    if !done.is_empty() {
        bits.push(format!(
            "done: {}",
            compact_contract_text(&done.join(" · "), 160)
        ));
    }
    if !val.is_empty() {
        let head = val.first().map(|s| s.as_str()).unwrap_or("").to_string();
        let extra = val.len().saturating_sub(1);
        let cmd = if extra > 0 {
            format!("`{}` (+{extra})", compact_contract_text(&head, 72))
        } else {
            format!("`{}`", compact_contract_text(&head, 90))
        };
        bits.push(format!("verify: {cmd}"));
    }
    if !bits.is_empty() {
        lines.push(format!("- {}", bits.join(" · ")));
    }
    lines.push(
        "- 收口: `framework_autopilot_goal` operation=complete（或 pause/block）。".to_string(),
    );
    lines.join("\n")
}

pub fn build_framework_statusline(repo_root: &Path) -> Result<String, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let supervisor_state = &snapshot.supervisor_state;
    let task = first_nonempty_text(&[
        continuity.get("task"),
        supervisor_state.get("task_summary"),
        Some(&Value::String("none".to_string())),
    ]);
    let phase = first_nonempty_text(&[
        continuity.get("phase"),
        supervisor_state.get("active_phase"),
        Some(&Value::String("idle".to_string())),
    ]);
    let status = first_nonempty_text(&[
        continuity.get("status"),
        Some(&Value::String("unknown".to_string())),
    ]);
    let route = statusline_route(&continuity);
    let (git_state, branch) = git_statusline_state(repo_root);
    let blockers = value_string_list(continuity.get("blockers"));
    let next_actions = value_string_list(continuity.get("next_actions"));
    let focus_task_id = snapshot.focus_task_id.clone().unwrap_or_default();
    let other_known_count = snapshot
        .known_task_ids
        .iter()
        .filter(|task_id| !task_id.is_empty() && **task_id != focus_task_id)
        .count();
    let other_recoverable_count = snapshot
        .recoverable_task_ids
        .iter()
        .filter(|task_id| !task_id.is_empty() && **task_id != focus_task_id)
        .count();
    Ok(format!(
        "{} | {} | {}/{} | task={} | route={} | nexts={} | blockers={} | others={} | resumable={} | git={}",
        branch,
        statusline_decision_hint(&blockers, &next_actions, &git_state, &status),
        phase,
        status,
        short_statusline_text(&task, 24),
        route,
        next_actions.len(),
        blockers.len(),
        other_known_count,
        other_recoverable_count,
        git_state,
    ))
}

fn first_nonempty_text(values: &[Option<&Value>]) -> String {
    values
        .iter()
        .map(|value| value_text(*value))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn statusline_route(continuity: &Value) -> String {
    let skills = continuity
        .get("route")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match skills.len() {
        0 => "none".to_string(),
        1 => skills[0].clone(),
        count => format!("{}+{}", skills[0], count - 1),
    }
}

fn git_statusline_state(repo_root: &Path) -> (String, String) {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg("--branch")
        .arg("--untracked-files=no")
        .current_dir(repo_root)
        .output();
    let Ok(output) = output else {
        return ("nogit".to_string(), "nogit".to_string());
    };
    if !output.status.success() {
        return ("nogit".to_string(), "nogit".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let branch = lines
        .next()
        .and_then(|line| line.strip_prefix("## "))
        .map(|line| line.split("...").next().unwrap_or(line).trim().to_string())
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let changed = lines.any(|line| !line.trim().is_empty());
    (if changed { "dirty" } else { "clean" }.to_string(), branch)
}

fn statusline_decision_hint(
    blockers: &[String],
    next_actions: &[String],
    git_state: &str,
    status: &str,
) -> String {
    if let Some(blocker) = blockers.iter().find(|item| !item.trim().is_empty()) {
        return format!("blocked={}", short_statusline_text(blocker, 36));
    }
    if status == "completed" {
        if let Some(action) = next_actions.iter().find(|item| !item.trim().is_empty()) {
            return format!("next={}", short_statusline_text(action, 36));
        }
        if git_state == "dirty" {
            return "next=review local changes".to_string();
        }
        return "next=pick task".to_string();
    }
    if next_actions.iter().any(|item| !item.trim().is_empty()) {
        return "next=/refresh".to_string();
    }
    "next=run verification".to_string()
}

fn short_statusline_text(value: &str, limit: usize) -> String {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        text
    } else if limit <= 3 {
        text.chars().take(limit).collect()
    } else {
        format!("{}...", text.chars().take(limit - 3).collect::<String>())
    }
}

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
        "team" => json!({
            "delegation_gate": alias_record_text(alias_record, &["delegation_gate"]),
            "execution_owners": alias_record_list(alias_record, &["execution_owners"]),
            "auto_route_allowed": alias_record_bool(alias_record, &["auto_route_allowed"]).unwrap_or(false),
            "route_mode": alias_record_text(alias_record, &["route_mode"]),
            "selection_signals": alias_value_at_path(alias_record, &["selection_signals"])
                .cloned()
                .unwrap_or(Value::Null),
            "transition_states": alias_record_list(alias_record, &["official_workflow", "transition_states"]),
            "worker_lifecycle": alias_record_list(alias_record, &["worker_lifecycle", "states"]),
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
    if let Ok(raw) = fs::read_to_string(&registry_path) {
        if let Ok(payload) = serde_json::from_str::<Value>(&raw) {
            if let Some(record) = payload
                .get("framework_commands")
                .and_then(Value::as_object)
                .and_then(|aliases| aliases.get(alias_name))
                .cloned()
            {
                return Ok(record);
            }
        }
    }
    fallback_framework_alias_record(alias_name)
        .ok_or_else(|| format!("Unknown framework alias: {alias_name}"))
}

fn fallback_framework_alias_record(alias_name: &str) -> Option<Value> {
    match alias_name {
        "autopilot" => Some(json!({
            "canonical_owner": "autopilot",
            "reroute_when_ambiguous": "deepinterview",
            "reroute_when_root_cause_unknown": "deepinterview",
            "skill_path": "skills/autopilot/SKILL.md",
            "lineage": {
                "source": "repo-native",
                "description": "Native repo autopilot workflow for end-to-end execution on the local Rust supervisor."
            },
            "official_workflow": {
                "phases": ["expansion", "planning", "execution", "qa", "validation", "cleanup"]
            },
            "implementation_bar": [
                "root-cause-first-when-unknown",
                "verification-evidence-required",
                "resume-and-recovery-required",
                "converge-until-bounded-scope-clean",
                "horizon-slice-macro-goals-with-exit-criteria-each-slice",
                "no-chat-turn-without-continuity-delta-when-task-active",
                "prefer-autopilot-deep-when-external-claims-drive-the-critical-path"
            ],
            "local_adaptations": [
                "store execution state in rust-session-supervisor plus continuity artifacts",
                "store specs and plans in artifacts/current task-local bootstrap outputs",
                "use deepinterview as the first-class clarification gate for vague requests",
                "treat each turn as a bus cycle: read alias+continuity then mutate repo then refresh SESSION_SUMMARY and NEXT_ACTIONS",
                "for goals larger than one context window: chain horizons; each horizon ends with explicit next_actions for cold resume"
            ],
            "autonomy_contract": {
                "auto_agent_orchestration": {
                    "enabled": true,
                    "default_mode": "bounded-sidecar-first",
                    "spawn_policy": "admit-when-lanes-are-clear",
                    "max_parallel_lanes": 3,
                    "require_reject_reason_when_not_spawning": true,
                    "reject_reasons": [
                        "small_task",
                        "shared_context_heavy",
                        "write_scope_overlap",
                        "next_step_blocked",
                        "verification_missing",
                        "token_overhead_dominates"
                    ]
                },
                "goal_style_execution": {
                    "enabled": true,
                    "run_to_completion": "until-done-or-blocked",
                    "requires_done_definition": true,
                    "requires_non_goals_definition": true,
                    "loop": [
                        "plan",
                        "implement",
                        "verify",
                        "repair",
                        "closeout"
                    ],
                    "lifecycle_states": [
                        "goal_defined",
                        "running",
                        "paused",
                        "blocked",
                        "verification_pending",
                        "completed"
                    ],
                    "lifecycle_states_implementation": {
                        "owner": "host_agent_layer",
                        "rust_runtime_coverage": "job_lifecycle_only",
                        "status": "host_owned_no_rust_native_state_machine",
                        "rationale": "Rust runtime tracks worker/job lifecycle (queued/running/interrupted/...). The goal-level six-state machine listed above is owned by the host agent layer (codex/cursor LLM + hooks). Treat the list as a host-side contract, not a Rust enum."
                    },
                    "control_surface": [
                        "goal_start",
                        "goal_pause",
                        "goal_resume",
                        "goal_clear"
                    ],
                    "control_surface_implementation": {
                        "owner": "hybrid_host_plus_rust_persistence",
                        "rust_runtime_coverage": "goal_persistence_and_drive_hook",
                        "status": "rust_stdio_goal_store_plus_cursor_followup",
                        "rationale": "Host still interprets goal_start/pause/resume/clear in natural language. Rust exposes stdio op `framework_autopilot_goal` (operations: start|status|checkpoint|pause|resume|complete|block|clear) persisting `artifacts/current/<task_id>/GOAL_STATE.json`. When `drive_until_done` is true and `status=running`, Cursor hooks merge an AUTOPILOT_DRIVE followup on stop/beforeSubmit (including hook-state lock failure and ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE) so sessions do not silently end. Disable hook injection with ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0."
                    },
                    "never_stop_at_plan_only": true,
                    "allow_network_research_for_unknowns": true,
                    "require_source_citation_for_external_claims": true,
                    "requires_checkpoint_log_each_loop": true,
                    "pause_requires_explicit_resume": true,
                    "checkpoint_artifacts": [
                        "SESSION_SUMMARY.md",
                        "NEXT_ACTIONS.json",
                        "EVIDENCE_INDEX.json",
                        "GOAL_STATE.json"
                    ]
                }
            },
            "execution_owners": [
                "autopilot",
                "deepinterview"
            ],
            "decision_contract": {
                "execute_when": [
                    "task is concrete enough to implement",
                    "acceptance criteria are already bounded",
                    "next actions are specific enough to continue"
                ],
                "clarify_when": [
                    "task is still ambiguous",
                    "user intent would materially change the implementation"
                ],
                "debug_when": [
                    "root cause is still unknown",
                    "the same failure pattern repeats without a validated explanation"
                ],
                "resume_when": [
                    "continuity state is active and recovery anchors are present"
                ],
                "refresh_when": [],
                "repair_when": [
                    "continuity state is inconsistent"
                ],
                "start_new_task_when": [
                    "current continuity is completed and should stay historical"
                ],
                "verify_when": [
                    "implementation changed but evidence is still missing",
                    "verification status is not yet passed or completed"
                ]
            },
            "host_entrypoints": {
                "codex-cli": "/autopilot",
                "cursor": "/autopilot"
            },
            "entrypoint_modes": {
                "quick": {
                    "codex-cli": "/autopilot-quick",
                    "cursor": "/autopilot-quick"
                },
                "deep": {
                    "codex-cli": "/autopilot-deep",
                    "cursor": "/autopilot-deep"
                }
            },
            "interaction_invariants": {
                "requires_explicit_entrypoint": true,
                "explicit_entrypoints": [
                    "/autopilot",
                    "$autopilot",
                    "/autopilot-quick",
                    "$autopilot-quick",
                    "/autopilot-deep",
                    "$autopilot-deep",
                    "/autopilot quick",
                    "/autopilot deep"
                ],
                "implicit_route_policy": "never"
            },
            "research_contract": {
                "quick": {
                    "target": "fast-check",
                    "default_output_style": "compact",
                    "max_rounds": 1
                },
                "deep": {
                    "target": "deep-research",
                    "default_output_style": "evidence-ledger",
                    "requires_multi_source_validation": true,
                    "minimum_independent_sources_per_major_claim": 2,
                    "requires_uncertainty_register": true,
                    "requires_counter_evidence": true,
                    "auto_continue_on_length_finish": true
                }
            }
        })),
        "deepinterview" => Some(json!({
            "canonical_owner": "deepinterview",
            "skill_path": "skills/deepinterview/SKILL.md",
            "lineage": {
                "source": "repo-native",
                "description": "Native repo deep-interview workflow for evidence-first clarification and convergence review."
            },
            "official_workflow": {
                "loop_rules": [
                    "one-question-at-a-time",
                    "target-weakest-clarity-dimension",
                    "score-ambiguity-after-each-answer",
                    "handoff-to-execution-only-below-threshold"
                ]
            },
            "implementation_bar": [
                "root-cause-first-when-unknown",
                "findings-first-with-severity-order",
                "verification-evidence-required",
                "fix-verify-loop-until-bounded-scope-clean"
            ],
            "local_adaptations": [
                "store interview progress in continuity artifacts and task-local bootstrap outputs",
                "use live repo evidence first for brownfield clarification before asking the user",
                "handoff into local autopilot and rust-session-supervisor after clarity is sufficient"
            ],
            "review_lanes": [
                "deepinterview",
                "visual-review",
                "gh-address-comments",
                "gh-fix-ci",
                "sentry"
            ],
            "host_entrypoints": {
                "codex-cli": "/deepinterview"
            },
            "interaction_invariants": {
                "requires_explicit_entrypoint": true,
                "explicit_entrypoints": ["/deepinterview", "$deepinterview"],
                "implicit_route_policy": "never"
            }
        })),
        "team" => Some(json!({
            "canonical_owner": "team",
            "delegation_gate": "agent-swarm-orchestration",
            "auto_route_allowed": false,
            "route_mode": "team-orchestration",
            "selection_signals": {
                "prefer_when": [
                    "multi-phase execution needs explicit worker lifecycle management",
                    "supervisor-owned continuity and lane-local outputs are required",
                    "integration, qa, cleanup, or resume/recovery are first-class workflow phases"
                ],
                "avoid_when": [
                    "task is a small tightly coupled local change",
                    "bounded sidecars are enough and orchestration overhead would dominate",
                    "the next supervisor step is blocked on the delegated result",
                    "worker write scopes would overlap or require shared editing context"
                ]
            },
            "spawn_admission_policy": {
                "default": "deny",
                "allow_when": [
                    "read-heavy exploration can run independently",
                    "independent hypotheses or domains can be investigated in parallel",
                    "review or verification can run without blocking the supervisor",
                    "write scopes are fully disjoint and lane-local"
                ],
                "reject_reasons": [
                    "small_task",
                    "shared_context_heavy",
                    "write_scope_overlap",
                    "next_step_blocked",
                    "verification_missing",
                    "token_overhead_dominates"
                ],
                "fallback": "local-supervisor-queue"
            },
            "skill_path": "skills/agent-swarm-orchestration/SKILL.md",
            "lineage": {
                "source": "repo-native",
                "description": "Native repo team workflow for Rust-first supervisor-led delegation and worker lifecycle management."
            },
            "official_workflow": {
                "phases": ["scoping", "delegation", "execution", "integration", "qa", "cleanup"],
                "transition_states": [
                    "delegation-planned",
                    "spawn-pending",
                    "spawn-blocked",
                    "worker-output-ready",
                    "integration-pending",
                    "resume-required"
                ],
                "recovery_states": [
                    "worker-failed-recoverable",
                    "stale-continuity",
                    "inconsistent-continuity"
                ],
                "terminal_states": ["cleanup-completed", "completed", "failed-terminal"]
            },
            "implementation_bar": [
                "worker-boundaries-required",
                "verification-evidence-required",
                "resume-and-recovery-required",
                "supervisor-owned-continuity"
            ],
            "local_adaptations": [
                "store team state in rust-session-supervisor plus continuity artifacts",
                "keep shared continuity supervisor-owned while workers emit lane-local outputs",
                "bind worker lifecycle to host tmux and resume capabilities instead of plugin state directories"
            ],
            "execution_owners": [
                "team",
                "agent-swarm-orchestration",
                "deepinterview"
            ],
            "supervisor_contract": {
                "shared_continuity_owner": "supervisor",
                "integration_owner": "supervisor",
                "verification_owner": "supervisor",
                "worker_write_scope": "lane-local-delta-only",
                "resume_requires_recovery_anchor": true
            },
            "lane_contract": {
                "required_fields": [
                    "lane_id",
                    "lane_owner",
                    "goal",
                    "bounded_scope",
                    "forbidden_scope",
                    "expected_output",
                    "integration_status",
                    "verification_status",
                    "recovery_anchor"
                ],
                "integration_statuses": ["planned", "running", "output-ready", "integrated", "blocked"],
                "verification_statuses": ["not-started", "pending", "passed", "failed"]
            },
            "worker_lifecycle": {
                "states": [
                    "planned",
                    "spawn-pending",
                    "running",
                    "stalled",
                    "failed-recoverable",
                    "failed-terminal",
                    "completed-unintegrated",
                    "integrated"
                ],
                "resume_state": "failed-recoverable",
                "fallback_mode": "local-supervisor-queue"
            },
            "recovery_contract": {
                "continuity_states": ["active", "stale", "inconsistent"],
                "requires_resume_judgment": [
                    "spawn-blocked",
                    "worker-failed-recoverable",
                    "stale-continuity",
                    "inconsistent-continuity"
                ],
                "required_artifacts": [
                    "SESSION_SUMMARY.md",
                    "NEXT_ACTIONS.json",
                    "EVIDENCE_INDEX.json",
                    "TRACE_METADATA.json",
                    ".supervisor_state.json"
                ]
            },
            "verification_contract": {
                "integration_requires_local_judgment": true,
                "verification_evidence_required_before_cleanup": true
            },
            "host_entrypoints": {
                "codex-cli": "/team"
            },
            "interaction_invariants": {
                "requires_explicit_entrypoint": true,
                "explicit_entrypoints": ["/team", "$team"],
                "implicit_route_policy": "never"
            }
        })),
        _ => None,
    }
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

fn alias_record_bool(value: &Value, path: &[&str]) -> Option<bool> {
    alias_value_at_path(value, path).and_then(Value::as_bool)
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
    match alias_name {
        "autopilot" => "skills/autopilot/SKILL.md".to_string(),
        "deepinterview" => "skills/deepinterview/SKILL.md".to_string(),
        "team" => "skills/agent-swarm-orchestration/SKILL.md".to_string(),
        _ => String::new(),
    }
}

fn team_current_state(continuity: &Value) -> String {
    let state = value_text(continuity.get("state"));
    let phase = value_text(continuity.get("phase"));
    let status = value_text(continuity.get("status"));

    if state == "stale" {
        return "stale-continuity".to_string();
    }
    if state == "inconsistent" {
        return "inconsistent-continuity".to_string();
    }
    if status == "completed" {
        return "cleanup-completed".to_string();
    }
    match phase.as_str() {
        "delegation" => "delegation-planned".to_string(),
        "execution" => "worker-running".to_string(),
        "integration" => "integration-pending".to_string(),
        "qa" => "qa-in-progress".to_string(),
        "cleanup" => "cleanup-pending".to_string(),
        _ if state == "active" => "scoping-active".to_string(),
        _ => "fresh-entry".to_string(),
    }
}

fn team_resume_action(current_state: &str) -> (&'static str, &'static str, &'static str) {
    match current_state {
        "stale-continuity" => (
            "resume_requires_refresh",
            "refresh_continuity_then_resume",
            "refresh-continuity",
        ),
        "inconsistent-continuity" => (
            "resume_requires_repair",
            "repair_continuity_then_resume",
            "repair-continuity",
        ),
        "delegation-planned" => (
            "resume_team_delegation",
            "review_worker_split_and_admit_or_fallback",
            "continue-current-task",
        ),
        "worker-running" => (
            "resume_team_execution",
            "review_lane_progress_and_integrate_when_ready",
            "continue-current-task",
        ),
        "integration-pending" => (
            "resume_team_integration",
            "integrate_lane_outputs_then_verify",
            "continue-current-task",
        ),
        "qa-in-progress" => (
            "resume_team_qa",
            "verify_integrated_result_and_close_loop",
            "continue-current-task",
        ),
        "cleanup-completed" => (
            "resume_blocked_completed",
            "start_new_task",
            "start-new-task",
        ),
        _ => ("fresh_team_entry", "start_team_supervision", "fresh-start"),
    }
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
        "team" => {
            let owner = alias_record_text(alias_record, &["canonical_owner"]);
            let delegation_gate = alias_record_text(alias_record, &["delegation_gate"]);
            let execution_owners = alias_record_list(alias_record, &["execution_owners"]);
            let transition_states =
                alias_record_list(alias_record, &["official_workflow", "transition_states"]);
            let recovery_states =
                alias_record_list(alias_record, &["official_workflow", "recovery_states"]);
            let lane_fields =
                alias_record_list(alias_record, &["lane_contract", "required_fields"]);
            let supervisor_write_scope =
                alias_record_text(alias_record, &["supervisor_contract", "worker_write_scope"]);
            let requires_recovery_anchor = alias_record_bool(
                alias_record,
                &["supervisor_contract", "resume_requires_recovery_anchor"],
            )
            .unwrap_or(false);
            route_rules.push(format!("主 owner -> `{owner}`"));
            route_rules.push(format!("team split gate -> `{delegation_gate}`"));
            route_rules.push(format!("bounded subagent lane -> `{delegation_gate}`"));
            route_rules.push("full orchestration route -> `team`".to_string());
            route_rules.push(format!("worker write scope -> `{supervisor_write_scope}`"));
            if requires_recovery_anchor {
                route_rules.push("恢复续跑必须保留 recovery anchor".to_string());
            }
            if !execution_owners.is_empty() {
                route_rules.push(format!(
                    "execution lanes -> {}",
                    execution_owners.join(", ")
                ));
            }
            if !transition_states.is_empty() {
                route_rules.push(format!(
                    "transition states -> {}",
                    transition_states.join(", ")
                ));
            }
            if !recovery_states.is_empty() {
                route_rules.push(format!("recovery states -> {}", recovery_states.join(", ")));
            }
            if !lane_fields.is_empty() {
                route_rules.push(format!("lane contract -> {}", lane_fields.join(", ")));
            }
            "进入 team。本仓原生团队编排流启动，worker 生命周期、lane 合同、恢复和 continuity 都走本地 Rust/supervisor。"
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
    let (current_state, recommended_action, resume_mode, resume_reason) = if alias_name == "team" {
        let current_state = team_current_state(continuity);
        let (_resume_state, action, mode) = team_resume_action(&current_state);
        let reason = match current_state.as_str() {
            "delegation-planned" => {
                "worker split exists but still needs supervisor admission or fallback"
            }
            "worker-running" => "active worker lanes require supervision before integration",
            "integration-pending" => "lane outputs are ready but not yet integrated",
            "qa-in-progress" => "integrated result still needs verification evidence",
            "cleanup-completed" => {
                "completed team execution should stay historical; start a new bounded task"
            }
            "stale-continuity" => "stale continuity cannot be resumed directly",
            "inconsistent-continuity" => "continuity artifacts disagree and must be repaired first",
            _ => "no active continuity is available; enter as a fresh team task",
        };
        (
            current_state,
            action.to_string(),
            mode.to_string(),
            reason.to_string(),
        )
    } else if alias_name == "autopilot" {
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
        "team" => json!({
            "default_mode": "supervise-team-locally",
            "rules": [
                {
                    "when": "task is still a single-lane change",
                    "target": "main-thread",
                    "action": "keep_local_ownership",
                },
                {
                    "when": "bounded sidecars improve throughput without full orchestration overhead",
                    "target": alias_record_text(alias_record, &["delegation_gate"]),
                    "action": "use_bounded_subagent_lane",
                },
                {
                    "when": "worker lifecycle, integration, qa, or resume/recovery must stay supervisor-led",
                    "target": "team",
                    "action": "keep_team_orchestration",
                },
                {
                    "when": "worker outputs are ready to merge",
                    "target": "supervisor-verification",
                    "action": "verify_and_close_loop",
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

fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        0
    } else {
        (trimmed.chars().count() / 4).max(1)
    }
}

fn load_framework_runtime_view(
    repo_root: &Path,
    artifact_root_override: Option<&Path>,
    task_id_override: Option<&str>,
) -> FrameworkRuntimeView {
    let mut control_plane_parse_errors: Vec<String> = Vec::new();
    let artifact_base =
        artifact_root_override.map_or_else(|| repo_root.join("artifacts"), Path::to_path_buf);
    let mirror_root = artifact_base.join(CURRENT_ARTIFACT_DIR);
    let supervisor_state_path = repo_root.join(SUPERVISOR_STATE_FILENAME);
    let supervisor_state = if supervisor_state_path.is_file() {
        normalize_supervisor_state(&read_json_control_plane_field(
            &supervisor_state_path,
            ".supervisor_state.json",
            &mut control_plane_parse_errors,
        ))
    } else {
        Map::new()
    };
    let active_task_pointer_path = mirror_root.join(ACTIVE_TASK_POINTER_NAME);
    let focus_task_pointer_path = mirror_root.join(FOCUS_TASK_POINTER_NAME);
    let task_registry_path = mirror_root.join(TASK_REGISTRY_NAME);
    let active_task_pointer_present = active_task_pointer_path.is_file();
    let focus_task_pointer_present = focus_task_pointer_path.is_file();
    let task_registry_present = task_registry_path.is_file();
    let pointer = read_json_control_plane_field(
        &active_task_pointer_path,
        "active_task.json",
        &mut control_plane_parse_errors,
    );
    let focus_pointer = read_json_control_plane_field(
        &focus_task_pointer_path,
        "focus_task.json",
        &mut control_plane_parse_errors,
    );
    let (registered_tasks, mut known_task_ids, mut recoverable_task_ids) =
        normalized_task_registry(&read_json_control_plane_field(
            &task_registry_path,
            "task_registry.json",
            &mut control_plane_parse_errors,
        ));
    let registry_task_ids_before_selection = known_task_ids.clone();
    let focus_task_id = {
        let direct = safe_slug(&value_text(focus_pointer.get("task_id")));
        if direct.is_empty().not() {
            Some(direct)
        } else {
            None
        }
    };
    let supervisor_task_id = safe_slug(&value_text(supervisor_state.get("task_id")));
    let pointer_task_id = safe_slug(&value_text(pointer.get("task_id")));
    let mut control_plane_inconsistency_reasons = control_plane_parse_errors;
    control_plane_inconsistency_reasons.extend(stable_line_items(vec![
        if !supervisor_task_id.is_empty()
            && focus_task_id
                .as_ref()
                .is_some_and(|task_id| task_id != &supervisor_task_id)
        {
            format!(
                "supervisor task_id '{supervisor_task_id}' disagrees with focus task pointer '{}'",
                focus_task_id.clone().unwrap_or_default()
            )
        } else {
            String::new()
        },
        if !supervisor_task_id.is_empty()
            && !pointer_task_id.is_empty()
            && supervisor_task_id != pointer_task_id
        {
            format!(
                "supervisor task_id '{supervisor_task_id}' disagrees with active task pointer '{pointer_task_id}'"
            )
        } else {
            String::new()
        },
        if focus_task_id
            .as_ref()
            .is_some_and(|task_id| !pointer_task_id.is_empty() && task_id != &pointer_task_id)
        {
            format!(
                "focus task pointer '{}' disagrees with active task pointer '{pointer_task_id}'",
                focus_task_id.clone().unwrap_or_default()
            )
        } else {
            String::new()
        },
    ]));
    let active_task_id = {
        let direct = safe_slug(task_id_override.unwrap_or(""));
        if direct.is_empty().not() {
            Some(direct)
        } else if let Some(focus_task_id) = focus_task_id.clone() {
            Some(focus_task_id)
        } else if pointer_task_id.is_empty().not() {
            Some(pointer_task_id.clone())
        } else {
            supervisor_task_id
                .is_empty()
                .not()
                .then_some(supervisor_task_id.clone())
        }
    };
    if task_id_override.is_none() {
        if let Some(task_id) = active_task_id.as_ref() {
            if task_registry_present
                && !registry_task_ids_before_selection
                    .iter()
                    .any(|existing| existing == task_id)
            {
                control_plane_inconsistency_reasons.push(format!(
                    "selected task_id '{task_id}' is missing from task_registry.json"
                ));
            }
        }
    }
    if let Some(task_id) = active_task_id.clone() {
        if !known_task_ids.iter().any(|existing| existing == &task_id) {
            known_task_ids.push(task_id.clone());
        }
        if supervisor_state
            .get("continuity")
            .and_then(Value::as_object)
            .and_then(|continuity| value_bool_or_none(continuity.get("resume_allowed")))
            == Some(true)
            && !recoverable_task_ids
                .iter()
                .any(|existing| existing == &task_id)
        {
            recoverable_task_ids.push(task_id);
        }
    }
    if let Some(task_id) = focus_task_id.clone() {
        if !known_task_ids.iter().any(|existing| existing == &task_id) {
            known_task_ids.push(task_id);
        }
    }
    let task_root = active_task_id
        .as_ref()
        .map_or_else(|| mirror_root.clone(), |task_id| mirror_root.join(task_id));
    let mirror_matches_selected = active_task_id
        .as_ref()
        .is_some_and(|task_id| task_id == &pointer_task_id);
    let preferred_root = if task_root.exists() {
        task_root.clone()
    } else if active_task_id.is_none() || mirror_matches_selected {
        mirror_root.clone()
    } else {
        task_root.clone()
    };
    let read_task_or_mirror = |file_name: &str| -> PathBuf {
        let preferred = preferred_root.join(file_name);
        if preferred.exists() {
            return preferred;
        }
        let mirror = mirror_root.join(file_name);
        if mirror.exists() {
            return mirror;
        }
        preferred
    };

    FrameworkRuntimeView {
        session_summary_text: read_text_if_exists(&read_task_or_mirror(SESSION_SUMMARY_FILENAME)),
        next_actions: read_json_if_exists(&read_task_or_mirror(NEXT_ACTIONS_FILENAME)),
        evidence_index: read_json_if_exists(&read_task_or_mirror(EVIDENCE_INDEX_FILENAME)),
        trace_metadata: read_json_if_exists(&read_task_or_mirror(TRACE_METADATA_FILENAME)),
        supervisor_state,
        routing_runtime_version: load_routing_runtime_version(repo_root),
        repo_root: repo_root.to_path_buf(),
        artifact_base,
        current_root: preferred_root,
        mirror_root,
        task_root,
        active_task_pointer_present,
        focus_task_pointer_present,
        task_registry_present,
        active_task_id,
        focus_task_id,
        control_plane_inconsistency_reasons,
        known_task_ids,
        recoverable_task_ids,
        registered_tasks,
        collected_at: current_local_timestamp(),
    }
}

fn classify_runtime_continuity(snapshot: &FrameworkRuntimeView) -> Value {
    let summary = parse_session_summary(&snapshot.session_summary_text);
    let supervisor = &snapshot.supervisor_state;
    let verification = object_field(supervisor, "verification");
    let continuity = object_field(supervisor, "continuity");
    let contract = supervisor_contract(supervisor);
    let trace_task = value_text(snapshot.trace_metadata.get("task"));
    let summary_task = value_text(summary.get("task"));
    let supervisor_task = first_nonempty(&[
        value_text(supervisor.get("task_summary")),
        value_text(supervisor.get("task_id")),
    ]);
    let task = first_nonempty(&[
        summary_task.clone(),
        trace_task.clone(),
        supervisor_task.clone(),
    ]);
    let summary_phase = value_text(summary.get("phase"));
    let supervisor_phase = value_text(supervisor.get("active_phase"));
    let verification_status = value_text(verification.get("verification_status"));
    let summary_status = value_text(summary.get("status"));
    let story_state = value_text(continuity.get("story_state"));
    let summary_terminal = is_terminal(&summary_phase, TERMINAL_PHASES)
        || is_terminal(&summary_status, TERMINAL_VERIFICATION_STATUSES);
    let supervisor_terminal = is_terminal(&supervisor_phase, TERMINAL_PHASES)
        || is_terminal(&verification_status, TERMINAL_VERIFICATION_STATUSES)
        || is_terminal(&story_state, TERMINAL_STORY_STATES);
    let supervisor_terminal_overrides_summary = supervisor_terminal
        && !summary_terminal
        && (summary_task.is_empty()
            || supervisor_task.is_empty()
            || looks_same_identity(&summary_task, &supervisor_task));
    let phase = if supervisor_terminal_overrides_summary {
        first_nonempty(&[supervisor_phase.clone(), summary_phase.clone()])
    } else {
        first_nonempty(&[summary_phase.clone(), supervisor_phase.clone()])
    };
    let status = if supervisor_terminal_overrides_summary {
        first_nonempty(&[
            verification_status.clone(),
            story_state.clone(),
            summary_status.clone(),
        ])
    } else {
        first_nonempty(&[
            summary_status.clone(),
            verification_status.clone(),
            story_state.clone(),
        ])
    };
    let authoritative_status = if status.is_empty() {
        synthesized_status(supervisor)
    } else {
        status.clone()
    };
    let next_actions = authoritative_next_actions(&snapshot.next_actions, supervisor);
    let route = authoritative_route(
        &snapshot.trace_metadata,
        supervisor,
        &task,
        &authoritative_status,
        snapshot.routing_runtime_version,
    );
    let blockers = supervisor
        .get("blockers")
        .and_then(Value::as_object)
        .and_then(|blockers| blockers.get("open_blockers"))
        .and_then(Value::as_array)
        .map(|items| {
            stable_line_items(
                items
                    .iter()
                    .map(|item| value_text(Some(item)))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    let scope = value_string_list(contract.get("scope"));
    let forbidden_scope = value_string_list(contract.get("forbidden_scope"));
    let acceptance_criteria = value_string_list(contract.get("acceptance_criteria"));
    let evidence_required = value_string_list(contract.get("evidence_required"));
    let evidence_count = normalize_evidence_index(&snapshot.evidence_index).len();
    let evidence_missing =
        evidence_count == 0 && (!evidence_required.is_empty() || !acceptance_criteria.is_empty());
    let missing_recovery_anchors = stable_line_items(vec![
        if snapshot.session_summary_text.trim().is_empty() {
            "SESSION_SUMMARY".to_string()
        } else {
            String::new()
        },
        if object_has_any_signal(&snapshot.next_actions).not() {
            "NEXT_ACTIONS".to_string()
        } else {
            String::new()
        },
        if object_has_any_signal(&snapshot.trace_metadata).not() {
            "TRACE_METADATA".to_string()
        } else {
            String::new()
        },
        if supervisor.is_empty() {
            "SUPERVISOR_STATE".to_string()
        } else {
            String::new()
        },
    ]);
    let terminal_reasons = terminal_continuity_reasons(
        &summary_phase,
        &summary_status,
        &supervisor_phase,
        &verification_status,
        &story_state,
    );
    let inconsistency_reasons = stable_line_items(vec![
        if !summary_task.is_empty()
            && !trace_task.is_empty()
            && !looks_same_identity(&summary_task, &trace_task)
        {
            format!(
                "session summary task '{summary_task}' disagrees with trace task '{trace_task}'"
            )
        } else {
            String::new()
        },
        if summary_terminal
            && !supervisor_terminal
            && (!supervisor_phase.is_empty() || !verification_status.is_empty())
        {
            "session summary marks the task terminal while supervisor still looks active"
                .to_string()
        } else {
            String::new()
        },
        if supervisor_terminal
            && !summary_terminal
            && (!summary_phase.is_empty() || !summary_status.is_empty())
            && !supervisor_terminal_overrides_summary
        {
            "supervisor marks the task terminal while the session summary still looks active"
                .to_string()
        } else {
            String::new()
        },
        if value_bool_or_none(continuity.get("resume_allowed")) == Some(true)
            && !terminal_reasons.is_empty()
        {
            "continuity.resume_allowed=true conflicts with terminal lifecycle metadata".to_string()
        } else {
            String::new()
        },
        join_lines(&snapshot.control_plane_inconsistency_reasons),
    ]);
    let stale_reasons = stale_continuity_reasons(
        StaleContinuityInputs {
            continuity: &continuity,
            story_state: &story_state,
            task: &task,
            supervisor_phase: &supervisor_phase,
            verification_status: &verification_status,
            next_actions: &next_actions,
            session_summary_missing: snapshot.session_summary_text.trim().is_empty(),
            terminal_reasons_empty: terminal_reasons.is_empty(),
        },
        Local::now().fixed_offset(),
    );
    let current_execution = json!({
        "task": task,
        "phase": phase,
        "status": if status.is_empty() && (!task.is_empty() || !next_actions.is_empty() || !blockers.is_empty()) {
            "in_progress".to_string()
        } else {
            status.clone()
        },
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
    });
    let recent_completed_execution = json!({
        "task": task,
        "phase": if phase.is_empty() {
            first_nonempty(&[story_state.clone(), supervisor_phase.clone()])
        } else {
            phase.clone()
        },
        "status": if status.is_empty() { "completed".to_string() } else { status.clone() },
        "route": route,
        "follow_up_notes": next_actions,
        "terminal_reasons": terminal_reasons,
    });
    let has_any_runtime_signal = !snapshot.session_summary_text.trim().is_empty()
        || object_has_any_signal(&snapshot.next_actions)
        || object_has_any_signal(&snapshot.evidence_index)
        || object_has_any_signal(&snapshot.trace_metadata)
        || !supervisor.is_empty();
    let missing_control_plane_anchors = missing_control_plane_anchors(snapshot);
    let has_missing_control_plane_anchors = !missing_control_plane_anchors.is_empty();
    let has_missing_recovery_anchors = !missing_recovery_anchors.is_empty();
    let state = if !has_any_runtime_signal
        || (task.is_empty() && (has_missing_recovery_anchors || has_missing_control_plane_anchors))
    {
        "missing"
    } else if !inconsistency_reasons.is_empty() {
        "inconsistent"
    } else if !terminal_reasons.is_empty() {
        "completed"
    } else if has_missing_recovery_anchors || has_missing_control_plane_anchors {
        "inconsistent"
    } else if !stale_reasons.is_empty() {
        "stale"
    } else {
        "active"
    };
    let can_resume = state == "active"
        && !has_missing_recovery_anchors
        && !has_missing_control_plane_anchors
        && !task.is_empty();
    let recovery_hints = match state {
        "missing" => json!([
            "Refresh SESSION_SUMMARY.md, NEXT_ACTIONS.json, TRACE_METADATA.json, and .supervisor_state.json before injecting continuity."
        ]),
        "completed" => json!([
            "Keep this task only as recent-completed context; do not inject it as current execution.",
            "Start a new standalone task before resuming related work."
        ]),
        "stale" => json!([
            "Re-read the live continuity artifacts and rebuild a fresh active task before injecting execution context.",
            "Do not continue from the stale snapshot without a new supervisor-owned continuity refresh."
        ]),
        "inconsistent" => json!([
            "Reconcile SESSION_SUMMARY.md, NEXT_ACTIONS.json, TRACE_METADATA.json, artifacts/current pointers, task_registry.json, and .supervisor_state.json before injecting continuity.",
            "Treat the current snapshot as blocked until the supervisor rewrites a consistent continuity bundle."
        ]),
        _ => json!([]),
    };
    json!({
        "state": state,
        "can_resume": can_resume,
        "task": task,
        "phase": phase,
        "status": status,
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "evidence_count": evidence_count,
        "evidence_missing": evidence_missing,
        "verification_status": if verification_status.is_empty() { Value::Null } else { Value::String(verification_status.clone()) },
        "missing_recovery_anchors": missing_recovery_anchors,
        "missing_control_plane_anchors": missing_control_plane_anchors,
        "current_execution": if state == "active" && !task.is_empty() { current_execution } else { Value::Null },
        "recent_completed_execution": if state == "completed" && !task.is_empty() { recent_completed_execution } else { Value::Null },
        "stale_reasons": stale_reasons,
        "terminal_reasons": terminal_reasons,
        "inconsistency_reasons": inconsistency_reasons,
        "recovery_hints": recovery_hints,
        "continuity": {
            "story_state": nonempty_string(Some(&Value::String(story_state))),
            "resume_allowed": value_bool_or_none(continuity.get("resume_allowed")),
            "last_updated_at": nonempty_string(continuity.get("last_updated_at")),
            "active_lease_expires_at": nonempty_string(continuity.get("active_lease_expires_at")),
            "state_reason": nonempty_string(continuity.get("state_reason")),
        },
        "summary_fields": summary,
        "paths": {
            "session_summary": snapshot.current_root.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": snapshot.current_root.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": snapshot.current_root.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": snapshot.current_root.join(TRACE_METADATA_FILENAME).display().to_string(),
            "task_root": snapshot.task_root.display().to_string(),
            "current_pointer_root": snapshot.mirror_root.display().to_string(),
            "supervisor_state": snapshot.repo_root.join(SUPERVISOR_STATE_FILENAME).display().to_string(),
        }
    })
}

fn missing_control_plane_anchors(snapshot: &FrameworkRuntimeView) -> Vec<String> {
    stable_line_items(vec![
        if snapshot.active_task_pointer_present {
            String::new()
        } else {
            ACTIVE_TASK_POINTER_NAME.to_string()
        },
        if snapshot.focus_task_pointer_present {
            String::new()
        } else {
            FOCUS_TASK_POINTER_NAME.to_string()
        },
        if snapshot.task_registry_present {
            String::new()
        } else {
            TASK_REGISTRY_NAME.to_string()
        },
        if snapshot.supervisor_state.is_empty() {
            SUPERVISOR_STATE_FILENAME.to_string()
        } else {
            String::new()
        },
    ])
}

#[derive(Debug, Clone, Copy)]
struct StaleContinuityInputs<'a> {
    continuity: &'a Map<String, Value>,
    story_state: &'a str,
    task: &'a str,
    supervisor_phase: &'a str,
    verification_status: &'a str,
    next_actions: &'a [String],
    session_summary_missing: bool,
    terminal_reasons_empty: bool,
}

fn object_field(map: &Map<String, Value>, key: &str) -> Map<String, Value> {
    map.get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn terminal_continuity_reasons(
    summary_phase: &str,
    summary_status: &str,
    supervisor_phase: &str,
    verification_status: &str,
    story_state: &str,
) -> Vec<String> {
    stable_line_items(vec![
        terminal_reason("summary phase is terminal", summary_phase, TERMINAL_PHASES),
        terminal_reason(
            "summary status is terminal",
            summary_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason(
            "supervisor phase is terminal",
            supervisor_phase,
            TERMINAL_PHASES,
        ),
        terminal_reason(
            "verification status is terminal",
            verification_status,
            TERMINAL_VERIFICATION_STATUSES,
        ),
        terminal_reason(
            "continuity story_state is terminal",
            story_state,
            TERMINAL_STORY_STATES,
        ),
    ])
}

fn stale_continuity_reasons(
    input: StaleContinuityInputs<'_>,
    now: DateTime<FixedOffset>,
) -> Vec<String> {
    let resume_allowed = value_bool_or_none(input.continuity.get("resume_allowed"));
    let state_reason = value_text(input.continuity.get("state_reason"));
    stable_line_items(vec![
        if is_terminal(input.story_state, STALE_STORY_STATES) {
            format!("continuity story_state is stale: {}", input.story_state)
        } else {
            String::new()
        },
        if resume_allowed == Some(false) && input.terminal_reasons_empty {
            "continuity explicitly disallows resume".to_string()
        } else {
            String::new()
        },
        match parse_iso_timestamp(input.continuity.get("active_lease_expires_at")) {
            Some(expires_at) if expires_at < now => {
                format!(
                    "active lease expired at {}",
                    value_text(input.continuity.get("active_lease_expires_at"))
                )
            }
            _ => String::new(),
        },
        if input.session_summary_missing
            && input.terminal_reasons_empty
            && (!input.task.is_empty()
                || !input.supervisor_phase.is_empty()
                || !input.verification_status.is_empty()
                || !input.next_actions.is_empty())
        {
            "session summary mirror is missing while supervisor still looks active".to_string()
        } else {
            String::new()
        },
        if !state_reason.is_empty()
            && (is_terminal(input.story_state, STALE_STORY_STATES) || resume_allowed == Some(false))
        {
            format!("state reason: {state_reason}")
        } else {
            String::new()
        },
    ])
}

fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn render_framework_refresh_prompt(
    continuity: &Value,
    contract: &Map<String, Value>,
    max_lines: usize,
) -> String {
    let capped_max_lines = max_lines.clamp(2, 4);
    let state = value_text(continuity.get("state"));
    let task = value_text(continuity.get("task"));
    let phase = value_text(continuity.get("phase"));
    let status = {
        let raw = value_text(continuity.get("status"));
        if raw.is_empty() {
            state.clone()
        } else {
            raw
        }
    };
    let route = value_string_list(continuity.get("route"));
    let paths_map = continuity
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = continuity
        .get("current_execution")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let completed = continuity
        .get("recent_completed_execution")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let recovery_hints = value_string_list(continuity.get("recovery_hints"));
    let continuity_next_actions = value_string_list(continuity.get("next_actions"));
    let continuity_blockers = value_string_list(continuity.get("blockers"));
    let verification_status = value_text(continuity.get("verification_status"));
    let effect_line = if state == "completed" {
        if verification_status == "completed" {
            "结果已经稳定，可以直接按已完成上下文来看。".to_string()
        } else {
            "这一轮已经收住，不用再把它当当前任务。".to_string()
        }
    } else {
        String::new()
    };
    let remaining_tasks = if state == "active" && !current.is_empty() {
        stable_line_items(
            contract
                .get("scope")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(
                    contract
                        .get("acceptance_criteria")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect(),
        )
    } else if state == "completed" && !completed.is_empty() {
        stable_line_items(vec!["最近一轮已经收尾".to_string()])
    } else if state == "inconsistent" {
        value_string_list(continuity.get("inconsistency_reasons"))
    } else {
        recovery_hints.clone()
    };
    let next_steps = if state == "active" && !current.is_empty() {
        let mut items = vec!["先核对恢复锚点和当前代码".to_string()];
        items.extend(continuity_next_actions.clone());
        stable_line_items(items)
    } else if state == "completed" && !completed.is_empty() {
        stable_line_items(vec![
            "如果还要继续相关工作，先新开一个 standalone task".to_string()
        ])
    } else if state == "stale" {
        let mut items = vec!["先重读锚点并重建上下文".to_string()];
        if continuity_next_actions.is_empty() {
            items.extend(recovery_hints.clone());
        } else {
            items.extend(continuity_next_actions.clone());
        }
        stable_line_items(items)
    } else if state == "inconsistent" {
        let mut items = vec!["先对齐摘要、轨迹和 supervisor".to_string()];
        items.extend(recovery_hints.clone());
        stable_line_items(items)
    } else {
        let mut items = vec!["先补齐缺失锚点并确认状态".to_string()];
        if continuity_next_actions.is_empty() {
            items.extend(recovery_hints.clone());
        } else {
            items.extend(continuity_next_actions.clone());
        }
        stable_line_items(items)
    };
    let blockers = if state == "completed" {
        Vec::new()
    } else {
        continuity_blockers.clone()
    };
    let anchors = stable_line_items(vec![
        value_text(paths_map.get("session_summary"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "SESSION_SUMMARY: {}",
                    value_text(paths_map.get("session_summary"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("next_actions"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "NEXT_ACTIONS: {}",
                    value_text(paths_map.get("next_actions"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("trace_metadata"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "TRACE_METADATA: {}",
                    value_text(paths_map.get("trace_metadata"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("supervisor_state"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "SUPERVISOR_STATE: {}",
                    value_text(paths_map.get("supervisor_state"))
                )
            })
            .unwrap_or_default(),
    ]);

    if state == "completed" && !completed.is_empty() {
        let mut lines = vec!["最近一轮已经收尾：".to_string()];
        lines.push(format!(
            "- {}",
            if task.is_empty() {
                "上一轮任务已完成"
            } else {
                &task
            }
        ));
        if !effect_line.is_empty() {
            lines.push(format!("- {effect_line}"));
        }
        lines.extend(
            next_steps
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
        lines.push(String::new());
        lines.push("先看这些恢复锚点：".to_string());
        lines.extend(
            anchors
                .into_iter()
                .take(capped_max_lines)
                .map(|anchor| format!("- {anchor}")),
        );
        return lines.join("\n") + "\n";
    }

    let mut lines = vec!["继续当前仓库，先看这些恢复锚点：".to_string()];
    lines.extend(
        anchors
            .into_iter()
            .take(capped_max_lines)
            .map(|anchor| format!("- {anchor}")),
    );
    lines.push(String::new());
    lines.push(format!(
        "任务：{}",
        if task.is_empty() { "未记录" } else { &task }
    ));
    lines.push(format!(
        "状态：{}",
        join_lines(&stable_line_items(vec![
            if phase.is_empty() {
                String::new()
            } else {
                phase.clone()
            },
            if status.is_empty() {
                if state.is_empty() {
                    "missing".to_string()
                } else {
                    state.clone()
                }
            } else {
                status.clone()
            },
            if state.is_empty() {
                String::new()
            } else {
                state.clone()
            },
        ]))
    ));
    if !route.is_empty() {
        lines.push(format!("路由：{}", join_lines(&route)));
    }
    if !remaining_tasks.is_empty() {
        lines.push(String::new());
        lines.push("剩余：".to_string());
        lines.extend(
            remaining_tasks
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    if !next_steps.is_empty() {
        lines.push(String::new());
        lines.push("先做：".to_string());
        lines.extend(
            next_steps
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    if !blockers.is_empty() {
        lines.push(String::new());
        lines.push("阻塞：".to_string());
        lines.extend(
            blockers
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    lines.push(String::new());
    lines.push("按既定串并行分工直接开始执行。".to_string());
    lines.join("\n") + "\n"
}

pub fn build_framework_prompt_compression_envelope(payload: Value) -> Result<Value, String> {
    let prompt = value_text(payload.get("prompt").or_else(|| payload.get("text")));
    let token_budget = payload
        .get("token_budget")
        .or_else(|| payload.get("budget"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            "framework prompt compression requires token_budget or budget".to_string()
        })?;
    let result = compress_prompt_with_rust_policy(&prompt, token_budget);
    Ok(json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "authority": FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY,
        "compression": result,
    }))
}

fn compress_prompt_with_rust_policy(prompt: &str, token_budget: usize) -> Value {
    let input_token_estimate = estimate_token_count(prompt);
    if token_budget == 0 {
        let output = "[omitted: token budget is zero]".to_string();
        return compression_payload(
            input_token_estimate,
            estimate_token_count(&output),
            &output,
            "zero_budget",
            true,
            &["all".to_string()],
        );
    }
    if input_token_estimate <= token_budget {
        return compression_payload(
            input_token_estimate,
            input_token_estimate,
            prompt,
            "unchanged",
            false,
            &[],
        );
    }

    let lines = prompt.lines().collect::<Vec<_>>();
    let target_chars = token_budget.saturating_mul(4).max(1);
    let (output, strategy, omitted_sections) = if lines.len() >= 6 {
        let head = lines
            .iter()
            .take(3)
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        let tail = lines
            .iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        let omitted = lines.len().saturating_sub(head.len() + tail.len());
        (
            [
                head,
                vec![format!("[omitted {omitted} middle lines]")],
                tail,
            ]
            .concat()
            .join("\n"),
            "structured_head_tail".to_string(),
            vec![format!("middle_lines:{omitted}")],
        )
    } else {
        let mut truncated = prompt.chars().take(target_chars).collect::<String>();
        truncated.push_str("\n[truncated tail]");
        (
            truncated,
            "tail_truncation".to_string(),
            vec!["tail".to_string()],
        )
    };
    let bounded_output = enforce_prompt_budget(output, token_budget);
    compression_payload(
        input_token_estimate,
        estimate_token_count(&bounded_output),
        &bounded_output,
        &strategy,
        true,
        &omitted_sections,
    )
}

fn enforce_prompt_budget(output: String, token_budget: usize) -> String {
    let max_chars = token_budget.saturating_mul(4).max(1);
    if output.chars().count() <= max_chars {
        return output;
    }
    let marker = "\n[truncated tail]";
    if max_chars <= marker.chars().count() {
        return "[truncated]".chars().take(max_chars).collect();
    }
    let keep = max_chars - marker.chars().count();
    format!(
        "{}{}",
        output.chars().take(keep).collect::<String>(),
        marker
    )
}

fn compression_payload(
    input_token_estimate: usize,
    output_token_estimate: usize,
    output: &str,
    strategy: &str,
    truncated: bool,
    omitted_sections: &[String],
) -> Value {
    json!({
        "schema_version": FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION,
        "policy_owner": "rust",
        "prompt_policy_owner": "rust",
        "input_token_estimate": input_token_estimate,
        "output_token_estimate": output_token_estimate,
        "output": output,
        "compressed_prompt": output,
        "omitted_sections": omitted_sections,
        "strategy": strategy,
        "truncated": truncated,
        "artifact_offload_decision": false,
    })
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    write_atomic_text(path, content)?;
    Ok(true)
}

fn write_atomic_text(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt")
    ));
    fs::write(&tmp_path, content)
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn current_file_hash(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!(
            "read file hash failed for {}: {err}",
            path.display()
        )),
    }
}

#[cfg(test)]
pub(crate) fn hash_file_for_test(path: &Path) -> Result<String, String> {
    current_file_hash(path)?.ok_or_else(|| format!("missing test file: {}", path.display()))
}

fn assert_expected_file_hash(
    path: &Path,
    expected_hash: Option<&str>,
    label: &str,
) -> Result<(), String> {
    let Some(expected_hash) = expected_hash else {
        return Ok(());
    };
    let current = current_file_hash(path)?;
    if current.as_deref() == Some(expected_hash) {
        return Ok(());
    }
    Err(format!(
        "stale {label} update rejected for {}; expected hash {expected_hash}, current hash {}",
        path.display(),
        current.unwrap_or_else(|| "<missing>".to_string())
    ))
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize JSON payload failed: {err}"))?
    );
    write_text_if_changed(path, &serialized)
}

fn join_lines(values: &[String]) -> String {
    values
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ")
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn render_session_summary(task: &str, phase: &str, status: &str, summary: &str) -> String {
    [
        "# SESSION_SUMMARY".to_string(),
        String::new(),
        format!("- task: {task}"),
        format!("- phase: {phase}"),
        format!("- status: {status}"),
        String::new(),
        "## Summary".to_string(),
        if summary.trim().is_empty() {
            "No summary provided.".to_string()
        } else {
            summary.trim().to_string()
        },
        String::new(),
    ]
    .join("\n")
}

fn build_next_actions_payload(actions: &[String]) -> Value {
    json!({
        "schema_version": NEXT_ACTIONS_SCHEMA_VERSION,
        "next_actions": actions,
    })
}

fn build_evidence_index_payload(entries: &[Value]) -> Value {
    json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": entries,
    })
}

fn build_trace_metadata_payload(
    task: &str,
    phase: &str,
    status: &str,
    trace_metadata: Option<&Value>,
    matched_skills: Option<&Value>,
) -> Value {
    let mut payload = trace_metadata
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "schema_version".to_string(),
        Value::String(TRACE_METADATA_SCHEMA_VERSION.to_string()),
    );
    payload.insert("task".to_string(), Value::String(task.to_string()));
    payload.insert("phase".to_string(), Value::String(phase.to_string()));
    payload.insert(
        "verification_status".to_string(),
        Value::String(status.to_string()),
    );
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(current_local_timestamp()));
    if let Some(skills) = normalized_string_array(matched_skills)
        .or_else(|| normalized_string_array(payload.get("matched_skills")))
    {
        payload.insert("matched_skills".to_string(), Value::Array(skills));
    } else {
        payload.insert("matched_skills".to_string(), Value::Array(Vec::new()));
    }
    Value::Object(payload)
}

fn build_session_supervisor_state_payload(input: SupervisorStateInput<'_>) -> Value {
    let mut payload = input
        .supervisor_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "schema_version".to_string(),
        Value::String(SUPERVISOR_STATE_SCHEMA_VERSION.to_string()),
    );
    payload.insert(
        "task_id".to_string(),
        Value::String(input.task_id.to_string()),
    );
    payload.insert(
        "task_summary".to_string(),
        Value::String(input.task.to_string()),
    );
    payload.insert(
        "active_phase".to_string(),
        Value::String(input.phase.to_string()),
    );
    payload.insert(
        "updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    if !input.summary.is_empty() {
        payload.insert(
            "last_summary".to_string(),
            Value::String(input.summary.to_string()),
        );
    }
    payload.insert(
        "verification".to_string(),
        normalized_verification(payload.get("verification"), input.status),
    );
    payload.insert(
        "continuity".to_string(),
        normalized_continuity(
            input.continuity.or_else(|| payload.get("continuity")),
            input.status,
        ),
    );
    payload.insert(
        "next_actions".to_string(),
        input
            .next_actions_payload
            .get("next_actions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    payload.insert(
        "evidence_count".to_string(),
        Value::from(normalize_evidence_index(input.evidence_payload).len()),
    );
    if let Some(contract) = input
        .execution_contract
        .or_else(|| payload.get("execution_contract"))
    {
        payload.insert("execution_contract".to_string(), contract.clone());
    }
    payload.insert(
        "blockers".to_string(),
        normalized_blockers(input.blockers.or_else(|| payload.get("blockers"))),
    );
    payload.insert(
        "trace_metadata".to_string(),
        input.trace_metadata_payload.clone(),
    );
    payload.insert(
        "artifact_refs".to_string(),
        json!({
            "task_root": input.artifact_dir.display().to_string(),
            "session_summary": input.artifact_dir.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": input.artifact_dir.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": input.artifact_dir.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": input.artifact_dir.join(TRACE_METADATA_FILENAME).display().to_string(),
            "continuity_journal": input.artifact_dir.join(CONTINUITY_JOURNAL_FILENAME).display().to_string(),
        }),
    );
    Value::Object(payload)
}

fn build_continuity_journal_payload(input: ContinuityJournalInput<'_>) -> Value {
    let summary_sha = sha256_hex(input.summary_text.as_bytes());
    let next_actions_sha = sha256_json(input.next_actions_payload);
    let evidence_sha = sha256_json(input.evidence_payload);
    let trace_sha = sha256_json(input.trace_metadata_payload);
    let supervisor_sha = sha256_json(input.supervisor_state_payload);
    let checkpoint_hash = sha256_hex(
        [
            summary_sha.as_str(),
            next_actions_sha.as_str(),
            evidence_sha.as_str(),
            trace_sha.as_str(),
            supervisor_sha.as_str(),
        ]
        .join(":")
        .as_bytes(),
    );
    let checkpoint = json!({
        "checkpoint_id": checkpoint_hash,
        "task_id": input.task_id,
        "task": input.task,
        "phase": input.phase,
        "status": input.status,
        "created_at": current_local_timestamp(),
        "artifact_hashes": {
            "session_summary": summary_sha,
            "next_actions": next_actions_sha,
            "evidence_index": evidence_sha,
            "trace_metadata": trace_sha,
            "supervisor_state": supervisor_sha,
        },
        "artifact_refs": {
            "task_root": input.artifact_dir.display().to_string(),
            "session_summary": input.artifact_dir.join(SESSION_SUMMARY_FILENAME).display().to_string(),
            "next_actions": input.artifact_dir.join(NEXT_ACTIONS_FILENAME).display().to_string(),
            "evidence_index": input.artifact_dir.join(EVIDENCE_INDEX_FILENAME).display().to_string(),
            "trace_metadata": input.artifact_dir.join(TRACE_METADATA_FILENAME).display().to_string(),
        }
    });
    let mut checkpoints = input
        .existing_journal
        .get("checkpoints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.get("checkpoint_id").and_then(Value::as_str) != Some(&checkpoint_hash))
        .collect::<Vec<_>>();
    checkpoints.push(checkpoint);
    while checkpoints.len() > 20 {
        checkpoints.remove(0);
    }
    json!({
        "schema_version": CONTINUITY_JOURNAL_SCHEMA_VERSION,
        "task_id": input.task_id,
        "task": input.task,
        "latest_checkpoint_id": checkpoint_hash,
        "checkpoint_count": checkpoints.len(),
        "checkpoints": checkpoints,
    })
}

fn sha256_json(value: &Value) -> String {
    sha256_hex(
        serde_json::to_string(value)
            .unwrap_or_else(|_| "null".to_string())
            .as_bytes(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn normalized_verification(existing: Option<&Value>, status: &str) -> Value {
    let mut payload = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    payload.insert(
        "verification_status".to_string(),
        Value::String(status.to_string()),
    );
    payload.insert(
        "updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    Value::Object(payload)
}

fn normalized_continuity(existing: Option<&Value>, status: &str) -> Value {
    let mut payload = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let terminal = is_terminal(status, TERMINAL_VERIFICATION_STATUSES)
        || is_terminal(status, TERMINAL_STORY_STATES);
    payload.insert(
        "story_state".to_string(),
        Value::String(if terminal { "completed" } else { "active" }.to_string()),
    );
    payload.insert("resume_allowed".to_string(), Value::Bool(!terminal));
    payload.insert(
        "last_updated_at".to_string(),
        Value::String(current_local_timestamp()),
    );
    Value::Object(payload)
}

fn normalized_blockers(existing: Option<&Value>) -> Value {
    let Some(value) = existing else {
        return json!({"open_blockers": []});
    };
    if value.is_object() {
        return value.clone();
    }
    if let Some(items) = normalized_string_array(Some(value)) {
        return json!({"open_blockers": items});
    }
    json!({"open_blockers": []})
}

fn normalized_string_array(value: Option<&Value>) -> Option<Vec<Value>> {
    let values = value_string_list(value);
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().map(Value::String).collect())
    }
}

fn read_json_if_exists(path: &Path) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Map::new())),
        Err(_) => Value::Object(Map::new()),
    }
}

fn read_json_strict(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read json failed for {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse json failed for {}: {err}", path.display()))
}

fn read_json_control_plane_field(
    path: &Path,
    label: &str,
    parse_errors: &mut Vec<String>,
) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match read_json_strict(path) {
        Ok(value) => value,
        Err(err) => {
            parse_errors.push(format!(
                "invalid control-plane json ({label}, {}): {err}",
                path.display()
            ));
            Value::Object(Map::new())
        }
    }
}

fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string()
}

fn build_task_id(task: &str, created_at: Option<&str>) -> String {
    let stamp = created_at
        .unwrap_or(&current_local_timestamp())
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    let slug = safe_slug(task);
    if stamp.is_empty() {
        slug
    } else {
        let suffix = if stamp.len() > 14 {
            &stamp[stamp.len() - 14..]
        } else {
            &stamp
        };
        format!("{slug}-{suffix}")
    }
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn nonempty_string(value: Option<&Value>) -> Option<String> {
    let text = value_text(value);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn value_bool_or_none(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
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

fn first_nonempty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn stable_line_items(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let value = item.trim().to_string();
        if value.is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        result.push(value);
    }
    result
}

fn parse_session_summary(text: &str) -> Map<String, Value> {
    let mut result = Map::new();
    for line in text.lines() {
        if !line.starts_with("- ") {
            continue;
        }
        let body = &line[2..];
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        result.insert(
            key.trim().to_string(),
            Value::String(value.trim().to_string()),
        );
    }
    result
}

fn normalized_task_registry(payload: &Value) -> (Value, Vec<String>, Vec<String>) {
    let focus_task_id = safe_slug(&value_text(payload.get("focus_task_id")));
    normalize_task_registry_rows(focus_task_id, registry_rows_from_payload(payload))
}

fn registry_rows_from_payload(payload: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(items) = payload.get("tasks").and_then(Value::as_array) {
        for item in items {
            let Some(row) = item.as_object() else {
                continue;
            };
            let task_id = safe_slug(&value_text(row.get("task_id")));
            if task_id.is_empty() {
                continue;
            }
            let task = value_text(row.get("task"));
            let task_value = if task.is_empty() {
                Value::String(task_id.clone())
            } else {
                Value::String(task)
            };
            rows.push(json!({
                "task_id": task_id,
                "task": task_value,
                "updated_at": nonempty_string(row.get("updated_at")),
                "status": nonempty_string(row.get("status")),
                "phase": nonempty_string(row.get("phase")),
                "resume_allowed": value_bool_or_none(row.get("resume_allowed")),
            }));
        }
    }
    rows
}

fn normalize_task_registry_rows(
    focus_task_id: String,
    mut rows: Vec<Value>,
) -> (Value, Vec<String>, Vec<String>) {
    rows.sort_by(|left, right| {
        registry_task_sort_key(right)
            .cmp(&registry_task_sort_key(left))
            .then_with(|| value_text(right.get("task_id")).cmp(&value_text(left.get("task_id"))))
    });

    let mut seen = HashSet::new();
    let mut tasks = Vec::new();
    let mut known_task_ids = Vec::new();
    let mut recoverable_task_ids = Vec::new();
    let mut overflow_count = 0usize;
    for row in rows {
        let task_id = safe_slug(&value_text(row.get("task_id")));
        if task_id.is_empty() || !seen.insert(task_id.clone()) {
            continue;
        }
        if value_bool_or_none(row.get("resume_allowed")) == Some(true) {
            recoverable_task_ids.push(task_id.clone());
        }
        known_task_ids.push(task_id);
        if tasks.len() >= 128 {
            overflow_count += 1;
            continue;
        }
        tasks.push(row);
    }
    tasks.sort_by(|left, right| {
        let left_focus = value_text(left.get("task_id")) == focus_task_id;
        let right_focus = value_text(right.get("task_id")) == focus_task_id;
        right_focus
            .cmp(&left_focus)
            .then_with(|| registry_task_sort_key(right).cmp(&registry_task_sort_key(left)))
            .then_with(|| value_text(left.get("task_id")).cmp(&value_text(right.get("task_id"))))
    });
    (
        json!({
            "schema_version": TASK_REGISTRY_SCHEMA_VERSION,
            "focus_task_id": if focus_task_id.is_empty() {
                Value::Null
            } else {
                Value::String(focus_task_id)
            },
            "tasks": tasks,
            "task_count": known_task_ids.len(),
            "recoverable_task_count": recoverable_task_ids.len(),
            "truncated": overflow_count > 0,
            "overflow_count": overflow_count,
        }),
        known_task_ids,
        recoverable_task_ids,
    )
}

fn registry_task_sort_key(row: &Value) -> String {
    first_nonempty(&[
        value_text(row.get("updated_at")),
        value_text(row.get("task_id")),
    ])
}

fn write_focus_task_pointer(
    mirror_root: &Path,
    task_id: &str,
    task: &str,
    updated_at: &str,
) -> Result<bool, String> {
    write_json_if_changed(
        &mirror_root.join(FOCUS_TASK_POINTER_NAME),
        &json!({
            "task_id": task_id,
            "task": task,
            "updated_at": updated_at,
        }),
    )
}

fn write_task_registry_entry(
    mirror_root: &Path,
    entry: TaskRegistryEntry<'_>,
) -> Result<bool, String> {
    let existing = read_json_strict(&mirror_root.join(TASK_REGISTRY_NAME))?;
    let focus_task = entry.focus_task_id.map_or_else(
        || safe_slug(&value_text(existing.get("focus_task_id"))),
        ToString::to_string,
    );
    let mut rows = registry_rows_from_payload(&existing);
    let mut replaced = false;
    for row in &mut rows {
        let Some(map) = row.as_object_mut() else {
            continue;
        };
        if safe_slug(&value_text(map.get("task_id"))) != entry.task_id {
            continue;
        }
        map.insert(
            "task_id".to_string(),
            Value::String(entry.task_id.to_string()),
        );
        map.insert("task".to_string(), Value::String(entry.task.to_string()));
        map.insert(
            "updated_at".to_string(),
            Value::String(entry.updated_at.to_string()),
        );
        map.insert(
            "status".to_string(),
            Value::String(entry.status.to_string()),
        );
        map.insert("phase".to_string(), Value::String(entry.phase.to_string()));
        map.insert(
            "resume_allowed".to_string(),
            entry.resume_allowed.map_or(Value::Null, Value::Bool),
        );
        replaced = true;
        break;
    }
    if !replaced {
        rows.push(json!({
            "task_id": entry.task_id,
            "task": entry.task,
            "updated_at": entry.updated_at,
            "status": entry.status,
            "phase": entry.phase,
            "resume_allowed": entry.resume_allowed,
        }));
    }
    let compacted = normalize_task_registry_rows(focus_task, rows).0;
    write_json_if_changed(&mirror_root.join(TASK_REGISTRY_NAME), &compacted)
}

fn defaulted_payload_text(payload: &Value, key: &str, fallback: &str) -> String {
    let value = value_text(payload.get(key));
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn session_artifact_payloads(payload: &Value) -> (Vec<String>, Vec<Value>) {
    let next_actions = payload
        .get("next_actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| value_text(Some(&item)))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let evidence = payload
        .get("evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .collect::<Vec<_>>();
    (next_actions, evidence)
}

fn required_payload_text(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    let value = value_text(payload.get(key));
    if value.is_empty() {
        Err(format!("{context} requires {key}"))
    } else {
        Ok(value)
    }
}

fn resolve_session_task_id(payload: &Value, task: &str) -> String {
    let direct = safe_slug(&value_text(payload.get("task_id")));
    if direct.is_empty() {
        build_task_id(task, None)
    } else {
        direct
    }
}

fn build_session_artifact_write_plan(payload: &Value) -> Result<SessionArtifactWritePlan, String> {
    let output_dir = value_text(payload.get("output_dir"));
    if output_dir.is_empty() {
        return Err("framework session artifact writer requires output_dir".to_string());
    }
    let task = required_payload_text(payload, "task", "framework session artifact writer")?;
    let phase = defaulted_payload_text(payload, "phase", "implementation");
    let status = defaulted_payload_text(payload, "status", "in_progress");
    let summary = value_text(payload.get("summary"));
    let (next_actions, evidence) = session_artifact_payloads(payload);
    let task_id = resolve_session_task_id(payload, &task);
    let focus = value_bool_or_none(payload.get("focus")).unwrap_or(false);
    let repo_root = value_text(payload.get("repo_root"));
    let mirror_output_dir = value_text(payload.get("mirror_output_dir"));
    let output_root = PathBuf::from(&output_dir);
    let primary_dir = if payload.get("task_id").is_some() || !repo_root.is_empty() {
        output_root.join(&task_id)
    } else {
        output_root.clone()
    };
    let summary_path = primary_dir.join(SESSION_SUMMARY_FILENAME);
    let next_actions_path = primary_dir.join(NEXT_ACTIONS_FILENAME);
    let evidence_path = primary_dir.join(EVIDENCE_INDEX_FILENAME);
    let trace_metadata_path = primary_dir.join(TRACE_METADATA_FILENAME);
    let journal_path = primary_dir.join(CONTINUITY_JOURNAL_FILENAME);
    let summary_text = render_session_summary(&task, &phase, &status, &summary);
    let next_actions_payload = build_next_actions_payload(&next_actions);
    let evidence_payload = build_evidence_index_payload(&evidence);
    let trace_metadata_payload = build_trace_metadata_payload(
        &task,
        &phase,
        &status,
        payload.get("trace_metadata"),
        payload.get("matched_skills"),
    );
    let supervisor_state_payload = build_session_supervisor_state_payload(SupervisorStateInput {
        task_id: &task_id,
        task: &task,
        phase: &phase,
        status: &status,
        summary: summary.trim(),
        next_actions_payload: &next_actions_payload,
        evidence_payload: &evidence_payload,
        trace_metadata_payload: &trace_metadata_payload,
        artifact_dir: &primary_dir,
        supervisor_state: payload.get("supervisor_state"),
        execution_contract: payload.get("execution_contract"),
        blockers: payload.get("blockers"),
        continuity: payload.get("continuity"),
    });
    let journal_payload = build_continuity_journal_payload(ContinuityJournalInput {
        task_id: &task_id,
        task: &task,
        phase: &phase,
        status: &status,
        artifact_dir: &primary_dir,
        summary_text: &summary_text,
        next_actions_payload: &next_actions_payload,
        evidence_payload: &evidence_payload,
        trace_metadata_payload: &trace_metadata_payload,
        supervisor_state_payload: &supervisor_state_payload,
        existing_journal: read_json_strict(&journal_path)?,
    });
    Ok(SessionArtifactWritePlan {
        task,
        phase,
        status,
        summary,
        task_id,
        focus,
        repo_root: (!repo_root.is_empty()).then(|| PathBuf::from(repo_root)),
        mirror_output_dir: (!mirror_output_dir.is_empty())
            .then(|| PathBuf::from(mirror_output_dir)),
        summary_path,
        next_actions_path,
        evidence_path,
        trace_metadata_path,
        journal_path,
        next_actions_payload,
        evidence_payload,
        trace_metadata_payload,
        supervisor_state_payload,
        journal_payload,
        expected_active_task_hash: nonempty_string(payload.get("expected_active_task_hash")),
        expected_focus_task_hash: nonempty_string(payload.get("expected_focus_task_hash")),
        expected_supervisor_state_hash: nonempty_string(
            payload.get("expected_supervisor_state_hash"),
        ),
        changed_paths: Vec::new(),
    })
}

fn write_primary_session_artifacts(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    let summary_text = render_session_summary(&plan.task, &plan.phase, &plan.status, &plan.summary);
    let next_actions_payload = plan.next_actions_payload.clone();
    let evidence_payload = plan.evidence_payload.clone();
    let trace_metadata_payload = plan.trace_metadata_payload.clone();
    let journal_payload = plan.journal_payload.clone();
    write_session_artifact_set(
        ArtifactPaths {
            summary: &plan.summary_path,
            next_actions: &plan.next_actions_path,
            evidence: &plan.evidence_path,
            trace_metadata: Some(&plan.trace_metadata_path),
            journal: Some(&plan.journal_path),
        },
        ArtifactPayloads {
            summary_text: &summary_text,
            next_actions: &next_actions_payload,
            evidence: &evidence_payload,
            trace_metadata: &trace_metadata_payload,
            journal: Some(&journal_payload),
        },
        &mut plan.changed_paths,
    )
}

fn write_optional_session_mirror(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    if plan.focus {
        let Some(mirror_root) = plan.mirror_output_dir.clone() else {
            return Ok(());
        };
        let mirror_summary = mirror_root.join(SESSION_SUMMARY_FILENAME);
        let mirror_next_actions = mirror_root.join(NEXT_ACTIONS_FILENAME);
        let mirror_evidence = mirror_root.join(EVIDENCE_INDEX_FILENAME);
        let mirror_trace = mirror_root.join(TRACE_METADATA_FILENAME);
        let mirror_journal = mirror_root.join(CONTINUITY_JOURNAL_FILENAME);
        let summary_text =
            render_session_summary(&plan.task, &plan.phase, &plan.status, &plan.summary);
        let next_actions_payload = plan.next_actions_payload.clone();
        let evidence_payload = plan.evidence_payload.clone();
        let trace_metadata_payload = plan.trace_metadata_payload.clone();
        let journal_payload = plan.journal_payload.clone();
        write_session_artifact_set(
            ArtifactPaths {
                summary: &mirror_summary,
                next_actions: &mirror_next_actions,
                evidence: &mirror_evidence,
                trace_metadata: Some(&mirror_trace),
                journal: Some(&mirror_journal),
            },
            ArtifactPayloads {
                summary_text: &summary_text,
                next_actions: &next_actions_payload,
                evidence: &evidence_payload,
                trace_metadata: &trace_metadata_payload,
                journal: Some(&journal_payload),
            },
            &mut plan.changed_paths,
        )?;
    }
    Ok(())
}

fn write_repo_session_focus(plan: &mut SessionArtifactWritePlan) -> Result<(), String> {
    let Some(repo_root) = plan.repo_root.clone() else {
        return Ok(());
    };
    let mirror_root = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let updated_at = current_local_timestamp();
    if write_task_registry_entry(
        &mirror_root,
        TaskRegistryEntry {
            task_id: &plan.task_id,
            task: &plan.task,
            phase: &plan.phase,
            status: &plan.status,
            resume_allowed: Some(!is_terminal(&plan.status, TERMINAL_VERIFICATION_STATUSES)),
            updated_at: &updated_at,
            focus_task_id: if plan.focus {
                Some(plan.task_id.as_str())
            } else {
                None
            },
        },
    )? {
        plan.changed_paths
            .push(mirror_root.join(TASK_REGISTRY_NAME).display().to_string());
    }
    if plan.focus {
        write_focused_repo_mirrors(plan, &repo_root, &mirror_root, &updated_at)?;
    }
    Ok(())
}

fn write_focused_repo_mirrors(
    plan: &mut SessionArtifactWritePlan,
    repo_root: &Path,
    mirror_root: &Path,
    updated_at: &str,
) -> Result<(), String> {
    let active_pointer = mirror_root.join(ACTIVE_TASK_POINTER_NAME);
    assert_expected_file_hash(
        &active_pointer,
        plan.expected_active_task_hash.as_deref(),
        "active task pointer",
    )?;
    if write_json_if_changed(
        &active_pointer,
        &json!({
            "task_id": plan.task_id,
            "task": plan.task,
            "updated_at": updated_at,
            "task_root": plan.summary_path.parent().map(|path| path.display().to_string()).unwrap_or_default(),
            "session_summary": plan.summary_path.display().to_string(),
            "next_actions": plan.next_actions_path.display().to_string(),
            "evidence_index": plan.evidence_path.display().to_string(),
            "trace_metadata": plan.trace_metadata_path.display().to_string(),
            "continuity_journal": plan.journal_path.display().to_string(),
        }),
    )? {
        plan.changed_paths
            .push(active_pointer.display().to_string());
    }
    let focus_pointer = mirror_root.join(FOCUS_TASK_POINTER_NAME);
    assert_expected_file_hash(
        &focus_pointer,
        plan.expected_focus_task_hash.as_deref(),
        "focus task pointer",
    )?;
    if write_focus_task_pointer(mirror_root, &plan.task_id, &plan.task, updated_at)? {
        plan.changed_paths.push(focus_pointer.display().to_string());
    }
    let supervisor_state_path = repo_root.join(SUPERVISOR_STATE_FILENAME);
    assert_expected_file_hash(
        &supervisor_state_path,
        plan.expected_supervisor_state_hash.as_deref(),
        "supervisor state",
    )?;
    if write_json_if_changed(&supervisor_state_path, &plan.supervisor_state_payload)? {
        plan.changed_paths
            .push(supervisor_state_path.display().to_string());
    }
    Ok(())
}

impl SessionArtifactWritePlan {
    fn into_response(self) -> Value {
        json!({
            "schema_version": FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
            "summary": self.summary_path.display().to_string(),
            "next_actions": self.next_actions_path.display().to_string(),
            "evidence": self.evidence_path.display().to_string(),
            "task_id": self.task_id,
            "changed_paths": self.changed_paths,
        })
    }
}

fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// 构建自动连续性检查点载荷（非完成态 `status=in_progress`，用于 Codex Stop 等钩子）。
///
/// `task_line` 用作路由摘要标题；`summary_text` 写入 SESSION_SUMMARY 正文片段。
pub fn build_automatic_continuity_checkpoint_payload(
    repo_root: &Path,
    task_line: &str,
    summary_text: &str,
) -> Value {
    let output_dir = repo_root.join("artifacts").join(CURRENT_ARTIFACT_DIR);
    let task = if task_line.trim().is_empty() {
        "session-checkpoint".to_string()
    } else {
        truncate_utf8_chars(task_line.trim(), 200)
    };
    let summary = if summary_text.trim().is_empty() {
        "Automatic continuity checkpoint. No summary text was provided; refine in the next turn."
            .to_string()
    } else {
        truncate_utf8_chars(summary_text.trim(), 8000)
    };
    json!({
        "output_dir": output_dir.to_string_lossy(),
        "repo_root": repo_root.to_string_lossy(),
        "task": task,
        "summary": summary,
        "phase": "execution",
        "status": "in_progress",
        "focus": true,
        "next_actions": [
            "Open artifacts/current/SESSION_SUMMARY.md on the next session.",
            "Optional: run `$refresh` or `router-rs framework refresh` for a compact handoff prompt.",
        ],
        "trace_metadata": {
            "checkpoint_kind": "automatic_stop_hook",
        }
    })
}

const MAX_POST_TOOL_EVIDENCE_ARTIFACTS: usize = 120;

fn continuity_post_tool_evidence_env_enabled() -> bool {
    router_rs_env_enabled_default_true("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE")
}

fn extract_codex_shell_command_preview(event: &Value) -> Option<String> {
    let input = event.get("tool_input").and_then(Value::as_object)?;
    let cmd = input
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            input
                .get("cmd")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            input
                .get("script")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            input
                .get("arguments")
                .and_then(Value::as_object)
                .and_then(|a| a.get("command"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })?;
    Some(truncate_utf8_chars(cmd, 2000))
}

fn coerce_exit_code_value(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(n) = value.as_u64() {
        return Some(n as i64);
    }
    if let Some(text) = value.as_str() {
        return text.trim().parse::<i64>().ok();
    }
    None
}

/// 从 Codex `PostToolUse` 载荷中提取退出码（兼容嵌套 `tool_output` / JSON 字符串）。
fn extract_codex_tool_exit_hint(event: &Value) -> Option<i64> {
    let candidates: Vec<Option<&Value>> = vec![
        event.get("exit_code"),
        event.get("exitCode"),
        event.get("tool_output").and_then(|v| v.get("exit_code")),
        event.get("tool_output").and_then(|v| v.get("exitCode")),
        event
            .get("tool_output")
            .and_then(|v| v.get("metadata"))
            .and_then(|m| m.get("exit_code")),
        event.get("result").and_then(|v| v.get("exit_code")),
        event.get("response").and_then(|v| v.get("exit_code")),
    ];
    if let Some(to) = event.get("tool_output") {
        if let Some(text) = to.as_str() {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                if let Some(code) = coerce_exit_code_value(parsed.get("exit_code")) {
                    return Some(code);
                }
                if let Some(code) = coerce_exit_code_value(parsed.get("exitCode")) {
                    return Some(code);
                }
            }
        }
    }
    for candidate in candidates {
        if let Some(code) = coerce_exit_code_value(candidate) {
            return Some(code);
        }
    }
    None
}

fn append_evidence_index_merged_row(
    repo_root: &Path,
    entry: Map<String, Value>,
) -> Result<(), String> {
    if !continuity_post_tool_evidence_env_enabled() {
        return Ok(());
    }
    let _guard = crate::task_write_lock::task_ledger_write_lock()
        .lock()
        .map_err(|_| "task ledger write lock poisoned".to_string())?;
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    if !continuity_session_ready_for_evidence_append(&snapshot) {
        return Ok(());
    }

    let evidence_path = snapshot.current_root.join(EVIDENCE_INDEX_FILENAME);
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create evidence dir: {err}"))?;
    }

    let existing = read_json_strict(&evidence_path)?;
    let mut rows: Vec<Map<String, Value>> = normalize_evidence_index(&existing);
    rows.push(entry);
    if rows.len() > MAX_POST_TOOL_EVIDENCE_ARTIFACTS {
        let drain = rows.len() - MAX_POST_TOOL_EVIDENCE_ARTIFACTS;
        rows.drain(0..drain);
    }
    let payload = json!({
        "schema_version": EVIDENCE_INDEX_SCHEMA_VERSION,
        "artifacts": rows.into_iter().map(Value::Object).collect::<Vec<Value>>(),
    });
    write_json_if_changed(&evidence_path, &payload)?;
    if let Some(tid) = crate::autopilot_goal::read_active_task_id(repo_root) {
        if !tid.is_empty() {
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &tid);
        }
    }
    Ok(())
}

/// `framework hook-evidence-append`：供 Cursor hook 等外部进程写入一条验证记录。
///
/// JSON：`repo_root`（可选）、`command_preview`（必填）、`exit_code`（可选）、`source`（可选，默认 `external_hook`）。
pub fn framework_hook_evidence_append(payload: Value) -> Result<Value, String> {
    let explicit = payload.get("repo_root").and_then(|v| {
        let s = value_text(Some(v));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    });
    let repo_root = resolve_repo_root_arg(explicit.as_deref())?;
    let preview = required_payload_text(&payload, "command_preview", "hook evidence append")?;
    let preview_trim = preview.trim();
    if preview_trim.is_empty() {
        return Err("hook evidence append requires non-empty command_preview".to_string());
    }
    let source = defaulted_payload_text(&payload, "source", "external_hook");
    let exit_code = payload
        .get("exit_code")
        .and_then(|v| coerce_exit_code_value(Some(v)));

    let cursor_hook = source.trim().to_ascii_lowercase().starts_with("cursor_");
    if !cursor_hook && !shell_command_looks_like_verification(preview_trim) {
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "command_preview did not match verification heuristics",
            "schema_version": "router-rs-hook-evidence-append-v1",
            "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
        }));
    }

    let preview_store = truncate_utf8_chars(preview_trim, 2000);
    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("external_hook_verification"));
    entry.insert("source".to_string(), json!(source.trim()));
    entry.insert("command_preview".to_string(), json!(preview_store));
    entry.insert("recorded_at".to_string(), json!(current_local_timestamp()));
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    append_evidence_index_merged_row(&repo_root, entry)?;
    Ok(json!({
        "ok": true,
        "skipped": false,
        "schema_version": "router-rs-hook-evidence-append-v1",
        "authority": FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY,
    }))
}

fn codex_tool_name_normalized(event: &Value) -> String {
    event
        .get("tool_name")
        .or(event.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn tool_name_is_shell_like(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n == "bash"
        || n == "sh"
        || n == "zsh"
        || n == "shell"
        || n.contains("terminal")
        || n.contains("shell")
        || n == "functions.run_terminal_cmd"
        || n == "run_terminal_cmd"
        || n == "powershell"
        || n == "pwsh"
}

fn shell_command_looks_like_verification(command: &str) -> bool {
    let c = command.to_ascii_lowercase();
    // Original (Rust / Python / JS test runners + lint).
    c.contains("cargo test")
        || c.contains("cargo check")
        || c.contains("cargo clippy")
        || c.contains("cargo build")
        || c.contains("cargo fmt")
        || c.contains("cargo nextest")
        || c.contains("cargo hack")
        || c.contains("nextest")
        || c.contains("pytest")
        || c.contains("npm test")
        || c.contains("pnpm test")
        || c.contains("yarn test")
        || c.contains("make test")
        || c.contains("make check")
        || c.contains("make ci")
        || c.contains("make verify")
        || c.contains("go test")
        || c.contains("go vet")
        || c.contains("dotnet test")
        || c.contains("maturin")
        || c.contains("tox")
        || c.contains("uv run")
        || c.contains("just test")
        || c.contains("just check")
        || c.contains("vitest")
        || c.contains("jest")
        || c.contains("ruby test")
        || c.contains("rake test")
        || c.contains("verify_cursor_hooks")
        || c.contains("policy_contracts")
        || c.contains("ruff check")
        || c.contains("ruff format")
        || c.contains("mypy")
        || c.contains("deno test")
        || c.contains("bun test")
        // TypeScript / JS tooling (no `test` keyword).
        || c.contains("tsc --noemit")
        || c.contains("tsc -p")
        || c.contains("eslint")
        || c.contains("prettier --check")
        || c.contains("biome check")
        || c.contains("biome ci")
        // JVM ecosystems.
        || c.contains("gradle test")
        || c.contains("gradlew test")
        || c.contains("gradle check")
        || c.contains("mvn test")
        || c.contains("mvn verify")
        || c.contains("mvn package")
        // E2E / cross-runner test frameworks.
        || c.contains("playwright test")
        || c.contains("nx test")
        || c.contains("nx affected")
        // Repo-local verifier scripts (any path under scripts/ ending with verify*).
        || c.contains("scripts/verify")
        || c.contains("/verify.sh")
        || c.contains("./verify.sh")
        || c.contains("task test")
        || c.contains("task check")
}

fn continuity_session_ready_for_evidence_append(snapshot: &FrameworkRuntimeView) -> bool {
    if snapshot.active_task_pointer_present {
        return true;
    }
    let summary_path = snapshot.current_root.join(SESSION_SUMMARY_FILENAME);
    summary_path.is_file()
}

/// 在宿主 `PostToolUse` 中追加一条「终端类验证命令」到 `EVIDENCE_INDEX.json`（与 session 写入共用锁）。
///
/// `kind` 用于区分来源（如 `codex_post_tool_verification` / `cursor_post_tool_verification`）。
/// 仅在连续性已初始化且命令启发式匹配验证类时写入。`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭。
pub fn try_append_post_tool_shell_evidence(
    repo_root: &Path,
    event: &Value,
    kind: &str,
) -> Result<(), String> {
    if !continuity_post_tool_evidence_env_enabled() {
        return Ok(());
    }
    let tool_name = codex_tool_name_normalized(event);
    if !tool_name_is_shell_like(&tool_name) {
        return Ok(());
    }
    let Some(command_preview) = extract_codex_shell_command_preview(event) else {
        return Ok(());
    };
    if !shell_command_looks_like_verification(&command_preview) {
        return Ok(());
    }

    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let exit_hint = extract_codex_tool_exit_hint(event);
    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!(kind));
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(command_preview));
    entry.insert("recorded_at".to_string(), json!(current_local_timestamp()));
    if !session_id.is_empty() {
        entry.insert("session_id".to_string(), json!(session_id));
    }
    if let Some(ec) = exit_hint {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    append_evidence_index_merged_row(repo_root, entry)?;
    Ok(())
}

/// 在 Codex `PostToolUse` 中追加一条验证类命令到 `EVIDENCE_INDEX.json`（与 session 写入共用锁）。
///
/// 仅在连续性已初始化（存在 `active_task.json` 或当前根的 `SESSION_SUMMARY.md`）且命令看起来像验证
///（如包含 `cargo test` / `pytest` 等）时写入。可通过 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 关闭。
pub fn try_append_codex_post_tool_evidence(repo_root: &Path, event: &Value) -> Result<(), String> {
    try_append_post_tool_shell_evidence(repo_root, event, "codex_post_tool_verification")
}

/// Cursor `PostToolUse`：在连续性就绪时记录终端里执行的验证类命令（与 Codex 启发式一致）。
///
/// 典型载荷由 `cursor_hooks` 归一成含 `tool_name` / `tool_input.command` / `tool_output` 的形状后传入。
pub fn try_append_cursor_post_tool_evidence(repo_root: &Path, event: &Value) -> Result<(), String> {
    try_append_post_tool_shell_evidence(repo_root, event, "cursor_post_tool_verification")
}

pub fn write_framework_session_artifacts(payload: Value) -> Result<Value, String> {
    let _guard = crate::task_write_lock::task_ledger_write_lock()
        .lock()
        .map_err(|_| "task ledger write lock poisoned".to_string())?;
    // Enforce closeout policy before artifact write when applicable.
    // 完成态：硬门禁开启时须附带可通过 evaluate 的 `closeout_record`（见 enforce_closeout_for_session_payload）。
    // 硬门禁：CI/GitHub Actions 下默认开启；本地未设置 ROUTER_RS_CLOSEOUT_ENFORCEMENT 时默认软；显式 `=0` 关闭硬门禁。
    // 非完成态：`enforce_closeout_for_session_payload` 不解析附带 record（避免草稿误伤），需要时请单独 `closeout evaluate`。
    let closeout_evaluation = enforce_closeout_for_session_payload(&payload)?;
    let mut plan = build_session_artifact_write_plan(&payload)?;
    let sync_repo = plan.repo_root.clone();
    let sync_tid = plan.task_id.clone();
    write_primary_session_artifacts(&mut plan)?;
    write_optional_session_mirror(&mut plan)?;
    write_repo_session_focus(&mut plan)?;
    let mut response = plan.into_response();
    if let Some(ref root) = sync_repo {
        if let Ok(resolved) = resolve_repo_root_arg(Some(root.as_path())) {
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &resolved, &sync_tid,
            );
        }
    }
    if let Some(eval) = closeout_evaluation {
        if let Some(obj) = response.as_object_mut() {
            obj.insert("closeout_evaluation".to_string(), eval);
        }
    }
    Ok(response)
}

fn in_ci_like_environment() -> bool {
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        return true;
    }
    match std::env::var("CI") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !t.is_empty() && !matches!(t.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => false,
    }
}

fn closeout_enforcement_disabled_by_env() -> bool {
    match std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "0" | "false" | "off" | "no")
        }
        // 未设置：本地个人场景默认软门禁；CI/GitHub Actions 默认硬门禁（团队/审计友好）。
        Err(_) => !in_ci_like_environment(),
    }
}

/// Apply closeout enforcement to a session-artifact write payload.
///
/// Returns:
/// - `Ok(Some(eval))` when status claims completion and a valid record was
///   provided that passes evaluation. The envelope is attached to the
///   response so callers see the evidence summary alongside the write.
/// - `Ok(None)` when status is not a completion claim. In that case
///   any incidental `closeout_record` is intentionally **not** parsed —
///   in-progress / planning / execution checkpoints often carry placeholder
///   or partial records, and `deny_unknown_fields` plus strict R-rule
///   evaluation would otherwise turn a benign in-progress write into a hard
///   error. Pre-completion validation is the caller's responsibility (run
///   `closeout evaluate` separately) so the artifact-write path stays
///   resilient against in-progress draft records.
/// - `Ok(None)` when status claims completion but programmatic enforcement is off:
///   explicit `ROUTER_RS_CLOSEOUT_ENFORCEMENT`=`0`/`false`/`off`/`no`, **or** the variable is unset
///   while not in CI/GitHub Actions（本地默认软；响应中不附带 `closeout_evaluation`）。
///   团队/CI：未设置且检测到 `CI` 或 `GITHUB_ACTIONS` 时默认硬门禁。
///   Note: `ROUTER_RS_CLOSEOUT_ENFORCEMENT` **set to empty string** is still “set” for this branch
///   resolution — it does **not** receive the unset/local-soft treatment.
/// - `Err(reason)` only when:
///   - status claims completion but no `closeout_record` is provided, or
///   - status claims completion and the provided record fails evaluation
///     (`closeout_allowed=false` or parse error).
fn enforce_closeout_for_session_payload(payload: &Value) -> Result<Option<Value>, String> {
    let status_lower = value_text(payload.get("status")).to_ascii_lowercase();
    let claims_completion = CLOSEOUT_COMPLETION_STATUSES
        .iter()
        .any(|allowed| *allowed == status_lower);
    if !claims_completion {
        return Ok(None);
    }
    if closeout_enforcement_disabled_by_env() {
        return Ok(None);
    }
    let closeout_record = payload.get("closeout_record").cloned().ok_or_else(|| {
        "framework session artifact write claims completion (status in {completed,done,passed,...}) but no closeout_record was provided. \
         A closeout record is required so closeout_enforcement can verify completion evidence (verification_status, commands_run, artifacts_checked, summary). \
         Re-issue the request with a closeout_record matching configs/framework/CLOSEOUT_RECORD_SCHEMA.json.".to_string()
    })?;
    // Try to attach an EvidenceContext so R8 (`claimed_passed_without_evidence_index_rows`) runs.
    // Both repo_root and task_id must resolve from the write payload; otherwise fall back to the
    // record-only evaluator (R7 still catches the most common self-attestation pattern).
    let repo_root_str = value_text(payload.get("repo_root"));
    let task_id_str = value_text(payload.get("task_id"));
    let evaluation = if !repo_root_str.is_empty() && !task_id_str.is_empty() {
        let repo_root = PathBuf::from(&repo_root_str);
        let (rows_non_empty, has_success) =
            crate::autopilot_goal::task_evidence_artifacts_summary_for_task(
                &repo_root,
                &task_id_str,
            );
        let ctx = CloseoutEvidenceContext {
            evidence_rows_non_empty: rows_non_empty,
            has_successful_verification: has_success,
        };
        evaluate_closeout_record_value_with_context(closeout_record, &ctx)
            .map_err(|err| format!("closeout enforcement failed: {err}"))?
    } else {
        evaluate_closeout_record_value(closeout_record)
            .map_err(|err| format!("closeout enforcement failed: {err}"))?
    };
    let allowed = evaluation
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !allowed {
        let violations = evaluation
            .get("violations")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        let missing = evaluation
            .get("missing_evidence")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "[]".to_string());
        return Err(format!(
            "closeout_enforcement blocked completion: closeout_allowed=false. \
             violations={violations} missing_evidence={missing}. \
             Resolve violations or downgrade status before re-issuing the artifact write."
        ));
    }
    Ok(Some(evaluation))
}

fn write_session_artifact_set(
    paths: ArtifactPaths<'_>,
    payloads: ArtifactPayloads<'_>,
    changed_paths: &mut Vec<String>,
) -> Result<(), String> {
    if write_text_if_changed(paths.summary, payloads.summary_text)? {
        changed_paths.push(paths.summary.display().to_string());
    }
    if write_json_if_changed(paths.next_actions, payloads.next_actions)? {
        changed_paths.push(paths.next_actions.display().to_string());
    }
    if write_json_if_changed(paths.evidence, payloads.evidence)? {
        changed_paths.push(paths.evidence.display().to_string());
    }
    if let Some(path) = paths.trace_metadata {
        write_json_artifact_if_changed(path, payloads.trace_metadata, changed_paths)?;
    }
    if let (Some(path), Some(payload)) = (paths.journal, payloads.journal) {
        write_json_artifact_if_changed(path, payload, changed_paths)?;
    }
    Ok(())
}

fn write_json_artifact_if_changed(
    path: &Path,
    payload: &Value,
    changed_paths: &mut Vec<String>,
) -> Result<(), String> {
    if write_json_if_changed(path, payload)? {
        changed_paths.push(path.display().to_string());
    }
    Ok(())
}

fn normalize_evidence_index(payload: &Value) -> Vec<Map<String, Value>> {
    let items = if payload.get("schema_version").and_then(Value::as_str)
        == Some(EVIDENCE_INDEX_SCHEMA_VERSION)
    {
        payload.get("artifacts")
    } else {
        payload.get("artifacts").or_else(|| payload.get("evidence"))
    };
    items
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_object().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_next_actions(payload: &Value) -> Vec<String> {
    let actions = if payload.get("schema_version").and_then(Value::as_str)
        == Some(NEXT_ACTIONS_SCHEMA_VERSION)
    {
        payload.get("next_actions")
    } else {
        payload
            .get("next_actions")
            .or_else(|| payload.get("actions"))
    };
    actions
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(coerce_next_action_line)
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_trace_skills(payload: &Value) -> Vec<String> {
    let skills = if payload.get("schema_version").and_then(Value::as_str)
        == Some(TRACE_METADATA_SCHEMA_VERSION)
    {
        payload.get("matched_skills")
    } else {
        payload
            .get("matched_skills")
            .or_else(|| payload.get("skills"))
    };
    skills
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn coerce_next_action_line(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.trim().to_string();
    }
    if let Some(map) = value.as_object() {
        for key in ["title", "summary", "action", "label", "details"] {
            let text = value_text(map.get(key));
            if !text.is_empty() {
                return text;
            }
        }
    }
    value_text(Some(value))
}

fn authoritative_next_actions(
    snapshot_payload: &Value,
    supervisor_state: &Map<String, Value>,
) -> Vec<String> {
    let supervisor_actions = supervisor_state
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|rows| {
            stable_line_items(
                rows.iter()
                    .map(coerce_next_action_line)
                    .filter(|item| !item.is_empty())
                    .collect(),
            )
        })
        .unwrap_or_default();
    if supervisor_actions.is_empty() {
        normalize_next_actions(snapshot_payload)
    } else {
        supervisor_actions
    }
}

fn fallback_route_from_supervisor(supervisor_state: &Map<String, Value>) -> Vec<String> {
    let controller = supervisor_state
        .get("controller")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    stable_line_items(vec![
        value_text(controller.get("gate")),
        value_text(controller.get("primary_owner")),
        value_text(controller.get("overlay")),
        value_text(controller.get("owner_lane")),
        value_text(supervisor_state.get("primary_owner")),
    ])
}

fn trace_payload_identity_matches(
    payload: &Value,
    task: &str,
    status: &str,
    current_routing_runtime_version: u64,
) -> bool {
    if !payload.is_object() {
        return false;
    }
    let payload_task = value_text(payload.get("task"));
    if payload_task.is_empty() && !normalize_trace_skills(payload).is_empty() {
        return true;
    }
    let payload_status = value_text(payload.get("verification_status"));
    if !looks_same_identity(&payload_task, task) {
        return false;
    }
    if !payload_status.is_empty() && payload_status != status {
        return false;
    }
    if let Some(version) = payload
        .get("routing_runtime_version")
        .and_then(Value::as_u64)
    {
        if version != current_routing_runtime_version {
            return false;
        }
    }
    true
}

fn authoritative_route(
    trace_payload: &Value,
    supervisor_state: &Map<String, Value>,
    task: &str,
    status: &str,
    current_routing_runtime_version: u64,
) -> Vec<String> {
    if trace_payload_identity_matches(trace_payload, task, status, current_routing_runtime_version)
    {
        let route = normalize_trace_skills(trace_payload);
        if !route.is_empty() {
            return route;
        }
    }
    fallback_route_from_supervisor(supervisor_state)
}

fn normalize_supervisor_state(payload: &Value) -> Map<String, Value> {
    let source = payload.as_object().cloned().unwrap_or_default();
    let mut normalized = source.clone();
    normalized.insert(
        "schema_version".to_string(),
        Value::String(SUPERVISOR_STATE_SCHEMA_VERSION.to_string()),
    );

    let delegation = source
        .get("delegation")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            for key in [
                "delegation_plan_created",
                "spawn_attempted",
                "spawn_block_reason",
                "fallback_mode",
                "delegated_sidecars",
            ] {
                if let Some(value) = source.get(key) {
                    map.insert(key.to_string(), value.clone());
                }
            }
            map
        });
    normalized.insert("delegation".to_string(), Value::Object(delegation));

    let verification = source
        .get("verification")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            for key in ["verification_status", "last_verification_summary"] {
                if let Some(value) = source.get(key) {
                    map.insert(key.to_string(), value.clone());
                }
            }
            map
        });
    normalized.insert("verification".to_string(), Value::Object(verification));

    let mut continuity = source
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for key in [
        "story_state",
        "resume_allowed",
        "last_updated_at",
        "active_lease_expires_at",
        "state_reason",
    ] {
        if !continuity.contains_key(key) {
            if let Some(value) = source.get(key) {
                continuity.insert(key.to_string(), value.clone());
            }
        }
    }
    normalized.insert("continuity".to_string(), Value::Object(continuity));

    let blockers = source
        .get("blockers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut map = Map::new();
            if let Some(value) = source.get("open_blockers") {
                map.insert("open_blockers".to_string(), value.clone());
            }
            map
        });
    normalized.insert("blockers".to_string(), Value::Object(blockers));
    normalized
}

fn supervisor_contract(state: &Map<String, Value>) -> Map<String, Value> {
    state
        .get("execution_contract")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn parse_iso_timestamp(value: Option<&Value>) -> Option<DateTime<FixedOffset>> {
    let text = value_text(value);
    if text.is_empty() {
        return None;
    }
    let normalized = if text.ends_with('Z') {
        format!("{}+00:00", &text[..text.len() - 1])
    } else {
        text
    };
    DateTime::parse_from_rfc3339(&normalized).ok()
}

fn load_routing_runtime_version(repo_root: &Path) -> u64 {
    let runtime_path = repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json");
    read_json_if_exists(&runtime_path)
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(1)
}

fn synthesized_status(supervisor_state: &Map<String, Value>) -> String {
    let verification = supervisor_state
        .get("verification")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let continuity = supervisor_state
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    first_nonempty(&[
        value_text(verification.get("verification_status")),
        value_text(continuity.get("story_state")),
        value_text(supervisor_state.get("active_phase")),
        "in_progress".to_string(),
    ])
}

fn object_has_any_signal(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Object(map) => !map.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn terminal_reason(prefix: &str, value: &str, terminal_values: &[&str]) -> String {
    if is_terminal(value, terminal_values) {
        format!("{prefix}: {value}")
    } else {
        String::new()
    }
}

fn is_terminal(value: &str, terminal_values: &[&str]) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    terminal_values
        .iter()
        .any(|candidate| lowered == *candidate)
}

fn looks_same_identity(left: &str, right: &str) -> bool {
    let left_token = safe_slug(&left.to_ascii_lowercase());
    let right_token = safe_slug(&right.to_ascii_lowercase());
    if left_token.is_empty() || right_token.is_empty() {
        return true;
    }
    left_token == right_token
        || left_token.contains(&right_token)
        || right_token.contains(&left_token)
}

#[cfg(test)]
mod resolve_repo_root_tests {
    use super::resolve_repo_root_arg;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn resolve_repo_root_walks_up_from_scripts_router_rs_subdir() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-root-resolve-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        fs::write(
            tmp.join("configs/framework/RUNTIME_REGISTRY.json"),
            r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{}}"#,
        )
        .unwrap();
        fs::create_dir_all(tmp.join("scripts/router-rs/src")).unwrap();
        fs::write(
            tmp.join("scripts/router-rs/Cargo.toml"),
            "[package]\nname = \"router-rs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let subdir = tmp.join("scripts/router-rs/src");
        let resolved = resolve_repo_root_arg(Some(subdir.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_unchanged_when_no_framework_markers() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-no-marker-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let resolved = resolve_repo_root_arg(Some(tmp.as_path())).unwrap();
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(resolved, expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_repo_root_from_cargo_manifest_dir_matches_framework_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let resolved = resolve_repo_root_arg(Some(manifest_dir.as_path())).unwrap();
        let expect = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("skill repo root should resolve");
        assert_eq!(
            resolved, expect,
            "router-rs crate cwd must resolve to framework repo root for continuity/RUNTIME_REGISTRY"
        );
    }
}
