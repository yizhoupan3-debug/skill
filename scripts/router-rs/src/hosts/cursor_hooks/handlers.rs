/// Stop 收尾：在**无**硬 `followup_message` 时每轮稳定注入一条软提示，避免仅依赖规则时「有时有续跑段落、有时什么也没有」。
///
/// 可用 `ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE=0|false|off|no` 关闭（默认开启）。
const SESSION_CLOSE_STYLE_LINE_PREFIX: &str = "SESSION_CLOSE_STYLE";

fn session_close_style_stop_nudge_enabled_by_env() -> bool {
    match std::env::var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE") {
        Err(_) => true,
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(t.as_str(), "" | "0" | "false" | "off" | "no")
        }
    }
}

fn merge_session_close_style_nudge_when_soft_terminal(output: &mut Value) {
    if output.get("followup_message").is_some() {
        return;
    }
    if !crate::router_env_flags::router_rs_operator_inject_globally_enabled() {
        return;
    }
    if !session_close_style_stop_nudge_enabled_by_env() {
        return;
    }
    let msg = concat!(
        "SESSION_CLOSE_STYLE: 收尾简短、像口头交代就行：这轮做了什么、效果如何、还有没有没擦干净的地方要不要接着弄；",
        "别默认摊开路径清单、长 diff 或整段命令，除非对方点名要。"
    );
    crate::autopilot_goal::merge_hook_nudge_paragraph(
        output,
        msg,
        SESSION_CLOSE_STYLE_LINE_PREFIX,
        false,
    );
}

/// Release L3 session hook lock before any L1 task-ledger work inside [`finalize_stop_hook_outputs`].
fn release_lock_then_finalize_stop(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
    skip_continuity_merge: bool,
    lock: &mut Option<LockGuard>,
) {
    release_state_lock(lock);
    finalize_stop_hook_outputs(repo_root, output, frame, skip_continuity_merge);
}

fn finalize_stop_hook_outputs(
    _repo_root: &Path,
    output: &mut Value,
    _frame: &crate::task_state::CursorContinuityFrame,
    _skip_continuity_merge: bool,
) {
    merge_session_close_style_nudge_when_soft_terminal(output);
}

/// Assistant 回复文本侧的完成宣称检测：先剥离引文 / 代码块 / URL，再交由 `hook_common`
/// 的单一 token 表扫描，与 `closeout_enforcement::summary_claims_completion` 共用一份关键词
/// 集合，避免漂移。中文使用多字短语，避开「完成度 / 讨论完成任务拆分」等子串误报。
fn completion_claimed_in_text(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    crate::hook_common::contains_completion_claim_token(&sanitized)
}

fn closeout_followup_for_completion_claim(
    repo_root: &Path,
    task_id: &str,
) -> Result<Option<String>, String> {
    if !crate::framework_runtime::closeout_programmatic_enforcement_enabled() {
        return Ok(None);
    }
    let record_path = crate::framework_runtime::closeout_record_path_for_task(repo_root, task_id)?;
    if !record_path.is_file() {
        return Ok(Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={task_id} reason=missing_record path={}\n\
请在完成态宣称前写入 closeout record 并通过评估：\n\
- 记录路径：{}\n\
- 评估命令：router-rs closeout evaluate --repo-root \"{}\" --task-id \"{}\" --record-path \"{}\"",
            record_path.display(),
            record_path.display(),
            repo_root.display(),
            task_id,
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

/// Strict closeout：**助手回复文本**中出现完成宣称且存在 continuation task（与 hydration 同指针语义）时的硬 Stop 文案（与 `dispatch`/`handle_stop` 共用，避免分叉）。
///
/// `Err(evaluator)` 与 `Ok(Some(..))` 均返回 `Some`；未宣称完成、`Ok(None)` 或无 task 时返回 `None`。
fn stop_hard_closeout_followup_for_assistant_response(
    repo_root: &Path,
    response_text: &str,
) -> Option<String> {
    if !completion_claimed_in_text(response_text) {
        return None;
    }
    let tid = crate::task_state::resolve_cursor_continuity_frame(repo_root)
        .hydration_goal
        .map(|(_, task_id)| task_id)
        .filter(|s| !s.is_empty())?;
    match closeout_followup_for_completion_claim(repo_root, &tid) {
        Ok(Some(msg)) => Some(msg),
        Ok(None) => None,
        Err(err) => Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluator_error error={err}"
        )),
    }
}

pub const STATE_VERSION: u32 = 3;

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
        Regex::new(r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|阻塞)")
            .expect("invalid regex")
    })
}

fn goal_chat_verify_zh_signal(text: &str) -> bool {
    crate::hook_common::GOAL_CHAT_VERIFY_ZH_PHRASES
        .iter()
        .any(|p| text.contains(p))
}

/// Task/subagent 调用里明示 `fork_context: true` 时视为与主会话共享上下文，不满足 autopilot 要求的「独立上下文」预检。
/// 部分宿主以字符串 `"true"` / `"false"` 下发，需与 JSON bool 同等解析。
fn fork_context_from_tool(event: &Value, tool_input: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
}

/// Cursor-only: optional inference when `fork_context` is omitted on countable deep-review lanes.
fn cursor_fork_context_from_tool(
    event: &Value,
    tool_input: &Value,
    sub_type: &str,
    agent_type: &str,
) -> Option<bool> {
    if let Some(parsed) = fork_context_from_tool(event, tool_input) {
        return Some(parsed);
    }
    if !crate::router_env_flags::router_rs_cursor_review_fork_context_missing_infer_false_enabled()
    {
        return None;
    }
    let lane = if !sub_type.is_empty() {
        sub_type
    } else {
        agent_type
    };
    if crate::hook_common::is_deep_review_gate_lane_normalized(lane) {
        Some(false)
    } else {
        None
    }
}

/// Goal gate：须同时满足 Goal、Non-goals、Validation commands 行内标题非空，且 Done when 至少两条（英/中标题均可）。
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
    goal_verify_or_block_re().is_match(text) || goal_chat_verify_zh_signal(text)
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

