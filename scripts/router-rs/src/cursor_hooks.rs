use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
    static TEST_CURSOR_REVIEW_GATE_DISABLE: Cell<Option<bool>> = Cell::new(None);
}

#[cfg(test)]
pub(crate) fn set_test_review_gate_disable_override(v: Option<bool>) {
    TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.set(v));
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
    let a = build_autopilot_drive_followup_using_frame(repo_root, frame);
    let b = build_rfv_loop_followup_using_frame(repo_root, frame);
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
    match output.get_mut(field) {
        Some(Value::String(existing)) => {
            let cleaned = strip_before_submit_continuity_paragraphs(existing);
            *existing = if cleaned.is_empty() {
                msg
            } else {
                format!("{cleaned}\n\n{msg}")
            };
        }
        _ => {
            if let Some(obj) = output.as_object_mut() {
                obj.insert(field.to_string(), Value::String(msg));
            }
        }
    }
}

pub const STATE_VERSION: u32 = 3;

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("invalid regex"))
        .collect()
}

fn review_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)\b(code|security|architecture|architect)\s+review\b",
            r"(?i)\breview\s+this\s+(pr|pull request)\b",
            r"(?i)\breview\s+(my\s+)?(pr|pull request)\b",
            r"(?i)\b(pr|pull request)\s+review\b",
            r"(?i)\breview\s+(code|security|architecture)\b",
            r"(?i)^\s*review\b.*\bagain\b",
            r"(?i)\bfocus on finding\b.*\bproblems\b",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review",
            r"(?i)review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)",
            r"(?i)(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)",
            r"(?i)(审查|评审|审核).*(PR|pull request|合并请求)",
        ])
    })
}

fn parallel_delegation_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)",
            r"(?i)(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)",
            r"(?i)(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)",
            r"(?i)\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification|worker|workers)\b",
            r"(?i)(并行|分路|分头|独立).*(lane|路线|路)",
        ])
    })
}

fn parallel_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(parallel|concurrent|in parallel|split lanes?|independent lanes?|split work)\b|(并行|同时|分头|分路|多路|多线|独立)")
            .expect("invalid regex")
    })
}

fn task_context_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(implement|build|run|execute|refactor|migrate|fix|change|ship)\b|(实现|执行|运行|构建|改|修|重构|迁移)")
            .expect("invalid regex")
    })
}

fn capability_domain_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(frontend|backend|test|testing|api|database|ui|security|performance|architecture|implementation|verification|module|lane|lanes)\b|(前端|后端|测试|数据库|安全|性能|架构|模块|方向)")
            .expect("invalid regex")
    })
}

fn override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
    })
}

fn review_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
        ])
    })
}

fn delegation_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
    })
}

fn reject_reason_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            "small_task",
            "shared_context_heavy",
            "write_scope_overlap",
            "next_step_blocked",
            "verification_missing",
            "token_overhead_dominates",
        ]
        .iter()
        .map(|reason| {
            Regex::new(&format!(
                "(?i)(^|[^a-z0-9_])({})($|[^a-z0-9_])",
                regex::escape(reason)
            ))
            .expect("invalid reject regex")
        })
        .collect()
    })
}

/// 与 `reject_reason_patterns` 同步；用于「整行仅 token」时的精确匹配（规避极少数 Unicode 边界与宿主格式差异）。
const REJECT_REASON_LINE_TOKENS: &[&str] = &[
    "small_task",
    "shared_context_heavy",
    "write_scope_overlap",
    "next_step_blocked",
    "verification_missing",
    "token_overhead_dominates",
];

/// 显式操作符：仅当单独成行（trim 后全串匹配）时生效，避免在正常句子里误触发。
const REVIEW_GATE_LINE_CLEAR_MARKERS: &[&str] = &["rg_clear", "/rg_clear"];

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

fn review_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\breview\b").expect("invalid regex"))
}

fn pr_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(pr|pull request)\b").expect("invalid regex"))
}

fn deep_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)")
            .expect("invalid regex")
    })
}

fn narrow_review_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))")
            .expect("invalid regex")
    })
}

fn framework_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(^|\s)/(autopilot|team|loop|gitx)\b").expect("invalid regex")
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

pub fn is_narrow_review_prompt(text: &str) -> bool {
    if !review_keyword_re().is_match(text) {
        return false;
    }
    if pr_keyword_re().is_match(text) {
        return false;
    }
    if deep_keyword_re().is_match(text) {
        return false;
    }
    narrow_review_prefix_re().is_match(text)
}

pub fn is_review_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    if is_narrow_review_prompt(&sanitized) {
        return false;
    }
    review_patterns().iter().any(|p| p.is_match(&sanitized))
}

pub fn is_parallel_delegation_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    let matched = parallel_delegation_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized));
    if !matched {
        return false;
    }
    if parallel_marker_re().is_match(&sanitized) {
        return task_context_re().is_match(&sanitized)
            || capability_domain_re().is_match(&sanitized);
    }
    true
}

fn has_goal_contract_signal(text: &str) -> bool {
    goal_contract_re().is_match(text)
}

fn has_goal_progress_signal(text: &str) -> bool {
    goal_progress_re().is_match(text)
}

fn has_goal_verify_or_block_signal(text: &str) -> bool {
    goal_verify_or_block_re().is_match(text)
}

pub fn has_override(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    override_patterns().iter().any(|p| p.is_match(&sanitized))
}

pub fn has_review_override(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    review_override_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized))
}

pub fn has_delegation_override(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    delegation_override_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized))
}

pub fn saw_reject_reason(text: &str) -> bool {
    if reject_reason_patterns().iter().any(|p| p.is_match(text)) {
        return true;
    }
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        // 用户有时会把门控提示原样贴回输入框（例如以 RG_FOLLOWUP / AG_FOLLOWUP 开头）。
        // 这种情况下直接视为“我看见了并请求清门”，否则会陷入无限循环。
        // 注：AG_FOLLOWUP 仅等同拒因/清门信号（满足 pre_goal 等分支）；完整 goal 收口仍依赖
        // Goal 关键词、`GOAL_STATE.json` hydrate，或显式 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`。
        if lower.starts_with("rg_followup") || lower.starts_with("ag_followup") {
            return true;
        }
        if REJECT_REASON_LINE_TOKENS.contains(&lower.as_str()) {
            return true;
        }
        if REVIEW_GATE_LINE_CLEAR_MARKERS.contains(&lower.as_str()) {
            return true;
        }
    }
    false
}

