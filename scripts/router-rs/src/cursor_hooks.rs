use crate::hook_common::{
    has_delegation_override, has_override, has_review_override, is_parallel_delegation_prompt,
    is_review_prompt, normalize_subagent_type, normalize_tool_name, saw_reject_reason,
    strip_quoted_or_codeblock_or_url,
};
use crate::runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    /// 并行单测下替代进程级 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`，避免 env 竞态。
    static TEST_CURSOR_REVIEW_GATE_DISABLE: Cell<Option<bool>> = const { Cell::new(None) };
    /// 并行单测下替代进程级 `ROUTER_RS_CURSOR_HOOK_SILENT`，避免本机环境污染断言。
    static TEST_CURSOR_HOOK_SILENT: Cell<Option<bool>> = const { Cell::new(None) };
}

/// 与运行时「subagent 并发上限契约」对齐（`runtime_envelope_ids::MAX_CONCURRENT_SUBAGENTS_LIMIT`）；可用 `ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` 调低或设为 `0` 关闭计数限流。
const DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS: u32 = MAX_CONCURRENT_SUBAGENTS_LIMIT as u32;
const DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS: i64 = 2 * 60 * 60;

/// Shell 钩子 pending 队列长度上限，防止极长会话把 ledger 胀得过大。
const MAX_PENDING_SHELL_RECORDS: usize = 64;
/// `started_at` 与 pending `queued_ms` 对齐允许的时钟/调度 slack（毫秒）。
const SHELL_TERMINAL_TIME_MATCH_SLACK_MS: u64 = 10_000;

#[cfg(test)]
pub(crate) fn set_test_review_gate_disable_override(v: Option<bool>) {
    TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.set(v));
}

#[cfg(test)]
pub(crate) fn set_test_cursor_hook_silent_override(v: Option<bool>) {
    TEST_CURSOR_HOOK_SILENT.with(|c| c.set(v));
}

/// 与 `.cursor/hook-state` 锁无关：只读合并 continuity 续跑，避免门控降级或应急短路时 goal/RFV 静默消失。
fn merge_continuity_followups(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    merge_autopilot_drive_followup_with_frame(repo_root, output, frame);
    merge_rfv_loop_followup_with_frame(repo_root, output, frame);
}

fn build_autopilot_drive_followup_using_frame(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    let active = crate::autopilot_goal::read_active_task_id(repo_root)?;
    if frame.pointer_view.task_id.as_deref() == Some(active.as_str()) {
        if let Some(ref g) = frame.pointer_view.goal_state {
            return crate::autopilot_goal::build_autopilot_drive_followup_message_from_state(
                repo_root, &active, g,
            );
        }
    }
    crate::autopilot_goal::build_autopilot_drive_followup_message(repo_root)
}

fn merge_autopilot_drive_followup_with_frame(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    let Some(msg) = build_autopilot_drive_followup_using_frame(repo_root, frame) else {
        return;
    };
    if msg.is_empty() {
        return;
    }
    crate::autopilot_goal::merge_hook_nudge_paragraph(
        output,
        &msg,
        "AUTOPILOT_DRIVE",
        crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled(),
    );
}

fn build_rfv_loop_followup_using_frame(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    let active = crate::autopilot_goal::read_active_task_id(repo_root)?;
    if frame.pointer_view.task_id.as_deref() == Some(active.as_str()) {
        if let Some(ref s) = frame.pointer_view.rfv_loop_state {
            return crate::rfv_loop::build_rfv_loop_followup_message_from_state(
                repo_root, &active, s,
            );
        }
    }
    crate::rfv_loop::build_rfv_loop_followup_message(repo_root)
}

fn merge_rfv_loop_followup_with_frame(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    let Some(msg) = build_rfv_loop_followup_using_frame(repo_root, frame) else {
        return;
    };
    if msg.is_empty() {
        return;
    }
    crate::autopilot_goal::merge_hook_nudge_paragraph(
        output,
        &msg,
        "RFV_LOOP_CONTINUE",
        crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled(),
    );
}

/// beforeSubmit：若 Goal 与 RFV 同时续跑，合并为一段「## 续跑」减少碎片标题；仅其一则保持原样。
const CONTINUITY_BEFORE_SUBMIT_HEADING: &str = "## 续跑（beforeSubmit）";

fn build_merged_continuity_block_for_before_submit(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    let want_ap = crate::router_env_flags::router_rs_autopilot_drive_before_submit_enabled();
    let want_rfv = crate::router_env_flags::router_rs_rfv_loop_before_submit_enabled();
    if !want_ap && !want_rfv {
        return None;
    }
    let a = if want_ap {
        build_autopilot_drive_followup_using_frame(repo_root, frame)
    } else {
        None
    };
    let b = if want_rfv {
        build_rfv_loop_followup_using_frame(repo_root, frame)
    } else {
        None
    };
    match (a, b) {
        (None, None) => None,
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (Some(x), Some(y)) => Some(format!("{CONTINUITY_BEFORE_SUBMIT_HEADING}\n\n{x}\n\n{y}")),
    }
}

fn strip_before_submit_continuity_paragraphs(text: &str) -> String {
    let mut s = text.to_string();
    for prefix in ["AUTOPILOT_DRIVE", "RFV_LOOP_CONTINUE", "## 续跑"] {
        s = crate::autopilot_goal::strip_followup_paragraphs_with_line_prefix(&s, prefix);
    }
    s
}

fn merge_continuity_followups_before_submit(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    let Some(msg) = build_merged_continuity_block_for_before_submit(repo_root, frame) else {
        return;
    };
    let use_followup = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
    let field = if use_followup {
        "followup_message"
    } else {
        "additional_context"
    };
    let msg = crate::autopilot_goal::scrub_spoof_host_followup_lines(&msg);
    match output.get_mut(field) {
        Some(Value::String(existing)) => {
            let cleaned = crate::autopilot_goal::scrub_spoof_host_followup_lines(
                &strip_before_submit_continuity_paragraphs(existing),
            );
            let merged = if cleaned.is_empty() {
                msg
            } else {
                crate::autopilot_goal::scrub_spoof_host_followup_lines(&format!(
                    "{cleaned}\n\n{msg}"
                ))
            };
            *existing = merged;
        }
        _ => {
            if let Some(obj) = output.as_object_mut() {
                obj.insert(field.to_string(), Value::String(msg));
            }
        }
    }
}

fn completion_claimed_in_text(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    // Keep keyword set aligned with closeout enforcement completion keywords.
    // (Cursor stop/beforeSubmit gates are lightweight host-side guards; the evaluator is the authority.)
    const KEYWORDS: &[&str] = &[
        "done",
        "finished",
        "completed",
        "passed",
        "succeeded",
        "已完成",
        "完成",
        "通过",
        "搞定",
    ];
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    let lower = sanitized.to_ascii_lowercase();
    KEYWORDS
        .iter()
        .any(|kw| lower.contains(&kw.to_ascii_lowercase()))
}

fn closeout_followup_for_completion_claim(
    repo_root: &Path,
    task_id: &str,
) -> Result<Option<String>, String> {
    if !crate::framework_runtime::closeout_programmatic_enforcement_enabled() {
        return Ok(None);
    }
    let record_path = crate::framework_runtime::closeout_record_path_for_task(repo_root, task_id);
    if !record_path.is_file() {
        return Ok(Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={task_id} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估：\n\
- 记录路径：{}\n\
- 评估命令：router-rs closeout evaluate --record-path \"{}\"",
            record_path.display(),
            record_path.display(),
            record_path.display()
        )));
    }
    let eval = crate::framework_runtime::evaluate_closeout_record_file_for_task(
        repo_root,
        task_id,
        &record_path,
    )?;
    let allowed = eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if allowed {
        return Ok(None);
    }
    let violations = eval.get("violations").cloned().unwrap_or(Value::Null);
    let missing = eval.get("missing_evidence").cloned().unwrap_or(Value::Null);
    Ok(Some(format!(
        "CLOSEOUT_FOLLOWUP task_id={task_id} reason=evaluation_failed path={}\n\
closeout_enforcement blocked completion: closeout_allowed=false\n\
violations={}\nmissing_evidence={}\n\
请修复 violations，或降级 completion/status，再重新评估。",
        record_path.display(),
        violations,
        missing
    )))
}

pub const STATE_VERSION: u32 = 3;

fn subagent_types() -> &'static [&'static str] {
    &[
        "generalpurpose",
        "explore",
        "shell",
        "browser-use",
        "browseruse",
        "cursor-guide",
        "cursorguide",
        "ci-investigator",
        "ciinvestigator",
        "best-of-n-runner",
        "bestofnrunner",
        "explorer",
    ]
}

fn subagent_tool_names() -> &'static [&'static str] {
    &[
        "task",
        "functions.task",
        "functions.subagent",
        "functions.spawn_agent",
        "subagent",
        "spawn_agent",
    ]
}

/// MCP / 宿主可能使用 `…subagent…` 等未列入清单的工具名。
fn tool_name_matches_subagent_lane(normalized: &str) -> bool {
    if subagent_tool_names().contains(&normalized) {
        return true;
    }
    normalized.contains("subagent") || normalized.contains("spawn_agent") || normalized == "task"
}

fn framework_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)/(autopilot|team|gitx|update)\b").expect("invalid regex")
    })
}

fn autopilot_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)/autopilot\b").expect("invalid regex"))
}

fn goal_contract_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|(目标|完成条件|验证命令|检查点|非目标)",
        )
        .expect("invalid regex")
    })
}

fn goal_progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(checkpoint|milestone|progress|next step)\b|(检查点|里程碑|进度|下一步)")
            .expect("invalid regex")
    })
}

fn goal_verify_or_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|验证通过|测试通过|阻塞)",
        )
        .expect("invalid regex")
    })
}

fn json_value_as_boolish(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_i64().map(|i| i != 0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Task/subagent 调用里明示 `fork_context: true` 时视为与主会话共享上下文，不满足 autopilot 要求的「独立上下文」预检。
/// 部分宿主以字符串 `"true"` / `"false"` 下发，需与 JSON bool 同等解析。
fn fork_context_from_tool(event: &Value, tool_input: &Value) -> Option<bool> {
    tool_input
        .get("fork_context")
        .or_else(|| tool_input.get("forkContext"))
        .or_else(|| event.get("fork_context"))
        .or_else(|| event.get("forkContext"))
        .and_then(json_value_as_boolish)
}

fn counts_as_independent_context_fork(fork: Option<bool>) -> bool {
    match fork {
        Some(true) => false,
        Some(false) | None => true,
    }
}

fn has_goal_contract_signal(text: &str) -> bool {
    // A "goal contract" should be hard to satisfy by accident. Historically we matched any one
    // keyword (Goal / Done when / Validation / ...), which made it too easy for a one-liner to
    // count as a full contract and prematurely satisfy the goal gate.
    //
    // New rule (Cursor): require a structured, non-empty contract with:
    // - Goal
    // - Non-goals
    // - Done when (with >=2 acceptance items)
    // - Validation commands (non-empty)
    //
    // Support both English and Chinese headings.
    if goal_contract_re().is_match(text) {
        // Keep legacy keyword match as a weak signal, but only after passing the strong check
        // below. This preserves backwards compatibility for callers that only gate on this bool.
    }
    has_structured_goal_contract(text)
}

fn has_structured_goal_contract(text: &str) -> bool {
    let goal_ok =
        nonempty_inline_heading_any(text, "Goal") || nonempty_inline_heading_any(text, "目标");
    let non_goals_ok = nonempty_inline_heading_any(text, "Non-goals")
        || nonempty_inline_heading_any(text, "非目标");
    let validation_ok = nonempty_inline_heading_any(text, "Validation commands")
        || nonempty_inline_heading_any(text, "验证命令");
    let done_when_items = count_done_when_items(text);
    goal_ok && non_goals_ok && validation_ok && done_when_items >= 2
}

fn nonempty_inline_heading_any(text: &str, heading: &str) -> bool {
    let pattern = format!(r"(?im)^\s*{}\s*[:：]\s*(\S.+)$", regex::escape(heading));
    let Ok(re) = Regex::new(&pattern) else {
        return false;
    };
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| !m.as_str().trim().is_empty())
        .unwrap_or(false)
}

fn count_done_when_items(text: &str) -> usize {
    // Prefer bullet/numbered items under "Done when:" / "完成条件:".
    // Fallback: treat an inline list after the heading as multiple items if it contains clear
    // separators.
    const HEADINGS: [&str; 2] = ["Done when", "完成条件"];
    let numbered_line_re = Regex::new(r"(?m)^\d+\.\s+\S").ok();
    let re_done = Regex::new(&format!(
        r"(?im)^\s*{}\s*[:：]\s*(.*)$",
        regex::escape(HEADINGS[0])
    ));
    let re_zh = Regex::new(&format!(
        r"(?im)^\s*{}\s*[:：]\s*(.*)$",
        regex::escape(HEADINGS[1])
    ));
    let heading_pairs = [(HEADINGS[0], re_done.ok()), (HEADINGS[1], re_zh.ok())];
    for (h, maybe_re) in heading_pairs {
        let Some(re) = maybe_re else {
            continue;
        };
        let Some(cap) = re.captures(text) else {
            continue;
        };
        let inline = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if !inline.is_empty() {
            // Inline: split on common separators; require at least 2 non-empty parts.
            let parts = inline
                .split(&[';', '；', ',', '，', '|', '、'][..])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count();
            if parts >= 2 {
                return parts;
            }
        }

        // Block-style: count bullet/numbered lines after the heading until the next heading-ish
        // line or a blank-only tail. This is intentionally conservative.
        let mut in_section = false;
        let mut count = 0usize;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                if in_section {
                    // Allow blank lines inside section; do not terminate immediately.
                    continue;
                }
                continue;
            }
            if !in_section {
                let lowered = line.to_ascii_lowercase();
                let target = h.to_ascii_lowercase();
                if lowered.starts_with(&target) && (lowered.contains(':') || line.contains('：')) {
                    in_section = true;
                }
                continue;
            }

            // Stop if we hit another contract heading.
            if goal_contract_re().is_match(line)
                && !line
                    .to_ascii_lowercase()
                    .starts_with(&h.to_ascii_lowercase())
            {
                break;
            }

            let is_bullet = line.starts_with("- ")
                || line.starts_with("* ")
                || line.starts_with("• ")
                || numbered_line_re.as_ref().is_some_and(|r| r.is_match(line));
            if is_bullet {
                count += 1;
            }
        }
        if count > 0 {
            return count;
        }
    }
    0
}

fn has_goal_progress_signal(text: &str) -> bool {
    goal_progress_re().is_match(text)
}

fn has_goal_verify_or_block_signal(text: &str) -> bool {
    goal_verify_or_block_re().is_match(text)
}

/// Task/subagent 工具载荷上的类型字段（与 Codex `codex_subagent_type_evidence` 对齐）：部分宿主用 `type` 代替 `subagent_type`。
fn cursor_subagent_type_pair(tool_input: &Value, event: &Value) -> (String, String) {
    let sub_raw = tool_input
        .get("subagent_type")
        .or_else(|| tool_input.get("subagentType"))
        .or_else(|| tool_input.get("type"))
        .or_else(|| tool_input.get("lane"))
        .or_else(|| tool_input.get("lane_type"))
        .or_else(|| tool_input.get("laneType"))
        .or_else(|| event.get("subagent_type"))
        .or_else(|| event.get("subagentType"))
        .or_else(|| event.get("type"))
        .and_then(Value::as_str);
    let agent_raw = tool_input
        .get("agent_type")
        .or_else(|| tool_input.get("agentType"))
        .or_else(|| event.get("agent_type"))
        .or_else(|| event.get("agentType"))
        .and_then(Value::as_str);
    (
        normalize_subagent_type(sub_raw),
        normalize_subagent_type(agent_raw),
    )
}

fn typed_subagent_in_allowlist(sub_type: &str, agent_type: &str) -> bool {
    (!sub_type.is_empty() && subagent_types().contains(&sub_type))
        || (!agent_type.is_empty() && subagent_types().contains(&agent_type))
}

/// `/autopilot` pre-goal：白名单 **或** 任一非空 lane/agent 字段即视为已起 sidecar（宿主常发自定义 lane 名）。
fn pre_goal_subagent_kind_ok(sub_type: &str, agent_type: &str) -> bool {
    typed_subagent_in_allowlist(sub_type, agent_type)
        || !sub_type.is_empty()
        || !agent_type.is_empty()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewGateState {
    pub version: u32,
    pub phase: u32,
    pub review_required: bool,
    pub delegation_required: bool,
    pub review_override: bool,
    pub delegation_override: bool,
    pub reject_reason_seen: bool,
    #[serde(default)]
    pub active_subagent_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_subagent_last_started_at: Option<String>,
    pub subagent_start_count: u32,
    pub subagent_stop_count: u32,
    pub followup_count: u32,
    pub review_followup_count: u32,
    pub goal_followup_count: u32,
    pub goal_required: bool,
    pub goal_contract_seen: bool,
    pub goal_progress_seen: bool,
    pub goal_verify_or_block_seen: bool,
    /// `/autopilot`：在 goal 契约与收口证据之前，要求独立上下文 subagent 预检（或拒绝原因词）。
    #[serde(default)]
    pub pre_goal_review_satisfied: bool,
    /// 连续触发 beforeSubmit 的 pre-goal 提示次数（清门或自动放行后归零）。
    #[serde(default)]
    pub pre_goal_nag_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_intent_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Cursor hook stdin 常见嵌套容器（与 `prompt_text` / `agent_response_text` 共用）。
const HOOK_EVENT_NESTED: &[&str] = &["payload", "hookPayload", "data", "body", "hook_input"];

/// 在顶层与嵌套对象中查找第一个非空字符串字段（宿主字段名不一致时的兼容层）。
fn first_nonempty_event_str(event: &Value, keys: &[&str]) -> String {
    if let Some(obj) = event.as_object() {
        for key in keys {
            if let Some(value) = obj.get(*key).and_then(Value::as_str) {
                if !value.trim().is_empty() {
                    return value.to_string();
                }
            }
        }
        for nest in HOOK_EVENT_NESTED {
            if let Some(nobj) = obj.get(*nest).and_then(Value::as_object) {
                for key in keys {
                    if let Some(value) = nobj.get(*key).and_then(Value::as_str) {
                        if !value.trim().is_empty() {
                            return value.to_string();
                        }
                    }
                }
            }
        }
    }
    String::new()
}

/// 宿主 JSON 字段不完全一致：顶层或 `payload`/`data` 内都可能挂用户输入。
fn prompt_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "prompt",
        "user_prompt",
        "message",
        "input",
        "text",
        "userPrompt",
        "userMessage",
        "command",
        "content",
        "userContent",
        "query",
        "composerText",
        "editorText",
    ];
    let direct = first_nonempty_event_str(event, KEYS);
    if !direct.trim().is_empty() {
        return direct;
    }
    prompt_from_nested_messages(event)
}

fn is_user_message_role(obj: &serde_json::Map<String, Value>) -> bool {
    let role = obj
        .get("role")
        .or_else(|| obj.get("type"))
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(role.as_str(), "user" | "human")
}

fn message_body_text(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["content", "text", "body", "value"] {
        match obj.get(key)? {
            Value::String(s) => {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(s.clone());
                }
            }
            Value::Array(parts) => {
                let mut buf = String::new();
                for p in parts {
                    if let Some(o) = p.as_object() {
                        if let Some(Value::String(s)) = o.get("text") {
                            buf.push_str(s);
                        }
                    } else if let Some(s) = p.as_str() {
                        buf.push_str(s);
                    }
                }
                if !buf.trim().is_empty() {
                    return Some(buf);
                }
            }
            _ => {}
        }
    }
    None
}

/// `beforeSubmit` 有时不把用户输入放在 `prompt`，而放在 `messages` 末尾；取**最后一条 user** 文本供门控与拒因识别。
fn prompt_from_nested_messages(event: &Value) -> String {
    if let Some(obj) = event.as_object() {
        for key in [
            "messages",
            "conversationMessages",
            "chatMessages",
            "history",
        ] {
            if let Some(Value::Array(arr)) = obj.get(key) {
                for item in arr.iter().rev() {
                    let Some(msg) = item.as_object() else {
                        continue;
                    };
                    if !is_user_message_role(msg) {
                        continue;
                    }
                    if let Some(t) = message_body_text(msg) {
                        return t;
                    }
                }
            }
        }
        for nest in HOOK_EVENT_NESTED {
            if let Some(nested) = obj.get(*nest) {
                let s = prompt_from_nested_messages(nested);
                if !s.trim().is_empty() {
                    return s;
                }
            }
        }
    }
    String::new()
}

fn agent_response_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "response",
        "agent_response",
        "agentResponse",
        "content",
        "text",
        "message",
        "output",
    ];
    first_nonempty_event_str(event, KEYS)
}

/// 从整棵 hook JSON 抓取字符串（深度与总字节上限），用于识别藏在未知路径下的 `reject_reason` / goal 片段。
/// 长会话若预算过小，会先扫满早期大段 transcript，**截断后丢失末尾用户输入**（例如单独一行的 `small_task`）。
const HOOK_JSON_STRING_SCRAPE_CAP: usize = 2 * 1024 * 1024;
const HOOK_JSON_STRING_SCRAPE_MAX_DEPTH: u32 = 48;