/// My implement pre-goal（`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`）：常态下与 `review_subagent_kind_ok` 对齐（仅可数深度 lane + 独立 fork 证据链）；
/// `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 应急开启时退化为「任一带名 lane/agent 字段」以免应急路径过严。
fn pre_goal_subagent_kind_ok(sub_type: &str, agent_type: &str) -> bool {
    if cursor_review_gate_disabled_by_env() {
        return !sub_type.is_empty() || !agent_type.is_empty();
    }
    review_subagent_kind_ok(sub_type, agent_type)
}

fn review_subagent_kind_ok_loose_when_cursor_gate_disabled(
    sub_type: &str,
    agent_type: &str,
) -> bool {
    fn lane_ok(lane: &str) -> bool {
        matches!(
            lane,
            "explore"
                | "explorer"
                | "general-purpose"
                | "generalpurpose"
                | "ci-investigator"
                | "ciinvestigator"
                | "cursor-guide"
                | "cursorguide"
                | "best-of-n-runner"
                | "bestofnrunner"
        )
    }
    (!sub_type.is_empty() && lane_ok(sub_type)) || (!agent_type.is_empty() && lane_ok(agent_type))
}

fn review_subagent_kind_ok(sub_type: &str, agent_type: &str) -> bool {
    if cursor_review_gate_disabled_by_env() {
        return review_subagent_kind_ok_loose_when_cursor_gate_disabled(sub_type, agent_type);
    }
    // 默认可清除 `REVIEW_GATE` 的深度审稿 lane：**不**含 `explore` / CI / guide（只做辅查，不算一轮独立深度 reviewer）。
    (!sub_type.is_empty() && crate::hook_common::is_deep_review_gate_lane_normalized(sub_type))
        || (!agent_type.is_empty()
            && crate::hook_common::is_deep_review_gate_lane_normalized(agent_type))
}