pub fn normalize_subagent_type(value: Option<&str>) -> String {
    value
        .map(|s| s.trim().to_lowercase().replace('_', "-"))
        .unwrap_or_default()
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

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_lowercase()).unwrap_or_default()
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
fn try_extract_session_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in [
        "session_id",
        "conversation_id",
        "thread_id",
        "agent_id",
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

/// 派生 `.cursor/hook-state/review-subagent-<key>.json` 文件名组件。
/// 顺序：`extract_first_session_string` → `ROUTER_RS_CURSOR_SESSION_NAMESPACE` → `cwd`（含嵌套 workspace 字段）→ 常量 fallback。
fn session_key(event: &Value) -> String {
    if let Some(raw) = extract_first_session_string(event) {
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

fn strip_quoted_or_codeblock_or_url(text: &str) -> String {
    static RE_FENCED: OnceLock<Regex> = OnceLock::new();
    static RE_INLINE: OnceLock<Regex> = OnceLock::new();
    static RE_URL: OnceLock<Regex> = OnceLock::new();
    static RE_BLOCKQUOTE: OnceLock<Regex> = OnceLock::new();
    static RE_QUOTED: OnceLock<Regex> = OnceLock::new();
    let mut cleaned = text.to_string();
    cleaned = RE_FENCED
        .get_or_init(|| Regex::new(r"(?s)```.*?```").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_INLINE
        .get_or_init(|| Regex::new(r"`[^`\n]*`").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_URL
        .get_or_init(|| Regex::new(r"https?://\S+").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_BLOCKQUOTE
        .get_or_init(|| Regex::new(r"(?m)^\s*>\s.*$").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    RE_QUOTED
        .get_or_init(|| Regex::new("\"[^\"\\n]*\"").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned()
}

fn loop_line_clear_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*/loop\s+clear\b").expect("invalid regex"))
}

fn loop_line_next_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*/loop\s+next\b").expect("invalid regex"))
}

/// 整段提示词在 strip 后仅含单行 `/loop clear|next` 时不视为 framework/delegation 入口。
fn is_loop_admin_only_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    let lines: Vec<&str> = sanitized
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() != 1 {
        return false;
    }
    let line = lines[0];
    loop_line_clear_re().is_match(line) || loop_line_next_re().is_match(line)
}

fn is_framework_entrypoint_prompt(text: &str) -> bool {
    if is_loop_admin_only_prompt(text) {
        return false;
    }
    framework_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

fn is_autopilot_entrypoint_prompt(text: &str) -> bool {
    autopilot_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

/// `/$team` 等框架入口仍视为委托/并行编排；**`gitx` 仅认 `/gitx`**（不认 `$gitx`）；**`/autopilot` 除外**（只走 goal 机）。
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

// --- adversarial loop（宿主保存预算，不向模型披露总轮数） ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AdversarialLoopState {
    #[serde(default = "adversarial_loop_schema_v")]
    schema_version: u32,
    /// 用户配置的轮次上限；仅存在于 `.cursor/hook-state`，永不写入注入正文。
    max_rounds: u32,
    /// 已完成并确认的轮次数（`LOOP_ROUND_COMPLETE` 或 `/loop next`）。
    completed_passes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

fn adversarial_loop_schema_v() -> u32 {
    1
}

fn adversarial_loop_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("adversarial-loop-{}.json", session_key(event)))
}

fn loop_line_init_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*([/$])loop(?:\s+(\d{1,2}))?\s*(.*)$").expect("invalid regex")
    })
}

fn loop_round_complete_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*LOOP_ROUND_COMPLETE\s*$").expect("invalid regex"))
}

#[derive(Debug, Clone, PartialEq)]
enum LoopFirstLineAction {
    None,
    Clear,
    Next,
    Init { max_rounds: u32 },
}

fn parse_loop_first_line(line: &str) -> LoopFirstLineAction {
    let t = line.trim();
    if loop_line_clear_re().is_match(t) {
        return LoopFirstLineAction::Clear;
    }
    if loop_line_next_re().is_match(t) {
        return LoopFirstLineAction::Next;
    }
    if let Some(cap) = loop_line_init_re().captures(t) {
        let n = cap.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
        let max_rounds = n.unwrap_or(3).clamp(1, 99);
        return LoopFirstLineAction::Init { max_rounds };
    }
    LoopFirstLineAction::None
}

/// 与 `is_loop_admin_only_prompt` 对齐：在 strip 后的正文中，取**第一条**可解析的 loop 指令行。
fn first_loop_directive_in_stripped_prompt(stripped: &str) -> LoopFirstLineAction {
    for line in stripped.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let action = parse_loop_first_line(line);
        if !matches!(action, LoopFirstLineAction::None) {
            return action;
        }
    }
    LoopFirstLineAction::None
}