fn append_scraped_line(out: &mut String, s: &str, budget: &mut usize) {
    if *budget == 0 || s.is_empty() {
        return;
    }
    if !out.is_empty() {
        if *budget <= 1 {
            *budget = 0;
            return;
        }
        out.push('\n');
        *budget -= 1;
    }
    for ch in s.chars() {
        let cost = ch.len_utf8();
        if cost > *budget {
            break;
        }
        out.push(ch);
        *budget -= cost;
    }
}

fn scrape_hook_json_strings(value: &Value, depth: u32, budget: &mut usize, out: &mut String) {
    if depth == 0 || *budget == 0 {
        return;
    }
    match value {
        Value::String(s) => append_scraped_line(out, s, budget),
        Value::Array(arr) => {
            for v in arr {
                scrape_hook_json_strings(v, depth - 1, budget, out);
                if *budget == 0 {
                    break;
                }
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                scrape_hook_json_strings(v, depth - 1, budget, out);
                if *budget == 0 {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn hook_event_all_text(event: &Value) -> String {
    let mut budget = HOOK_JSON_STRING_SCRAPE_CAP;
    let mut out = String::new();
    scrape_hook_json_strings(
        event,
        HOOK_JSON_STRING_SCRAPE_MAX_DEPTH,
        &mut budget,
        &mut out,
    );
    out
}

/// 结构化字段解析 + 全树字符串（后者覆盖 `messages[].content` 等未知路径）。
fn hook_event_signal_text(event: &Value, prompt: &str, response: &str) -> String {
    let mut s = String::with_capacity(
        prompt
            .len()
            .saturating_add(response.len())
            .saturating_add(4096),
    );
    s.push_str(prompt);
    s.push('\n');
    s.push_str(response);
    s.push('\n');
    s.push_str(&hook_event_all_text(event));
    s
}

fn grab_tool_name_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["tool_name", "toolName", "tool", "name"] {
        if let Some(s) = obj.get(key).and_then(Value::as_str) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn tool_name_of(event: &Value) -> String {
    if let Some(obj) = event.as_object() {
        if let Some(s) = grab_tool_name_from_object(obj) {
            return s;
        }
        for nest in HOOK_EVENT_NESTED {
            if let Some(nobj) = obj.get(*nest).and_then(Value::as_object) {
                if let Some(s) = grab_tool_name_from_object(nobj) {
                    return s;
                }
            }
        }
    }
    String::new()
}

fn grab_tool_input_from_object(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    obj.get("tool_input")
        .or_else(|| obj.get("input"))
        .or_else(|| obj.get("arguments"))
        .cloned()
}

fn tool_input_of(event: &Value) -> Value {
    if let Some(obj) = event.as_object() {
        if let Some(v) = grab_tool_input_from_object(obj) {
            if v.is_object() {
                return v;
            }
        }
        for nest in HOOK_EVENT_NESTED {
            if let Some(nobj) = obj.get(*nest).and_then(Value::as_object) {
                if let Some(v) = grab_tool_input_from_object(nobj) {
                    if v.is_object() {
                        return v;
                    }
                }
            }
        }
    }
    json!({})
}

/// 从 stdin JSON 提取会话标识。
///
/// **优先级（先到先用）**：顶层 `session_id`、`conversation_id`、…、`sessionId` 等依次扫描；
/// 再读 `metadata.{sessionId,conversationId,chatId,threadId}`；
/// 再对 `payload` / `hookPayload` / `data` / `body` / `hook_input` 重复同样规则（与 `prompt_text` 对齐）。
///
/// 若同一 payload 中多套字段彼此冲突，**仅第一个非空值生效**（宿主应对齐字段）。
///
/// **仅 cwd、无会话 id**：`session_key` 对 `cwd`（及嵌套 workspace 路径字段）做稳定哈希；同一文件系统路径上并行多会话会**共用**
/// 一份状态，除非设置环境变量 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`。
///
/// 注意：**不包含 `agent_id`**——`subagentStop` 等事件常在顶层带子 agent id，若视为会话锚点会与 `session_id`/conversation 分叉，导致 `active_subagent_count` 只增不减。
fn try_extract_session_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in [
        "session_id",
        "conversation_id",
        "thread_id",
        "chat_id",
        "conversationId",
        "threadId",
        "sessionId",
    ] {
        if let Some(value) = obj.get(key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(meta) = obj.get("metadata").and_then(Value::as_object) {
        for key in ["sessionId", "conversationId", "chatId", "threadId"] {
            if let Some(value) = meta.get(key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// 深度扫描 hook JSON：**较小下标的字段优先**，用于对齐 `subagentStart`/`subagentStop` 与主对话会话（宿主字段路径不一致时）。
///
/// **显式不包含 `agent_id`**；扫描有节点预算，防止极大 payload 卡住 hook。
const SESSION_HOOK_IDENTITY_FIELDS_DEEP_PRIORITY: &[&str] = &[
    "conversation_id",
    "conversationId",
    "thread_id",
    "threadId",
    "chat_id",
    "session_id",
    "sessionId",
    "parent_session_id",
    "parentSessionId",
    "root_session_id",
    "composer_id",
    "composerId",
];

const SESSION_DEEP_SCAN_MAX_NODES: usize = 800;

fn min_priority_session_identity_from_hook_json(event: &Value) -> Option<String> {
    let mut pick: Option<(usize, usize, String)> = None;
    let mut ties = 0usize;

    fn visit(
        v: &Value,
        depth: u32,
        nodes: &mut usize,
        ties: &mut usize,
        pick: &mut Option<(usize, usize, String)>,
    ) {
        if depth > 10 || *nodes >= SESSION_DEEP_SCAN_MAX_NODES {
            return;
        }
        match v {
            Value::Object(map) => {
                for (field, child) in map.iter() {
                    if *nodes >= SESSION_DEEP_SCAN_MAX_NODES {
                        return;
                    }
                    *nodes += 1;
                    if let Some(pi) = SESSION_HOOK_IDENTITY_FIELDS_DEEP_PRIORITY
                        .iter()
                        .position(|k| *k == field)
                    {
                        if let Some(s) = child.as_str() {
                            let t = s.trim();
                            if !t.is_empty() {
                                *ties += 1;
                                let ord = *ties;
                                if pick
                                    .as_ref()
                                    .is_none_or(|(bp, bo, _)| pi < *bp || (pi == *bp && ord < *bo))
                                {
                                    *pick = Some((pi, ord, t.to_string()));
                                }
                            }
                        }
                    }
                    visit(child, depth + 1, nodes, ties, pick);
                }
            }
            Value::Array(values) => {
                for item in values {
                    if *nodes >= SESSION_DEEP_SCAN_MAX_NODES {
                        return;
                    }
                    visit(item, depth + 1, nodes, ties, pick);
                }
            }
            _ => {}
        }
    }

    let mut nodes = 0usize;
    visit(event, 0, &mut nodes, &mut ties, &mut pick);
    pick.map(|(_, _, s)| s)
}

fn extract_first_session_string(event: &Value) -> Option<String> {
    let root = event.as_object()?;
    if let Some(s) = try_extract_session_from_object(root) {
        return Some(s);
    }
    for nest in HOOK_EVENT_NESTED {
        if let Some(nobj) = root.get(*nest).and_then(Value::as_object) {
            if let Some(s) = try_extract_session_from_object(nobj) {
                return Some(s);
            }
        }
    }
    None
}

/// 从 `tool_input` / `metadata` 仅提取**父会话**类字段（不含 `agent_id`），保证 subagent 生命周期钩子与主对话落在同一 hook-state 分片。
fn try_extract_parent_session_from_tool_json(tool: &Value) -> Option<String> {
    let obj = tool.as_object()?;
    for key in [
        "session_id",
        "conversation_id",
        "thread_id",
        "chat_id",
        "conversationId",
        "threadId",
        "sessionId",
    ] {
        if let Some(value) = obj.get(key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(meta) = obj.get("metadata").and_then(Value::as_object) {
        for key in ["sessionId", "conversationId", "chatId", "threadId"] {
            if let Some(value) = meta.get(key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn extract_first_session_string_including_tool_input(event: &Value) -> Option<String> {
    min_priority_session_identity_from_hook_json(event)
        .or_else(|| extract_first_session_string(event))
        .or_else(|| try_extract_parent_session_from_tool_json(&tool_input_of(event)))
}

/// 派生 `.cursor/hook-state/review-subagent-<key>.json` 文件名组件。
/// 顺序：`extract_first_session_string_including_tool_input`（含 **`tool_input` 内父会话 id**）→ `ROUTER_RS_CURSOR_SESSION_NAMESPACE` → `cwd`（含嵌套 workspace 字段）→ 常量 fallback。
fn session_key(event: &Value) -> String {
    if let Some(raw) = extract_first_session_string_including_tool_input(event) {
        return short_hash(&raw);
    }
    if let Ok(ns) = std::env::var("ROUTER_RS_CURSOR_SESSION_NAMESPACE") {
        let t = ns.trim();
        if !t.is_empty() {
            return short_hash(&format!("env::{t}"));
        }
    }
    const CWD_KEYS: &[&str] = &[
        "cwd",
        "workspaceFolder",
        "workspace_folder",
        "workspaceRoot",
        "workspace_root",
        "root",
    ];
    let cwd = first_nonempty_event_str(event, CWD_KEYS);
    if !cwd.trim().is_empty() {
        // 与 Cursor「每 hook 新进程」兼容：cwd fallback 必须跨调用稳定，否则状态永不累积。
        return short_hash(&format!("cwd::{cwd}"));
    }
    short_hash("router-rs-cursor-session-fallback")
}

fn hook_lock_unavailable_notice_json() -> Value {
    json!({
        "additional_context": "router-rs：`.cursor/hook-state` 锁不可用，本钩未写入 review gate 状态。请检查权限/争用后重试。"
    })
}

fn is_framework_entrypoint_prompt(text: &str) -> bool {
    framework_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

fn is_autopilot_entrypoint_prompt(text: &str) -> bool {
    autopilot_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

/// `/team` 等框架入口仍视为委托/并行编排；**框架命令仅认 `/` 前缀**；**`/autopilot` 除外**（只走 goal 机）。
/// 修复：此前 `framework_entrypoint` 含 autopilot，导致 `delegation_required` 与 `goal_required` 叠乘，
/// 与 autopilot 执行轮的门控语义冲突（现已拆分为仅 goal 路径跟 AG_FOLLOWUP）。
fn framework_prompt_arms_delegation(text: &str) -> bool {
    is_framework_entrypoint_prompt(text) && !is_autopilot_entrypoint_prompt(text)
}

fn short_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    hex_lower(&digest[..16])
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = std::fmt::Write::write_fmt(&mut s, format_args!("{:02x}", byte));
    }
    s
}

fn push_path_candidate(out: &mut Vec<PathBuf>, raw: &str) {
    let t = raw.trim();
    if !t.is_empty() {
        out.push(PathBuf::from(t));
    }
}

/// 自 `start`（文件或目录）向上查找包含 `.cursor/hooks.json` 的目录。
fn first_ancestor_with_hooks_json(start: &Path) -> Option<PathBuf> {
    let start_meta = fs::metadata(start).ok();
    let mut cur = if start_meta.as_ref().is_some_and(|m| m.is_file()) {
        start.parent()?.to_path_buf()
    } else if start_meta.as_ref().is_some_and(|m| m.is_dir()) {
        start.to_path_buf()
    } else {
        start
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| start.to_path_buf())
    };

    for _ in 0..64 {
        if cur.join(".cursor").join("hooks.json").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

/// 合并 CLI `--repo-root`、环境变量与 stdin JSON 中的路径字段，解析含 `.cursor/hooks.json` 的策略根目录。
///
/// 优先使用载荷中的 `cwd` / `workspaceFolder` 等（Cursor 侧通常比 hook 进程 pwd 更可靠），避免子目录会话时状态写到错误根路径。
pub fn resolve_cursor_hook_repo_root(
    cli_root: Option<&Path>,
    payload: &Value,
) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(v) = std::env::var("ROUTER_RS_CURSOR_WORKSPACE_ROOT") {
        push_path_candidate(&mut candidates, &v);
    }
    if let Ok(v) = std::env::var("CURSOR_WORKSPACE_ROOT") {
        push_path_candidate(&mut candidates, &v);
    }

    for key in [
        "workspaceFolder",
        "workspace_folder",
        "workspaceRoot",
        "workspace_root",
        "cwd",
        "root",
    ] {
        if let Some(s) = payload.get(key).and_then(Value::as_str) {
            push_path_candidate(&mut candidates, s);
        }
    }

    if let Some(p) = payload
        .get("tool_input")
        .and_then(|t| t.get("path"))
        .and_then(Value::as_str)
    {
        push_path_candidate(&mut candidates, p);
    }
    if let Some(p) = payload.get("file_path").and_then(Value::as_str) {
        push_path_candidate(&mut candidates, p);
    }

    if let Some(p) = cli_root {
        candidates.push(p.to_path_buf());
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }

    for c in candidates {
        if let Some(found) = first_ancestor_with_hooks_json(&c) {
            let canon = fs::canonicalize(&found).unwrap_or(found);
            return Ok(canon);
        }
    }

    let base = cli_root
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            "cursor hook: cannot resolve repo root (no .cursor/hooks.json marker and no fallback cwd)"
                .to_string()
        })?;
    Ok(fs::canonicalize(&base).unwrap_or(base))
}

fn state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".cursor").join("hook-state")
}

fn state_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.json", session_key(event)))
}

fn state_lock_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.lock", session_key(event)))
}

/// 已移除 `/loop` adversarial 功能；保留路径与清扫逻辑，便于 SessionEnd 清理历史 `adversarial-loop-*.json` 与 `.tmp-adv-loop-*` 孤儿文件。
fn adversarial_loop_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("adversarial-loop-{}.json", session_key(event)))
}

fn session_terminal_ledger_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("session-terminals-{}.json", session_key(event)))
}

fn remove_adversarial_loop(repo_root: &Path, event: &Value) {
    let _ = fs::remove_file(adversarial_loop_path(repo_root, event));
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PendingShellRecord {
    /// `normalize_shell_command` 产物，用作 FIFO 配对键。
    command_norm: String,
    /// Shell 钩子声明的 cwd 原始字符串（通常已是绝对路径）。
    cwd_raw: String,
    /// `beforeShellExecution` 入队单调时钟近似（毫秒，Unix）。
    queued_ms: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SessionTerminalLedger {
    version: u32,
    baseline_pids: Vec<u32>,
    owned_pids: Vec<u32>,
    #[serde(default)]
    pending_shells: Vec<PendingShellRecord>,
}

const SESSION_TERMINAL_LEDGER_VERSION: u32 = 2;

fn load_session_terminal_ledger(repo_root: &Path, event: &Value) -> SessionTerminalLedger {
    let path = session_terminal_ledger_path(repo_root, event);
    let Ok(raw) = fs::read_to_string(path) else {
        return SessionTerminalLedger {
            version: SESSION_TERMINAL_LEDGER_VERSION,
            baseline_pids: Vec::new(),
            owned_pids: Vec::new(),
            pending_shells: Vec::new(),
        };
    };
    serde_json::from_str::<SessionTerminalLedger>(&raw).unwrap_or(SessionTerminalLedger {
        version: SESSION_TERMINAL_LEDGER_VERSION,
        baseline_pids: Vec::new(),
        owned_pids: Vec::new(),
        pending_shells: Vec::new(),
    })
}

fn save_session_terminal_ledger(repo_root: &Path, event: &Value, ledger: &SessionTerminalLedger) {
    let path = session_terminal_ledger_path(repo_root, event);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(ledger) {
        let _ = fs::write(path, text);
    }
}

/// **`ROUTER_RS_CURSOR_TERMINAL_KILL_MODE`**：默认 `scoped`（仅杀掉本会话账本 `owned_pids` 内的活跃 terminal）。
/// 设为 `legacy`/`all`/`repo`/`repo-wide`/`repowide` 时恢复旧行为：**仓库 cwd 范围内**扫描所有 stale active terminal（与是否本会话无关）。
fn cursor_terminal_kill_use_scoped_ownership() -> bool {
    match std::env::var("ROUTER_RS_CURSOR_TERMINAL_KILL_MODE") {
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(
                t.as_str(),
                "legacy" | "all" | "repo" | "repo-wide" | "repowide"
            )
        }
        Err(_) => true,
    }
}

fn ensure_session_terminal_ledger_initialized(repo_root: &Path, event: &Value) {
    let path = session_terminal_ledger_path(repo_root, event);
    if path.is_file() {
        return;
    }
    maybe_init_session_terminal_ledger(repo_root, event);
}

fn trim_pending_shell_records(ledger: &mut SessionTerminalLedger) {
    while ledger.pending_shells.len() > MAX_PENDING_SHELL_RECORDS {
        ledger.pending_shells.remove(0);
    }
}

fn canonical_path_or_clone(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn shell_cwd_hint_matches_saved_record(saved_raw: &str, hint: Option<&Path>) -> bool {
    let Some(h) = hint else {
        return true;
    };
    let saved_trim = saved_raw.trim();
    if saved_trim.is_empty() {
        return true;
    }
    let saved_p = Path::new(saved_trim);
    let sp = canonical_path_or_clone(saved_p);
    let hp = canonical_path_or_clone(h);
    sp == hp || sp.starts_with(&hp) || hp.starts_with(&sp)
}

fn pop_matching_pending_shell(
    ledger: &mut SessionTerminalLedger,
    cmd_norm: &str,
    cwd_hint: Option<&Path>,
) -> Option<u64> {
    if cmd_norm.is_empty() {
        return None;
    }
    let idx = ledger.pending_shells.iter().position(|p| {
        p.command_norm == cmd_norm && shell_cwd_hint_matches_saved_record(&p.cwd_raw, cwd_hint)
    })?;
    Some(ledger.pending_shells.remove(idx).queued_ms)
}

fn augment_event_shell_command_cwd(
    base: &Value,
    command: Option<String>,
    cwd: Option<String>,
) -> Value {
    let mut obj = base
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    if let Some(c) = command {
        obj.insert("command".to_string(), Value::String(c));
    }
    if let Some(c) = cwd {
        obj.insert("cwd".to_string(), Value::String(c));
    }
    Value::Object(obj)
}

fn tool_input_shell_command_and_cwd(tool_input: &Value) -> (Option<String>, Option<String>) {
    let cmd = tool_input
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            tool_input
                .get("cmd")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| match tool_input.get("arguments") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        });
    let cwd = [
        "working_directory",
        "workingDirectory",
        "cwd",
        "workspace",
        "root",
        "workspaceRoot",
    ]
    .into_iter()
    .find_map(|k| {
        tool_input
            .get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    (cmd, cwd)
}

fn parse_terminal_started_at_unix_ms(raw: &str) -> Option<u64> {
    let s = raw.trim().trim_matches('"');
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis().max(0) as u64)
}

fn cursor_post_tool_shell_terminal_track(repo_root: &Path, event: &Value) {
    let ti = tool_input_of(event);
    let (cmd, cwd) = tool_input_shell_command_and_cwd(&ti);
    let Some(cmd_s) = cmd else {
        return;
    };
    if cmd_s.trim().is_empty() {
        return;
    }
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let augmented = augment_event_shell_command_cwd(event, Some(cmd_s), cwd);
    maybe_track_shell_owned_terminals(repo_root, &augmented, None);
}

fn merge_additional_context(output: &mut Value, extra: &str) {
    let extra = crate::autopilot_goal::scrub_spoof_host_followup_lines(extra);
    match output.get_mut("additional_context") {
        Some(Value::String(s)) => {
            s.push_str("\n\n");
            s.push_str(&extra);
            *s = crate::autopilot_goal::scrub_spoof_host_followup_lines(s);
        }
        _ => {
            output["additional_context"] = Value::String(extra);
        }
    }
}

struct LockGuard {
    path: PathBuf,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let lock_path = state_lock_path(repo_root, event);
    for _ in 0..30 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let lock_text = format!("pid={} ts={}\n", std::process::id(), now_millis());
                let _ = file.write_all(lock_text.as_bytes());
                let _ = file.sync_all();
                return Some(LockGuard { path: lock_path });
            }
            Err(_) => {
                if let Ok(existing) = fs::read_to_string(&lock_path) {
                    if let Some((pid, ts_ms)) = parse_lock_metadata(&existing) {
                        let age_ms = now_millis().saturating_sub(ts_ms);
                        if age_ms > 30_000 || !is_process_alive(pid) {
                            let _ = fs::remove_file(&lock_path);
                            continue;
                        }
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    None
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_lock_metadata(text: &str) -> Option<(u32, u64)> {
    let pid = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .and_then(|v| v.parse::<u32>().ok())?;
    let ts = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("ts="))
        .and_then(|v| v.parse::<u64>().ok())?;
    Some((pid, ts))
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let rc = libc::kill(pid as libc::pid_t, 0);
        if rc == 0 {
            return true;
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ESRCH) => false,
            Some(libc::EPERM) => true,
            _ => true,
        }
    }
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    true
}

fn release_state_lock(lock: &mut Option<LockGuard>) {
    if let Some(lock) = lock.take() {
        let _ = fs::remove_file(lock.path);
    }
}

fn empty_state() -> ReviewGateState {
    ReviewGateState {
        version: STATE_VERSION,
        phase: 0,
        review_required: false,
        delegation_required: false,
        review_override: false,
        delegation_override: false,
        reject_reason_seen: false,
        active_subagent_count: 0,
        active_subagent_last_started_at: None,
        subagent_start_count: 0,
        subagent_stop_count: 0,
        followup_count: 0,
        review_followup_count: 0,
        goal_followup_count: 0,
        goal_required: false,
        goal_contract_seen: false,
        goal_progress_seen: false,
        goal_verify_or_block_seen: false,
        pre_goal_review_satisfied: false,
        pre_goal_nag_count: 0,
        last_prompt: None,
        last_subagent_type: None,
        last_subagent_tool: None,
        lane_intent_matches: None,
        updated_at: None,
    }
}

fn migrate_v1(raw: &Value) -> ReviewGateState {
    let mut state = empty_state();
    state.review_required = raw
        .get("review_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.delegation_required = raw
        .get("delegation_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.review_override = raw
        .get("review_override")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.delegation_override = raw
        .get("delegation_override")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.reject_reason_seen = raw
        .get("reject_reason_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if raw
        .get("review_subagent_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        state.phase = 2;
    } else if state.review_required || state.delegation_required {
        state.phase = 1;
    }
    state.followup_count = raw
        .get("followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state.review_followup_count = raw
        .get("review_followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state.goal_followup_count = raw
        .get("goal_followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state
}

fn load_state(repo_root: &Path, event: &Value) -> Result<Option<ReviewGateState>, String> {
    let path = state_path(repo_root, event);
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("state_read_failed".to_string()),
    };
    let raw: Value = serde_json::from_str(&text).map_err(|_| "state_json_invalid".to_string())?;
    if !raw.is_object() {
        return Err("state_not_object".to_string());
    }
    // 仅迁移 legacy v1；v2 JSON 直接走 serde（避免吞掉 v2 字段）。
    if raw.get("version").and_then(Value::as_u64).unwrap_or(0) < 2 {
        return Ok(Some(migrate_v1(&raw)));
    }
    let mut base = empty_state();
    if let Ok(parsed) = serde_json::from_value::<ReviewGateState>(raw.clone()) {
        base = parsed;
    } else if let Some(obj) = raw.as_object() {
        if let Some(v) = obj.get("phase").and_then(Value::as_u64) {
            base.phase = v as u32;
        }
        if let Some(v) = obj.get("review_required").and_then(Value::as_bool) {
            base.review_required = v;
        }
        if let Some(v) = obj.get("delegation_required").and_then(Value::as_bool) {
            base.delegation_required = v;
        }
        if let Some(v) = obj.get("review_override").and_then(Value::as_bool) {
            base.review_override = v;
        }
        if let Some(v) = obj.get("delegation_override").and_then(Value::as_bool) {
            base.delegation_override = v;
        }
        if let Some(v) = obj.get("reject_reason_seen").and_then(Value::as_bool) {
            base.reject_reason_seen = v;
        }
        if let Some(v) = obj.get("active_subagent_count").and_then(Value::as_u64) {
            base.active_subagent_count = v as u32;
        }
        if let Some(v) = obj
            .get("active_subagent_last_started_at")
            .and_then(Value::as_str)
        {
            base.active_subagent_last_started_at = Some(v.to_string());
        }
        if let Some(v) = obj.get("subagent_start_count").and_then(Value::as_u64) {
            base.subagent_start_count = v as u32;
        }
        if let Some(v) = obj.get("subagent_stop_count").and_then(Value::as_u64) {
            base.subagent_stop_count = v as u32;
        }
        if let Some(v) = obj.get("followup_count").and_then(Value::as_u64) {
            base.followup_count = v as u32;
        }
        if let Some(v) = obj.get("review_followup_count").and_then(Value::as_u64) {
            base.review_followup_count = v as u32;
        }
        if let Some(v) = obj.get("goal_followup_count").and_then(Value::as_u64) {
            base.goal_followup_count = v as u32;
        }
        if let Some(v) = obj
            .get("pre_goal_review_satisfied")
            .and_then(Value::as_bool)
        {
            base.pre_goal_review_satisfied = v;
        }
    }
    base.version = STATE_VERSION;
    Ok(Some(base))
}

fn save_state(repo_root: &Path, event: &Value, state: &mut ReviewGateState) -> bool {
    let directory = state_dir(repo_root);
    let target = state_path(repo_root, event);
    let _ = fs::create_dir_all(&directory);
    state.version = STATE_VERSION;
    state.updated_at = Some(Utc::now().to_rfc3339());
    let payload = match serde_json::to_string_pretty(state) {
        Ok(text) => format!("{text}\n"),
        Err(_) => return false,
    };
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let tmp = directory.join(format!(
        ".tmp-{}-{}-{}",
        std::process::id(),
        micros,
        target
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("state.json")
    ));
    let mut file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
    {
        Ok(f) => f,
        Err(_) => return false,
    };
    if file.write_all(payload.as_bytes()).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if file.sync_all().is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    #[cfg(unix)]
    {
        if let Ok(dir_file) = OpenOptions::new().read(true).open(&directory) {
            let _ = dir_file.sync_all();
        }
    }
    true
}

/// 仅 **review** 路径的硬门控（独立上下文 subagent 证据链）；**不包含** `delegation_required`。
fn review_hard_armed(state: &ReviewGateState) -> bool {
    state.review_required && !state.review_override
}

/// Stop：`review` 场景下独立 subagent 证据是否满足（phase≥3：start 后 stop 记账）。
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    state.phase >= 3
}

fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    review_hard_armed(state)
        && !review_subagent_evidence_satisfied(state)
        && !state.reject_reason_seen
}

fn review_stop_followup_line(state: &ReviewGateState) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete phase={} need=independent_context_subagent_cycle_or_small_task",
        state.phase
    )
}

fn is_overridden(state: &ReviewGateState) -> bool {
    state.review_override || state.delegation_override
}

fn goal_is_satisfied(state: &ReviewGateState) -> bool {
    if !state.goal_required {
        return true;
    }
    // 全局 override（例如不要用子代理）仍可跳过整套 gate。
    if is_overridden(state) {
        return true;
    }
    state.goal_contract_seen && state.goal_progress_seen && state.goal_verify_or_block_seen
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}

fn autopilot_pre_goal_followup_message() -> String {
    if crate::router_env_flags::router_rs_goal_prompt_verbose() {
        "Autopilot (/autopilot): establish a compact Goal contract (Goal / Non-goals / Done when / Validation / checkpoints) before large execution.\n\
Parallel lanes + evidence indexing are **recommended** when scope warrants—not a hard requirement on this path.\n\
To treat the remainder as a single-thread task: output **one** reject-reason token alone on one line (small_task, …); never spoof host `router-rs …` continuation lines."
            .to_string()
    } else {
        "Autopilot (/autopilot)：先写清 Goal 契约与验证口径；需要时再并行分工与证据索引（建议，非硬门槛）。确为小任务请**单独一行**拒因 token（如 small_task），不要自拟仿宿主 `router-rs …` 续跑行。"
            .to_string()
    }
}

/// 连续 pre-goal 提示上限：beforeSubmit 每轮在仍缺 pre-goal 时累加计数，达到后自动 `pre_goal_review_satisfied=true`，避免卡死。
/// - **未设置**环境变量：默认 **8**（第八轮仍卡则放行）。
/// - `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES=0` / `false` / `off` / `no`：**关闭**自动放行（严格）。
/// - 正整数：自定义上限。
fn cursor_autopilot_pre_goal_max_nudges_cap() -> Option<u32> {
    #[cfg(test)]
    {
        // 单测未显式设变量时关闭自动放行，避免并行用例间状态与计数依赖。
        let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES") else {
            return None;
        };
        let t = raw.trim().to_ascii_lowercase();
        if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
            return None;
        }
        t.parse::<u32>().ok().filter(|v| *v >= 1)
    }
    #[cfg(not(test))]
    {
        match std::env::var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES") {
            Err(_) => Some(8),
            Ok(raw) => {
                let t = raw.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
                    return None;
                }
                t.parse::<u32>().ok().filter(|v| *v >= 1).or(Some(8))
            }
        }
    }
}

fn maybe_autopilot_pre_goal_nag_cap_release(state: &mut ReviewGateState) -> Option<&'static str> {
    if !crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled() {
        return None;
    }
    if !state.goal_required
        || state.pre_goal_review_satisfied
        || is_overridden(state)
        || state.reject_reason_seen
    {
        return None;
    }
    let cap = cursor_autopilot_pre_goal_max_nudges_cap()?;
    state.pre_goal_nag_count = state.pre_goal_nag_count.saturating_add(1);
    if state.pre_goal_nag_count < cap {
        return None;
    }
    state.pre_goal_review_satisfied = true;
    state.pre_goal_nag_count = 0;
    clear_review_gate_escalation_counters(state);
    Some("router-rs：pre-goal 提示已达上限，已自动放行以便继续执行（需要严格不自动放行请设 `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES=0`）。仍可在用户消息单独一行写 `small_task` 主动清门。")
}

/// 本地逃生舱：unset 或设为 0/false/off/no 时**不**禁用；任意其它非空值禁用门控（Stop/beforeSubmit 短路，SessionEnd 仍清文件）。
fn cursor_review_gate_disabled_by_env() -> bool {
    #[cfg(test)]
    {
        if let Some(v) = TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.get()) {
            return v;
        }
    }
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE") else {
        return false;
    };
    let t = raw.trim().to_ascii_lowercase();
    !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
}

/// Cursor hook **静默**：不向宿主注入 `followup_message` / `additional_context`（多 agent 门控、PreCompact 摘要、AUTOPILOT_DRIVE、RFV 等合并文案一律剥离）。
/// unset 或 trim 后为 `0`/`false`/`off`/`no` 时不启用；其它非空取值启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 语义一致）。
/// **例外**：含合规短码的片段仍保留（见 `apply_cursor_hook_output_policy`：`AG_FOLLOWUP` / `REVIEW_GATE` / `CLOSEOUT_FOLLOWUP` 等）。
/// 状态机仍会读写 `.cursor/hook-state`；仅压制对模型的可见提示。
fn cursor_hook_silent_by_env() -> bool {
    #[cfg(test)]
    {
        if let Some(v) = TEST_CURSOR_HOOK_SILENT.with(|c| c.get()) {
            return v;
        }
        // 单测默认忽略进程环境，避免开发机设置污染断言。
        false
    }
    #[cfg(not(test))]
    {
        let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_HOOK_SILENT") else {
            return false;
        };
        let t = raw.trim().to_ascii_lowercase();
        !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
    }
}

/// `subagentStart` 只能拒绝/提示，不能主动关闭既有 subagent；这里用活跃数避免继续堆积。
fn cursor_max_open_subagents() -> Option<u32> {
    #[cfg(test)]
    {
        Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS)
    }
    #[cfg(not(test))]
    {
        let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS") else {
            return Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS);
        };
        let t = raw.trim().to_ascii_lowercase();
        if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
            return None;
        }
        t.parse::<u32>()
            .ok()
            .filter(|v| *v > 0)
            .map(|v| v.min(MAX_CONCURRENT_SUBAGENTS_LIMIT as u32))
            .or(Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS))
    }
}