fn first_nonempty_tool_or_event_str(event: &Value, tool_input: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = tool_input.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(value) = event.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

fn review_subagent_cycle_key(
    event: &Value,
    tool_input: &Value,
    sub_type: &str,
    agent_type: &str,
) -> Option<String> {
    let id = first_nonempty_tool_or_event_str(
        event,
        tool_input,
        &[
            "subagent_id",
            "subagentId",
            "agent_id",
            "agentId",
            "task_id",
            "taskId",
            "run_id",
            "runId",
            "id",
        ],
    );
    if !id.is_empty() {
        return Some(format!("id:{id}"));
    }
    let lane = if !sub_type.is_empty() {
        sub_type
    } else {
        agent_type
    };
    if lane.is_empty() {
        None
    } else {
        Some(format!("lane:{lane}"))
    }
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
    /// 仅统计 **`SubagentStart`** 上 qualifying review 入队次数；**`PostToolUse`**  multiset 入队不递增（与 `review_subagent_pending_cycle_keys` 长度不同步属刻意）。
    pub subagent_start_count: u32,
    pub subagent_stop_count: u32,
    pub followup_count: u32,
    pub review_followup_count: u32,
    pub goal_followup_count: u32,
    pub goal_required: bool,
    /// `/implementx|/verifyx` 本轮已武装（my-light 下可不设 `goal_required` 但仍跟踪 pre-goal）。
    #[serde(default)]
    pub goal_drive_entry_active: bool,
    pub goal_contract_seen: bool,
    pub goal_progress_seen: bool,
    pub goal_verify_or_block_seen: bool,
    /// My implement pre-goal：在 goal 契约与收口证据之前，要求独立上下文 subagent 预检（或拒绝原因词）。
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
    #[serde(default)]
    pub review_subagent_cycle_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_subagent_cycle_key: Option<String>,
    /// 武装 review gate 后，每次 qualifying subagent **start**（PostToolUse / subagentStart）压入一条 cycle key（multiset）；qualifying **stop** 命中时**移除一条**同 key 记录，**仅当**本队列为空时升相位 3 并记 `subagent_stop_count`。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_subagent_pending_cycle_keys: Vec<String>,
    /// Set when multiset push refused at cap (operator-visible on Stop).
    #[serde(default)]
    pub review_pending_cap_refused: bool,
    /// PostTool / subagentStart pending push timestamp for orphan stale recovery when count==0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_pending_last_pushed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn merge_fail_closed_user_messages(out: &mut Value, msg: &str) {
    out["followup_message"] = Value::String(msg.to_string());
    out["user_message"] = Value::String(msg.to_string());
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

fn is_assistant_message_role(obj: &serde_json::Map<String, Value>) -> bool {
    let role = obj
        .get("role")
        .or_else(|| obj.get("type"))
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(
        role.as_str(),
        "assistant" | "ai" | "model" | "bot" | "agent"
    )
}

fn message_body_text(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["content", "text", "body", "value"] {
        let Some(value) = obj.get(key) else {
            continue;
        };
        match value {
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

/// `Stop` / 部分宿主事件不把助手正文放在顶层 `response` / `content`；与 `prompt_from_nested_messages`
/// 对称，从 `messages`（及嵌套 payload）**逆序**取最后一条助手消息，避免 `signal_text` 缺助手段导致
/// `has_structured_goal_contract` 永远失败、反复注入 `AG_FOLLOWUP missing_parts=goal_contract`。
fn agent_response_from_nested_messages(event: &Value) -> String {
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
                    if !is_assistant_message_role(msg) {
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
                let s = agent_response_from_nested_messages(nested);
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
    let direct = first_nonempty_event_str(event, KEYS);
    if !direct.trim().is_empty() {
        return direct;
    }
    agent_response_from_nested_messages(event)
}

/// 从整棵 hook JSON 抓取字符串（深度与总字节上限），仅用于显式兼容 fallback。
/// 默认热路径只读结构化字段，避免长会话 transcript 把末尾用户输入挤出预算。
const HOOK_JSON_STRING_SCRAPE_CAP: usize = 2 * 1024 * 1024;
const HOOK_JSON_STRING_SCRAPE_MAX_DEPTH: u32 = 48;
const CURSOR_FULL_JSON_SCRAPE_ENV: &str = "ROUTER_RS_CURSOR_HOOK_FULL_JSON_SCRAPE";

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

fn cursor_full_json_scrape_enabled() -> bool {
    std::env::var(CURSOR_FULL_JSON_SCRAPE_ENV)
        .ok()
        .map(|raw| {
            let value = raw.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn hook_event_signal_text_with_scrape_mode(
    event: &Value,
    prompt: &str,
    response: &str,
    full_scrape: bool,
) -> String {
    let response = crate::hook_common::hook_assistant_tail_window(
        response,
        crate::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    );
    let mut s = String::with_capacity(
        prompt
            .len()
            .saturating_add(response.len())
            .saturating_add(4096),
    );
    s.push_str(prompt);
    s.push('\n');
    s.push_str(&response);
    s.push('\n');
    if full_scrape {
        s.push_str(&hook_event_all_text(event));
    }
    s
}

/// 结构化字段解析；显式开关启用时再追加全树字符串兼容未知宿主路径。
fn hook_event_signal_text(event: &Value, prompt: &str, response: &str) -> String {
    hook_event_signal_text_with_scrape_mode(
        event,
        prompt,
        response,
        cursor_full_json_scrape_enabled(),
    )
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

pub(crate) fn tool_name_of(event: &Value) -> String {
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
    crate::hook_common::tool_input_value_from_map(obj)
}

pub(crate) fn tool_input_of(event: &Value) -> Value {
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
    "session_id",
    "sessionId",
    "parent_session_id",
    "parentSessionId",
    "root_session_id",
    "thread_id",
    "threadId",
    "chat_id",
    "conversation_id",
    "conversationId",
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

pub(crate) fn extract_first_session_string(event: &Value) -> Option<String> {
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
    if let Some(root) = event.as_object() {
        if let Some(s) = try_extract_session_from_object(root) {
            return Some(s);
        }
    }
    // Parent session in tool_input must win over nested `hookPayload.conversation_id` (subagent threads).
    if let Some(s) = try_extract_parent_session_from_tool_json(&tool_input_of(event)) {
        return Some(s);
    }
    if let Some(s) = extract_first_session_string(event) {
        return Some(s);
    }
    min_priority_session_identity_from_hook_json(event)
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

fn hook_state_lock_fail_closed_for_review_json() -> Value {
    json!({
        "permission": "deny",
        "user_message": "router-rs：`.cursor/hook-state` 锁不可用，review 证据路径已 fail-closed。请检查目录权限或争用后重试。"
    })
}

/// Best-effort read without holding the session lock (TOCTOU-safe only for fail-closed branches).
fn peek_review_hard_armed(repo_root: &Path, event: &Value) -> bool {
    for _ in 0..3 {
        match load_state(repo_root, event) {
            Ok(Some(ref state)) => return review_hard_armed(state),
            Ok(None) => return false,
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    false
}

fn hook_state_lock_failure_output(repo_root: &Path, event: &Value) -> Value {
    if peek_review_hard_armed(repo_root, event) {
        hook_state_lock_fail_closed_for_review_json()
    } else {
        hook_lock_unavailable_notice_json()
    }
}

/// Live subagent cycle evidence (start/stop/pending). Excludes legacy `phase>=2` alone (wave-2 / P0-4).
fn review_subagent_live_evidence_seen(state: &ReviewGateState) -> bool {
    state.subagent_start_count > 0
        || state.subagent_stop_count > 0
        || !state.review_subagent_pending_cycle_keys.is_empty()
}

/// My execution-zone commands arm goal continuity gates (`/implementx`, `/verifyx`).
fn is_framework_goal_drive_entry_prompt(prompt: &str, signal_text: &str) -> bool {
    let _ = signal_text;
    crate::hook_common::is_framework_goal_entry_prompt(prompt)
}

/// 显式委托/并行入口走 bounded sidecar gate；goal 入口（My 执行区 `/implementx` 等）只走 goal 机。
fn framework_prompt_arms_delegation(text: &str) -> bool {
    crate::hook_common::is_framework_non_goal_entrypoint_prompt(text)
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

fn prune_session_terminal_ledger(ledger: &mut SessionTerminalLedger) {
    ledger.owned_pids.retain(|pid| is_process_alive(*pid));
}

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
    let mut ledger =
        serde_json::from_str::<SessionTerminalLedger>(&raw).unwrap_or(SessionTerminalLedger {
            version: SESSION_TERMINAL_LEDGER_VERSION,
            baseline_pids: Vec::new(),
            owned_pids: Vec::new(),
            pending_shells: Vec::new(),
        });
    prune_session_terminal_ledger(&mut ledger);
    ledger
}

fn save_session_terminal_ledger(repo_root: &Path, event: &Value, ledger: &SessionTerminalLedger) {
    let path = session_terminal_ledger_path(repo_root, event);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut ledger = ledger.clone();
    prune_session_terminal_ledger(&mut ledger);
    if let Ok(text) = serde_json::to_string_pretty(&ledger) {
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

#[cfg(unix)]
struct UnixLockState {
    path: PathBuf,
    _file: std::fs::File,
}

#[cfg(windows)]
struct WindowsStateMutex {
    handle: *mut std::ffi::c_void,
}

#[cfg(windows)]
impl WindowsStateMutex {
    fn acquire(session_key: &str, timeout_ms: u32) -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null_mut;

        type LPCWSTR = *const u16;
        type HANDLE = *mut std::ffi::c_void;
        type DWORD = u32;
        type BOOL = i32;

        const WAIT_OBJECT_0: DWORD = 0x00000000;
        const WAIT_ABANDONED: DWORD = 0x00000080;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateMutexW(lpMutexAttributes: *mut std::ffi::c_void, bInitialOwner: BOOL, lpName: LPCWSTR) -> HANDLE;
            fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
            fn CloseHandle(hObject: HANDLE) -> BOOL;
            fn GetLastError() -> DWORD;
        }

        let mutex_name = format!("Local\\review-subagent-lock-{}", session_key);
        let mut name_w: Vec<u16> = OsStr::new(&mutex_name).encode_wide().collect();
        name_w.push(0);

        unsafe {
            let handle = CreateMutexW(null_mut(), 0, name_w.as_ptr());
            if handle.is_null() {
                return Err(format!("CreateMutexW failed with GetLastError={}", GetLastError()));
            }
            let wait_res = WaitForSingleObject(handle, timeout_ms);
            if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
                Ok(Self { handle })
            } else {
                CloseHandle(handle);
                Err(format!("WaitForSingleObject lock timeout/failed (res={})", wait_res))
            }
        }
    }

    fn release(self) {
        unsafe {
            #[link(name = "kernel32")]
            extern "system" {
                fn ReleaseMutex(hMutex: *mut std::ffi::c_void) -> i32;
                fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            }
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub(crate) struct LockGuard {
    #[cfg(unix)]
    unix: UnixLockState,
    #[cfg(windows)]
    windows: WindowsStateMutex,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    #[cfg(test)]
    if should_force_hook_state_lock_failure_for_test() {
        return None;
    }
    let wait_start = std::time::Instant::now();
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let session = session_key(event);
    let lock_path = state_lock_path(repo_root, event);

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use fs2::FileExt;

        let retries = crate::router_env_flags::router_rs_cursor_hook_state_lock_retries();
        for _ in 0..retries {
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
            {
                Ok(file) => file,
                Err(_) => {
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
            };

            match file.try_lock_exclusive() {
                Ok(()) => {
                    let fd_metadata = match file.metadata() {
                        Ok(meta) => meta,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    };
                    let fd_inode = fd_metadata.ino();

                    let path_inode = match fs::metadata(&lock_path) {
                        Ok(meta) => Some(meta.ino()),
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    };

                    if Some(fd_inode) != path_inode {
                        drop(file);
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    let lock_text = format!("pid={} ts={}\n", std::process::id(), now_millis());
                    let mut owned = file;
                    let _ = owned.set_len(0);
                    use std::io::Seek;
                    let _ = owned.seek(std::io::SeekFrom::Start(0));
                    let _ = owned.write_all(lock_text.as_bytes());
                    let _ = owned.sync_all();

                    crate::hook_timing::add_lock_wait_ms(wait_start.elapsed().as_millis() as u64);
                    return Some(LockGuard {
                        unix: UnixLockState {
                            path: lock_path,
                            _file: owned,
                        }
                    });
                }
                Err(_) => {
                    drop(file);
                    const HOOK_STATE_LOCK_STALE_MS: u64 = 30_000;
                    if let Ok(existing) = fs::read_to_string(&lock_path) {
                        if let Some((pid, ts_ms)) = parse_lock_metadata(&existing) {
                            let age_ms = now_millis().saturating_sub(ts_ms);
                            if !is_process_alive(pid) {
                                // Do not delete to preserve POSIX flock inode guarantee.
                            } else if age_ms > HOOK_STATE_LOCK_STALE_MS {
                                eprintln!(
                                    "[router-rs] hook-state lock held (pid={pid} age_ms={age_ms}); waiting (no remove_file)"
                                );
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        None
    }

    #[cfg(windows)]
    {
        match WindowsStateMutex::acquire(&session, 3500) {
            Ok(win_lock) => {
                crate::hook_timing::add_lock_wait_ms(wait_start.elapsed().as_millis() as u64);
                Some(LockGuard {
                    windows: win_lock
                })
            }
            Err(e) => {
                eprintln!("[router-rs] Windows NamedMutex acquisition failed: {}", e);
                None
            }
        }
    }
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
fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::raw::HANDLE;

        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> HANDLE;
            fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut u32) -> i32;
            fn CloseHandle(hObject: HANDLE) -> i32;
        }

        if pid == 0 {
            return false;
        }

        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return std::io::Error::last_os_error().raw_os_error() != Some(87);
            }

            let mut exit_code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);

            if ok != 0 {
                exit_code == STILL_ACTIVE
            } else {
                true
            }
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn release_state_lock(lock: &mut Option<LockGuard>) {
    if let Some(guard) = lock.take() {
        #[cfg(unix)]
        {
            drop(guard.unix);
        }
        #[cfg(windows)]
        {
            guard.windows.release();
        }
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
        goal_drive_entry_active: false,
        goal_contract_seen: false,
        goal_progress_seen: false,
        goal_verify_or_block_seen: false,
        pre_goal_review_satisfied: false,
        pre_goal_nag_count: 0,
        last_prompt: None,
        last_subagent_type: None,
        last_subagent_tool: None,
        lane_intent_matches: None,
        review_subagent_cycle_open: false,
        review_subagent_cycle_key: None,
        review_subagent_pending_cycle_keys: Vec::new(),
        review_pending_cap_refused: false,
        review_pending_last_pushed_at: None,
        updated_at: None,
    }
}

fn sync_review_cycle_legacy_fields(state: &mut ReviewGateState) {
    state.review_subagent_cycle_open = !state.review_subagent_pending_cycle_keys.is_empty();
    state.review_subagent_cycle_key = state.review_subagent_pending_cycle_keys.last().cloned();
}

fn hydrate_legacy_review_cycles_into_pending(state: &mut ReviewGateState) {
    if !state.review_subagent_pending_cycle_keys.is_empty() {
        sync_review_cycle_legacy_fields(state);
        return;
    }
    if state.review_subagent_cycle_open {
        if let Some(k) = state.review_subagent_cycle_key.clone() {
            state.review_subagent_pending_cycle_keys.push(k);
        }
    }
    sync_review_cycle_legacy_fields(state);
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
        if let Some(arr) = obj
            .get("review_subagent_pending_cycle_keys")
            .and_then(Value::as_array)
        {
            base.review_subagent_pending_cycle_keys = arr
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect();
        }
        if let Some(v) = obj
            .get("review_subagent_cycle_open")
            .and_then(Value::as_bool)
        {
            base.review_subagent_cycle_open = v;
        }
        if let Some(Value::String(v)) = obj.get("review_subagent_cycle_key") {
            let t = v.trim();
            if !t.is_empty() {
                base.review_subagent_cycle_key = Some(t.to_string());
            }
        }
    }
    hydrate_legacy_review_cycles_into_pending(&mut base);
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
    if crate::router_env_flags::router_rs_cursor_hook_state_file_sync_enabled() {
        if file.sync_all().is_err() {
            let _ = fs::remove_file(&tmp);
            return false;
        }
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    #[cfg(unix)]
    if crate::router_env_flags::router_rs_cursor_hook_state_dir_sync_enabled() {
        if let Ok(dir_file) = OpenOptions::new().read(true).open(&directory) {
            let _ = dir_file.sync_all();
        }
    }
    true
}
/// 仅 **review** 路径的硬门控（独立上下文 subagent 证据链）；**不包含** `delegation_required`。
fn review_hard_armed(state: &ReviewGateState) -> bool {
    review_gate_armed(state.review_required, state.review_override)
}

/// Stop：`review` 场景下独立 subagent 证据是否满足（phase≥3 且 pending multiset 已排空）。
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    state.phase >= 3 && state.review_subagent_pending_cycle_keys.is_empty()
}

fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    review_hard_armed(state) && !review_subagent_evidence_satisfied(state)
}

/// Compact bump requires live cycle progress beyond orphan `subagent_start_count` (stale hygiene may clear pending).
fn compact_bump_review_evidence_seen(state: &ReviewGateState) -> bool {
    review_subagent_live_evidence_seen(state)
        && (state.subagent_stop_count > 0 || !state.review_subagent_pending_cycle_keys.is_empty())
}

/// 主线程 compact findings **不得**在无可数深度子代理证据时单独升 phase 3 清 REVIEW_GATE（P0-4 / wave-2）。
fn maybe_bump_review_phase_for_main_thread_compact_findings(
    state: &mut ReviewGateState,
    assistant_tail: &str,
) -> bool {
    if !review_hard_armed(state) || state.phase >= 3 {
        return false;
    }
    if !compact_bump_review_evidence_seen(state) {
        return false;
    }
    if !state.review_subagent_pending_cycle_keys.is_empty() {
        return false;
    }
    if !crate::review_output_lint::assistant_has_substantive_compact_review_finding_line(
        assistant_tail,
    ) {
        return false;
    }
    bump_phase(state, 3);
    clear_review_gate_escalation_counters(state);
    true
}

/// Stop 硬门控（REVIEW_GATE / AG_FOLLOWUP）与 My/RFV 续跑注入互斥。
fn stop_hard_gate_blocks_continuity_merge(state: &ReviewGateState) -> bool {
    review_stop_followup_needed(state)
        || (tracks_goal_or_drive_entry(state) && !goal_is_satisfied(state))
}

/// Stop / 观测 fixture 共用的 `need=` 段（前缀仍须含 `REVIEW_GATE` 供 `router_rs_observation` 分类）。
pub(crate) const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n|deep-reviewer fork_context=false";

/// Short, stable tail for `REVIEW_GATE incomplete` lines (after `need=`). Does not change the first
/// `router-rs` token (`REVIEW_GATE`) used by observation classification.
pub(crate) const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

fn review_stop_followup_line(state: &ReviewGateState) -> String {
    let cap_need = if state.review_pending_cap_refused {
        format!(
            " need=pending_cycle_keys_at_cap max={}",
            crate::router_env_flags::router_rs_cursor_review_pending_cycle_max()
        )
    } else {
        String::new()
    };
    format!(
        "router-rs REVIEW_GATE incomplete phase={} {}{} {}",
        state.phase,
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT,
        cap_need,
        REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

/// `merge_hook_nudge_paragraph` 去重前缀：首行须与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 常量一致以便每轮刷新同一段落。
pub(crate) const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

pub(crate) const CURSOR_HOOK_STATE_UNREADABLE: &str =
    "router-rs CURSOR_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

/// 超过「完整硬行」上限后写入 `followup_message` 的短行（仍以 `router-rs REVIEW_GATE` 开头供观测分类）。
pub(crate) fn review_stop_followup_soft_line(
    state: &ReviewGateState,
    full_line_cap: u32,
) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete mode=soft_nag full_line_cap={full_line_cap} phase={} stop_nudge_count={} see=.cursor/hook-state rg_clear|ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1|ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES=0(strict)|detail=additional_context",
        state.phase, state.review_followup_count
    )
}

/// 完整 `need=`/`hint=` 行：降级到 `additional_context` 时与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 首行合并。
pub(crate) fn review_stop_followup_detail_paragraph(state: &ReviewGateState) -> String {
    format!(
        "{}\n{}",
        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
        review_stop_followup_line(state)
    )
}

fn is_overridden(state: &ReviewGateState) -> bool {
    state.review_override || state.delegation_override
}

fn tracks_goal_or_drive_entry(state: &ReviewGateState) -> bool {
    state.goal_required || state.goal_drive_entry_active
}

fn goal_is_satisfied(state: &ReviewGateState) -> bool {
    if !tracks_goal_or_drive_entry(state) {
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

fn my_pre_goal_followup_message() -> String {
    "My implement (/implementx)：先写清 Goal 契约与验证口径（`GOAL_STATE.json`）；需要时再并行分工与证据索引（建议，非硬门槛）。确为小任务请**单独一行**拒因 token（如 small_task），不要自拟仿宿主 `router-rs …` 续跑行。"
        .to_string()
}

/// 连续 pre-goal 提示上限（**仅**显式 env 启用）：beforeSubmit 每轮在仍缺 pre-goal 时累加计数，达到后自动 `pre_goal_review_satisfied=true`。
/// - **未设置** / `0` / `false` / `off` / `no`：**不**自动放行（默认严格，P1-1）。
/// - 正整数：自定义上限（运维 opt-in）。
fn cursor_autopilot_pre_goal_max_nudges_cap() -> Option<u32> {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES") else {
        return None;
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<u32>().ok().filter(|v| *v >= 1)
}

fn maybe_autopilot_pre_goal_nag_cap_release(state: &mut ReviewGateState) -> Option<&'static str> {
    if !crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled() {
        return None;
    }
    if !tracks_goal_or_drive_entry(state)
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

/// 本地逃生舱：**仅当** `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 为 `1` / `true` / `yes` / `on`（大小写不敏感）时关闭门控；
/// unset、空串或其它任意值 **保持门控启用**（对齐 `router_rs_env_enabled_default_false`，避免任意非空误触）。
fn cursor_review_gate_disabled_by_env() -> bool {
    #[cfg(test)]
    {
        if let Some(v) = TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.get()) {
            return v;
        }
    }
    crate::router_env_flags::router_rs_env_enabled_default_false(
        "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
    )
}

/// Env disable **or** `lifecycle_profile: my-light` (prompt / GOAL_STATE) — profile-scoped, not global.
fn cursor_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if cursor_review_gate_disabled_by_env() {
        return true;
    }
    if !crate::hook_common::my_light_profile_active(Some(repo_root), text) {
        return false;
    }
    crate::runtime_registry::lifecycle_profile_disables_review_gate_hard_block(
        Some(repo_root),
        "my-light",
    )
    .unwrap_or(true)
}

/// `subagentStart` 只能拒绝/提示，不能主动关闭既有 subagent；这里用活跃数避免继续堆积。
fn cursor_max_open_subagents() -> Option<u32> {
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

fn cursor_open_subagent_stale_after_secs() -> Option<i64> {
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

fn review_pending_cycle_cap_denial(cap: usize) -> Value {
    json!({
        "permission": "deny",
        "user_message": format!(
            "router-rs：review 子代理 pending 已达上限 {cap}（ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX）。请先等待已有 review subagentStop 核销 pending，或 Stop 后按 REVIEW_GATE 指引清门（rg_clear / 完成深度 lane）。"
        )
    })
}

/// When `ROUTER_RS_CURSOR_HOOK_SILENT=1`: drop advisory `additional_context`; keep hard
/// `followup_message` lines that start with the `router-rs ` leader prefix.
pub(crate) fn apply_cursor_hook_silent_policy(output: &mut Value) {
    if !crate::router_env_flags::router_rs_cursor_hook_silent_enabled() {
        return;
    }
    if let Some(obj) = output.as_object_mut() {
        obj.remove("additional_context");
    }
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        let kept: Vec<&str> = s
            .lines()
            .filter(|line| line.trim_start().starts_with("router-rs "))
            .collect();
        if kept.is_empty() {
            if let Some(obj) = output.as_object_mut() {
                obj.remove("followup_message");
            }
        } else {
            *s = kept.join("\n");
        }
    }
}

pub(crate) fn apply_cursor_hook_output_policy(output: &mut Value) {
    crate::router_rs_observation::attach_router_rs_observation(
        output,
        crate::router_rs_observation::HookObservationHost::Cursor,
    );
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    if let Some(Value::String(s)) = output.get_mut("additional_context") {
        let next = truncate_cursor_hook_outbound_context_preserving_gate(s.as_str(), max_out);
        *s = next;
    }

    let absurd_followup_threshold =
        crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes()
            .saturating_mul(4)
            .max(32 * 1024);
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        if s.len() > absurd_followup_threshold {
            *s = truncate_cursor_hook_followup_preserving_review_gate(s.as_str(), max_out);
        }
    }
}

/// Cursor outbound truncation: UTF-8 byte cap; prefix retained; **fixed suffix** so operators can
/// tell budget clipping from gate logic. (Variable names may say `_CHARS`; semantics are bytes.)
pub(crate) const CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX: &str = "...[~trunc]";

/// Cursor 出站 `additional_context` / 极端 `followup_message`：**UTF-8 字节预算**，前缀优先，末尾固定
/// [`CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX`]（与 Codex `truncate_codex_additional_context_bytes` 的 `...` 相比更可观测）。
fn truncate_cursor_hook_outbound_context(combined: &str, max_bytes: usize) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    // `combined` may be borrowed; allocation only when truncating.
    let suf = CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX;
    let suf_len = suf.len();
    if max_bytes <= suf_len {
        let mut cut = max_bytes.min(combined.len());
        while cut > 0 && !combined.is_char_boundary(cut) {
            cut -= 1;
        }
        return combined[..cut].to_string();
    }
    let budget = max_bytes.saturating_sub(suf_len);
    let mut cut = budget.min(combined.len());
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    if let Some(pos) = combined[..cut].rfind('\n') {
        if pos > 0 {
            cut = pos;
        }
    }
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &combined[..cut], suf)
}

fn cursor_hook_outbound_line_is_protected(line: &str) -> bool {
    let t = line.trim_start();
    t.contains("router-rs REVIEW_GATE")
        || t.starts_with(REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX)
        || t.contains("continuity_suppressed=")
}

/// Outbound truncation: keep REVIEW_GATE / continuity_suppressed lines; truncate filler.
pub(crate) fn truncate_cursor_hook_outbound_context_preserving_gate(
    combined: &str,
    max_bytes: usize,
) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let mut protected: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for line in combined.lines() {
        if cursor_hook_outbound_line_is_protected(line) {
            protected.push(line);
        } else {
            rest.push(line);
        }
    }
    let protected_body = protected.join("\n");
    if protected_body.len() >= max_bytes {
        return truncate_cursor_hook_outbound_context(&protected_body, max_bytes);
    }
    let rest_body = rest.join("\n");
    if rest_body.is_empty() {
        return protected_body;
    }
    let sep_len = if protected_body.is_empty() { 0 } else { 1 };
    let rest_budget = max_bytes.saturating_sub(protected_body.len() + sep_len);
    let truncated_rest = truncate_cursor_hook_outbound_context(&rest_body, rest_budget);
    if protected_body.is_empty() {
        truncated_rest
    } else if truncated_rest.is_empty() {
        protected_body
    } else {
        let mut out = protected_body;
        out.push('\n');
        out.push_str(&truncated_rest);
        if out.len() > max_bytes {
            truncate_cursor_hook_outbound_context(&out, max_bytes)
        } else {
            out
        }
    }
}

fn truncate_cursor_hook_followup_preserving_review_gate(
    combined: &str,
    max_bytes: usize,
) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let mut gate: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for line in combined.lines() {
        if line.trim_start().starts_with("router-rs REVIEW_GATE") {
            gate.push(line);
        } else {
            rest.push(line);
        }
    }
    let gate_body = gate.join("\n");
    if gate_body.len() >= max_bytes {
        return truncate_cursor_hook_outbound_context(&gate_body, max_bytes);
    }
    let rest_body = rest.join("\n");
    if rest_body.is_empty() {
        return gate_body;
    }
    let sep_len = if gate_body.is_empty() { 0 } else { 1 };
    let rest_budget = max_bytes.saturating_sub(gate_body.len() + sep_len);
    let truncated_rest = truncate_cursor_hook_outbound_context(&rest_body, rest_budget);
    if gate_body.is_empty() {
        truncated_rest
    } else if truncated_rest.is_empty() {
        gate_body
    } else {
        let mut out = gate_body;
        out.push('\n');
        out.push_str(&truncated_rest);
        if out.len() > max_bytes {
            truncate_cursor_hook_outbound_context(&out, max_bytes)
        } else {
            out
        }
    }
}

/// 应急关闭门控时仍执行 PostToolUse/Subagent 状态更新，但不对模型注入门控类提示（与 SILENT 剥离字段一致）。
fn strip_cursor_hook_user_visible_nags(output: &mut Value) {
    if let Some(obj) = output.as_object_mut() {
        obj.remove("followup_message");
        obj.remove("additional_context");
        crate::router_rs_observation::strip_router_rs_observation(output);
    }
}

/// 清门或 subagent 满足 review 后归零，避免 `followup_count` 长期累积导致 **escalation** 粘住。
fn clear_review_gate_escalation_counters(state: &mut ReviewGateState) {
    state.followup_count = 0;
    state.review_followup_count = 0;
    state.pre_goal_nag_count = 0;
}

/// Reset review-cycle progress (phase / pending / subagent counters). Parity with Codex UPS
/// when my-light disarms review, goal drive suppresses review, or a fresh deep-review cycle starts.
///
/// When `preserve_session_guards` is true (fresh deep-review re-arm), retain pending-cap refusal
/// so `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` cannot be bypassed via UPS. Open-subagent count
/// always resets on re-arm (P1-16: stale count without matching subagentStop).
fn reset_review_cycle_progress(state: &mut ReviewGateState, preserve_session_guards: bool) {
    state.phase = 0;
    state.subagent_start_count = 0;
    state.subagent_stop_count = 0;
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    if !preserve_session_guards {
        state.review_pending_cap_refused = false;
    }
    state.review_subagent_pending_cycle_keys.clear();
    state.review_pending_last_pushed_at = None;
    state.review_followup_count = 0;
    sync_review_cycle_legacy_fields(state);
}

/// Same-submit review + My goal drive: review stays disarmed; operator-visible split hint (non-my-light only).
const CURSOR_REVIEW_MY_SAME_ROUND_NUDGE: &str = "router-rs：本轮提交同时包含「代码审查 / review」信号与 My 执行区入口（`/implementx`、`/verifyx`）；门控下 **不会** 在本回合因 review 措辞新武装 `REVIEW_GATE`。若需先跑独立审稿，请拆开用户消息（先发 review-only，再发 `/implementx`）或先落盘 `GOAL_STATE`。详见 `docs/framework_operator_primer.md`。";

/// `GOAL_STATE` 列表字段是否含至少一条非空字符串（避免 `[""]` 这种伪非空数组）。
/// 用 `GOAL_STATE.json` + `EVIDENCE_INDEX.json` 补全 goal 门控（只置 true，不收回）；逻辑在 `ship_readiness.rs`。
///
/// `arm_if_goal_file`：**Stop** 路径传 `true` 以便在 GOAL 被 purge 时清除陈旧的 `goal_required`；
/// **不再**因盘上残留 GOAL 而武装 `goal_required`。
/// **beforeSubmit** 传 `false`。
///
/// **`pre_goal_review_satisfied`（磁盘旁路）**：在 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` 开启时
/// **不**因仅存在磁盘 GOAL 而置真（beforeSubmit 与 Stop 均适用）；其余 goal 字段的 hydrate
/// （contract/progress/verify 等）仍执行。
fn hydrate_goal_gate_from_disk(
    repo_root: &Path,
    state: &mut ReviewGateState,
    arm_if_goal_file: bool,
    frame: &crate::task_state::CursorContinuityFrame,
    goal_drive_entrypoint: bool,
) {
    if !state.goal_required
        && !arm_if_goal_file
        && !goal_drive_entrypoint
        && !state.goal_drive_entry_active
    {
        return;
    }
    let Some((goal, task_id)) = frame.hydration_goal.as_ref() else {
        // Stop-only: verifyx purge removes GOAL_STATE while hook-state may still carry
        // `goal_required` from an earlier /implementx|/verifyx arm.
        if arm_if_goal_file && state.goal_required {
            state.goal_required = false;
            state.goal_drive_entry_active = false;
        }
        return;
    };
    if !crate::router_env_flags::router_rs_cursor_pre_goal_strict_disk_enabled()
        && (state.goal_required || goal_drive_entrypoint)
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
    }
    if state.goal_required || arm_if_goal_file || state.goal_drive_entry_active {
        let readiness = crate::ship_readiness::evaluate_goal_readiness_from_disk(
            repo_root,
            goal,
            task_id.as_str(),
        );
        if readiness.contract {
            state.goal_contract_seen = true;
        }
        if readiness.progress {
            state.goal_progress_seen = true;
        }
        if readiness.verification {
            state.goal_verify_or_block_seen = true;
        }
    }
}

/// Stop 上的 goal 门控短码（磁盘优先 evaluator；见 `ship_readiness.rs`）。
fn goal_stop_followup_line(state: &ReviewGateState) -> String {
    crate::ship_readiness::goal_stop_followup_line(
        state.goal_contract_seen,
        state.goal_progress_seen,
        state.goal_verify_or_block_seen,
        state.goal_followup_count,
    )
}

fn state_lock_degraded_followup() -> &'static str {
    "router-rs：hook-state 锁不可用，本闸门控降级。收口前须见独立 subagent lane，或在**用户消息**中单独一行写拒因。"
}

fn lock_failure_followup_for_before_submit(repo_root: &Path, event: &Value) -> (bool, String) {
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text);
    let disk_review_armed = load_state(repo_root, event)
        .ok()
        .flatten()
        .is_some_and(|s| review_hard_armed(&s));

    let strong_constraint =
        ((review_arms || delegation || goal_drive_entrypoint) && !overridden) || disk_review_armed;
    if strong_constraint {
        return (
            false,
            "router-rs：hook-state 锁不可用，本条为严格 review/委托/My 执行区门控，**已拦截提交**。请修锁/权限后重试，或起 subagent / 写明拒因。"
                .to_string(),
        );
    }

    (
        true,
        "router-rs：hook-state 锁不可用，门控**降级**；非严格提示仍可继续。".to_string(),
    )
}

fn stop_lock_failure_is_fail_closed(repo_root: &Path, event: &Value) -> bool {
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text) || saw_reject_reason(&signal_text, &text);
    let disk_review_armed = load_state(repo_root, event)
        .ok()
        .flatten()
        .is_some_and(|s| review_hard_armed(&s) || s.goal_required);
    ((review_arms || delegation || goal_drive_entrypoint) && !overridden) || disk_review_armed
}

fn review_gate_stop_lock_unavailable_line() -> String {
    format!(
        "router-rs REVIEW_GATE incomplete phase=0 {} hook_state_lock_unavailable {}",
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

fn lock_failure_followup_for_stop(repo_root: &Path, event: &Value) -> String {
    if stop_lock_failure_is_fail_closed(repo_root, event) {
        return review_gate_stop_lock_unavailable_line();
    }
    state_lock_degraded_followup().to_string()
}
/// 将一条 `review_subagent_cycle_key` 压入 multiset 并同步 legacy 字段。
///
/// **双事件去重**：宿主可能对同一子代理先发 `subagentStart` 再发 `PostToolUse`（同一 `subagent_id`）。对 **`id:`** 前缀的稳定 key，若 pending 已含该字符串，则 **PostToolUse 路径不再 push**，避免「一次 stop 只核销一条」语义下出现双 pending。
///
/// **`subagent_start_count`** 仅在 **`handle_subagent_start`** 的 qualifying review 分支递增；PostToolUse 仅负责 multiset 入队（及 phase bump），**不**增加该计数，以免与宿主双事件重复计数。
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PendingCyclePush {
    NewlyInserted,
    AlreadyPresent,
    AtCap,
}

fn push_review_pending_cycle_key(
    state: &mut ReviewGateState,
    cycle_key: Option<String>,
    from_posttool: bool,
) -> PendingCyclePush {
    let Some(k) = cycle_key else {
        return PendingCyclePush::AtCap;
    };
    if from_posttool && state.review_subagent_pending_cycle_keys.contains(&k) {
        return PendingCyclePush::AlreadyPresent;
    }
    if !from_posttool
        && k.starts_with("id:")
        && state.review_subagent_pending_cycle_keys.contains(&k)
    {
        return PendingCyclePush::AlreadyPresent;
    }
    let max = crate::router_env_flags::router_rs_cursor_review_pending_cycle_max();
    if state.review_subagent_pending_cycle_keys.len() >= max {
        eprintln!("[router-rs] review_pending_cycle_keys_at_cap_refused cap={max} key={k}");
        state.review_pending_cap_refused = true;
        return PendingCyclePush::AtCap;
    }
    state.review_subagent_pending_cycle_keys.push(k);
    state.review_pending_last_pushed_at = Some(Utc::now().to_rfc3339());
    sync_review_cycle_legacy_fields(state);
    PendingCyclePush::NewlyInserted
}

/// Clear pending review cycle keys when subagent activity is stale (avoids permanent REVIEW_GATE).
fn prune_stale_review_pending_cycle_keys(state: &mut ReviewGateState) {
    if state.review_subagent_pending_cycle_keys.is_empty() {
        return;
    }
    let Some(stale_after_secs) = cursor_open_subagent_stale_after_secs() else {
        // Align with `reset_stale_active_subagents`: stale recovery off → do not prune pending.
        return;
    };
    if state.active_subagent_count == 0 {
        let raw = state
            .active_subagent_last_started_at
            .as_deref()
            .or(state.review_pending_last_pushed_at.as_deref());
        let Some(raw) = raw else {
            eprintln!(
                "[router-rs] review_pending_orphan_no_timestamp: skip clear (v1 migrate safety)"
            );
            return;
        };
        let clear = chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|started_at| {
                let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
                age.num_seconds() > stale_after_secs
            })
            .unwrap_or(false);
        if clear {
            // Anti false-negative: stale orphan recovery must not satisfy Stop without qualifying stop.
            if state.subagent_stop_count == 0 && state.phase >= 3 {
                state.phase = 2;
            }
            eprintln!(
                "[router-rs] cleared review_subagent_pending_cycle_keys (no open subagents, stale pending)"
            );
            state.review_subagent_pending_cycle_keys.clear();
            sync_review_cycle_legacy_fields(state);
        }
        return;
    }
    let Some(started_at) = state.active_subagent_last_started_at.as_deref() else {
        return;
    };
    let Ok(started_at) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return;
    };
    let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
    if age.num_seconds() <= stale_after_secs {
        return;
    }
    state.review_subagent_pending_cycle_keys.clear();
    sync_review_cycle_legacy_fields(state);
}

fn apply_subagent_stale_hygiene(state: &mut ReviewGateState) -> bool {
    let stale_reset = reset_stale_active_subagents(state);
    if stale_reset {
        state.review_subagent_pending_cycle_keys.clear();
        state.subagent_start_count = 0;
        state.subagent_stop_count = 0;
        sync_review_cycle_legacy_fields(state);
    } else {
        prune_stale_review_pending_cycle_keys(state);
    }
    stale_reset
}


include!("handlers_parts/handlers_before_submit.inc.rs");

include!("handlers_parts/handlers_subagent.inc.rs");

include!("handlers_parts/handlers_post_tool.inc.rs");

include!("handlers_parts/handlers_stop.inc.rs");

include!("handlers_parts/handlers_session.inc.rs");