fn normalize_crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn load_adversarial_loop(repo_root: &Path, event: &Value) -> Option<AdversarialLoopState> {
    let path = adversarial_loop_path(repo_root, event);
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_adversarial_loop(
    repo_root: &Path,
    event: &Value,
    state: &mut AdversarialLoopState,
) -> bool {
    let directory = state_dir(repo_root);
    let target = adversarial_loop_path(repo_root, event);
    let _ = fs::create_dir_all(&directory);
    state.schema_version = 1;
    state.updated_at = Some(Utc::now().to_rfc3339());
    let payload = match serde_json::to_string_pretty(state) {
        Ok(text) => format!("{text}\n"),
        Err(_) => return false,
    };
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let tmp = directory.join(format!(".tmp-adv-loop-{}-{}", std::process::id(), micros));
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
    fs::rename(&tmp, &target).is_ok()
}

fn remove_adversarial_loop(repo_root: &Path, event: &Value) {
    let _ = fs::remove_file(adversarial_loop_path(repo_root, event));
}

/// 会话盐 + 已完成轮次混合选档，避免固定周期暴露线性进度。
fn adversarial_tier_slot(event: &Value, completed_passes: u32) -> usize {
    let mut hasher = Sha256::new();
    hasher.update(b"router-rs-adversarial-loop-tier-v1");
    hasher.update(session_key(event).as_bytes());
    hasher.update(completed_passes.to_le_bytes());
    let digest = hasher.finalize();
    let word = u64::from_le_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (word % 4) as usize
}

fn adversarial_loop_injection_text(event: &Value, state: &AdversarialLoopState) -> String {
    if state.completed_passes >= state.max_rounds {
        return "router-rs adversarial-loop: host-controlled multi-pass window for this session has ended (no numeric totals disclosed). Provide verification evidence and close out, or ask the user to re-arm with /loop.".to_string();
    }
    let slot = adversarial_tier_slot(event, state.completed_passes);
    let tier_body = match slot {
        0 => "Tier A — correctness, completeness, obvious failures, tests vs intent",
        1 => "Tier B — edge cases, invalid inputs, security boundaries, threat-ish misuse",
        2 => "Tier C — performance, maintainability, diagnostics, operational risks",
        3 => "Tier D — adversarial assumptions, regression vectors, hidden coupling",
        _ => "Tier — broaden adversarial coverage",
    };
    format!(
        "router-rs adversarial-loop: progressive rubric only — {tier_body}. Do NOT infer remaining passes or total budget from this message. Use independent reviewer context when possible; fix surgically; rerun verify commands. When this pass is truly finished, output a single line containing exactly: LOOP_ROUND_COMPLETE outside markdown fenced code blocks (that line must be alone, no other text on the same line; CRLF is normalized)"
    )
}

/// 解析用户提示中的 `/loop` 指令并更新状态文件（经 `strip_quoted_or_codeblock_or_url` 后扫描**第一条**指令行，与 admin-only 门禁一致）；返回供 `additional_context` 注入的文本（无活跃会话则 None）。
fn adversarial_loop_process_prompt(
    repo_root: &Path,
    event: &Value,
    prompt: &str,
) -> Option<String> {
    let stripped = strip_quoted_or_codeblock_or_url(prompt);
    match first_loop_directive_in_stripped_prompt(&stripped) {
        LoopFirstLineAction::Clear => {
            remove_adversarial_loop(repo_root, event);
            return None;
        }
        LoopFirstLineAction::Next => {
            if let Some(mut st) = load_adversarial_loop(repo_root, event) {
                if st.completed_passes < st.max_rounds {
                    st.completed_passes += 1;
                    let _ = save_adversarial_loop(repo_root, event, &mut st);
                }
            }
        }
        LoopFirstLineAction::Init { max_rounds } => {
            let mut st = AdversarialLoopState {
                schema_version: 1,
                max_rounds,
                completed_passes: 0,
                updated_at: None,
            };
            let _ = save_adversarial_loop(repo_root, event, &mut st);
        }
        LoopFirstLineAction::None => {}
    }
    load_adversarial_loop(repo_root, event).map(|st| adversarial_loop_injection_text(event, &st))
}

fn adversarial_loop_on_response_complete(repo_root: &Path, event: &Value, response: &str) {
    let normalized = normalize_crlf(response);
    let stripped = strip_quoted_or_codeblock_or_url(&normalized);
    if !loop_round_complete_re().is_match(&stripped) {
        return;
    }
    let Some(mut st) = load_adversarial_loop(repo_root, event) else {
        return;
    };
    if st.completed_passes < st.max_rounds {
        st.completed_passes += 1;
        let _ = save_adversarial_loop(repo_root, event, &mut st);
    }
}

fn merge_additional_context(output: &mut Value, extra: &str) {
    match output.get_mut("additional_context") {
        Some(Value::String(s)) => {
            s.push_str("\n\n");
            s.push_str(extra);
        }
        _ => {
            output["additional_context"] = Value::String(extra.to_string());
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

fn release_state_lock(lock: Option<LockGuard>) {
    if let Some(lock) = lock {
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

fn is_armed(state: &ReviewGateState) -> bool {
    state.review_required || state.delegation_required
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
    // reject_reason 只用于「未Spawn预检 subagent」免责，不再一键放行完整 goal 收口。
    if !state.pre_goal_review_satisfied {
        return false;
    }
    state.goal_contract_seen && state.goal_progress_seen && state.goal_verify_or_block_seen
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}

fn autopilot_pre_goal_followup_message() -> String {
    if crate::router_env_flags::router_rs_goal_prompt_verbose() {
        "Autopilot (/autopilot): spawn an independent-context reviewer subagent now (fresh lane; not fork_context=true), then publish the goal contract. If you will not spawn, output one explicit reject reason token (small_task, shared_context_heavy, …) before proceeding.".to_string()
    } else {
        "Autopilot (/autopilot)：先起独立 reviewer subagent（新 lane，勿 fork_context=true）；否则单独一行拒因（small_task 等）。".to_string()
    }
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
/// 状态机仍会读写 `.cursor/hook-state`；仅压制对模型的可见提示。
fn cursor_hook_silent_by_env() -> bool {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_HOOK_SILENT") else {
        return false;
    };
    let t = raw.trim().to_ascii_lowercase();
    !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
}

fn apply_cursor_hook_output_policy(output: &mut Value) {
    if !cursor_hook_silent_by_env() {
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
    let gtext = goal
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let has_goal_text = !gtext.is_empty();
    let done_when_nonempty = goal_state_list_any_nonempty_string(goal, "done_when");
    let validation_nonempty = goal_state_list_any_nonempty_string(goal, "validation_commands");
    let non_goals_nonempty = goal_state_list_any_nonempty_string(goal, "non_goals");
    let horizon_nonempty = goal
        .get("current_horizon")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if has_goal_text
        || done_when_nonempty
        || validation_nonempty
        || non_goals_nonempty
        || horizon_nonempty
    {
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
    let disk_contract_signal =
        done_when_nonempty || validation_nonempty || non_goals_nonempty || horizon_nonempty;
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
    if !state.pre_goal_review_satisfied {
        missing.push("pre_goal_independent_subagent_or_reject_reason");
    }
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
        || saw_reject_reason(&signal_text);

    let strong_constraint = (review_arms || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return "router-rs：hook-state 锁不可用，本轮须严格 review/委托/autopilot 证据。合并前请修复锁/权限并重试，或 subagent/拒因。".to_string();
    }
    state_lock_degraded_followup().to_string()
}

fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let lock = acquire_state_lock(repo_root, event);
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
    state.delegation_required = state.delegation_required || delegation;
    state.review_override = state.review_override || review_override;
    state.delegation_override = state.delegation_override || delegation_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
    state.goal_contract_seen = state.goal_contract_seen || has_goal_contract_signal(&signal_text);
    state.goal_progress_seen = state.goal_progress_seen || has_goal_progress_signal(&signal_text);
    state.goal_verify_or_block_seen =
        state.goal_verify_or_block_seen || has_goal_verify_or_block_signal(&signal_text);
    // 用户在本轮提交里写出 reject_reason token 时须即时生效；否则仅能在助手回复或 Stop 里识别，导致 autopilot pre-goal 与 AG_FOLLOWUP 循环。
    // `signal_text` 含整树字符串，覆盖仅出现在 `messages[].content` 等深层路径的 token。
    if saw_reject_reason(&signal_text) {
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

    let persisted = save_state(repo_root, event, &mut state);

    // Review/delegation：不在 beforeSubmit 注入 RG 文案；phase 由 subagent/PostToolUse 推进（见 review_armed 等测试）。
    let needs_autopilot_pre_goal = state.goal_required
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
    if let Some(loop_ctx) = adversarial_loop_process_prompt(repo_root, event, &text) {
        merge_additional_context(&mut output, &loop_ctx);
    }
    let persisted_after_followup = if needs_autopilot_pre_goal {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    release_state_lock(lock);
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
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let tool_input = tool_input_of(event);
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let armed = is_armed(&state);
    let mut mutated = false;
    // 与 PostToolUse 对齐：pre-goal 在独立 fork 且存在 lane 类型证据时满足（含非白名单 lane 名）。
    if state.goal_required && pre_goal_kind && independent_fork {
        state.pre_goal_review_satisfied = true;
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
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_subagent_stop(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    if is_armed(&state) {
        // Intentional hardening vs baseline: stop evidence only counts after start reached phase 2.
        if state.phase < 2 {
            release_state_lock(lock);
            return json!({});
        }
        bump_phase(&mut state, 3);
        state.subagent_stop_count += 1;
        state.lane_intent_matches = Some(true);
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = is_armed(&state);
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
        mutated = true;
    }
    if tool_name_matches_subagent_lane(&name) && pre_goal_kind && armed {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.last_subagent_tool = Some(name);
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
    release_state_lock(lock);

    // 与 Codex PostTool 对齐：终端执行验证类命令时写入 EVIDENCE_INDEX（连续性就绪且未关闭 POSTTOOL_EVIDENCE）。
    let syn = synthetic_codex_shape_for_post_tool_evidence(event);
    if let Err(err) =
        crate::framework_runtime::try_append_cursor_post_tool_evidence(repo_root, &syn)
    {
        eprintln!("[router-rs] cursor post-tool evidence append failed (non-fatal): {err}");
    }

    json!({})
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
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = is_armed(&state);
    let track_goal = state.goal_required || armed;
    let prompt = prompt_text(event);
    let text = agent_response_text(event);
    let signal = hook_event_signal_text(event, &prompt, &text);
    adversarial_loop_on_response_complete(repo_root, event, &text);
    let mut dirty = false;
    if saw_reject_reason(&signal) {
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
    release_state_lock(lock);
    json!({})
}

fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let lock = acquire_state_lock(repo_root, event);
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
            if saw_reject_reason(&signal_text) {
                state.reject_reason_seen = true;
                if state.goal_required {
                    state.pre_goal_review_satisfied = true;
                }
                clear_review_gate_escalation_counters(&mut state);
            }
            hydrate_goal_gate_from_disk(repo_root, &mut state, true, &frame);
            // Review/delegation 不再注入 RG_FOLLOWUP；Stop 仅对未满足的 autopilot goal 发 AG_FOLLOWUP。
            if !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = format!("AG_FOLLOWUP missing_parts={}", goal_missing_parts(&state));
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
                if state.reject_reason_seen {
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
    release_state_lock(lock);
    output
}

fn handle_pre_compact(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "additional_context": "router-rs：hook-state 锁不可用，preCompact 未读到持久化门控状态。"
        });
    }
    let out = match load_state(repo_root, event) {
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
    release_state_lock(lock);
    out
}

fn handle_session_end(repo_root: &Path, event: &Value) -> Value {
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
    // 最后回收本仓库 Cursor terminal 留下的 stale 子进程（cargo/python/实验脚本等）。
    let report = terminate_stale_terminal_processes(repo_root);
    if !cursor_hook_silent_by_env() {
        if !report.killed.is_empty() {
            eprintln!(
                "router-rs SessionEnd: terminated {} stale terminal pid(s) {:?} (scanned={}, outside_repo={}, dead={})",
                report.killed.len(),
                report.killed,
                report.scanned,
                report.skipped_outside_repo,
                report.skipped_dead,
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
///    - `save_adversarial_loop` 留下 `.tmp-adv-loop-<pid>-<micros>`（无扩展名）。
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
/// `adversarial_loop_path` / `save_state` / `save_adversarial_loop` 保持一致。
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
    // 原子写入孤儿（崩溃残留）。`save_state` 的 tmp 形如
    // `.tmp-<pid>-<micros>-review-subagent-<key>.json`，故用「以 `.tmp-` 起、且包含 `review-subagent-`」识别；
    // `save_adversarial_loop` 的 tmp 形如 `.tmp-adv-loop-<pid>-<micros>`（无扩展名），故单独前缀识别。
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
    failed: Vec<(u32, String)>,
}

#[derive(Debug, Default, Clone)]
struct TerminalHeader {
    pid: Option<u32>,
    cwd: Option<PathBuf>,
    is_active: bool,
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
            _ => {}
        }
    }
    Some(header)
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

#[cfg(not(unix))]
fn process_pgid(_pid: u32) -> Option<u32> {
    None
}

#[cfg(unix)]
fn signal_pid_or_pgrp(pid: u32, pgid: Option<u32>, signal: libc::c_int) {
    let target = match pgid {
        Some(g) => -(g as libc::pid_t),
        None => pid as libc::pid_t,
    };
    unsafe {
        let _ = libc::kill(target, signal);
    }
}

/// SIGTERM → 最多等 2s → SIGKILL；优先按进程组信号，覆盖 `cargo test`/`python -m` 这类 fork 子进程的命令。
#[cfg(unix)]
fn terminate_pid(pid: u32) -> Result<(), String> {
    let pgid = process_pgid(pid);
    signal_pid_or_pgrp(pid, pgid, libc::SIGTERM);
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(100));
        if !is_process_alive(pid) {
            return Ok(());
        }
    }
    signal_pid_or_pgrp(pid, pgid, libc::SIGKILL);
    thread::sleep(Duration::from_millis(50));
    if is_process_alive(pid) {
        Err(format!("SIGKILL did not reap pid={pid}"))
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn terminate_pid(_pid: u32) -> Result<(), String> {
    Err("non-unix terminate not implemented".into())
}

fn terminate_stale_terminal_processes(repo_root: &Path) -> StaleTerminalKillReport {
    if cursor_kill_stale_terminals_disabled_by_env() {
        return StaleTerminalKillReport::default();
    }
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return StaleTerminalKillReport::default();
    };
    terminate_stale_terminal_processes_in_dir(repo_root, &terminals_dir)
}

/// 纯逻辑形式：调用方提供 terminals 目录（便于测试与显式覆盖路径）。不再读 env 开关。
fn terminate_stale_terminal_processes_in_dir(
    repo_root: &Path,
    terminals_dir: &Path,
) -> StaleTerminalKillReport {
    let mut report = StaleTerminalKillReport::default();
    let entries = match fs::read_dir(terminals_dir) {
        Ok(e) => e,
        Err(_) => return report,
    };
    let our_pid = std::process::id();
    let abs_repo = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.ends_with(".txt") {
            continue;
        }
        report.scanned += 1;
        // header 在前 ~4KB 内，避免读整个 terminal 输出文件。
        let mut buf = String::new();
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
        // 范围过滤：cwd 必须落在本仓库内，避免误杀同机器其他项目的 terminal。
        // 先于 is_process_alive：pid 已消失但仍带“外仓 cwd”的文件应记为 skipped_outside_repo，而非 skipped_dead。
        let Some(cwd) = header.cwd.as_ref() else {
            report.skipped_outside_repo += 1;
            continue;
        };
        let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        if !cwd_canon.starts_with(&abs_repo) {
            report.skipped_outside_repo += 1;
            continue;
        }
        if !is_process_alive(pid) {
            report.skipped_dead += 1;
            continue;
        }
        match terminate_pid(pid) {
            Ok(()) => report.killed.push(pid),
            Err(err) => report.failed.push((pid, err)),
        }
    }
    report
}

fn dispatch_event(repo_root: &Path, event_name: &str, payload: &Value) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    if cursor_review_gate_disabled_by_env() {
        let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
        return match lowered {
            "beforesubmitprompt" | "userpromptsubmit" => {
                let mut out = json!({ "continue": true });
                merge_continuity_followups_before_submit(repo_root, &mut out, &frame);
                out
            }
            "sessionend" => handle_session_end(repo_root, payload),
            "stop" => {
                let mut out = json!({});
                merge_continuity_followups(repo_root, &mut out, &frame);
                out
            }
            "posttooluse" => {
                let mut out = handle_post_tool_use(repo_root, payload);
                strip_cursor_hook_user_visible_nags(&mut out);
                out
            }
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
            _ => json!({}),
        };
    }
    match lowered {
        "beforesubmitprompt" | "userpromptsubmit" => handle_before_submit(repo_root, payload),
        "subagentstart" => handle_subagent_start(repo_root, payload),
        "subagentstop" => handle_subagent_stop(repo_root, payload),
        "posttooluse" => handle_post_tool_use(repo_root, payload),
        "afteragentresponse" => handle_after_agent_response(repo_root, payload),
        "stop" => handle_stop(repo_root, payload),
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

fn read_stdin_json() -> Result<Value, String> {
    let mut stdin = std::io::stdin();
    read_stdin_json_from_reader(&mut stdin)
}

pub fn run_cursor_review_gate(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    let payload = read_stdin_json()?;
    let repo_root = resolve_cursor_hook_repo_root(cli_repo_root, &payload)?;
    let mut output = dispatch_event(&repo_root, event, &payload);
    apply_cursor_hook_output_policy(&mut output);
    let mut stdout = std::io::stdout();
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::{env, fs};

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

    #[test]
    fn adversarial_loop_parse_first_line() {
        assert_eq!(
            parse_loop_first_line("/loop clear"),
            LoopFirstLineAction::Clear
        );
        assert_eq!(
            parse_loop_first_line("$loop next"),
            LoopFirstLineAction::Next
        );
        match parse_loop_first_line("/loop 7 do thing") {
            LoopFirstLineAction::Init { max_rounds } => assert_eq!(max_rounds, 7),
            other => panic!("expected Init, got {other:?}"),
        }
        match parse_loop_first_line("/loop fix auth") {
            LoopFirstLineAction::Init { max_rounds } => assert_eq!(max_rounds, 3),
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn adversarial_loop_before_submit_emits_additional_context() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("advloop1", "/loop 3 tighten hooks"),
        );
        let ctx = out
            .get("additional_context")
            .and_then(Value::as_str)
            .expect("additional_context");
        assert!(ctx.contains("router-rs adversarial-loop"));
        assert!(ctx.contains("LOOP_ROUND_COMPLETE"));
        let ev = json!({ "session_id": "advloop1", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("adv loop state");
        assert_eq!(st.max_rounds, 3);
        assert_eq!(st.completed_passes, 0);
    }

    #[test]
    fn adversarial_loop_response_complete_increments_passes() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("advloop2", "/loop 2 x"));
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({
                "session_id": "advloop2",
                "cwd": "/Users/joe/Documents/skill",
                "response": "done\nLOOP_ROUND_COMPLETE"
            }),
        );
        let ev = json!({ "session_id": "advloop2", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("state");
        assert_eq!(st.completed_passes, 1);
    }

    #[test]
    fn adversarial_loop_inline_complete_does_not_increment() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("advloop3", "/loop 2 x"));
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({
                "session_id": "advloop3",
                "cwd": "/Users/joe/Documents/skill",
                "response": "done LOOP_ROUND_COMPLETE trailing"
            }),
        );
        let ev = json!({ "session_id": "advloop3", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("state");
        assert_eq!(st.completed_passes, 0);
    }

    #[test]
    fn first_loop_directive_scans_stripped_multiline() {
        let stripped = strip_quoted_or_codeblock_or_url("intro\n\n/loop 5 goal");
        assert_eq!(
            first_loop_directive_in_stripped_prompt(&stripped),
            LoopFirstLineAction::Init { max_rounds: 5 }
        );
    }

    #[test]
    fn adversarial_loop_multiline_stripped_init() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("advml", "please\n\n/loop 4 task"),
        );
        let ev = json!({ "session_id": "advml", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("state");
        assert_eq!(st.max_rounds, 4);
        assert_eq!(st.completed_passes, 0);
    }

    #[test]
    fn adversarial_loop_complete_inside_fence_ignored() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("advf", "/loop 2 x"));
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({
                "session_id": "advf",
                "cwd": "/Users/joe/Documents/skill",
                "response": "```\nLOOP_ROUND_COMPLETE\n```"
            }),
        );
        let ev = json!({ "session_id": "advf", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("state");
        assert_eq!(st.completed_passes, 0);
    }

    #[test]
    fn adversarial_loop_crlf_standalone_complete_counts() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("advcr", "/loop 2 x"));
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({
                "session_id": "advcr",
                "cwd": "/Users/joe/Documents/skill",
                "response": "ok\r\nLOOP_ROUND_COMPLETE\r\n"
            }),
        );
        let ev = json!({ "session_id": "advcr", "cwd": "/Users/joe/Documents/skill" });
        let st = load_adversarial_loop(&repo, &ev).expect("state");
        assert_eq!(st.completed_passes, 1);
    }

    #[test]
    fn loop_admin_only_clear_does_not_arm_delegation() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("advloop4", "/loop clear"),
        );
        assert!(out.get("followup_message").is_none());
        let state = load_state_for(&repo, "advloop4");
        assert!(!state.delegation_required);
        assert!(!state.review_required);
    }

    #[test]
    fn review_prompt_chinese_full_review_arms_state() {
        let repo = fresh_repo();
        let out = dispatch_event(
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
    fn parallel_delegation_arms_delegation() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s2", "请前端后端测试并行分头执行"),
        );
        let state = load_state_for(&repo, "s2");
        assert!(state.delegation_required);
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn autopilot_entry_does_not_arm_delegation_or_review_from_fix_copy() {
        let repo = fresh_repo();
        let _ = dispatch_event(
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
            "drive_until_done": true,
        }))
        .expect("goal start");
        let out = dispatch_event(
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
            "done_when": ["tests green"],
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("ev-gate", "/autopilot finish fixes"),
        );
        let out = dispatch_event(
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
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("noflag", "hello"));
        assert!(
            !load_state_for(&repo, "noflag").goal_required,
            "plain prompt must not arm goal_required before hydrate"
        );
        let out = dispatch_event(
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
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"no active_task json","status":"running","checkpoints":[{"note":"step"}],"done_when":["ship"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
        fs::write(
            repo.join("artifacts/current/t-orph/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
        )
        .expect("ev");
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("orph", "hello"));
        let out = dispatch_event(
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
            "drive_until_done": true,
        }))
        .expect("goal start");
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("run-gate", "/autopilot continue"),
        );
        let out = dispatch_event(
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
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"hand-written without status","checkpoints":[],"done_when":[],"validation_commands":[]}"#,
        )
        .expect("goal json");
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("ns-gate", "/autopilot continue"),
        );
        let out = dispatch_event(
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
        let out = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s4", "全面review这个仓库"),
        );
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({ "session_id": "s4", "response": "reject reason: small_task" }),
        );
        let out = dispatch_event(&repo, "stop", &event("s4", "reject reason: small_task"));
        assert_eq!(out, json!({}));
    }

    #[test]
    // renamed from reject_reason_in_user_prompt_does_not_satisfy_gate after stop-stage parity fix
    fn reject_reason_in_user_prompt_satisfies_gate_on_stop() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13", "全面review这个仓库"),
        );
        let out = dispatch_event(&repo, "stop", &event("s13", "reject reason: small_task"));
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13b", "全面review这个仓库"),
        );
        let out = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13nest-r", "全面review这个仓库"),
        );
        let out = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13nest-a", "全面review这个仓库"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13c", "全面review这个仓库"),
        );
        let _ = dispatch_event(
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
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
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
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
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
        let out = dispatch_event(&repo, "stop", &payload);
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
            "drive_until_done": true,
        }))
        .expect("goal start");

        let payload = event("s15b", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir lock parent");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_event(&repo, "stop", &payload);
        let blob = hook_user_visible_blob(&out);
        assert!(blob.contains("锁不可用"), "{blob}");
        assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");
    }

    #[test]
    fn before_submit_merges_goal_and_rfv_when_both_on_disk() {
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
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"checkpoints":[],"done_when":[],"validation_commands":[]}"#,
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
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &event("merge-t", "hello"));
        let msg = hook_user_visible_blob(&out);
        assert!(msg.contains("## 续跑（beforeSubmit）"), "{msg}");
        assert!(msg.contains("AUTOPILOT_DRIVE"), "{msg}");
        assert!(msg.contains("RFV_LOOP_CONTINUE"), "{msg}");
        assert_eq!(
            msg.matches("## 续跑").count(),
            1,
            "expected single merged heading; msg={msg}"
        );
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
            "drive_until_done": true,
        }))
        .expect("goal start");

        let mut out = {
            let _rg = ReviewGateDisableTestGuard::new();
            dispatch_event(&repo, "stop", &event("sg1", "hi"))
        };
        let blob = hook_user_visible_blob(&out);
        assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");

        let prev_silent = env::var_os("ROUTER_RS_CURSOR_HOOK_SILENT");
        env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", "1");
        apply_cursor_hook_output_policy(&mut out);
        match prev_silent {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v),
            None => env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT"),
        }
        assert!(out.get("followup_message").is_none());
        assert!(out.get("additional_context").is_none());
    }

    /// 应急 ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE 时仍应更新 pre_goal/phase，避免恢复门控后状态脱节。
    #[test]
    fn review_gate_disabled_post_tool_use_still_advances_phase_after_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("srg-pu2", "全面review这个仓库"),
        );
        assert!(load_state_for(&repo, "srg-pu2").phase < 2);

        let out = {
            let _rg = ReviewGateDisableTestGuard::new();
            dispatch_event(
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
        apply_cursor_hook_output_policy(&mut keep);
        assert_eq!(keep["followup_message"], json!("keep"));

        env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", "1");
        let mut strip = json!({
            "continue": false,
            "followup_message": "nag",
            "additional_context": "ctx"
        });
        apply_cursor_hook_output_policy(&mut strip);
        assert_eq!(strip, json!({ "continue": false }));

        match prev {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v),
            None => env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT"),
        }
    }

    #[test]
    fn subagent_start_promotes_phase_to_2() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s5", "全面review这个仓库"),
        );
        let _ = dispatch_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s5", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s5");
        assert_eq!(state.phase, 2);
        assert_eq!(state.subagent_start_count, 1);
    }

    #[test]
    fn saw_reject_reason_accepts_line_only_tokens_and_rg_clear() {
        assert!(saw_reject_reason("small_task"));
        assert!(saw_reject_reason("\n  SMALL_TASK  \n"));
        assert!(saw_reject_reason("rg_clear"));
        assert!(saw_reject_reason("/rg_clear"));
        assert!(!saw_reject_reason("small_tasking"));
    }

    #[test]
    fn subagent_stop_without_start_does_not_promote_phase() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s6", "全面review这个仓库"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s6b", "全面review这个仓库"),
        );
        let _ = dispatch_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s6b", "subagent_type": "explore" }),
        );
        let _ = dispatch_event(
            &repo,
            "subagentStop",
            &json!({ "session_id": "s6b", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s6b");
        assert_eq!(state.phase, 3);
        assert_eq!(state.subagent_stop_count, 1);
    }

    #[test]
    fn stop_without_subagent_has_no_review_followup() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s7", "全面review这个仓库"),
        );
        let out = dispatch_event(&repo, "stop", &event("s7", "继续"));
        assert!(
            out.get("followup_message").is_none(),
            "review/delegation gate must not inject RG_FOLLOWUP; out={out:?}"
        );
    }

    #[test]
    fn pre_compact_emits_additional_context_summary() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s8", "全面review这个仓库"),
        );
        let out = dispatch_event(
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
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
        let path = state_path(&repo, &payload);
        assert!(path.exists());
        let _ = dispatch_event(&repo, "sessionEnd", &payload);
        assert!(!path.exists());
    }

    #[test]
    fn session_end_cleans_stale_lock_if_present() {
        let repo = fresh_repo();
        let payload = event("s9b", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"pid=1 ts=1").expect("seed lock");
        let _ = dispatch_event(&repo, "sessionEnd", &payload);
        assert!(!lock_path.exists());
    }

    /// SessionEnd 必须按文件名前缀清扫整个 `.cursor/hook-state/`，覆盖
    /// payload 缺 `session_id` / `cwd` 时旧会话状态泄漏的情况
    /// （AGENTS.md → Continuity → Cursor「sessionEnd 应清 hook-state 下 review gate 状态」）。
    #[test]
    fn session_end_sweeps_review_gate_state_with_unrelated_session_key() {
        let repo = fresh_repo();
        let stale_payload = event("stale-session", "全面review这个仓库");
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &stale_payload);
        let stale_state = state_path(&repo, &stale_payload);
        let stale_lock = state_lock_path(&repo, &stale_payload);
        let stale_loop = adversarial_loop_path(&repo, &stale_payload);
        fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
        fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
        fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
        assert!(stale_state.exists());

        // 与遗留状态完全不同的 session key，保证按 session_key 精准删法删不到 stale_*。
        let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
        let _ = dispatch_event(&repo, "sessionEnd", &unrelated_payload);

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

        let _ = dispatch_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));
        assert!(unrelated.exists(), "unrelated hook state must be preserved");
    }

    /// SessionEnd sweep 必须回收 `save_state` / `save_adversarial_loop` 因崩溃残留的原子写入孤儿，
    /// 避免长期累积消耗 `.cursor/hook-state/` 卫生（命名规则见 `save_state` / `save_adversarial_loop`）。
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

        let _ = dispatch_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));

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
        let out = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s12", "全面review这个仓库"),
        );
        let _ = dispatch_event(
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
        let first = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s16", "全面review这个仓库"),
        );
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !first_msg.contains("RG_FOLLOWUP")
                && !first_msg.contains("Broad/deep review detected")
                && !first_msg.contains("Parallel lane request detected"),
            "first_msg={first_msg:?}"
        );
        assert!(load_state_for(&repo, "s16").review_required);
        let second = dispatch_event(&repo, "stop", &event("s16", "继续"));
        assert!(
            second.get("followup_message").is_none(),
            "Stop must not emit RG_FOLLOWUP; second={second:?}"
        );
    }

    #[test]
    fn goal_stop_followup_is_short_code_only() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s17",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore"}
            }),
        );
        let first = dispatch_event(&repo, "stop", &event("s17", "继续"));
        let first_msg = hook_user_visible_blob(&first);
        assert!(
            first_msg.contains("AG_FOLLOWUP missing_parts="),
            "Stop uses short goal hint only; msg={first_msg:?}"
        );
        assert!(
            !first_msg.contains("Autopilot goal mode:"),
            "Stop must not dump full goal contract prose; msg={first_msg:?}"
        );
        let second = dispatch_event(&repo, "stop", &event("s17", "继续"));
        let second_msg = hook_user_visible_blob(&second);
        assert!(second_msg.contains("AG_FOLLOWUP missing_parts="));
    }

    #[test]
    fn autopilot_before_submit_prompts_pre_goal_review() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17b", "/autopilot 完成任务"),
        );
        let msg = hook_user_visible_blob(&out);
        assert!(
            msg.contains("Autopilot (/autopilot)") || msg.contains("independent-context"),
            "surface={msg:?}"
        );
    }

    #[test]
    fn deep_json_strings_satisfy_pre_goal_reject_on_before_submit() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("deep-s1", "/autopilot 任务"),
        );
        let deep = json!({
            "session_id": "deep-s1",
            "cwd": "/Users/joe/Documents/skill",
            "messages": [{ "role": "user", "content": "small_task" }]
        });
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &deep);
        assert!(load_state_for(&repo, "deep-s1").pre_goal_review_satisfied);
    }

    #[test]
    fn messages_tail_user_text_clears_review_gate_when_top_level_prompt_empty() {
        let repo = fresh_repo();
        let _ = dispatch_event(
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
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &ev);
        let state = load_state_for(&repo, "s-msg-only");
        assert!(state.reject_reason_seen);
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            !msg.contains("RG_FOLLOWUP") && !msg.contains("Broad/deep review detected"),
            "expected gate clear from messages[].content; msg={msg:?} out={out:?}"
        );
        assert_eq!(state.followup_count, 0);
        assert_eq!(state.review_followup_count, 0);
    }

    #[test]
    fn before_submit_reject_reason_token_in_user_prompt_satisfies_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17e", "/autopilot 第一轮"),
        );
        let out = dispatch_event(
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
        let _ = dispatch_event(
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
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &nested);
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
        let _ = dispatch_event(
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
        let _ = dispatch_event(&repo, "stop", &nested_stop);
        assert!(load_state_for(&repo, "s17stop-n").pre_goal_review_satisfied);
    }

    #[test]
    fn post_tool_use_fork_context_true_does_not_satisfy_pre_goal() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17c", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17d", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17mcp", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17nest-tu", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-lane", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-fkstr", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s18", "```请 review 这段代码```"),
        );
        assert_eq!(load_state_for(&repo, "s18").phase, 0);
    }

    #[test]
    fn review_keyword_inside_inline_code_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s19", "这是 `review` 函数"),
        );
        assert_eq!(load_state_for(&repo, "s19").phase, 0);
    }

    #[test]
    fn review_keyword_inside_url_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s20", "https://example.com/review/123"),
        );
        assert_eq!(load_state_for(&repo, "s20").phase, 0);
    }

    #[test]
    fn review_keyword_inside_blockquote_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s21", "> 用户说 review 一下"),
        );
        assert_eq!(load_state_for(&repo, "s21").phase, 0);
    }

    #[test]
    fn quoted_review_token_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s22", r#"他说 "review hook""#),
        );
        assert_eq!(load_state_for(&repo, "s22").phase, 0);
    }

    #[test]
    fn parallel_alone_does_not_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s23", "请解释 parallel 的含义"),
        );
        assert_eq!(load_state_for(&repo, "s23").phase, 0);
    }

    #[test]
    fn parallel_with_task_verb_arms() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s24", "用 parallel workers 实现 X"),
        );
        assert_eq!(load_state_for(&repo, "s24").phase, 0);
    }

    #[test]
    fn english_concurrent_alone_no_arm() {
        let repo = fresh_repo();
        let _ = dispatch_event(
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
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &before);
        let stop = json!({
            "cwd": cwd,
            "payload": {
                "sessionId": sid,
                "prompt": "small_task\nGoal: g\nNon-goals: n\nDone when: d\nValidation commands: cargo test"
            }
        });
        let out = dispatch_event(&repo, "stop", &stop);
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
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s-sub-pre", "/autopilot 完成任务"),
        );
        let _ = dispatch_event(
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
        let _ = dispatch_event(
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
        release_state_lock(Some(lock));
    }

    #[test]
    fn cursor_lock_recovers_from_stale_timestamp() {
        let repo = fresh_repo();
        let payload = event("s27", "review");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        let stale_ts = now_millis().saturating_sub(60_000);
        fs::write(&lock_path, format!("pid=999999 ts={stale_ts}\n")).expect("seed stale lock");
        let lock = acquire_state_lock(&repo, &payload);
        assert!(lock.is_some());
        release_state_lock(lock);
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
                    release_state_lock(Some(lock));
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
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
        let path = state_path(&repo, &payload);
        let before = fs::read_to_string(&path).expect("read before");
        let _ = dispatch_event(&repo, "preCompact", &payload);
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
        let report = terminate_stale_terminal_processes_in_dir(&repo, &repo.join("missing"));
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

        // 2) outside_repo：spawn 一个实际活着的 sleep，cwd 指向仓库外。
        let mut alive_outside = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn outside sleep");
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

        let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir);
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
        assert_eq!(report.scanned, 3);
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
        use std::process::{Command, Stdio};

        let repo = fresh_repo();
        let term_dir = repo.join("__terminals");

        let mut child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        // 给子进程一点时间真正进入运行态。
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

        let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir);
        assert_eq!(report.killed, vec![pid], "report={report:?}");
        assert!(!is_process_alive(pid), "child must be reaped");
        // 防 zombie：测试结束前 wait 一次（kill 之后会立即返回）。
        let _ = child.wait();
    }

    #[test]
    fn handle_session_end_respects_kill_disable_env() {
        // 走 dispatch_event 真实路径；只验证 env=0 时不会因 terminals 路径推导失败而 panic，
        // 也不影响既有 `.cursor/hook-state` 清扫。
        let repo = fresh_repo();
        let payload = event("kill-disable-sess", "全面review这个仓库");
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
        let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
        std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", "0");
        let out = dispatch_event(&repo, "sessionEnd", &payload);
        assert_eq!(out, json!({}));
        assert!(!state_path(&repo, &payload).exists(), "state still cleared");
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v),
            None => std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS"),
        }
    }
}