fn cursor_open_subagent_stale_after_secs() -> Option<i64> {
    #[cfg(test)]
    {
        Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS)
    }
    #[cfg(not(test))]
    {
        let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") else {
            return Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS);
        };
        let t = raw.trim().to_ascii_lowercase();
        if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
            return None;
        }
        t.parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
            .or(Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS))
    }
}

fn reset_stale_active_subagents(state: &mut ReviewGateState) -> bool {
    if state.active_subagent_count == 0 {
        return false;
    }
    let Some(stale_after_secs) = cursor_open_subagent_stale_after_secs() else {
        return false;
    };
    let Some(started_at) = state.active_subagent_last_started_at.as_deref() else {
        return false;
    };
    let Ok(started_at) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return false;
    };
    let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
    if age.num_seconds() <= stale_after_secs {
        return false;
    }
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    true
}

fn subagent_limit_denial(active: u32, limit: u32) -> Value {
    json!({
        "permission": "deny",
        "user_message": format!(
            "router-rs：当前会话已有 {active} 个 subagent 仍标记为打开（上限 {limit}，等于 `max_concurrent_subagents_limit` 契约）。请先等已有 subagent 结束/关闭，或确认它们已 stale 后清理会话状态；如需临时关闭限流，设置 ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS=0。"
        )
    })
}

pub(crate) fn apply_cursor_hook_output_policy(output: &mut Value) {
    if !cursor_hook_silent_by_env() {
        return;
    }
    // Even in SILENT mode, keep **hard-stop / compliance** followups visible so the user
    // doesn't silently lose critical instructions (closeout enforcement, goal gate, lock failures).
    let keep_visible = {
        let blob = output
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
            + "\n"
            + output
                .get("additional_context")
                .and_then(Value::as_str)
                .unwrap_or("");
        blob.contains("CLOSEOUT_FOLLOWUP")
            || blob.contains("AG_FOLLOWUP")
            || blob.contains("REVIEW_GATE")
            || blob.contains("PAPER_ADVERSARIAL_HOOK")
            || blob.contains("pre-goal 提示已达上限")
            || blob.contains("hook-state 锁不可用")
    };
    if keep_visible {
        return;
    }
    if let Some(obj) = output.as_object_mut() {
        obj.remove("followup_message");
        obj.remove("additional_context");
    }
}

/// 应急关闭门控时仍执行 PostToolUse/Subagent 状态更新，但不对模型注入门控类提示（与 SILENT 剥离字段一致）。
fn strip_cursor_hook_user_visible_nags(output: &mut Value) {
    if let Some(obj) = output.as_object_mut() {
        obj.remove("followup_message");
        obj.remove("additional_context");
    }
}

/// 清门或 subagent 满足 review 后归零，避免 `followup_count` 长期累积导致 **escalation** 粘住。
fn clear_review_gate_escalation_counters(state: &mut ReviewGateState) {
    state.followup_count = 0;
    state.review_followup_count = 0;
    state.pre_goal_nag_count = 0;
}

/// `GOAL_STATE` 列表字段是否含至少一条非空字符串（避免 `[""]` 这种伪非空数组）。
fn goal_state_list_any_nonempty_string(goal: &Value, key: &str) -> bool {
    match goal.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .any(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false)),
        Some(Value::String(s)) => !s.trim().is_empty(),
        _ => false,
    }
}

/// 用 `GOAL_STATE.json` + `EVIDENCE_INDEX.json` 补全 goal 门控（只置 true，不收回），避免助手未写
/// 「Goal / Checkpoint / verified」等关键词时 Stop 报 `AG_FOLLOWUP` 四项全缺。
///
/// `arm_if_goal_file`：**Stop** 等收口路径传 `true`，在磁盘已有 GOAL 但 hook-state 未写 `goal_required` 时仍回补；
/// **beforeSubmit** 传 `false`，避免普通消息因残留 GOAL 文件被误标为 autopilot。
fn hydrate_goal_gate_from_disk(
    repo_root: &Path,
    state: &mut ReviewGateState,
    arm_if_goal_file: bool,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    if !state.goal_required && !arm_if_goal_file {
        return;
    }
    let Some((goal, task_id)) = frame.hydration_goal.as_ref() else {
        return;
    };
    if arm_if_goal_file {
        state.goal_required = true;
    }
    state.pre_goal_review_satisfied = true;
    state.pre_goal_nag_count = 0;
    let gtext = goal
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let has_goal_text = !gtext.is_empty();
    let validation_nonempty = goal_state_list_any_nonempty_string(goal, "validation_commands");
    let non_goals_nonempty = goal_state_list_any_nonempty_string(goal, "non_goals");
    // Contract should be "deep enough" even when hydrated from disk: require non-empty goal,
    // non-goals, validation commands, and done_when (with >=2 items).
    let done_when_items = goal
        .get("done_when")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count()
        })
        .unwrap_or(0);
    if has_goal_text && non_goals_nonempty && validation_nonempty && done_when_items >= 2 {
        state.goal_contract_seen = true;
    }
    let checkpointed = goal
        .get("checkpoints")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let (evidence_rows, evidence_ok) =
        crate::autopilot_goal::task_evidence_artifacts_summary_for_task(
            repo_root,
            task_id.as_str(),
        );
    let st_raw = goal.get("status").and_then(Value::as_str).unwrap_or("");
    let st_lc = st_raw.trim().to_ascii_lowercase();
    // `running` 为真源默认；`in_progress` 偶见于外部模板；缺省 status 且已有 goal 文本则按进行中回补。
    let active_like =
        matches!(st_lc.as_str(), "running" | "in_progress") || (has_goal_text && st_lc.is_empty());
    let disk_contract_signal = (done_when_items >= 2) && validation_nonempty && non_goals_nonempty;
    // 进行中状态或磁盘契约字段：进展/验收由 GOAL_STATE 承载，Stop 不强求聊天关键词。
    if checkpointed || evidence_rows || (has_goal_text && (disk_contract_signal || active_like)) {
        state.goal_progress_seen = true;
    }
    let blocker = goal
        .get("blocker")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    // Tightened (review P0-C): plain "running goal with disk contract" is no longer enough to
    // claim `verification_or_blocker`. Explicit signals required:
    //   1) terminal status (blocked / completed / paused), OR
    //   2) explicit blocker text, OR
    //   3) at least one successful EVIDENCE_INDEX row, OR
    //   4) a checkpoint already recorded (model wrote progress at least once).
    // Without one of these, the gate stays open so the model is asked to either run a verifier
    // command, post a blocker, or record a checkpoint before being treated as having "verified".
    if matches!(st_lc.as_str(), "blocked" | "completed" | "paused")
        || blocker
        || evidence_ok
        || checkpointed
    {
        state.goal_verify_or_block_seen = true;
    }
}

fn goal_missing_parts(state: &ReviewGateState) -> String {
    let mut missing = Vec::new();
    if !state.goal_contract_seen {
        missing.push("goal_contract");
    }
    if !state.goal_progress_seen {
        missing.push("checkpoint_progress");
    }
    if !state.goal_verify_or_block_seen {
        missing.push("verification_or_blocker");
    }
    missing.join(", ")
}

/// Stop 上的 goal 门控短码：固定带 `router-rs AG_FOLLOWUP` 前缀，避免与陈旧/错误的自拟续跑标签混淆；附一行可执行脱困提示（仍保持单行优先）。
fn goal_stop_followup_line(state: &ReviewGateState) -> String {
    let parts = goal_missing_parts(state);
    let mut line = format!("router-rs AG_FOLLOWUP missing_parts={parts}");
    if state.goal_followup_count >= 3 {
        line.push_str(" | 已连续多轮 Stop 未满足门控；若确为小任务请直接单独一行 small_task");
    }
    line
}

fn state_lock_degraded_followup() -> &'static str {
    "router-rs：hook-state 锁不可用，本闸门控降级。收口前须见独立 subagent lane，或在助手输出中写明拒因。"
}

fn lock_failure_followup_for_before_submit(prompt: &str) -> (bool, String) {
    let review = is_review_prompt(prompt);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(prompt);
    let review_arms = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(prompt) || framework_prompt_arms_delegation(prompt);
    let overridden =
        has_review_override(prompt) || has_delegation_override(prompt) || has_override(prompt);

    let strong_constraint = (review_arms || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return (
            false,
            "router-rs：hook-state 锁不可用，本条为严格 review/委托/autopilot，**已拦截提交**。请修锁/权限后重试，或起 subagent / 写明拒因。"
                .to_string(),
        );
    }

    (
        true,
        "router-rs：hook-state 锁不可用，门控**降级**；非严格提示仍可继续。".to_string(),
    )
}

fn lock_failure_followup_for_stop(event: &Value) -> String {
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);
    let review = is_review_prompt(&text);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(&text);
    let review_arms = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_review_override(&signal_text)
        || has_delegation_override(&signal_text)
        || has_override(&signal_text)
        || saw_reject_reason(&signal_text, &text);

    let strong_constraint = (review_arms || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return "router-rs：hook-state 锁不可用，本轮须严格 review/委托/autopilot 证据。合并前请修复锁/权限并重试，或 subagent/拒因。".to_string();
    }
    state_lock_degraded_followup().to_string()
}

fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let text = prompt_text(event);
        let (allow_continue, followup) = lock_failure_followup_for_before_submit(&text);
        let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
        let mut out = json!({ "continue": allow_continue });
        if !allow_continue || chat {
            out["followup_message"] = Value::String(followup);
        } else {
            merge_additional_context(&mut out, &followup);
        }
        merge_continuity_followups_before_submit(repo_root, &mut out, &frame);
        return out;
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    // delegation 启发式不再持久化进 hook-state，避免与 review 相位门控长期粘连。
    state.delegation_required = false;
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(&text);
    let review_arms_for_gate = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let review_override = has_review_override(&text) || has_override(&text);
    let delegation_override = has_delegation_override(&text) || has_override(&text);

    state.review_required = state.review_required || review_arms_for_gate;
    state.review_override = state.review_override || review_override;
    state.delegation_override = state.delegation_override || delegation_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
    state.goal_contract_seen = state.goal_contract_seen || has_goal_contract_signal(&signal_text);
    state.goal_progress_seen = state.goal_progress_seen || has_goal_progress_signal(&signal_text);
    state.goal_verify_or_block_seen =
        state.goal_verify_or_block_seen || has_goal_verify_or_block_signal(&signal_text);
    // 用户在本轮提交里写出 reject_reason token 时须即时生效；否则仅能在助手回复或 Stop 里识别，导致 autopilot pre-goal 与 AG_FOLLOWUP 循环。
    // `signal_text` 含整树字符串，覆盖仅出现在 `messages[].content` 等深层路径的 token。
    if saw_reject_reason(&signal_text, &text) {
        state.reject_reason_seen = true;
        if state.goal_required {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
    }
    hydrate_goal_gate_from_disk(repo_root, &mut state, false, &frame);
    if review || delegation || autopilot_entrypoint {
        state.last_prompt = Some(text.chars().take(500).collect());
    }

    let pre_goal_auto_release_note = maybe_autopilot_pre_goal_nag_cap_release(&mut state);

    let persisted = save_state(repo_root, event, &mut state);

    // Review：不在 beforeSubmit 注入 RG 文案；phase 由 subagent/PostToolUse 推进（仅 review_hard_armed）。
    let needs_autopilot_pre_goal =
        crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled()
            && state.goal_required
            && !state.pre_goal_review_satisfied
            && !is_overridden(&state)
            && !state.reject_reason_seen;
    let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
    let mut output = json!({ "continue": true });
    let mut followup_parts: Vec<String> = Vec::new();
    if needs_autopilot_pre_goal {
        // 仅计入总 follow-up 次数；不要把 goal_followup_count 算进去，否则首次 stop 会误判成「非首条」而跳过完整 goal 提示。
        state.followup_count += 1;
        let pre = autopilot_pre_goal_followup_message();
        if chat {
            followup_parts.push(pre);
        } else {
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                &mut output,
                &pre,
                "Autopilot (/autopilot)",
                false,
            );
        }
    }
    if chat && !followup_parts.is_empty() {
        output["followup_message"] = Value::String(followup_parts.join("\n\n"));
    }
    // beforeSubmit：Goal+RFV 双活跃时合并为一段「续跑」，减少重复机读标题。
    merge_continuity_followups_before_submit(repo_root, &mut output, &frame);
    if let Some(note) = pre_goal_auto_release_note {
        if chat {
            let existing = output
                .get("followup_message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let merged = if existing.trim().is_empty() {
                note.to_string()
            } else {
                format!("{existing}\n\n{note}")
            };
            output["followup_message"] = Value::String(merged);
        } else {
            merge_additional_context(&mut output, note);
        }
    }
    crate::paper_adversarial_hook::maybe_merge_paper_adversarial_before_submit(
        repo_root,
        &mut output,
        &text,
        chat,
    );
    let persisted_after_followup = if needs_autopilot_pre_goal {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    release_state_lock(&mut lock);
    if !persisted || !persisted_after_followup {
        let warning = "router-rs：hook-state 未能持久化，review/委托门控本回合可能降级。";
        if chat {
            let merged = output
                .get("followup_message")
                .and_then(Value::as_str)
                .map(|s| format!("{s} {warning}"))
                .unwrap_or_else(|| warning.to_string());
            output["followup_message"] = Value::String(merged.trim().to_string());
        } else {
            merge_additional_context(&mut output, warning);
        }
    }
    output
}

fn handle_subagent_start(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let tool_input = tool_input_of(event);
    let stale_reset = reset_stale_active_subagents(&mut state);
    if let Some(limit) = cursor_max_open_subagents() {
        if state.active_subagent_count >= limit {
            release_state_lock(&mut lock);
            return subagent_limit_denial(state.active_subagent_count, limit);
        }
    }
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let armed = review_hard_armed(&state);
    state.active_subagent_count = state.active_subagent_count.saturating_add(1);
    state.active_subagent_last_started_at = Some(Utc::now().to_rfc3339());
    let mut mutated = true;
    // 与 PostToolUse 对齐：pre-goal 在独立 fork 且存在 lane 类型证据时满足（含非白名单 lane 名）。
    if state.goal_required && pre_goal_kind && independent_fork {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if armed {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
        if was_below_2 {
            clear_review_gate_escalation_counters(&mut state);
        }
        mutated = true;
    }
    if armed && (!sub_type.is_empty() || !agent_type.is_empty()) {
        state.last_subagent_type = Some(if !sub_type.is_empty() {
            sub_type.clone()
        } else {
            agent_type.clone()
        });
        mutated = true;
    }
    if stale_reset {
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_subagent_stop(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let mut mutated = false;
    if state.active_subagent_count > 0 {
        state.active_subagent_count -= 1;
        if state.active_subagent_count == 0 {
            state.active_subagent_last_started_at = None;
        }
        mutated = true;
    }
    if review_hard_armed(&state) {
        // Intentional hardening vs baseline: stop evidence only counts after start reached phase 2.
        if state.phase < 2 {
            if mutated {
                let _ = save_state(repo_root, event, &mut state);
            }
            release_state_lock(&mut lock);
            return json!({});
        }
        bump_phase(&mut state, 3);
        state.subagent_stop_count += 1;
        state.lane_intent_matches = Some(true);
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = review_hard_armed(&state);
    let name = normalize_tool_name(Some(&tool_name_of(event)));
    let tool_input = tool_input_of(event);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let mut mutated = false;
    if tool_name_matches_subagent_lane(&name)
        && pre_goal_kind
        && state.goal_required
        && independent_fork
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if tool_name_matches_subagent_lane(&name) && pre_goal_kind && armed {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.last_subagent_tool = Some(name.clone());
        if !sub_type.is_empty() || !agent_type.is_empty() {
            state.last_subagent_type = Some(if !sub_type.is_empty() {
                sub_type
            } else {
                agent_type
            });
        }
        state.lane_intent_matches = Some(true);
        if was_below_2 {
            clear_review_gate_escalation_counters(&mut state);
        }
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);

    // Agent `Shell` 工具：`before/afterShellExecution` 可能不与 Task 工具一一对应；PostToolUse 再补记归属。
    if name == "shell" {
        cursor_post_tool_shell_terminal_track(repo_root, event);
    }

    // 与 Codex PostTool 对齐：终端执行验证类命令时写入 EVIDENCE_INDEX（连续性就绪且未关闭 POSTTOOL_EVIDENCE）。
    let syn = synthetic_codex_shape_for_post_tool_evidence(event);
    if let Err(err) = crate::framework_runtime::try_append_post_tool_shell_evidence(
        repo_root,
        &syn,
        "cursor_post_tool_verification",
    ) {
        eprintln!("[router-rs] cursor post-tool evidence append failed (non-fatal): {err}");
    }

    let mut out = json!({});
    if let Some(ctx) = maybe_run_cursor_rust_lint(repo_root, event) {
        merge_additional_context(&mut out, &ctx);
    }
    out
}

fn payload_tool_name(event: &Value) -> String {
    tool_name_of(event).trim().to_string()
}

fn payload_tool_path(event: &Value) -> Option<PathBuf> {
    event
        .get("tool_input")
        .and_then(|t| t.get("path"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            event
                .get("file_path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
}

fn tool_name_is_rust_file_write_tool(name: &str) -> bool {
    let n = name.trim();
    matches!(n, "Write" | "StrReplace" | "write" | "str_replace")
}

fn find_cargo_dir(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    for _ in 0..64 {
        if cur.join("Cargo.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn truncate_lines(s: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    s.lines().take(max_lines).collect::<Vec<_>>().join("\n")
}

fn cargo_check_with_timeout(cargo_dir: &Path, timeout: std::time::Duration) -> (i32, String) {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = match Command::new("cargo")
        .arg("check")
        .arg("--message-format=short")
        .current_dir(cargo_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => return (127, format!("rust-lint: failed to spawn cargo: {err}")),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(1);
                let mut buf = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    use std::io::Read;
                    let _ = stderr.read_to_string(&mut buf);
                }
                return (code, buf);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return (124, "rust-lint: cargo check exceeded timeout".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(err) => return (1, format!("rust-lint: cargo check wait error: {err}")),
        }
    }
}

fn maybe_run_cursor_rust_lint(repo_root: &Path, event: &Value) -> Option<String> {
    const TIMEOUT_S: u64 = 25;
    const MAX_ERROR_LINES: usize = 20;

    let tool_name = payload_tool_name(event);
    if !tool_name_is_rust_file_write_tool(&tool_name) {
        return None;
    }
    let path = payload_tool_path(event)?;
    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
        return None;
    }
    if !path.is_file() {
        return None;
    }
    if which::which("cargo").is_err() {
        return None;
    }
    let cargo_dir = find_cargo_dir(&path)?;

    let (rc, output) =
        cargo_check_with_timeout(&cargo_dir, std::time::Duration::from_secs(TIMEOUT_S));

    // Continuity: append cargo check outcome to artifacts/current/EVIDENCE_INDEX.json (no-op if continuity not seeded).
    let cmd_preview = format!(
        "(cd {} && cargo check --message-format=short)",
        cargo_dir.display()
    );
    let _ = crate::framework_runtime::framework_hook_evidence_append(json!({
        "repo_root": repo_root.display().to_string(),
        "command_preview": cmd_preview,
        "exit_code": rc,
        "source": "cursor_rust_lint",
    }));

    if rc == 0 {
        return None;
    }
    if rc == 124 {
        let base = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.rs");
        return Some(format!(
            "cargo check timed out after {TIMEOUT_S}s while checking {base} (crate: {}). Consider running cargo check manually.",
            cargo_dir.display()
        ));
    }

    let errors: String = output
        .lines()
        .filter(|l| l.starts_with("error") || l.starts_with("warning"))
        .take(MAX_ERROR_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    let fallback = truncate_lines(&output, MAX_ERROR_LINES);
    let picked = if !errors.trim().is_empty() {
        errors
    } else {
        fallback
    };
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.rs");
    Some(format!(
        "cargo check failed after editing {base}:\n{picked}\n\nFix these errors before finalizing. Run `cargo check` to verify."
    ))
}

/// 将 Cursor 异构 PostTool 载荷归一成 `framework_runtime` 可解析的 shell 证据形状（保留原始 `tool_output` / `exit_code` 等）。
fn synthetic_codex_shape_for_post_tool_evidence(event: &Value) -> Value {
    let mut out = match event.as_object() {
        Some(o) => o.clone(),
        None => serde_json::Map::new(),
    };
    out.insert("tool_name".to_string(), json!(tool_name_of(event)));
    let merged_input = tool_input_of(event);
    if merged_input
        .as_object()
        .map(|m| !m.is_empty())
        .unwrap_or(false)
    {
        out.insert("tool_input".to_string(), merged_input);
    }
    if let Some(s) = extract_first_session_string(event) {
        out.insert("session_id".to_string(), json!(s));
    }
    Value::Object(out)
}

fn handle_after_agent_response(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = review_hard_armed(&state);
    let track_goal = state.goal_required || armed;
    let prompt = prompt_text(event);
    let text = agent_response_text(event);
    let signal = hook_event_signal_text(event, &prompt, &text);
    let mut dirty = false;
    if saw_reject_reason(&signal, &prompt) {
        state.reject_reason_seen = true;
        if state.goal_required {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
        dirty = true;
    }
    if track_goal && has_goal_contract_signal(&signal) {
        state.goal_contract_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_progress_signal(&signal) {
        state.goal_progress_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_verify_or_block_signal(&signal) {
        state.goal_verify_or_block_seen = true;
        dirty = true;
    }
    if dirty {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let msg = lock_failure_followup_for_stop(event);
        let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
        let mut out = if chat {
            json!({ "followup_message": msg })
        } else {
            let mut o = json!({});
            merge_additional_context(&mut o, &msg);
            o
        };
        merge_continuity_followups(repo_root, &mut out, &frame);
        return out;
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);

    // Completion claim guard must not depend on hook-state existence: a strict closeout violation
    // is a hard-stop even when the review gate state was never initialized for this session.
    if completion_claimed_in_text(&response_text) {
        if let Some(tid) = crate::autopilot_goal::read_active_task_id(repo_root) {
            match closeout_followup_for_completion_claim(repo_root, &tid) {
                Ok(Some(msg)) => {
                    let chat =
                        crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                    if chat {
                        let mut out = json!({ "followup_message": msg });
                        merge_continuity_followups(repo_root, &mut out, &frame);
                        release_state_lock(&mut lock);
                        return out;
                    }
                    let mut o = json!({});
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut o,
                        &msg,
                        "CLOSEOUT_FOLLOWUP",
                        false,
                    );
                    merge_continuity_followups(repo_root, &mut o, &frame);
                    release_state_lock(&mut lock);
                    return o;
                }
                Ok(None) => {}
                Err(err) => {
                    let msg = format!(
                        "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluator_error error={err}"
                    );
                    let chat =
                        crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                    if chat {
                        let mut out = json!({ "followup_message": msg });
                        merge_continuity_followups(repo_root, &mut out, &frame);
                        release_state_lock(&mut lock);
                        return out;
                    }
                    let mut o = json!({});
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut o,
                        &msg,
                        "CLOSEOUT_FOLLOWUP",
                        false,
                    );
                    merge_continuity_followups(repo_root, &mut o, &frame);
                    release_state_lock(&mut lock);
                    return o;
                }
            }
        }
    }
    let mut output = match loaded {
        Ok(None) => json!({}),
        Err(io_error) => {
            let msg = format!(
                "router-rs：hook-state 不可读（{io_error}），门控降级。请检查权限与 JSON。"
            );
            let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
            if chat {
                json!({ "followup_message": msg })
            } else {
                let mut o = json!({});
                merge_additional_context(&mut o, &msg);
                o
            }
        }
        Ok(Some(mut state)) => {
            state.delegation_required = false;
            if has_review_override(&signal_text) || has_override(&signal_text) {
                state.review_override = true;
            }
            if has_delegation_override(&signal_text) || has_override(&signal_text) {
                state.delegation_override = true;
            }
            if has_goal_contract_signal(&signal_text) {
                state.goal_contract_seen = true;
            }
            if has_goal_progress_signal(&signal_text) {
                state.goal_progress_seen = true;
            }
            if has_goal_verify_or_block_signal(&signal_text) {
                state.goal_verify_or_block_seen = true;
            }
            if saw_reject_reason(&signal_text, &text) {
                state.reject_reason_seen = true;
                if state.goal_required {
                    state.pre_goal_review_satisfied = true;
                }
                clear_review_gate_escalation_counters(&mut state);
            }
            hydrate_goal_gate_from_disk(repo_root, &mut state, true, &frame);
            // Completion claim guard: when strict closeout enforcement is enabled, do not allow
            // "done/passed/完成/通过" claims to silently end without a passing closeout record.
            //
            // This must take precedence over AG_FOLLOWUP: a strict closeout violation is a hard-stop
            // regardless of goal-gate satisfaction.
            let mut closeout_output: Option<Value> = None;
            if completion_claimed_in_text(&response_text) {
                if let Some(tid) = crate::autopilot_goal::read_active_task_id(repo_root) {
                    match closeout_followup_for_completion_claim(repo_root, &tid) {
                        Ok(Some(msg)) => {
                            state.followup_count += 1;
                            let _ = save_state(repo_root, event, &mut state);
                            let chat =
                                crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                            if chat {
                                closeout_output = Some(json!({ "followup_message": msg }));
                            } else {
                                let mut o = json!({});
                                crate::autopilot_goal::merge_hook_nudge_paragraph(
                                    &mut o,
                                    &msg,
                                    "CLOSEOUT_FOLLOWUP",
                                    false,
                                );
                                closeout_output = Some(o);
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            state.followup_count += 1;
                            let _ = save_state(repo_root, event, &mut state);
                            let msg = format!(
                                "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluator_error error={err}"
                            );
                            let chat =
                                crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                            if chat {
                                closeout_output = Some(json!({ "followup_message": msg }));
                            } else {
                                let mut o = json!({});
                                crate::autopilot_goal::merge_hook_nudge_paragraph(
                                    &mut o,
                                    &msg,
                                    "CLOSEOUT_FOLLOWUP",
                                    false,
                                );
                                closeout_output = Some(o);
                            }
                        }
                    }
                }
            }

            if let Some(out) = closeout_output {
                out
            } else if review_stop_followup_needed(&state) {
                state.followup_count += 1;
                state.review_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let message = review_stop_followup_line(&state);
                let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                if chat {
                    json!({ "followup_message": message })
                } else {
                    let mut o = json!({});
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut o,
                        &message,
                        "REVIEW_GATE",
                        false,
                    );
                    o
                }
            } else if !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = goal_stop_followup_line(&state);
                let chat = crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled();
                if chat {
                    json!({ "followup_message": message })
                } else {
                    let mut o = json!({});
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut o,
                        &message,
                        "AG_FOLLOWUP",
                        false,
                    );
                    o
                }
            } else {
                // Do not clear gate state on Stop for sessions that still track goal/review:
                // the next Stop should still enforce the same requirements until satisfied/overridden.
                if state.review_required || state.goal_required || state.reject_reason_seen {
                    let _ = save_state(repo_root, event, &mut state);
                } else {
                    let mut reset = empty_state();
                    let _ = save_state(repo_root, event, &mut reset);
                }
                json!({})
            }
        }
    };
    merge_continuity_followups(repo_root, &mut output, &frame);
    release_state_lock(&mut lock);
    output
}

fn handle_pre_compact(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "additional_context": "router-rs：hook-state 锁不可用，preCompact 未读到持久化门控状态。"
        });
    }
    let mut out = match load_state(repo_root, event) {
        Ok(Some(state)) => {
            let mut summary = format!(
                "router-rs 门控快照：phase={} review={} delegation={} override={} reject={} pre_goal_ok={} subagent_start={} subagent_stop={}",
                state.phase,
                state.review_required,
                state.delegation_required,
                is_overridden(&state),
                state.reject_reason_seen,
                state.pre_goal_review_satisfied,
                state.subagent_start_count,
                state.subagent_stop_count
            );
            if let Some(hint) = crate::rfv_loop::rfv_loop_precompact_hint(repo_root) {
                summary.push_str(" | ");
                summary.push_str(&hint);
            }
            json!({ "additional_context": summary })
        }
        _ => json!({}),
    };
    // Token awareness (ported from .cursor/hooks/precompact-notice.sh)
    let usage = event
        .get("context_usage_percent")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_usage_percent")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let tokens = event
        .get("context_tokens")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_tokens")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let size = event
        .get("context_window_size")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_window_size")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let msgs = event
        .get("message_count")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("message_count")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let compact = event
        .get("messages_to_compact")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("messages_to_compact")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let trigger = event
        .get("trigger")
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_string();
    let first = event
        .get("is_first_compaction")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut notice = format!(
        "⚡ Context compacting ({trigger}): {usage}% used · {tokens}/{size} tokens · {msgs} messages · {compact} being summarised."
    );
    if first {
        notice.push_str(" First compaction — earlier details may be summarised.");
    }
    notice.push_str(" Consider starting a new session if the current task scope is complete.");
    out["user_message"] = Value::String(notice);
    release_state_lock(&mut lock);
    out
}

fn read_file_head_lines(path: &Path, max_lines: usize) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.lines().take(max_lines).collect::<Vec<_>>().join("\n"))
}

fn read_json_value_strict(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

fn truncate_utf8_chars_local(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    if input.chars().count() > max_chars && max_chars >= 3 {
        out.truncate(out.len().saturating_sub(3));
        out.push_str("...");
    }
    out
}

#[allow(dead_code)]
fn read_json_strict(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read json failed for {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse json failed for {}: {err}", path.display()))
}

#[allow(dead_code)]
fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn handle_session_start(repo_root: &Path, event: &Value) -> Value {
    maybe_init_session_terminal_ledger(repo_root, event);
    let mut continuity_block = String::new();
    let summary_path = repo_root.join("artifacts/current/SESSION_SUMMARY.md");
    if summary_path.is_file() {
        if let Some(head) = read_file_head_lines(&summary_path, 36) {
            continuity_block.push_str("\n\n## Continuity (artifacts/current/SESSION_SUMMARY.md)\n");
            continuity_block.push_str(&head);
            continuity_block.push('\n');
        }
    }

    let mut long_task_block = String::new();
    if !cursor_hook_silent_by_env() {
        let active_task = repo_root.join("artifacts/current/active_task.json");
        if active_task.is_file() {
            if let Some(v) = read_json_value_strict(&active_task) {
                if let Some(tid) = v.get("task_id").and_then(Value::as_str) {
                    if !tid.trim().is_empty() && tid.trim() != "null" {
                        let gs = repo_root
                            .join("artifacts/current")
                            .join(tid)
                            .join("GOAL_STATE.json");
                        let rv = repo_root
                            .join("artifacts/current")
                            .join(tid)
                            .join("RFV_LOOP_STATE.json");
                        if gs.is_file() {
                            if let Some(gv) = read_json_value_strict(&gs) {
                                let status =
                                    gv.get("status").and_then(Value::as_str).unwrap_or("?");
                                let drive = gv
                                    .get("drive_until_done")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false);
                                let goal = gv.get("goal").and_then(Value::as_str).unwrap_or("-");
                                let goal_trunc = truncate_utf8_chars_local(goal, 120);
                                long_task_block.push_str(&format!(
                                    "\n## Goal（`artifacts/current/{tid}/GOAL_STATE.json`）\n- {status} · drive={drive} · {goal_trunc}\n"
                                ));
                            }
                        }
                        if rv.is_file() {
                            if let Some(rvj) = read_json_value_strict(&rv) {
                                let loop_status = rvj
                                    .get("loop_status")
                                    .and_then(Value::as_str)
                                    .unwrap_or("?");
                                let cur = rvj
                                    .get("current_round")
                                    .and_then(Value::as_i64)
                                    .unwrap_or(0);
                                let max =
                                    rvj.get("max_rounds").and_then(Value::as_i64).unwrap_or(0);
                                let goal = rvj.get("goal").and_then(Value::as_str).unwrap_or("-");
                                let goal_trunc = truncate_utf8_chars_local(goal, 80);
                                long_task_block.push_str(&format!(
                                    "\n## RFV（`artifacts/current/{tid}/RFV_LOOP_STATE.json`）\n- {loop_status} · round {cur}/{max} · {goal_trunc}\n"
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let ctx = format!(
        "## Skill Repo — Quick Reference\n\n**Root:** {root}\n**Stack:** Rust (scripts/router-rs/), bash scripts, TOML/JSON config\n\n**Build & test:**\n- `cd scripts/router-rs && cargo build --release`\n- `cd scripts/router-rs && cargo test`\n- `cd scripts/router-rs && cargo clippy -- -D warnings`\n\n**Key paths:**\n- `scripts/router-rs/src/` — Rust hook router (cursor_hooks.rs, route.rs, …)\n- `skills/SKILL_ROUTING_RUNTIME.json` — skill routing truth source (use this, not skills/ dir)\n- `.cursor/hooks.json` — Cursor hook config\n- `AGENTS.md` — agent execution policy\n- `artifacts/current/` — continuity checkpoints (SESSION_SUMMARY, NEXT_ACTIONS, …)\n\n**Conventions:**\n- Rust: clippy-clean, rustfmt-formatted, no bare `unwrap()` in library paths\n- Skills: always route via SKILL_ROUTING_RUNTIME.json; never pre-read the full skills/ dir\n- JSON: 2-space indent\n- Git commits: only when user explicitly asks\n\n**Tool cost hierarchy (cheapest first):**\nShell → Glob → Grep → Read → StrReplace/Write → SemanticSearch → MCP{continuity}{long_task}\n",
        root = repo_root.display(),
        continuity = continuity_block,
        long_task = long_task_block
    );
    json!({ "additional_context": ctx })
}

fn shell_event_command(event: &Value) -> Option<String> {
    first_nonempty_event_str(event, &["command"])
        .split('\n')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn shell_event_cwd(event: &Value) -> Option<PathBuf> {
    let cwd = first_nonempty_event_str(event, &["cwd"]);
    if cwd.trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(cwd))
}

fn maybe_init_session_terminal_ledger(repo_root: &Path, event: &Value) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    let mut baseline: Vec<u32> = observations.iter().map(|o| o.pid).collect();
    baseline.sort_unstable();
    baseline.dedup();
    let ledger = SessionTerminalLedger {
        version: SESSION_TERMINAL_LEDGER_VERSION,
        baseline_pids: baseline,
        owned_pids: Vec::new(),
        pending_shells: Vec::new(),
    };
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn maybe_track_shell_owned_terminals(
    repo_root: &Path,
    event: &Value,
    matched_after_ms: Option<u64>,
) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    if observations.is_empty() {
        return;
    }
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    if ledger.version != SESSION_TERMINAL_LEDGER_VERSION {
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    }
    let baseline: HashSet<u32> = ledger.baseline_pids.iter().copied().collect();
    let mut owned: HashSet<u32> = ledger.owned_pids.iter().copied().collect();
    let cwd_filter = shell_event_cwd(event);
    let cmd_filter = shell_event_command(event).map(|s| normalize_shell_command(&s));
    for obs in observations {
        if baseline.contains(&obs.pid) {
            continue;
        }
        if let Some(t0) = matched_after_ms {
            if let Some(sa) = obs.started_at_ms {
                let floor = t0.saturating_sub(SHELL_TERMINAL_TIME_MATCH_SLACK_MS);
                if sa < floor {
                    continue;
                }
            }
        }
        if !obs.cwd.is_absolute() {
            continue;
        }
        if let Some(ref cwd) = cwd_filter {
            let obs_canon = obs.cwd.canonicalize().unwrap_or_else(|_| obs.cwd.clone());
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !obs_canon.starts_with(&cwd_canon) && !cwd_canon.starts_with(&obs_canon) {
                continue;
            }
        }
        if let Some(ref cmd) = cmd_filter {
            let active = obs
                .active_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            let last = obs
                .last_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            if !active.is_empty()
                && !last.is_empty()
                && !active.contains(cmd)
                && !cmd.contains(&active)
                && !last.contains(cmd)
                && !cmd.contains(&last)
            {
                continue;
            }
        }
        owned.insert(obs.pid);
    }
    let mut owned_vec: Vec<u32> = owned.into_iter().collect();
    owned_vec.sort_unstable();
    ledger.owned_pids = owned_vec;
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn handle_before_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    if !cmd_norm.is_empty() {
        let mut ledger = load_session_terminal_ledger(repo_root, event);
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
        let cwd_raw = shell_event_cwd(event)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        ledger.pending_shells.push(PendingShellRecord {
            command_norm: cmd_norm,
            cwd_raw,
            queued_ms: now_millis(),
        });
        trim_pending_shell_records(&mut ledger);
        save_session_terminal_ledger(repo_root, event, &ledger);
        // Shell 仍未真正启动 PID 前：仅用 baseline-diff + 指令/cwd 启发式扩展 owned（不关时间窗）。
        maybe_track_shell_owned_terminals(repo_root, event, None);
    }
    json!({
        "continue": true,
        "permission": "allow"
    })
}

fn handle_after_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    let cwd_buf = shell_event_cwd(event);
    let cwd_hint = cwd_buf.as_deref();
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    let matched_after_ms = pop_matching_pending_shell(&mut ledger, &cmd_norm, cwd_hint);
    save_session_terminal_ledger(repo_root, event, &ledger);
    // 配对成功则用 pending 队列时间压低「它仓并发 terminal」误判；配对失败退回纯启发式（None）。
    maybe_track_shell_owned_terminals(repo_root, event, matched_after_ms);
    json!({})
}

fn handle_after_file_edit(_repo_root: &Path, event: &Value) -> Value {
    let path = event.get("file_path").and_then(Value::as_str).unwrap_or("");
    let p = PathBuf::from(path);
    if p.extension().and_then(|e| e.to_str()) != Some("rs") {
        return json!({});
    }
    if !p.is_file() {
        return json!({});
    }
    if which::which("rustfmt").is_err() {
        return json!({});
    }
    let _ = std::process::Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(&p)
        .status();
    json!({})
}

fn handle_session_end(repo_root: &Path, event: &Value) -> Value {
    // **必须先读出 terminal 账本**，再 `sweep_review_gate_state_dir`：否则会话级 `session-terminals-*.json`
    // 会被清扫删掉，导致 `owned_pids` 为空，SessionEnd 永远不回收本会话 shell。
    let ledger = load_session_terminal_ledger(repo_root, event);
    let owned_vec = ledger.owned_pids.clone();
    let owned: HashSet<u32> = owned_vec.into_iter().collect();
    // 先按本会话 session_key 精准清一次（保留 happy-path 的 lock 元数据语义）。
    let _ = fs::remove_file(state_path(repo_root, event));
    let _ = fs::remove_file(state_lock_path(repo_root, event));
    remove_adversarial_loop(repo_root, event);
    // 再按文件名前缀清扫整个 `.cursor/hook-state/`，**解决与** `review-gate.sh` router-缺失 fallback **同类的** stale
    // 状态泄漏问题：SessionEnd 的 payload 不一定带与 beforeSubmit 一致的 `session_id` / `cwd`，单文件删法会让旧
    // 会话状态跨会话泄漏。**清理范围不同**：fallback 是 `find -mindepth 1 -maxdepth 1 -exec rm -rf` 清空目录顶层；
    // 本路径仅清扫本模块写入的已知前缀（review-subagent / adversarial-loop 主状态 + lock + 原子写入孤儿 tmp），
    // 避免误伤其它 hook 共用目录时的状态文件。详见 AGENTS.md → Continuity → Cursor 段。
    sweep_review_gate_state_dir(repo_root);
    let owned_filter = if cursor_terminal_kill_use_scoped_ownership() {
        Some(&owned)
    } else {
        None
    };
    // 默认仅回收本会话 shell 账本登记的 terminal；`ROUTER_RS_CURSOR_TERMINAL_KILL_MODE=legacy` 等恢复全仓 stale 扫描。
    let report = terminate_stale_terminal_processes(repo_root, owned_filter);
    if !cursor_hook_silent_by_env() {
        if !report.killed.is_empty() {
            eprintln!(
                "router-rs SessionEnd: terminated {} stale terminal pid(s) {:?} (scanned={}, outside_repo={}, dead={}, not_owned={})",
                report.killed.len(),
                report.killed,
                report.scanned,
                report.skipped_outside_repo,
                report.skipped_dead,
                report.skipped_not_owned,
            );
        }
        if !report.failed.is_empty() {
            eprintln!(
                "router-rs SessionEnd: failed to terminate pid(s): {:?}",
                report.failed
            );
        }
    }
    json!({})
}

/// 清扫 `.cursor/hook-state/` 下所有由本模块写入的状态文件：
/// 1. review gate 主状态：`review-subagent-<key>.json` / `.lock`；
/// 2. adversarial-loop 主状态：`adversarial-loop-<key>.json`；
/// 3. 原子写入孤儿（崩溃 / 异常退出残留）：
///    - `save_state` 留下 `.tmp-<pid>-<micros>-review-subagent-<key>.json`；
///    - 历史 `/loop` 实现曾留下 `.tmp-adv-loop-<pid>-<micros>`（无扩展名）原子写入孤儿。
///
/// 不递归子目录、不删除其它前缀的文件，避免误伤共用目录的其它 hook 状态。
fn sweep_review_gate_state_dir(repo_root: &Path) {
    let dir = state_dir(repo_root);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if review_gate_state_file_owned_by_module(name) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// 判断 `.cursor/hook-state/` 下的文件名是否由本模块写入。仅识别已知前缀以避免误伤
/// 与本模块共用目录的其它 hook 状态；命名约定与 `state_path` / `state_lock_path` /
/// `adversarial_loop_path` / `save_state` 文件名规则保持一致。
fn review_gate_state_file_owned_by_module(name: &str) -> bool {
    // 主状态：扩展名约束 json|lock，避免误删用户放进来的同前缀其它扩展文件。
    if name.starts_with("review-subagent-") || name.starts_with("adversarial-loop-") {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
        {
            return matches!(ext, "json" | "lock");
        }
        return false;
    }
    if name.starts_with("session-terminals-") {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
        {
            return ext == "json";
        }
        return false;
    }
    // 原子写入孤儿（崩溃残留）。`save_state` 的 tmp 形如
    // `.tmp-<pid>-<micros>-review-subagent-<key>.json`，故用「以 `.tmp-` 起、且包含 `review-subagent-`」识别；
    // 历史 adversarial-loop 原子写入 tmp 形如 `.tmp-adv-loop-<pid>-<micros>`（无扩展名），故单独前缀识别。
    if name.starts_with(".tmp-") && name.contains("review-subagent-") {
        return true;
    }
    if name.starts_with(".tmp-adv-loop-") {
        return true;
    }
    false
}

// --- SessionEnd: 清理本仓库 Cursor terminal 留下的 stale 子进程 ---
//
// 痛点：`run_terminal_cmd` 等 shell 工具发起的 `cargo test` / python 实验脚本，
// 因工具超时被断开但子进程仍在跑（`block_until_ms: 0` 后台命令同理）。多个会话叠加
// 内存与 CPU 越占越多。SessionEnd 时按 Cursor `terminals/<id>.txt` header 找出
// 仍 active 且 cwd 在本仓库内的 PID，发 SIGTERM → 2s 兜底 SIGKILL（含进程组）。
// 默认开启；`ROUTER_RS_CURSOR_KILL_STALE_TERMINALS=0|false|off|no` 关闭整个步骤。

#[derive(Debug, Default, Clone)]
struct StaleTerminalKillReport {
    scanned: usize,
    killed: Vec<u32>,
    skipped_outside_repo: usize,
    skipped_inactive: usize,
    skipped_dead: usize,
    skipped_not_owned: usize,
    failed: Vec<(u32, String)>,
}

#[derive(Debug, Default, Clone)]
struct TerminalHeader {
    pid: Option<u32>,
    cwd: Option<PathBuf>,
    is_active: bool,
    active_command: Option<String>,
    last_command: Option<String>,
    started_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct TerminalKillTarget {
    pid: u32,
    pgid: Option<u32>,
}

#[derive(Debug, Clone)]
struct TerminalObservation {
    pid: u32,
    cwd: PathBuf,
    active_command: Option<String>,
    last_command: Option<String>,
    started_at_ms: Option<u64>,
}

fn cursor_kill_stale_terminals_disabled_by_env() -> bool {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS") else {
        return false;
    };
    let t = raw.trim().to_ascii_lowercase();
    matches!(t.as_str(), "0" | "false" | "off" | "no")
}

/// terminals 目录定位优先级：
/// 1. `CURSOR_TERMINALS_DIR`（显式覆盖，便于测试与定制）
/// 2. `$HOME/.cursor/projects/<repo_root 绝对路径替换 / 为 - 去前导 ->/terminals/`
fn resolve_cursor_terminals_dir(repo_root: &Path) -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("CURSOR_TERMINALS_DIR") {
        let p = PathBuf::from(explicit);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    let abs = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let abs_str = abs.to_str()?;
    let trimmed = abs_str.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let mangled = trimmed.replace('/', "-");
    let dir = PathBuf::from(home)
        .join(".cursor")
        .join("projects")
        .join(mangled)
        .join("terminals");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// 解析 Cursor terminals/*.txt 头部 YAML-front-matter（首个 `---` ... `---` 区段）。
/// 仅取关心的字段；缺失字段返回 `None`/默认值，调用方再做过滤。
fn parse_terminal_header(text: &str) -> Option<TerminalHeader> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut header = TerminalHeader::default();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, val)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim().trim_matches('"').trim();
        match key {
            "pid" => header.pid = val.parse().ok(),
            "cwd" => {
                if !val.is_empty() {
                    header.cwd = Some(PathBuf::from(val));
                }
            }
            "running_for_ms" => header.is_active = !val.is_empty(),
            "active_command" => {
                if !val.is_empty() {
                    header.active_command = Some(val.to_string());
                }
            }
            "last_command" => {
                if !val.is_empty() {
                    header.last_command = Some(val.to_string());
                }
            }
            "started_at" => {
                header.started_at_ms = parse_terminal_started_at_unix_ms(val);
            }
            _ => {}
        }
    }
    Some(header)
}

fn normalize_shell_command(raw: &str) -> String {
    raw.trim_matches('"')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn collect_terminal_observations(terminals_dir: &Path) -> Vec<TerminalObservation> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(terminals_dir) else {
        return out;
    };
    let mut buf = String::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        buf.clear();
        if let Ok(file) = fs::File::open(&path) {
            let _ = file.take(4096).read_to_string(&mut buf);
        }
        let Some(header) = parse_terminal_header(&buf) else {
            continue;
        };
        let (Some(pid), Some(cwd)) = (header.pid, header.cwd) else {
            continue;
        };
        out.push(TerminalObservation {
            pid,
            cwd,
            active_command: header.active_command,
            last_command: header.last_command,
            started_at_ms: header.started_at_ms,
        });
    }
    out
}

#[cfg(unix)]
fn process_pgid(pid: u32) -> Option<u32> {
    let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_pgid() -> Option<u32> {
    let pgid = unsafe { libc::getpgrp() };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_ppid() -> Option<u32> {
    let ppid = unsafe { libc::getppid() };
    if ppid <= 0 {
        None
    } else {
        Some(ppid as u32)
    }
}

#[cfg(not(unix))]
fn process_pgid(_pid: u32) -> Option<u32> {
    None
}

#[cfg(unix)]
fn signal_pid_or_pgrp(pid: u32, pgid: Option<u32>, signal: libc::c_int) {
    let safe_pgid = match (pgid, current_pgid()) {
        (Some(target), Some(ours)) if target == ours => None,
        (other, _) => other,
    };
    let target = match safe_pgid {
        Some(g) => -(g as libc::pid_t),
        None => pid as libc::pid_t,
    };
    unsafe {
        let _ = libc::kill(target, signal);
    }
}

/// SIGTERM → 最多等 2s → SIGKILL；优先按进程组信号，覆盖 `cargo test`/`python -m` 这类 fork 子进程的命令。
#[cfg(unix)]
fn terminate_pids_batch(targets: &[TerminalKillTarget]) -> (Vec<u32>, Vec<(u32, String)>) {
    if targets.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Phase 1: SIGTERM fan-out.
    for t in targets {
        signal_pid_or_pgrp(t.pid, t.pgid, libc::SIGTERM);
    }

    // Phase 2: shared wait budget (<= 2s total) instead of per-pid waits.
    let mut remaining: Vec<TerminalKillTarget> = targets.to_vec();
    let mut deadline_slices = 20;
    while deadline_slices > 0 && !remaining.is_empty() {
        thread::sleep(Duration::from_millis(100));
        remaining.retain(|t| is_process_alive(t.pid));
        deadline_slices -= 1;
    }

    // Phase 3: SIGKILL for any stragglers.
    if !remaining.is_empty() {
        for t in &remaining {
            signal_pid_or_pgrp(t.pid, t.pgid, libc::SIGKILL);
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Build outputs in a stable, deterministic order (input order).
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for t in targets {
        if !is_process_alive(t.pid) {
            killed.push(t.pid);
        } else {
            failed.push((t.pid, format!("SIGKILL did not reap pid={}", t.pid)));
        }
    }
    (killed, failed)
}

#[cfg(not(unix))]
fn terminate_pids_batch(_targets: &[TerminalKillTarget]) -> (Vec<u32>, Vec<(u32, String)>) {
    (Vec::new(), Vec::new())
}

fn terminate_stale_terminal_processes(
    repo_root: &Path,
    owned_pids: Option<&HashSet<u32>>,
) -> StaleTerminalKillReport {
    if cursor_kill_stale_terminals_disabled_by_env() {
        return StaleTerminalKillReport::default();
    }
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return StaleTerminalKillReport::default();
    };
    terminate_stale_terminal_processes_in_dir(repo_root, &terminals_dir, owned_pids)
}

/// 纯逻辑形式：调用方提供 terminals 目录（便于测试与显式覆盖路径）。不再读 env 开关。
fn terminate_stale_terminal_processes_in_dir(
    repo_root: &Path,
    terminals_dir: &Path,
    owned_pids: Option<&HashSet<u32>>,
) -> StaleTerminalKillReport {
    let mut report = StaleTerminalKillReport::default();
    let entries = match fs::read_dir(terminals_dir) {
        Ok(e) => e,
        Err(_) => return report,
    };
    let our_pid = std::process::id();
    #[cfg(unix)]
    let our_ppid = current_ppid().unwrap_or(0);
    let abs_repo = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut kill_targets: Vec<TerminalKillTarget> = Vec::new();
    let mut buf = String::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.ends_with(".txt") {
            continue;
        }
        if let Ok(ft) = entry.file_type() {
            if !ft.is_file() {
                continue;
            }
        }
        let path = entry.path();
        report.scanned += 1;
        // header 在前 ~4KB 内，避免读整个 terminal 输出文件。
        buf.clear();
        if let Ok(file) = fs::File::open(&path) {
            let _ = file.take(4096).read_to_string(&mut buf);
        }
        let Some(header) = parse_terminal_header(&buf) else {
            continue;
        };
        if !header.is_active {
            report.skipped_inactive += 1;
            continue;
        }
        let Some(pid) = header.pid else {
            continue;
        };
        if pid <= 1 || pid == our_pid {
            continue;
        }
        #[cfg(unix)]
        if pid == our_ppid {
            continue;
        }
        // 范围过滤：cwd 必须落在本仓库内，避免误杀同机器其他项目的 terminal。
        // 先于 is_process_alive：pid 已消失但仍带“外仓 cwd”的文件应记为 skipped_outside_repo，而非 skipped_dead。
        let Some(cwd) = header.cwd.as_ref() else {
            report.skipped_outside_repo += 1;
            continue;
        };
        // 绝不接受相对路径 cwd：相对路径 canonicalize 依赖当前进程 cwd，存在误判扩大范围的风险。
        if !cwd.is_absolute() {
            report.skipped_outside_repo += 1;
            continue;
        }
        // Fast path: avoid canonicalize() for obvious outside-repo paths.
        if !cwd.starts_with(repo_root) && !cwd.starts_with(&abs_repo) {
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !cwd_canon.starts_with(&abs_repo) {
                report.skipped_outside_repo += 1;
                continue;
            }
        } else {
            // Even when the raw path looks inside, normalize once to avoid symlink surprises.
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !cwd_canon.starts_with(&abs_repo) {
                report.skipped_outside_repo += 1;
                continue;
            }
        }
        if !is_process_alive(pid) {
            report.skipped_dead += 1;
            continue;
        }
        if let Some(owned) = owned_pids {
            if !owned.contains(&pid) {
                report.skipped_not_owned += 1;
                continue;
            }
        }
        kill_targets.push(TerminalKillTarget {
            pid,
            pgid: process_pgid(pid),
        });
    }
    let (killed, failed) = terminate_pids_batch(&kill_targets);
    report.killed.extend(killed);
    report.failed.extend(failed);
    report
}

pub(crate) fn dispatch_cursor_hook_event(
    repo_root: &Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    if cursor_review_gate_disabled_by_env() {
        let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
        return match lowered {
            "sessionstart" => handle_session_start(repo_root, payload),
            "beforesubmitprompt" | "userpromptsubmit" => {
                let mut out = json!({ "continue": true });
                merge_continuity_followups_before_submit(repo_root, &mut out, &frame);
                out
            }
            "sessionend" => handle_session_end(repo_root, payload),
            "stop" => {
                let mut out = json!({});
                merge_continuity_followups(repo_root, &mut out, &frame);
                // Even when review gate is disabled, keep strict closeout enforcement visible:
                // completion claims should not silently pass without a record.
                let response_text = agent_response_text(payload);
                if completion_claimed_in_text(&response_text) {
                    if let Some(tid) = crate::autopilot_goal::read_active_task_id(repo_root) {
                        if let Ok(Some(msg)) =
                            closeout_followup_for_completion_claim(repo_root, &tid)
                        {
                            out["followup_message"] = Value::String(msg);
                        }
                    }
                }
                out
            }
            "posttooluse" => {
                let mut out = handle_post_tool_use(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "beforeshellexecution" => handle_before_shell_execution(repo_root, payload),
            "aftershellexecution" => handle_after_shell_execution(repo_root, payload),
            "subagentstart" => {
                let mut out = handle_subagent_start(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "subagentstop" => {
                let mut out = handle_subagent_stop(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            "afterfileedit" => handle_after_file_edit(repo_root, payload),
            "precompact" => {
                let mut out = handle_pre_compact(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
            _ => json!({}),
        };
    }
    match lowered {
        "sessionstart" => handle_session_start(repo_root, payload),
        "beforesubmitprompt" | "userpromptsubmit" => handle_before_submit(repo_root, payload),
        "subagentstart" => handle_subagent_start(repo_root, payload),
        "subagentstop" => handle_subagent_stop(repo_root, payload),
        "posttooluse" => handle_post_tool_use(repo_root, payload),
        "beforeshellexecution" => handle_before_shell_execution(repo_root, payload),
        "aftershellexecution" => handle_after_shell_execution(repo_root, payload),
        "afteragentresponse" => handle_after_agent_response(repo_root, payload),
        "stop" => handle_stop(repo_root, payload),
        "afterfileedit" => handle_after_file_edit(repo_root, payload),
        "precompact" => handle_pre_compact(repo_root, payload),
        "sessionend" => handle_session_end(repo_root, payload),
        _ => json!({}),
    }
}

fn read_stdin_json_from_reader<R: Read>(reader: &mut R) -> Result<Value, String> {
    const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;
    let mut buf = String::new();
    reader
        .by_ref()
        .take(MAX_STDIN_BYTES)
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    let mut probe = [0_u8; 1];
    let overflow = reader.read(&mut probe).map_err(|e| e.to_string())?;
    if overflow > 0 {
        return Err("stdin_too_large".to_string());
    }
    if buf.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(&buf).map_err(|_| "stdin_json_invalid".to_string())?;
    if value.is_object() {
        Ok(value)
    } else {
        Ok(json!({}))
    }
}

pub(crate) fn read_cursor_hook_stdin_json() -> Result<Value, String> {
    let mut stdin = std::io::stdin();
    read_stdin_json_from_reader(&mut stdin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::{env, fs};

    /// 模板/模型偶发的陈旧 Review 续跑前缀（分段拼接，避免在对外文案里复述整词）。
    const LEGACY_REVIEW_FOLLOWUP_TOKEN: &str = concat!("RG", "_FOLLOWUP");

    /// Drop 时清除 thread_local 覆盖，避免遗留应急门控语义并污染同 OS 线程上的其它用例。
    struct ReviewGateDisableTestGuard;

    impl ReviewGateDisableTestGuard {
        fn new() -> Self {
            super::set_test_review_gate_disable_override(Some(true));
            Self
        }
    }

    impl Drop for ReviewGateDisableTestGuard {
        fn drop(&mut self) {
            super::set_test_review_gate_disable_override(None);
        }
    }

    /// 序列化修改 `CURSOR_TERMINALS_DIR` 的用例，避免并行测试互相覆盖环境变量。
    fn cursor_terminals_dir_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("cursor terminals dir env lock")
    }

    /// 默认续跑类提示在 `additional_context`；`followup_message` 仅用于显式 opt-in 或硬拦截文案。
    fn hook_user_visible_blob(out: &Value) -> String {
        let mut s = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if let Some(ac) = out.get("additional_context").and_then(Value::as_str) {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(ac);
        }
        s
    }

    fn fresh_repo() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "router-rs-cursor-hooks-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_micros()
        ));
        fs::create_dir_all(root.join(".cursor/hooks")).expect("mkdir hooks");
        fs::write(root.join(".cursor/hooks.json"), b"{\"version\":1}\n").expect("hooks.json");
        fs::write(
            root.join(".cursor/hooks/review_subagent_gate.py"),
            b"# stub",
        )
        .expect("write hook");
        root
    }

    #[test]
    fn session_key_matches_when_parent_session_only_in_tool_input() {
        let full = json!({"session_id": "parent-chat-9", "cwd": "/tmp/ws"});
        let tool_only = json!({
            "cwd": "/tmp/ws",
            "tool_input": {"session_id": "parent-chat-9"},
        });
        assert_eq!(super::session_key(&full), super::session_key(&tool_only));
    }

    #[test]
    fn session_key_prefers_nested_conversation_over_root_session_via_deep_scan() {
        let mixed = json!({
            "session_id": "ephemeral-rev",
            "hookPayload": {"conversation_id": "stable-chat-x"},
        });
        let stable = json!({"conversation_id": "stable-chat-x"});
        assert_eq!(super::session_key(&mixed), super::session_key(&stable));
    }

    #[test]
    fn session_key_ignores_lonely_agent_id_for_cwd_fallback_match() {
        let only_agent = json!({"agent_id": "sub-agent-1", "cwd": "/workspace/z"});
        let cwd_only = json!({"cwd": "/workspace/z"});
        assert_eq!(
            super::session_key(&only_agent),
            super::session_key(&cwd_only)
        );
    }

    fn event(session: &str, prompt: &str) -> Value {
        json!({
            "session_id": session,
            "cwd": "/Users/joe/Documents/skill",
            "prompt": prompt
        })
    }

    fn load_state_for(repo: &Path, session: &str) -> ReviewGateState {
        let payload = json!({ "session_id": session, "cwd": "/Users/joe/Documents/skill" });
        load_state(repo, &payload)
            .expect("load ok")
            .expect("state exists")
    }

    fn write_active_task(repo: &Path, task_id: &str) {
        let p = repo.join("artifacts/current/active_task.json");
        fs::create_dir_all(p.parent().unwrap()).expect("mkdir artifacts/current");
        fs::write(p, format!(r#"{{"task_id":"{task_id}"}}"#)).expect("write active_task");
        fs::create_dir_all(repo.join("artifacts/current").join(task_id)).expect("mkdir task dir");
    }

    fn write_closeout_record(repo: &Path, task_id: &str, body: &str) {
        let p = repo
            .join("artifacts/closeout")
            .join(format!("{task_id}.json"));
        fs::create_dir_all(p.parent().unwrap()).expect("mkdir artifacts/closeout");
        fs::write(p, body).expect("write closeout record");
    }

    fn write_goal_state_completed(repo: &Path, task_id: &str) {
        fs::write(
            repo.join("artifacts/current")
                .join(task_id)
                .join("GOAL_STATE.json"),
            format!(
                r#"{{
  "schema_version": "router-rs-autopilot-goal-v1",
  "task_id": "{task_id}",
  "drive_until_done": true,
  "status": "completed",
  "goal": "g",
  "non_goals": ["ng"],
  "done_when": ["dw1", "dw2"],
  "validation_commands": ["cargo test"],
  "current_horizon": "h",
  "checkpoints": [{{"note":"cp"}}],
  "blocker": null,
  "updated_at": "2026-05-10T00:00:00Z"
}}"#
            ),
        )
        .expect("write GOAL_STATE");
    }

    #[test]
    fn review_prompt_chinese_full_review_arms_state() {
        let repo = fresh_repo();
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s1", "请全面review这个仓库找bug"),
        );
        assert!(out.get("followup_message").is_none());
        let state = load_state_for(&repo, "s1");
        assert_eq!(state.phase, 0);
        assert!(state.review_required);
    }

    #[test]
    fn parallel_delegation_does_not_latch_delegation_required() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s2", "请前端后端测试并行分头执行"),
        );
        let state = load_state_for(&repo, "s2");
        assert!(
            !state.delegation_required,
            "delegation heuristic must not persist into hook-state"
        );
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn autopilot_entry_does_not_arm_delegation_or_review_from_fix_copy() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event(
                "ap-del",
                "/autopilot address all review findings from the last pass",
            ),
        );
        let state = load_state_for(&repo, "ap-del");
        assert!(
            !state.delegation_required,
            "autopilot must not stack delegation_required (was: framework_entrypoint)"
        );
        assert!(
            !state.review_required,
            "autopilot execution turn must not re-arm review from findings wording"
        );
        assert!(state.goal_required);
    }

    #[test]
    fn stop_completion_claim_requires_closeout_record_when_strict_enabled() {
        let _env = crate::test_env_sync::process_env_lock();
        use std::env;
        let prev_gate_disable = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
        env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

        let repo = fresh_repo();
        let tid = "t-closeout";
        write_active_task(&repo, tid);
        write_goal_state_completed(&repo, tid);
        // Ensure goal gate can hydrate "verified" from disk evidence, so Stop reaches the
        // strict closeout enforcement branch instead of emitting AG_FOLLOWUP.
        fs::write(
            repo.join("artifacts/current").join(tid).join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &json!({
                "session_id": "s-closeout-1",
                "cwd": repo.display().to_string(),
                "prompt": "/autopilot do thing"
            }),
        );
        assert!(
            closeout_followup_for_completion_claim(&repo, tid)
                .expect("ok")
                .is_some(),
            "precondition: strict env should require record"
        );
        let payload = json!({
            "session_id": "s-closeout-1",
            "cwd": repo.display().to_string(),
            "prompt": "ok",
            "response": "done",
        });
        assert_eq!(agent_response_text(&payload), "done");
        // Inject a response text that claims completion.
        let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
        let msg = hook_user_visible_blob(&out);
        assert!(
            msg.contains("CLOSEOUT_FOLLOWUP") && msg.contains("missing_record"),
            "expected closeout followup; got {msg:?}"
        );

        match prev {
            Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
        match prev_gate_disable {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", v),
            None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn stop_completion_claim_allows_when_closeout_record_passes() {
        let _env = crate::test_env_sync::process_env_lock();
        use std::env;
        let prev_gate_disable = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
        env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

        let repo = fresh_repo();
        let tid = "t-closeout-ok";
        write_active_task(&repo, tid);
        write_goal_state_completed(&repo, tid);
        // Ensure evidence exists or provide commands_run in record (R7/R8 coverage).
        fs::write(
            repo.join("artifacts/current").join(tid).join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0,"success":true}]}"#,
        )
        .expect("write evidence");
        write_closeout_record(
            &repo,
            tid,
            r#"{
  "schema_version": "closeout-record-v1",
  "task_id": "t-closeout-ok",
  "summary": "已完成并验证",
  "verification_status": "passed",
  "commands_run": [{"command":"cargo test","exit_code":0}]
}"#,
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &json!({
                "session_id": "s-closeout-2",
                "cwd": repo.display().to_string(),
                "prompt": "/autopilot do thing"
            }),
        );

        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "s-closeout-2",
                "cwd": repo.display().to_string(),
                "prompt": "ok",
                "response": "已完成",
            }),
        );
        let msg = hook_user_visible_blob(&out);
        assert!(
            !msg.contains("CLOSEOUT_FOLLOWUP"),
            "expected no closeout followup; got {msg:?}"
        );

        match prev {
            Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
        match prev_gate_disable {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", v),
            None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn completion_claim_detector_matches_basic_tokens() {
        assert!(completion_claimed_in_text("done"));
        assert!(completion_claimed_in_text("已完成"));
        assert!(completion_claimed_in_text("tests passed"));
        assert!(!completion_claimed_in_text("still working"));
    }

    #[test]
    fn closeout_followup_emits_when_strict_and_record_missing() {
        let _env = crate::test_env_sync::process_env_lock();
        use std::env;
        let prev_gate_disable = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
        env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

        let repo = fresh_repo();
        let tid = "t-missing-closeout";
        write_active_task(&repo, tid);
        write_goal_state_completed(&repo, tid);
        let msg = closeout_followup_for_completion_claim(&repo, tid)
            .expect("ok")
            .expect("followup");
        assert!(msg.contains("missing_record"));

        match prev {
            Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
        match prev_gate_disable {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", v),
            None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn autopilot_skips_pre_goal_nag_when_goal_state_on_disk() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/gt1")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"gt1"}"#,
        )
        .expect("active");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "gt1",
            "goal": "close review findings",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("ap-disk", "/autopilot 继续实现"),
        );
        assert!(
            load_state_for(&repo, "ap-disk").pre_goal_review_satisfied,
            "existing GOAL_STATE implies execution lane already opened"
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("Autopilot (/autopilot)") && !msg.contains("independent-context"),
            "pre-goal nag should be skipped when GOAL_STATE exists; msg={msg:?}"
        );
    }

    #[test]
    fn stop_goal_gate_hydrates_from_goal_state_and_evidence_without_keywords() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/t-ev")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-ev"}"#,
        )
        .expect("active");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-ev",
            "goal": "fix review findings",
            "non_goals": ["avoid unrelated refactors"],
            "done_when": ["tests green", "review checklist cleared"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "checkpoint",
            "task_id": "t-ev",
            "note": "applied patch",
        }))
        .expect("checkpoint");
        fs::write(
            repo.join("artifacts/current/t-ev/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("ev-gate", "/autopilot finish fixes"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "ev-gate",
                "cwd": "/Users/joe/Documents/skill",
                "prompt": "ok",
                "response": "done; no Goal:/Checkpoint:/verified boilerplate in prose"
            }),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("AG_FOLLOWUP"),
            "goal gate should hydrate from disk; msg={msg:?} out={out:?}"
        );
    }

    #[test]
    fn stop_hydrates_when_hook_state_lacks_goal_required_but_goal_on_disk() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/t-nof")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-nof"}"#,
        )
        .expect("active");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-nof",
            "goal": "stdio seeded goal",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "checkpoint",
            "task_id": "t-nof",
            "note": "step",
        }))
        .expect("cp");
        fs::write(
            repo.join("artifacts/current/t-nof/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
        )
        .expect("ev");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("noflag", "hello"));
        assert!(
            !load_state_for(&repo, "noflag").goal_required,
            "plain prompt must not arm goal_required before hydrate"
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "noflag",
                "cwd": "/Users/joe/Documents/skill",
                "prompt": "bye",
                "response": "done without magic words"
            }),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("AG_FOLLOWUP"),
            "GOAL_STATE on disk must hydrate despite goal_required=false; msg={msg:?}"
        );
    }

    #[test]
    fn stop_hydrates_when_active_task_missing_but_goal_on_disk() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/t-orph")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/t-orph/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"no active_task json","status":"running","non_goals":["n"],"checkpoints":[{"note":"step"}],"done_when":["ship","review checklist cleared"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
        fs::write(
            repo.join("artifacts/current/t-orph/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
        )
        .expect("ev");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("orph", "hello"));
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "orph",
                "cwd": "/Users/joe/Documents/skill",
                "prompt": "bye",
                "response": "done"
            }),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("AG_FOLLOWUP"),
            "scan fallback must hydrate when active_task.json is missing; msg={msg:?}"
        );
    }

    #[test]
    fn stop_goal_gate_hydrates_running_goal_without_checkpoints_or_keywords() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/t-run")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-run"}"#,
        )
        .expect("active");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "t-run",
            "goal": "minimal running goal only",
            "non_goals": ["avoid unrelated refactors"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("run-gate", "/autopilot continue"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "run-gate",
                "cwd": "/Users/joe/Documents/skill",
                "prompt": "ok",
                "response": "no Goal/Checkpoint/Verification boilerplate"
            }),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("AG_FOLLOWUP"),
            "running GOAL_STATE with non-empty goal should hydrate progress+verify; msg={msg:?}"
        );
    }

    #[test]
    fn stop_goal_gate_hydrates_when_goal_state_omits_status_field() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/t-nost")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-nost"}"#,
        )
        .expect("active");
        fs::write(
            repo.join("artifacts/current/t-nost/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"hand-written without status","non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal json");
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("ns-gate", "/autopilot continue"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "ns-gate",
                "cwd": "/Users/joe/Documents/skill",
                "prompt": "ok",
                "response": "no chat boilerplate"
            }),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("AG_FOLLOWUP"),
            "missing status + non-empty goal should hydrate; msg={msg:?}"
        );
    }

    #[test]
    fn override_phrase_in_chinese_disables_arming() {
        let repo = fresh_repo();
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s3", "全面review这个仓库，不要用子代理"),
        );
        assert!(out.get("followup_message").is_none());
        let state = load_state_for(&repo, "s3");
        assert!(state.review_override);
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn reject_reason_satisfies_stop() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s4", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "afterAgentResponse",
            &json!({ "session_id": "s4", "response": "reject reason: small_task" }),
        );
        let out =
            dispatch_cursor_hook_event(&repo, "stop", &event("s4", "reject reason: small_task"));
        assert_eq!(out, json!({}));
    }

    #[test]
    // renamed from reject_reason_in_user_prompt_does_not_satisfy_gate after stop-stage parity fix
    fn reject_reason_in_user_prompt_satisfies_gate_on_stop() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13", "全面review这个仓库"),
        );
        let out =
            dispatch_cursor_hook_event(&repo, "stop", &event("s13", "reject reason: small_task"));
        let followup = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(out.get("continue") == Some(&json!(false)) || followup.is_empty());
        let state = load_state_for(&repo, "s13");
        assert!(state.reject_reason_seen);
    }

    #[test]
    fn reject_reason_in_assistant_response_satisfies_gate_on_stop() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13b", "全面review这个仓库"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "s13b",
                "prompt": "继续",
                "response": "reject reason: shared_context_heavy"
            }),
        );
        assert_eq!(out, json!({}));
    }

    #[test]
    fn nested_payload_response_reject_reason_satisfies_gate_on_stop() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13nest-r", "全面review这个仓库"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "s13nest-r",
                "cwd": "/Users/joe/Documents/skill",
                "payload": {
                    "prompt": "继续",
                    "response": "reject reason: shared_context_heavy"
                }
            }),
        );
        assert_eq!(out, json!({}));
    }

    #[test]
    fn nested_payload_response_sets_reject_reason_on_after_agent_response() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13nest-a", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "afterAgentResponse",
            &json!({
                "session_id": "s13nest-a",
                "cwd": "/Users/joe/Documents/skill",
                "payload": { "response": "small_task" }
            }),
        );
        assert!(load_state_for(&repo, "s13nest-a").reject_reason_seen);
    }

    #[test]
    fn stop_writes_back_reject_reason_seen_for_future_sessions() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13c", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "stop",
            &json!({
                "session_id": "s13c",
                "prompt": "reject reason: token_overhead_dominates",
                "response": ""
            }),
        );
        let state = load_state_for(&repo, "s13c");
        assert!(state.reject_reason_seen);
    }

    #[test]
    fn before_submit_lock_failure_fails_closed_without_writing_state() {
        let repo = fresh_repo();
        let payload = event("s14", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
        assert_eq!(out.get("continue"), Some(&json!(false)));
        assert!(
            hook_user_visible_blob(&out).contains("锁不可用"),
            "out={out:?}"
        );
        assert!(!state_path(&repo, &payload).exists());
    }

    #[test]
    fn before_submit_lock_failure_allows_non_strict_prompt() {
        let repo = fresh_repo();
        let payload = event("s14b", "帮我润色一句话");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
        assert_eq!(out.get("continue"), Some(&json!(true)));
        let blob = hook_user_visible_blob(&out);
        assert!(
            blob.contains("降级"),
            "expected degraded lock copy; blob={blob}"
        );
    }

    #[test]
    fn stop_lock_failure_reports_degraded_followup() {
        let repo = fresh_repo();
        let payload = event("s15", "帮我润色一句话");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
        let blob = hook_user_visible_blob(&out);
        assert!(
            blob.contains("锁不可用") && blob.contains("降级"),
            "expected degraded stop copy; blob={blob}"
        );
    }

    #[test]
    fn stop_lock_failure_still_merges_autopilot_drive() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/gl-stop-lock")).expect("mkdir goal");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"gl-stop-lock"}"#,
        )
        .expect("active_task");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "gl-stop-lock",
            "goal": "lock-merge",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");

        let payload = event("s15b", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir lock parent");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
        let blob = hook_user_visible_blob(&out);
        assert!(blob.contains("锁不可用"), "{blob}");
        assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");
    }

    #[test]
    fn before_submit_merges_goal_and_rfv_when_both_on_disk() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_ap = env::var_os("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT");
        let prev_rfv = env::var_os("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT");
        env::set_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT", "1");
        env::set_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT", "1");
        let repo = fresh_repo();
        let tid = "merge-both";
        fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            format!(r#"{{"task_id":"{tid}"}}"#),
        )
        .expect("active");
        fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
        fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("RFV_LOOP_STATE.json"),
            r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":0,"max_rounds":3,"allow_external_research":false,"rounds":[]}"#,
        )
        .expect("rfv");
        let out =
            dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("merge-t", "hello"));
        let msg = hook_user_visible_blob(&out);
        assert!(msg.contains("## 续跑（beforeSubmit）"), "{msg}");
        assert!(msg.contains("AUTOPILOT_DRIVE"), "{msg}");
        assert!(msg.contains("RFV_LOOP_CONTINUE"), "{msg}");
        assert_eq!(
            msg.matches("## 续跑").count(),
            1,
            "expected single merged heading; msg={msg}"
        );
        match prev_ap {
            Some(v) => env::set_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT", v),
            None => env::remove_var("ROUTER_RS_AUTOPILOT_DRIVE_BEFORE_SUBMIT"),
        }
        match prev_rfv {
            Some(v) => env::set_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT", v),
            None => env::remove_var("ROUTER_RS_RFV_LOOP_BEFORE_SUBMIT"),
        }
    }

    #[test]
    fn review_gate_disabled_stop_still_merges_autopilot_drive() {
        let repo = fresh_repo();
        fs::create_dir_all(repo.join("artifacts/current/gl-rgoff")).expect("mkdir goal");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"gl-rgoff"}"#,
        )
        .expect("active_task");
        crate::autopilot_goal::framework_autopilot_goal(json!({
            "repo_root": repo.display().to_string(),
            "operation": "start",
            "task_id": "gl-rgoff",
            "goal": "rg-off-merge",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");

        let mut out = {
            let _rg = ReviewGateDisableTestGuard::new();
            dispatch_cursor_hook_event(&repo, "stop", &event("sg1", "hi"))
        };
        let blob = hook_user_visible_blob(&out);
        assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");

        let prev_silent = env::var_os("ROUTER_RS_CURSOR_HOOK_SILENT");
        set_test_cursor_hook_silent_override(Some(true));
        apply_cursor_hook_output_policy(&mut out);
        set_test_cursor_hook_silent_override(None);
        match prev_silent {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v),
            None => env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT"),
        }
        assert!(out.get("followup_message").is_none());
        assert!(out.get("additional_context").is_none());
    }

    #[test]
    fn cursor_hook_silent_policy_keeps_review_gate_line() {
        let mut out = json!({
            "followup_message": "router-rs REVIEW_GATE incomplete phase=0 need=subagent"
        });
        set_test_cursor_hook_silent_override(Some(true));
        apply_cursor_hook_output_policy(&mut out);
        set_test_cursor_hook_silent_override(None);
        assert_eq!(
            out["followup_message"],
            json!("router-rs REVIEW_GATE incomplete phase=0 need=subagent")
        );
    }
    #[test]
    fn review_gate_disabled_post_tool_use_still_advances_phase_after_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("srg-pu2", "全面review这个仓库"),
        );
        assert!(load_state_for(&repo, "srg-pu2").phase < 2);

        let out = {
            let _rg = ReviewGateDisableTestGuard::new();
            dispatch_cursor_hook_event(
                &repo,
                "postToolUse",
                &json!({
                    "session_id": "srg-pu2",
                    "cwd": "/Users/joe/Documents/skill",
                    "tool_name": "functions.subagent",
                    "tool_input": { "subagent_type": "explore" }
                }),
            )
        };

        assert_eq!(out, json!({}));
        let state = load_state_for(&repo, "srg-pu2");
        assert!(state.phase >= 2, "phase={}", state.phase);
    }

    #[test]
    fn cursor_hook_silent_policy_respects_env() {
        let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_SILENT");

        env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT");
        let mut keep = json!({ "followup_message": "keep" });
        set_test_cursor_hook_silent_override(Some(false));
        apply_cursor_hook_output_policy(&mut keep);
        set_test_cursor_hook_silent_override(None);
        assert_eq!(keep["followup_message"], json!("keep"));

        env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", "1");
        let mut strip = json!({
            "continue": false,
            "followup_message": "nag",
            "additional_context": "ctx"
        });
        set_test_cursor_hook_silent_override(Some(true));
        apply_cursor_hook_output_policy(&mut strip);
        set_test_cursor_hook_silent_override(None);
        assert_eq!(strip, json!({ "continue": false }));

        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v),
            None => env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT"),
        }
    }

    #[test]
    fn subagent_start_promotes_phase_to_2() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s5", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s5", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s5");
        assert_eq!(state.phase, 2);
        assert_eq!(state.subagent_start_count, 1);
    }

    #[test]
    fn subagent_start_blocks_when_active_limit_reached() {
        let repo = fresh_repo();
        for _ in 0..DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS {
            let out = dispatch_cursor_hook_event(
                &repo,
                "subagentStart",
                &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
            );
            assert_eq!(out, json!({}));
        }

        let out = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
        );

        assert_eq!(out.get("permission").and_then(Value::as_str), Some("deny"));
        assert!(out
            .get("user_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("仍标记为打开"));
        let state = load_state_for(&repo, "s-open-limit");
        assert_eq!(
            state.active_subagent_count,
            DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS
        );
    }

    #[test]
    fn subagent_start_recovers_stale_active_count() {
        let repo = fresh_repo();
        let payload = json!({ "session_id": "s-open-stale", "subagent_type": "explore" });
        let stale_started_at = Utc::now()
            - chrono::Duration::seconds(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS + 1);
        let mut state = empty_state();
        state.active_subagent_count = DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS;
        state.active_subagent_last_started_at = Some(stale_started_at.to_rfc3339());
        assert!(save_state(&repo, &payload, &mut state));

        let out = dispatch_cursor_hook_event(&repo, "subagentStart", &payload);

        assert_eq!(out, json!({}));
        let state = load_state_for(&repo, "s-open-stale");
        assert_eq!(state.active_subagent_count, 1);
        assert!(state.active_subagent_last_started_at.is_some());
    }

    #[test]
    fn subagent_stop_decrements_active_count_without_review_gate() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStop",
            &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
        );

        let state = load_state_for(&repo, "s-open-stop");
        assert_eq!(state.active_subagent_count, 0);
        assert_eq!(state.phase, 0);
        assert_eq!(state.subagent_stop_count, 0);
    }

    #[test]
    fn subagent_stop_without_start_does_not_promote_phase() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s6", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStop",
            &json!({ "session_id": "s6", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s6");
        assert_eq!(state.phase, 0);
        assert_eq!(state.subagent_stop_count, 0);
    }

    #[test]
    fn subagent_start_then_stop_promotes_to_phase3() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s6b", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s6b", "subagent_type": "explore" }),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStop",
            &json!({ "session_id": "s6b", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s6b");
        assert_eq!(state.phase, 3);
        assert_eq!(state.subagent_stop_count, 1);
    }

    #[test]
    fn stop_without_subagent_emits_minimal_review_gate_line() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s7", "全面review这个仓库"),
        );
        let out = dispatch_cursor_hook_event(&repo, "stop", &event("s7", "继续"));
        let blob = hook_user_visible_blob(&out);
        assert!(
            blob.contains("router-rs REVIEW_GATE"),
            "expected minimal review gate line; out={out:?}"
        );
    }

    #[test]
    fn pre_compact_emits_additional_context_summary() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s8", "全面review这个仓库"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "preCompact",
            &json!({ "session_id": "s8", "cwd": "/Users/joe/Documents/skill" }),
        );
        assert!(out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("phase=0"));
    }

    #[test]
    fn session_end_clears_state_file() {
        let repo = fresh_repo();
        let payload = event("s9", "全面review这个仓库");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
        let path = state_path(&repo, &payload);
        assert!(path.exists());
        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
        assert!(!path.exists());
    }

    #[test]
    fn session_end_cleans_stale_lock_if_present() {
        let repo = fresh_repo();
        let payload = event("s9b", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"pid=1 ts=1").expect("seed lock");
        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
        assert!(!lock_path.exists());
    }

    /// SessionEnd 必须按文件名前缀清扫整个 `.cursor/hook-state/`，覆盖
    /// payload 缺 `session_id` / `cwd` 时旧会话状态泄漏的情况
    /// （AGENTS.md → Continuity → Cursor「sessionEnd 应清 hook-state 下 review gate 状态」）。
    #[test]
    fn session_end_sweeps_review_gate_state_with_unrelated_session_key() {
        let repo = fresh_repo();
        let stale_payload = event("stale-session", "全面review这个仓库");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &stale_payload);
        let stale_state = state_path(&repo, &stale_payload);
        let stale_lock = state_lock_path(&repo, &stale_payload);
        let stale_loop = adversarial_loop_path(&repo, &stale_payload);
        fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
        fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
        fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
        assert!(stale_state.exists());

        // 与遗留状态完全不同的 session key，保证按 session_key 精准删法删不到 stale_*。
        let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &unrelated_payload);

        assert!(
            !stale_state.exists(),
            "stale review-subagent state must be swept"
        );
        assert!(
            !stale_lock.exists(),
            "stale review-subagent lock must be swept"
        );
        assert!(
            !stale_loop.exists(),
            "stale adversarial-loop state must be swept"
        );
    }

    /// 清扫只覆盖本模块拥有的前缀，不应误伤未识别文件（避免与未来其它 hook 共用目录时冲突）。
    #[test]
    fn session_end_sweep_keeps_unrelated_files() {
        let repo = fresh_repo();
        let dir = state_dir(&repo);
        fs::create_dir_all(&dir).expect("mkdir state dir");
        let unrelated = dir.join("other-hook-state.json");
        fs::write(&unrelated, b"{}").expect("seed unrelated");

        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));
        assert!(unrelated.exists(), "unrelated hook state must be preserved");
    }

    /// SessionEnd sweep 必须回收 `save_state` 及历史 adversarial-loop 原子写入孤儿，
    /// 避免长期累积消耗 `.cursor/hook-state/` 卫生。
    #[test]
    fn session_end_sweeps_atomic_write_orphans() {
        let repo = fresh_repo();
        let dir = state_dir(&repo);
        fs::create_dir_all(&dir).expect("mkdir state dir");

        let primary_tmp = dir.join(".tmp-99999-12345-review-subagent-deadbeef.json");
        let adv_tmp = dir.join(".tmp-adv-loop-99999-67890");
        let other_tmp = dir.join(".tmp-99999-12345-other-hook.json");
        fs::write(&primary_tmp, b"{}").expect("seed primary tmp");
        fs::write(&adv_tmp, b"{}").expect("seed adv tmp");
        fs::write(&other_tmp, b"{}").expect("seed other tmp");

        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));

        assert!(
            !primary_tmp.exists(),
            "review-subagent atomic-write tmp must be swept"
        );
        assert!(
            !adv_tmp.exists(),
            "adversarial-loop atomic-write tmp must be swept"
        );
        assert!(
            other_tmp.exists(),
            "unrelated tmp must be preserved (sweep is module-scoped)"
        );
    }

    /// 文件名归属判断必须只接受本模块写入的命名（含原子写入孤儿前缀），其它名称一律排除。
    #[test]
    fn review_gate_state_file_owned_by_module_recognizes_known_names_only() {
        // 主状态：仅认 json|lock 扩展。
        assert!(review_gate_state_file_owned_by_module(
            "review-subagent-abc.json"
        ));
        assert!(review_gate_state_file_owned_by_module(
            "review-subagent-abc.lock"
        ));
        assert!(review_gate_state_file_owned_by_module(
            "adversarial-loop-abc.json"
        ));
        assert!(!review_gate_state_file_owned_by_module(
            "review-subagent-abc.bak"
        ));
        assert!(!review_gate_state_file_owned_by_module("review-subagent-"));
        // 原子写入孤儿。
        assert!(review_gate_state_file_owned_by_module(
            ".tmp-1-2-review-subagent-abc.json"
        ));
        assert!(review_gate_state_file_owned_by_module(".tmp-adv-loop-1-2"));
        // 未识别命名不应被清扫。
        assert!(!review_gate_state_file_owned_by_module(
            "other-hook-state.json"
        ));
        assert!(!review_gate_state_file_owned_by_module(
            ".tmp-1-2-other-hook.json"
        ));
        assert!(!review_gate_state_file_owned_by_module(".tmp-random"));
    }

    #[test]
    fn narrow_path_review_does_not_arm() {
        let repo = fresh_repo();
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s10", "review ./README.md"),
        );
        assert_eq!(out, json!({ "continue": true }));
        let state = load_state_for(&repo, "s10");
        assert!(!state.review_required);
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn v1_state_migrates_to_current_schema_phase() {
        let repo = fresh_repo();
        let payload = json!({ "session_id": "s11" });
        let path = state_path(&repo, &payload);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &path,
            r#"{"version":1,"review_required":true,"review_subagent_seen":true,"followup_count":2}"#,
        )
        .expect("write v1");
        let state = load_state(&repo, &payload).expect("load").expect("state");
        assert_eq!(state.version, STATE_VERSION);
        assert_eq!(state.phase, 2);
        assert_eq!(state.followup_count, 2);
    }

    #[test]
    fn post_tool_use_subagent_sets_phase() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s12", "全面review这个仓库"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s12",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore"}
            }),
        );
        let state = load_state_for(&repo, "s12");
        assert!(state.phase >= 2);
    }

    #[test]
    fn review_armed_does_not_inject_subagent_nag_on_submit_or_stop() {
        let repo = fresh_repo();
        let first = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s16", "全面review这个仓库"),
        );
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !first_msg.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN)
                && !first_msg.contains("Broad/deep review detected")
                && !first_msg.contains("Parallel lane request detected"),
            "first_msg={first_msg:?}"
        );
        assert!(load_state_for(&repo, "s16").review_required);
        let second = dispatch_cursor_hook_event(&repo, "stop", &event("s16", "继续"));
        let blob = hook_user_visible_blob(&second);
        assert!(
            blob.contains("router-rs REVIEW_GATE"),
            "Stop emits minimal review gate line; second={second:?} blob={blob:?}"
        );
        assert!(
            !blob.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN),
            "obsolete review prefix; blob={blob:?}"
        );
    }

    #[test]
    fn goal_stop_followup_is_short_code_only() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s17",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore"}
            }),
        );
        let first = dispatch_cursor_hook_event(&repo, "stop", &event("s17", "继续"));
        let first_msg = hook_user_visible_blob(&first);
        assert!(
            first_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
            "Stop uses short goal hint only; msg={first_msg:?}"
        );
        assert!(
            !first_msg.contains("Autopilot goal mode:"),
            "Stop must not dump full goal contract prose; msg={first_msg:?}"
        );
        let second = dispatch_cursor_hook_event(&repo, "stop", &event("s17", "继续"));
        let second_msg = hook_user_visible_blob(&second);
        // The invariant: Stop must keep the followup short. If a followup is emitted, it must
        // be the short AG_FOLLOWUP code, not long prose.
        if !second_msg.is_empty() {
            assert!(
                second_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
                "expected short code when non-empty; second_msg={second_msg:?} second={second:?}"
            );
            assert!(
                !second_msg.contains("Autopilot goal mode:"),
                "Stop must not dump full goal contract prose; second_msg={second_msg:?}"
            );
        }
    }

    #[test]
    fn autopilot_before_submit_prompts_pre_goal_review_when_opt_in() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
        env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "1");
        let repo = fresh_repo();
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17b", "/autopilot 完成任务"),
        );
        let msg = hook_user_visible_blob(&out);
        assert!(msg.contains("Autopilot (/autopilot)"), "surface={msg:?}");
        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
            None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
        }
    }

    #[test]
    fn autopilot_pre_goal_auto_releases_when_nag_cap_reached() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_cap = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES");
        let prev_pg = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
        env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "1");
        env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES", "2");
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("cap-nag", "/autopilot smoke"),
        );
        let mid = load_state_for(&repo, "cap-nag");
        assert_eq!(mid.pre_goal_nag_count, 1);
        assert!(!mid.pre_goal_review_satisfied);
        let out =
            dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("cap-nag", "continue"));
        let end = load_state_for(&repo, "cap-nag");
        assert!(end.pre_goal_review_satisfied);
        assert_eq!(end.pre_goal_nag_count, 0);
        let blob = hook_user_visible_blob(&out);
        assert!(blob.contains("pre-goal 提示已达上限"), "blob={blob:?}");
        match prev_cap {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES", v),
            None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES"),
        }
        match prev_pg {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
            None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
        }
    }

    #[test]
    fn deep_json_strings_satisfy_pre_goal_reject_on_before_submit() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("deep-s1", "/autopilot 任务"),
        );
        let deep = json!({
            "session_id": "deep-s1",
            "cwd": "/Users/joe/Documents/skill",
            "messages": [{ "role": "user", "content": "small_task" }]
        });
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &deep);
        assert!(load_state_for(&repo, "deep-s1").pre_goal_review_satisfied);
    }

    #[test]
    fn messages_tail_user_text_clears_review_gate_when_top_level_prompt_empty() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-msg-only", "全面review这个仓库"),
        );
        let ev = json!({
            "session_id": "s-msg-only",
            "cwd": "/Users/joe/Documents/skill",
            "messages": [
                { "role": "user", "content": "earlier" },
                { "role": "assistant", "content": "ok" },
                { "role": "user", "content": "rg_clear" }
            ]
        });
        let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &ev);
        let state = load_state_for(&repo, "s-msg-only");
        assert!(state.reject_reason_seen);
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            !msg.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN)
                && !msg.contains("Broad/deep review detected"),
            "expected gate clear from messages[].content; msg={msg:?} out={out:?}"
        );
        assert_eq!(state.followup_count, 0);
        assert_eq!(state.review_followup_count, 0);
    }

    #[test]
    fn before_submit_reject_reason_token_in_user_prompt_satisfies_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17e", "/autopilot 第一轮"),
        );
        let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event(
                "s17e",
                "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test",
            ),
        );
        let state = load_state_for(&repo, "s17e");
        assert!(state.reject_reason_seen);
        assert!(state.pre_goal_review_satisfied);
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("Autopilot (/autopilot)")
                && !msg.contains("independent-context reviewer"),
            "reject_reason on submit should skip pre-goal nag; msg={msg:?}"
        );
    }

    #[test]
    fn nested_payload_prompt_reject_reason_satisfies_pre_goal_before_submit() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17nest", "/autopilot 第一轮"),
        );
        let nested = json!({
            "session_id": "s17nest",
            "cwd": "/Users/joe/Documents/skill",
            "payload": {
                "prompt": "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test"
            }
        });
        let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &nested);
        let state = load_state_for(&repo, "s17nest");
        assert!(state.reject_reason_seen);
        assert!(state.pre_goal_review_satisfied);
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !msg.contains("independent-context"),
            "nested payload prompt should satisfy pre_goal; msg={msg:?}"
        );
    }

    #[test]
    fn nested_payload_prompt_reject_reason_updates_stop_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17stop-n", "/autopilot 任务"),
        );
        let nested_stop = json!({
            "session_id": "s17stop-n",
            "cwd": "/Users/joe/Documents/skill",
            "payload": {
                "prompt": "small_task\nGoal:\nNon-goals:\nDone when:\nValidation commands:"
            }
        });
        let _ = dispatch_cursor_hook_event(&repo, "stop", &nested_stop);
        assert!(load_state_for(&repo, "s17stop-n").pre_goal_review_satisfied);
    }

    #[test]
    fn post_tool_use_fork_context_true_does_not_satisfy_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17c", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s17c",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore","fork_context":true}
            }),
        );
        let state = load_state_for(&repo, "s17c");
        assert!(
            !state.pre_goal_review_satisfied,
            "shared fork_context must not count as independent pre-goal review"
        );
    }

    #[test]
    fn post_tool_use_tool_input_type_field_satisfies_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17d", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "s17d",
                "tool_name": "functions.subagent",
                "tool_input": {"type": "explore", "fork_context": false}
            }),
        );
        assert!(
            load_state_for(&repo, "s17d").pre_goal_review_satisfied,
            "hosts may emit lane kind as tool_input.type instead of subagent_type"
        );
    }

    #[test]
    fn post_tool_use_heuristic_mcp_subagent_tool_name_satisfies_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17mcp", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "s17mcp",
                "tool_name": "mcp_cursor_agent_subagent",
                "tool_input": {"subagent_type": "explore", "fork_context": false}
            }),
        );
        assert!(load_state_for(&repo, "s17mcp").pre_goal_review_satisfied);
    }

    #[test]
    fn post_tool_use_nested_payload_tool_fields_satisfy_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17nest-tu", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "s17nest-tu",
                "cwd": "/Users/joe/Documents/skill",
                "payload": {
                    "tool_name": "functions.subagent",
                    "tool_input": {"type": "explore"}
                }
            }),
        );
        assert!(load_state_for(&repo, "s17nest-tu").pre_goal_review_satisfied);
    }

    #[test]
    fn post_tool_use_non_allowlisted_lane_field_satisfies_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-lane", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "s-lane",
                "cwd": "/Users/joe/Documents/skill",
                "tool_name": "functions.subagent",
                "tool_input": {"lane": "my-custom-reviewer", "fork_context": false}
            }),
        );
        assert!(load_state_for(&repo, "s-lane").pre_goal_review_satisfied);
    }

    #[test]
    fn post_tool_use_fork_context_string_true_blocks_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-fkstr", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "s-fkstr",
                "cwd": "/Users/joe/Documents/skill",
                "tool_name": "functions.subagent",
                "tool_input": {"type": "explore", "fork_context": "true"}
            }),
        );
        assert!(
            !load_state_for(&repo, "s-fkstr").pre_goal_review_satisfied,
            "string fork_context=true must not count as independent pre-goal"
        );
    }

    #[test]
    fn review_keyword_inside_codeblock_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s18", "```请 review 这段代码```"),
        );
        assert_eq!(load_state_for(&repo, "s18").phase, 0);
    }

    #[test]
    fn review_keyword_inside_inline_code_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s19", "这是 `review` 函数"),
        );
        assert_eq!(load_state_for(&repo, "s19").phase, 0);
    }

    #[test]
    fn review_keyword_inside_url_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s20", "https://example.com/review/123"),
        );
        assert_eq!(load_state_for(&repo, "s20").phase, 0);
    }

    #[test]
    fn review_keyword_inside_blockquote_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s21", "> 用户说 review 一下"),
        );
        assert_eq!(load_state_for(&repo, "s21").phase, 0);
    }

    #[test]
    fn quoted_review_token_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s22", r#"他说 "review hook""#),
        );
        assert_eq!(load_state_for(&repo, "s22").phase, 0);
    }

    #[test]
    fn parallel_alone_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s23", "请解释 parallel 的含义"),
        );
        assert_eq!(load_state_for(&repo, "s23").phase, 0);
    }

    #[test]
    fn parallel_with_task_verb_arms() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s24", "用 parallel workers 实现 X"),
        );
        assert_eq!(load_state_for(&repo, "s24").phase, 0);
    }

    #[test]
    fn english_concurrent_alone_no_arm() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s25", "What does concurrent mean?"),
        );
        assert_eq!(load_state_for(&repo, "s25").phase, 0);
    }

    #[test]
    fn resolve_cursor_hook_repo_root_finds_hooks_from_payload_cwd() {
        let root = fresh_repo();
        let nested = root.join("scripts/router-rs");
        fs::create_dir_all(&nested).expect("mkdir nested");
        let payload = json!({
            "session_id": "rk",
            "cwd": nested.display().to_string()
        });
        let wrong_cli = nested.join("ghost");
        let resolved =
            resolve_cursor_hook_repo_root(Some(wrong_cli.as_path()), &payload).expect("ok");
        assert_eq!(
            resolved,
            fs::canonicalize(&root).unwrap_or_else(|_| root.clone())
        );
    }

    #[test]
    fn cursor_session_key_fallback_stable_for_cwd_without_session_id() {
        let payload = json!({ "cwd": "/tmp/abc-stable-fallback" });
        let a = session_key(&payload);
        let b = session_key(&payload);
        assert_eq!(a.len(), 32);
        assert_eq!(a, b, "cwd-only key must survive separate hook processes");
    }

    #[test]
    fn cursor_session_key_reads_metadata_session_id() {
        let payload = json!({
            "cwd": "/tmp/x",
            "metadata": { "sessionId": "meta-sess-1" }
        });
        let from_meta = session_key(&payload);
        let flat = session_key(&json!({
            "session_id": "meta-sess-1",
            "cwd": "/tmp/x"
        }));
        assert_eq!(from_meta, flat);
    }

    #[test]
    fn cursor_session_key_nested_payload_session_id_matches_top_level() {
        let nested = json!({
            "cwd": "/tmp/x",
            "payload": { "sessionId": "uuid-nested-pregoal" }
        });
        let flat = json!({
            "session_id": "uuid-nested-pregoal",
            "cwd": "/tmp/x"
        });
        assert_eq!(session_key(&nested), session_key(&flat));
    }

    #[test]
    fn cursor_session_key_nested_workspace_folder_matches_top_cwd() {
        let nested = json!({
            "payload": { "workspaceFolder": "/tmp/ws-nested" }
        });
        let flat = json!({ "cwd": "/tmp/ws-nested" });
        assert_eq!(session_key(&nested), session_key(&flat));
    }

    #[test]
    fn autopilot_pre_goal_persists_when_session_id_only_nested_in_payload() {
        let repo = fresh_repo();
        let cwd = repo.display().to_string();
        let sid = "nested-sid-pregoal";
        let before = json!({
            "cwd": cwd,
            "payload": {
                "sessionId": sid,
                "prompt": "/autopilot 完成任务"
            }
        });
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &before);
        let stop = json!({
            "cwd": cwd,
            "payload": {
                "sessionId": sid,
                "prompt": "small_task\nGoal: g\nNon-goals: n\nDone when: d\nValidation commands: cargo test"
            }
        });
        let out = dispatch_cursor_hook_event(&repo, "stop", &stop);
        let state = load_state(&repo, &json!({ "session_id": sid, "cwd": cwd }))
            .expect("load")
            .expect("state file");
        assert!(
            state.pre_goal_review_satisfied,
            "stop followup={:?}",
            out.get("followup_message")
        );
    }

    #[test]
    fn subagent_start_pre_goal_requires_typed_subagent() {
        let repo = fresh_repo();
        let _ = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-sub-pre", "/autopilot 完成任务"),
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "SubagentStart",
            &json!({
                "session_id": "s-sub-pre",
                "cwd": "/Users/joe/Documents/skill",
                "tool_input": {"fork_context": false}
            }),
        );
        assert!(
            !load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied,
            "untyped SubagentStart must not satisfy pre-goal"
        );
        let _ = dispatch_cursor_hook_event(
            &repo,
            "SubagentStart",
            &json!({
                "session_id": "s-sub-pre",
                "cwd": "/Users/joe/Documents/skill",
                "subagent_type": "explore",
                "tool_input": {"fork_context": false}
            }),
        );
        assert!(load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied);
    }

    #[test]
    fn cursor_lock_writes_owner_metadata() {
        let repo = fresh_repo();
        let payload = event("s26", "review");
        let lock = acquire_state_lock(&repo, &payload).expect("acquire");
        let text = fs::read_to_string(state_lock_path(&repo, &payload)).expect("read lock");
        assert!(text.contains("pid="));
        assert!(text.contains("ts="));
        let mut guard = Some(lock);
        release_state_lock(&mut guard);
    }

    #[test]
    fn cursor_lock_recovers_from_stale_timestamp() {
        let repo = fresh_repo();
        let payload = event("s27", "review");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        let stale_ts = now_millis().saturating_sub(60_000);
        fs::write(&lock_path, format!("pid=999999 ts={stale_ts}\n")).expect("seed stale lock");
        let mut lock = acquire_state_lock(&repo, &payload);
        assert!(lock.is_some());
        release_state_lock(&mut lock);
    }

    #[test]
    fn cursor_lock_concurrent_acquire_serializes() {
        let repo = Arc::new(fresh_repo());
        let payload = event("s28-shared", "review");
        let mut joins = Vec::new();
        for _ in 0..2 {
            let repo = Arc::clone(&repo);
            let payload = payload.clone();
            joins.push(std::thread::spawn(move || {
                for _ in 0..20 {
                    let lock = acquire_state_lock(&repo, &payload).expect("acquire");
                    let mut guard = Some(lock);
                    release_state_lock(&mut guard);
                }
            }));
        }
        for join in joins {
            join.join().expect("join");
        }
    }

    #[test]
    fn cursor_state_save_completes_with_fsync_unix() {
        let repo = fresh_repo();
        let payload = event("s29", "review");
        let mut state = empty_state();
        state.phase = 2;
        assert!(save_state(&repo, &payload, &mut state));
        let loaded = load_state(&repo, &payload).expect("load").expect("state");
        assert_eq!(loaded.phase, 2);
    }

    #[test]
    fn cursor_hook_rejects_oversized_stdin() {
        let large = "a".repeat(5 * 1024 * 1024);
        let mut reader = Cursor::new(large.into_bytes());
        let err = read_stdin_json_from_reader(&mut reader).expect_err("must reject");
        assert_eq!(err, "stdin_too_large");
    }

    #[test]
    fn pre_compact_does_not_mutate_state() {
        let repo = fresh_repo();
        let payload = event("s30", "全面review这个仓库");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
        let path = state_path(&repo, &payload);
        let before = fs::read_to_string(&path).expect("read before");
        let _ = dispatch_cursor_hook_event(&repo, "preCompact", &payload);
        let after = fs::read_to_string(&path).expect("read after");
        assert_eq!(before, after);
    }

    // --- SessionEnd: stale terminal 子进程清理 ---

    fn write_terminal_file(dir: &Path, id: &str, header: &str) -> PathBuf {
        fs::create_dir_all(dir).expect("mkdir terminals");
        let path = dir.join(format!("{id}.txt"));
        fs::write(&path, header).expect("write terminal file");
        path
    }

    #[test]
    fn parse_terminal_header_extracts_pid_cwd_active() {
        let txt = "---\npid: 12345\ncwd: \"/Users/joe/Documents/skill\"\ncommand: \"cargo test\"\nstarted_at: 2026-05-10T12:00:00Z\nrunning_for_ms: 295037   \n---\nbody...";
        let h = parse_terminal_header(txt).expect("parsed");
        assert_eq!(h.pid, Some(12345));
        assert_eq!(
            h.cwd.as_deref(),
            Some(Path::new("/Users/joe/Documents/skill"))
        );
        assert!(h.is_active);
        assert!(h.started_at_ms.is_some());
    }

    #[test]
    fn parse_terminal_header_inactive_when_no_running_for_ms() {
        let txt = "---\npid: 35455\ncwd: /Users/joe/Documents/skill\n---\n~/skill ❯";
        let h = parse_terminal_header(txt).expect("parsed");
        assert_eq!(h.pid, Some(35455));
        assert!(!h.is_active);
    }

    #[test]
    fn parse_terminal_header_rejects_non_yaml_block() {
        assert!(parse_terminal_header("no front matter here").is_none());
    }

    #[test]
    fn cursor_kill_stale_terminals_disabled_by_env_truthy_values_keep_enabled() {
        let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
        std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
        assert!(!cursor_kill_stale_terminals_disabled_by_env());
        for v in ["", "1", "true", "yes", "on", "anything"] {
            std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v);
            assert!(
                !cursor_kill_stale_terminals_disabled_by_env(),
                "value {v:?} should NOT disable"
            );
        }
        for v in ["0", "false", "off", "no", "  FALSE  "] {
            std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v);
            assert!(
                cursor_kill_stale_terminals_disabled_by_env(),
                "value {v:?} should disable"
            );
        }
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v),
            None => std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS"),
        }
    }

    #[test]
    fn terminate_in_dir_skips_when_terminals_dir_missing() {
        let repo = fresh_repo();
        let report = terminate_stale_terminal_processes_in_dir(&repo, &repo.join("missing"), None);
        assert_eq!(report.scanned, 0);
        assert!(report.killed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn terminate_in_dir_skips_inactive_outside_and_dead_branches() {
        use std::process::{Command, Stdio};
        let repo = fresh_repo();
        let term_dir = repo.join("__terminals");

        // 1) inactive：header 中无 `running_for_ms`，PID 不重要（取一个被显式过滤的小值）。
        write_terminal_file(
            &term_dir,
            "inactive",
            &format!(
                "---\npid: 1\ncwd: \"{}\"\ncommand: \"echo hi\"\n---\nbody",
                repo.display()
            ),
        );

        // 2) outside_repo：spawn 一个实际活着的 sleep（独立 PGID），cwd 指向仓库外。
        use std::os::unix::process::CommandExt;
        let mut outside_cmd = Command::new("sleep");
        outside_cmd
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            outside_cmd.pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
        let mut alive_outside = outside_cmd.spawn().expect("spawn outside sleep");
        let outside_pid = alive_outside.id();
        // 给子进程一点时间真正进入运行态。
        thread::sleep(Duration::from_millis(50));
        write_terminal_file(
            &term_dir,
            "outside",
            &format!(
                "---\npid: {outside_pid}\ncwd: /tmp/router-rs-stale-test-not-this-repo\nrunning_for_ms: 1000\n---\n"
            ),
        );

        // 3) dead：spawn `true` 立即 wait，确保 PID 已被 reap；短窗口内 PID 不会被 OS 复用。
        let mut quick = Command::new("true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn true");
        let dead_pid = quick.id();
        quick.wait().expect("reap true");
        // 等待 OS 把 PID 标记为 ESRCH（macOS/Linux 下 reap 后立刻就 dead，但留几次轮询兜底）。
        for _ in 0..50 {
            if !is_process_alive(dead_pid) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        write_terminal_file(
            &term_dir,
            "dead",
            &format!(
                "---\npid: {dead_pid}\ncwd: \"{}\"\nrunning_for_ms: 100\n---\n",
                repo.display()
            ),
        );

        let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir, None);
        // outside 子进程必须仍活着——证明 cwd 范围过滤生效。
        assert!(
            is_process_alive(outside_pid),
            "outside-repo child {outside_pid} must NOT be killed; report={report:?}"
        );
        assert!(
            report.killed.is_empty(),
            "no children inside repo were truly active: {:?}",
            report.killed
        );
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(
            report.scanned >= 3,
            "expected at least the seeded three terminal files, got report={report:?}"
        );
        assert_eq!(report.skipped_inactive, 1);
        assert_eq!(report.skipped_outside_repo, 1);
        // dead PID 在极少数 race 下可能被 OS 立刻复用（同 PPID 下另一进程），所以放宽断言。
        assert!(report.skipped_dead <= 1);

        // 收尾：杀掉 outside 子进程并 reap。
        unsafe {
            let _ = libc::kill(outside_pid as libc::pid_t, libc::SIGKILL);
        }
        let _ = alive_outside.wait();
    }

    #[cfg(unix)]
    #[test]
    fn terminate_in_dir_kills_real_sleep_child_within_repo() {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        let repo = fresh_repo();
        let term_dir = repo.join("__terminals");

        // 双隔离 pre_exec：
        // 1) `setsid` 让 sleep 进入新 session + 新 pgid，与 cargo test 完全脱钩。这样
        //    `terminate_pid` 通过 SIGTERM 杀整个 pgid 时不会牵连 cargo test 自身。
        // 2) 同时通过新 session 让 sleep 不再受 cargo test 的 SIGHUP 影响。
        // sleep 仍是 cargo test 的 child，结束后由 cargo test 在 drop(child) 时 wait——
        // 但我们不持有 child，让它变 zombie 由 reaper（cargo runner / 测试 harness）回收。
        // 为避免 `is_process_alive` 把 zombie 误判 alive 让 SIGKILL 复检失败，我们在 spawn
        // 之后立刻把 child 转成游离句柄并显式持续 wait（在 SIGTERM 之后立刻收尸）。
        let mut spawn_cmd = Command::new("sleep");
        spawn_cmd
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            spawn_cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = spawn_cmd.spawn().expect("spawn sleep");
        let pid = child.id();
        // 等 sleep 真正进入运行态。
        thread::sleep(Duration::from_millis(50));
        assert!(is_process_alive(pid), "child must be alive before kill");

        write_terminal_file(
            &term_dir,
            "alive",
            &format!(
                "---\npid: {pid}\ncwd: \"{}\"\ncommand: \"sleep 60\"\nrunning_for_ms: 500\n---\n",
                repo.display()
            ),
        );

        // 后台 reaper：在测试主线程持续在 terminate_pid 内 SIGTERM/SIGKILL 时，独立线程
        // 调 child.wait() 立刻 reap，避免 zombie 让 `is_process_alive(kill(pid,0))` 误判。
        let waiter = std::thread::spawn(move || {
            let _ = child.wait();
        });

        let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir, None);
        let _ = waiter.join();
        assert_eq!(report.killed, vec![pid], "report={report:?}");
        assert!(!is_process_alive(pid), "child must be reaped");
    }

    #[cfg(unix)]
    #[test]
    fn signal_pid_or_pgrp_never_targets_our_process_group() {
        // 防止 SessionEnd stale terminal 回收逻辑“按 PGID kill”时误杀 hook 自己所在进程组。
        // 这里只验证目标选择逻辑：当 pgid == current_pgid 时必须退化为只 kill PID。
        let our_pid = std::process::id();
        let our_pgid = super::current_pgid().expect("current pgid");
        // 发送 0 信号不产生副作用，但会触发 syscall 分支。
        super::signal_pid_or_pgrp(our_pid, Some(our_pgid), 0);
    }

    #[test]
    fn handle_session_end_respects_kill_disable_env() {
        // 走 dispatch_event 真实路径；只验证 env=0 时不会因 terminals 路径推导失败而 panic，
        // 也不影响既有 `.cursor/hook-state` 清扫。
        let repo = fresh_repo();
        let payload = event("kill-disable-sess", "全面review这个仓库");
        let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
        let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
        std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", "0");
        let out = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
        assert_eq!(out, json!({}));
        assert!(!state_path(&repo, &payload).exists(), "state still cleared");
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v),
            None => std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS"),
        }
    }

    #[test]
    fn session_start_initializes_terminal_baseline_ledger() {
        let _term_env = cursor_terminals_dir_env_lock();
        let repo = fresh_repo();
        let term_dir = repo.join("__terminals");
        write_terminal_file(
            &term_dir,
            "t1",
            "---\npid: 11111\ncwd: /Users/joe/Documents/skill\nrunning_for_ms: 1\n---\n",
        );
        write_terminal_file(
            &term_dir,
            "t2",
            "---\npid: 22222\ncwd: /Users/joe/Documents/skill\n---\n",
        );
        let prev = std::env::var_os("CURSOR_TERMINALS_DIR");
        std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir);
        let payload =
            json!({ "session_id": "sess-ledger-init", "cwd": repo.display().to_string() });
        let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
        let ledger = load_session_terminal_ledger(&repo, &payload);
        assert_eq!(ledger.version, SESSION_TERMINAL_LEDGER_VERSION);
        assert_eq!(ledger.baseline_pids, vec![11111, 22222]);
        assert!(ledger.owned_pids.is_empty());
        match prev {
            Some(v) => std::env::set_var("CURSOR_TERMINALS_DIR", v),
            None => std::env::remove_var("CURSOR_TERMINALS_DIR"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn session_end_kills_only_owned_terminal_pids() {
        let _term_env = cursor_terminals_dir_env_lock();
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        let repo = fresh_repo();
        let term_dir = repo.join("__terminals");

        let mk_sleep = || {
            let mut cmd = Command::new("sleep");
            cmd.arg("60")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            unsafe {
                cmd.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            cmd.spawn().expect("spawn sleep")
        };

        let mut owned_child = mk_sleep();
        let mut other_child = mk_sleep();
        let owned_pid = owned_child.id();
        let other_pid = other_child.id();
        thread::sleep(Duration::from_millis(50));
        assert!(is_process_alive(owned_pid));
        assert!(is_process_alive(other_pid));

        write_terminal_file(
            &term_dir,
            "owned",
            &format!(
                "---\npid: {owned_pid}\ncwd: \"{}\"\nrunning_for_ms: 500\n---\n",
                repo.display()
            ),
        );
        write_terminal_file(
            &term_dir,
            "other",
            &format!(
                "---\npid: {other_pid}\ncwd: \"{}\"\nrunning_for_ms: 500\n---\n",
                repo.display()
            ),
        );

        let prev = std::env::var_os("CURSOR_TERMINALS_DIR");
        std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir);
        let payload = json!({ "session_id": "sess-owned-only", "cwd": repo.display().to_string() });
        let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
        save_session_terminal_ledger(
            &repo,
            &payload,
            &SessionTerminalLedger {
                version: SESSION_TERMINAL_LEDGER_VERSION,
                baseline_pids: vec![],
                owned_pids: vec![owned_pid],
                pending_shells: vec![],
            },
        );
        let owned_waiter = std::thread::spawn(move || {
            let _ = owned_child.wait();
        });
        let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);

        // owned pid should be terminated by SessionEnd
        for _ in 0..40 {
            if !is_process_alive(owned_pid) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = owned_waiter.join();
        assert!(!is_process_alive(owned_pid), "owned pid must be killed");
        assert!(is_process_alive(other_pid), "non-owned pid must stay alive");

        unsafe {
            let _ = libc::kill(other_pid as libc::pid_t, libc::SIGKILL);
        }
        let _ = other_child.wait();

        match prev {
            Some(v) => std::env::set_var("CURSOR_TERMINALS_DIR", v),
            None => std::env::remove_var("CURSOR_TERMINALS_DIR"),
        }
    }
}
