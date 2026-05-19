/// 从完整 RFV followup 文案中提取结构化外研 schema 指针行（若存在），供 Goal+RFV 合并进 `GSD_GOAL_CONTINUE` 时保留（原实现只取首行会丢掉该行）。
fn rfv_external_struct_schema_hint_line(rfv_msg: &str) -> Option<&str> {
    let needle = crate::rfv_loop::RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH;
    rfv_msg.lines().map(str::trim).find(|l| l.contains(needle))
}

/// 与 `.cursor/hook-state` 锁无关：只读合并 continuity 续跑，避免门控降级或应急短路时 goal/RFV 静默消失。
fn merge_continuity_followups(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
) {
    let autopilot = build_autopilot_drive_followup_using_frame(repo_root, frame);
    let rfv = build_rfv_loop_followup_using_frame(repo_root, frame);
    match (autopilot, rfv) {
        (Some(ap_msg), Some(rfv_msg)) if !ap_msg.is_empty() && !rfv_msg.is_empty() => {
            // Goal + RFV 同时活跃：只保留 **一条** `GSD_GOAL_CONTINUE` 段落，把 RFV 压缩成尾注，
            // 避免再插第二段 `RFV_LOOP_CONTINUE` 头行（token 与 scan 噪声双高）。
            let stripped = rfv_msg.lines().next().map(str::trim).unwrap_or("");
            let note = stripped
                .strip_prefix("RFV_LOOP_CONTINUE:")
                .map(str::trim)
                .unwrap_or(stripped);
            let struct_hint = rfv_external_struct_schema_hint_line(&rfv_msg);
            let merged = match (note.is_empty(), struct_hint) {
                (true, Some(h)) => format!("{ap_msg}\nAlso: RFV active\n{h}"),
                (true, None) => format!("{ap_msg}\nAlso: RFV active"),
                (false, Some(h)) => format!("{ap_msg}\nAlso: RFV active ({note})\n{h}"),
                (false, None) => format!("{ap_msg}\nAlso: RFV active ({note})"),
            };
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &merged,
                crate::autopilot_goal::GSD_GOAL_CONTINUE_PARAGRAPH_PREFIX,
                false,
            );
        }
        (Some(msg), _) if !msg.is_empty() => {
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &msg,
                crate::autopilot_goal::GSD_GOAL_CONTINUE_PARAGRAPH_PREFIX,
                false,
            );
        }
        (_, Some(msg)) if !msg.is_empty() => {
            crate::autopilot_goal::merge_hook_nudge_paragraph(
                output,
                &msg,
                "RFV_LOOP_CONTINUE",
                false,
            );
        }
        _ => {}
    }
}

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

fn finalize_stop_hook_outputs(
    repo_root: &Path,
    output: &mut Value,
    frame: &crate::task_state::CursorContinuityFrame,
    skip_continuity_merge: bool,
) {
    if !skip_continuity_merge {
        merge_continuity_followups(repo_root, output, frame);
    }
    merge_session_close_style_nudge_when_soft_terminal(output);
    try_write_cursor_continuity_checkpoint_on_stop(repo_root);
}

fn try_write_cursor_continuity_checkpoint_on_stop(repo_root: &Path) {
    if !crate::router_env_flags::router_rs_env_enabled_default_true(
        "ROUTER_RS_CONTINUITY_STOP_CHECKPOINT",
    ) {
        return;
    }
    let summary_body = "Stop hook automatic checkpoint (Cursor).";
    let payload = crate::framework_runtime::build_automatic_continuity_checkpoint_payload(
        repo_root,
        "cursor-stop",
        summary_body,
    );
    if let Err(err) = crate::framework_runtime::write_framework_session_artifacts(payload) {
        eprintln!("[router-rs] cursor continuity checkpoint write failed (non-fatal): {err}");
    }
}

fn build_autopilot_drive_followup_using_frame(
    repo_root: &Path,
    frame: &crate::task_state::CursorContinuityFrame,
) -> Option<String> {
    if let (Some(task_id), Some(goal)) = (
        frame.pointer_view.task_id.as_deref(),
        frame.pointer_view.goal_state.as_ref(),
    ) {
        return crate::autopilot_goal::build_autopilot_drive_followup_message_from_state(
            repo_root, task_id, goal,
        );
    }
    crate::autopilot_goal::build_autopilot_drive_followup_message(repo_root)
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

/// Strict closeout：**助手回复文本**中出现完成宣称且存在 `active_task` 时的硬 Stop 文案（与 `dispatch`/`handle_stop` 共用，避免分叉）。
///
/// `Err(evaluator)` 与 `Ok(Some(..))` 均返回 `Some`；未宣称完成、`Ok(None)` 或无 task 时返回 `None`。
fn stop_hard_closeout_followup_for_assistant_response(
    repo_root: &Path,
    response_text: &str,
) -> Option<String> {
    if !completion_claimed_in_text(response_text) {
        return None;
    }
    let tid = crate::autopilot_goal::read_active_task_id(repo_root)?;
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
        Regex::new(
            r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|验证通过|测试通过|阻塞)",
        )
        .expect("invalid regex")
    })
}

/// Task/subagent 调用里明示 `fork_context: true` 时视为与主会话共享上下文，不满足 autopilot 要求的「独立上下文」预检。
/// 部分宿主以字符串 `"true"` / `"false"` 下发，需与 JSON bool 同等解析。
fn fork_context_from_tool(event: &Value, tool_input: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
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

/// `/autopilot` pre-goal：常态下与 `review_subagent_kind_ok` 对齐（仅可数深度 lane + 独立 fork 证据链）；
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
    #[serde(default)]
    pub review_subagent_cycle_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_subagent_cycle_key: Option<String>,
    /// 武装 review gate 后，每次 qualifying subagent **start**（PostToolUse / subagentStart）压入一条 cycle key（multiset）；qualifying **stop** 命中时**移除一条**同 key 记录，**仅当**本队列为空时升相位 3 并记 `subagent_stop_count`。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_subagent_pending_cycle_keys: Vec<String>,
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
    extract_first_session_string(event)
        .or_else(|| try_extract_parent_session_from_tool_json(&tool_input_of(event)))
        .or_else(|| min_priority_session_identity_from_hook_json(event))
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

/// GSD execution-zone commands arm goal continuity gates (`/gsd-execute-phase|verify-work|ship`).
fn is_framework_goal_drive_entry_prompt(prompt: &str, signal_text: &str) -> bool {
    let _ = signal_text;
    crate::hook_common::is_framework_goal_entry_prompt(prompt)
}

#[allow(dead_code)]
fn is_autopilot_goal_entry_prompt(prompt: &str, signal_text: &str) -> bool {
    is_framework_goal_drive_entry_prompt(prompt, signal_text)
}

/// 显式委托/并行入口走 bounded sidecar gate；goal 入口（`/autopilot`、`/gsd*`）只走 goal 机。
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
    /// 保持 POSIX `flock(LOCK_EX)` 风格独占锁存活；销毁句柄才释放。同路径仍写入 `pid=`/`ts=`
    /// 供日志与在无独立锁 API 环境下的 stale 判断（fallback）。
    _file: std::fs::File,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    #[cfg(test)]
    if should_force_hook_state_lock_failure_for_test() {
        return None;
    }
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let lock_path = state_lock_path(repo_root, event);
    for _ in 0..30 {
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
                let lock_text = format!("pid={} ts={}\n", std::process::id(), now_millis());
                let mut owned = file;
                let _ = owned.set_len(0);
                let _ = owned.seek(std::io::SeekFrom::Start(0));
                let _ = owned.write_all(lock_text.as_bytes());
                let _ = owned.sync_all();
                return Some(LockGuard {
                    path: lock_path,
                    _file: owned,
                });
            }
            Err(_) => {
                drop(file);
                if let Ok(existing) = fs::read_to_string(&lock_path) {
                    if let Some((pid, ts_ms)) = parse_lock_metadata(&existing) {
                        let age_ms = now_millis().saturating_sub(ts_ms);
                        if age_ms > 30_000 || !is_process_alive(pid) {
                            let _ = fs::remove_file(&lock_path);
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
    if let Some(guard) = lock.take() {
        let path = guard.path.clone();
        drop(guard);
        let _ = fs::remove_file(path);
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
        review_subagent_cycle_open: false,
        review_subagent_cycle_key: None,
        review_subagent_pending_cycle_keys: Vec::new(),
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
    review_gate_armed(state.review_required, state.review_override)
}

/// Stop：`review` 场景下独立 subagent 证据是否满足（phase≥3：独立 start 后 stop 记账）。
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    state.phase >= 3
}

fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    review_hard_armed(state) && !review_subagent_evidence_satisfied(state)
}

/// Stop / 观测 fixture 共用的 `need=` 段（前缀仍须含 `REVIEW_GATE` 供 `router_rs_observation` 分类）。
pub(crate) const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n fork_context=false";

/// Short, stable tail for `REVIEW_GATE incomplete` lines (after `need=`). Does not change the first
/// `router-rs` token (`REVIEW_GATE`) used by observation classification.
pub(crate) const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

fn review_stop_followup_line(state: &ReviewGateState) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete phase={} {} {}",
        state.phase, REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

/// `merge_hook_nudge_paragraph` 去重前缀：首行须与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 常量一致以便每轮刷新同一段落。
pub(crate) const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

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

fn gsd_pre_goal_followup_message() -> String {
    "GSD execute (/gsd-execute-phase)：先写清 Goal 契约与验证口径（`GOAL_STATE.json`）；需要时再并行分工与证据索引（建议，非硬门槛）。确为小任务请**单独一行**拒因 token（如 small_task），不要自拟仿宿主 `router-rs …` 续跑行。"
        .to_string()
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

pub(crate) fn apply_cursor_hook_output_policy(output: &mut Value) {
    crate::router_rs_observation::attach_router_rs_observation(
        output,
        crate::router_rs_observation::HookObservationHost::Cursor,
    );
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    if let Some(Value::String(s)) = output.get_mut("additional_context") {
        let next = truncate_cursor_hook_outbound_context(s.as_str(), max_out);
        *s = next;
    }

    let absurd_followup_threshold =
        crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes()
            .saturating_mul(4)
            .max(32 * 1024);
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        if s.len() > absurd_followup_threshold {
            *s = truncate_cursor_hook_outbound_context(s.as_str(), max_out);
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
///
/// **`pre_goal_review_satisfied`（磁盘旁路）**：Stop 路径始终可由 hydration 置真。beforeSubmit 路径
/// 在 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` 开启时**不**因仅存在磁盘 GOAL 而置真，以免遗留
/// `GOAL_STATE` 误放行 pre-goal；其余 goal 字段的 hydrate（contract/progress/verify 等）仍执行。
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
    if arm_if_goal_file || !crate::router_env_flags::router_rs_cursor_pre_goal_strict_disk_enabled()
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
    }
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
    "router-rs：hook-state 锁不可用，本闸门控降级。收口前须见独立 subagent lane，或在**用户消息**中单独一行写拒因。"
}

fn lock_failure_followup_for_before_submit(event: &Value) -> (bool, String) {
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text);

    let strong_constraint = (review_arms || delegation || goal_drive_entrypoint) && !overridden;
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
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text) || saw_reject_reason(&signal_text, &text);

    let strong_constraint = (review_arms || delegation || goal_drive_entrypoint) && !overridden;
    if strong_constraint {
        return "router-rs：hook-state 锁不可用，本轮须严格 review/委托/autopilot 证据。合并前请修复锁/权限并重试，或 subagent/拒因。".to_string();
    }
    state_lock_degraded_followup().to_string()
}
/// 首次武装时注入一行指针，避免 `additional_context` 过长刷屏；细则见 skill / harness §5.0。
const CURSOR_DEEP_REVIEW_DEFAULT_NUDGE: &str = "深度审稿：`skills/code-review-deep/SKILL.md`（默认≥2 路只读并行；lane 仅 general-purpose / best-of-n-runner；每路 JSON 布尔 fork_context=false）。";

/// 同一条用户提交里同时出现 review 信号与 `/autopilot` 入口时追加；与 `review_arms_for_gate` 语义对齐。
const CURSOR_REVIEW_GSD_SAME_ROUND_NUDGE: &str = "router-rs：本轮提交同时包含「代码审查 / review」信号与 GSD 执行区入口（`/gsd-execute-phase` 等）；门控下 **不会** 在本回合因 review 措辞新武装 `REVIEW_GATE`。若需先跑独立审稿，请拆开用户消息（先发 review-only，再发 `/gsd-execute-phase`）或先落盘 `GOAL_STATE`。详见 `docs/framework_operator_primer.md`。";

/// 将一条 `review_subagent_cycle_key` 压入 multiset 并同步 legacy 字段。
///
/// **双事件去重**：宿主可能对同一子代理先发 `subagentStart` 再发 `PostToolUse`（同一 `subagent_id`）。对 **`id:`** 前缀的稳定 key，若 pending 已含该字符串，则 **PostToolUse 路径不再 push**，避免「一次 stop 只核销一条」语义下出现双 pending。
///
/// **`subagent_start_count`** 仅在 **`handle_subagent_start`** 的 qualifying review 分支递增；PostToolUse 仅负责 multiset 入队（及 phase bump），**不**增加该计数，以免与宿主双事件重复计数。
fn push_review_pending_cycle_key(
    state: &mut ReviewGateState,
    cycle_key: Option<String>,
    from_posttool: bool,
) {
    let Some(k) = cycle_key else {
        return;
    };
    if from_posttool
        && k.starts_with("id:")
        && state.review_subagent_pending_cycle_keys.contains(&k)
    {
        return;
    }
    state.review_subagent_pending_cycle_keys.push(k);
    sync_review_cycle_legacy_fields(state);
}

fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let (allow_continue, followup) = lock_failure_followup_for_before_submit(event);
        let mut out = json!({ "continue": allow_continue });
        if !allow_continue {
            out["followup_message"] = Value::String(followup);
        } else {
            merge_additional_context(&mut out, &followup);
        }
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
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms_for_gate = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let user_gate_override = has_override(&text);

    let prior_review_required = state.review_required;
    state.review_required = state.review_required || review_arms_for_gate;
    state.review_override = state.review_override || user_gate_override;
    state.delegation_override = state.delegation_override || user_gate_override;
    state.goal_required = state.goal_required || goal_drive_entrypoint;
    state.goal_contract_seen =
        state.goal_contract_seen || has_structured_goal_contract(&signal_text);
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
    if review || delegation || goal_drive_entrypoint {
        state.last_prompt = Some(text.chars().take(500).collect());
    }

    let pre_goal_auto_release_note = maybe_autopilot_pre_goal_nag_cap_release(&mut state);

    let persisted = save_state(repo_root, event, &mut state);

    // Review：首次武装门控时注入默认「深度+广度」契约指针（短）；相位仍只靠 subagent/PostToolUse（仅 review_hard_armed）。
    let needs_autopilot_pre_goal =
        crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled()
            && state.goal_required
            && !state.pre_goal_review_satisfied
            && !is_overridden(&state)
            && !state.reject_reason_seen;
    let mut output = json!({ "continue": true });
    if review_arms_for_gate
        && !prior_review_required
        && !cursor_review_gate_disabled_by_env()
        && !state.review_override
    {
        merge_additional_context(&mut output, CURSOR_DEEP_REVIEW_DEFAULT_NUDGE);
    }
    if review && goal_drive_entrypoint && !cursor_review_gate_disabled_by_env() {
        merge_additional_context(&mut output, CURSOR_REVIEW_GSD_SAME_ROUND_NUDGE);
    }
    if needs_autopilot_pre_goal {
        // 仅计入总 follow-up 次数；不要把 goal_followup_count 算进去，否则首次 stop 会误判成「非首条」而跳过完整 goal 提示。
        state.followup_count += 1;
        let pre = gsd_pre_goal_followup_message();
        crate::autopilot_goal::merge_hook_nudge_paragraph(
            &mut output,
            &pre,
            "GSD execute (/gsd-execute-phase)",
            false,
        );
    }
    if let Some(note) = pre_goal_auto_release_note {
        merge_additional_context(&mut output, note);
    }
    if crate::hook_common::is_gsd_pre_execution_entry_prompt(&text) {
        merge_additional_context(&mut output, crate::hook_common::GSD_PRE_EXECUTION_HOOK_NUDGE);
    }
    crate::paper_adversarial_hook::maybe_merge_paper_adversarial_before_submit(
        repo_root,
        &mut output,
        &text,
        false,
    );
    let persisted_after_followup = if needs_autopilot_pre_goal {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    release_state_lock(&mut lock);
    if !persisted || !persisted_after_followup {
        let warning = "router-rs：hook-state 未能持久化，review/委托门控本回合可能降级。";
        merge_additional_context(&mut output, warning);
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
    let independent_fork = independent_context_fork(fork);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
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
    if armed && independent_fork && review_kind {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        // 仅 SubagentStart 事件计数；PostToolUse 入 multiset 不递增（见 `push_review_pending_cycle_key` 模块注释）。
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
        push_review_pending_cycle_key(&mut state, cycle_key, false);
        if was_below_2 {
            clear_review_gate_escalation_counters(&mut state);
        }
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
        let tool_input = tool_input_of(event);
        let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
        let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
        let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        let cycle_matches = !state.review_subagent_pending_cycle_keys.is_empty()
            && cycle_key.as_ref().is_some_and(|k| {
                state
                    .review_subagent_pending_cycle_keys
                    .iter()
                    .any(|p| p == k)
            });
        // Stop：命中 pending  multiset 中**一条**同 key 的 start 记录则移除该条；**仅当** pending 排空时升 phase 3
        // 并记 `subagent_stop_count`（并行多路需各路各一次 qualifying stop，同 lane 无 id 时依赖重复 `lane:` key）。
        if state.phase < 2 || !review_kind || !cycle_matches {
            if mutated {
                let _ = save_state(repo_root, event, &mut state);
            }
            release_state_lock(&mut lock);
            return json!({});
        }
        if let Some(ref k) = cycle_key {
            if let Some(pos) = state
                .review_subagent_pending_cycle_keys
                .iter()
                .position(|p| p == k)
            {
                state.review_subagent_pending_cycle_keys.remove(pos);
            }
        }
        sync_review_cycle_legacy_fields(&mut state);
        if state.review_subagent_pending_cycle_keys.is_empty() {
            bump_phase(&mut state, 3);
            state.subagent_stop_count += 1;
            state.lane_intent_matches = Some(true);
        }
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let name = normalize_tool_name(Some(&tool_name_of(event)));
    if let Err(e) = crate::session_call_tracker::record_tool_call(repo_root, &name) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = review_hard_armed(&state);
    let tool_input = tool_input_of(event);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = independent_context_fork(fork);
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
    if tool_name_matches_subagent_lane(&name)
        && review_subagent_kind_ok(&sub_type, &agent_type)
        && armed
        && independent_fork
    {
        let start_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.last_subagent_tool = Some(name.clone());
        push_review_pending_cycle_key(&mut state, start_key, true);
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
    let syn = crate::hook_posttool_normalize::synthetic_post_tool_evidence_shape(event);
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
    if !crate::path_guard::path_is_within_repo_root(repo_root, &path) {
        return None;
    }
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
    if !crate::path_guard::path_is_within_repo_root(repo_root, &cargo_dir) {
        return None;
    }

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
    if track_goal && has_structured_goal_contract(&signal) {
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
    if cursor_review_gate_disabled_by_env() {
        let response_text = agent_response_text(event);
        let closeout_msg =
            stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text);
        let mut out = json!({});
        if let Some(msg) = closeout_msg {
            out["followup_message"] = Value::String(msg);
        }
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
        return out;
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let msg = lock_failure_followup_for_stop(event);
        let mut out = json!({ "followup_message": msg });
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
        return out;
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);

    // Completion claim guard must not depend on hook-state existence: a strict closeout violation
    // is a hard-stop even when the review gate state was never initialized for this session.
    if let Some(msg) = stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text)
    {
        let mut out = json!({ "followup_message": msg });
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
        release_state_lock(&mut lock);
        return out;
    }
    let (mut output, skip_continuity_merge) = match loaded {
        Ok(None) => (json!({}), false),
        Err(io_error) => {
            let msg = format!(
                "router-rs：hook-state 不可读（{io_error}），门控降级。请检查权限与 JSON。"
            );
            (json!({ "followup_message": msg }), false)
        }
        Ok(Some(mut state)) => {
            state.delegation_required = false;
            // Override 句式仅承认用户本轮 prompt（与 beforeSubmit 一致）；勿用含助手输出的
            // `signal_text`，避免助手复述「不要用子代理」类话术误清空 REVIEW_GATE。
            if has_override(&text) {
                state.review_override = true;
                state.delegation_override = true;
            }
            if has_structured_goal_contract(&signal_text) {
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
            if review_stop_followup_needed(&state) {
                state.followup_count += 1;
                state.review_followup_count += 1;
                let cap =
                    crate::router_env_flags::router_rs_cursor_review_gate_stop_max_nudges_cap();
                let use_full = match cap {
                    None => true,
                    Some(n) => state.review_followup_count <= n,
                };
                let skip_continuity_merge = !use_full;
                let out = if use_full {
                    json!({ "followup_message": review_stop_followup_line(&state) })
                } else {
                    let full_cap = cap.expect("soft branch implies cap=Some");
                    let soft = review_stop_followup_soft_line(&state, full_cap);
                    let mut soft_out = json!({ "followup_message": soft });
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut soft_out,
                        &review_stop_followup_detail_paragraph(&state),
                        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
                        false,
                    );
                    soft_out
                };
                let _ = save_state(repo_root, event, &mut state);
                (out, skip_continuity_merge)
            } else if !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = goal_stop_followup_line(&state);
                (json!({ "followup_message": message }), false)
            } else {
                // Do not clear gate state on Stop for sessions that still track goal/review:
                // the next Stop should still enforce the same requirements until satisfied/overridden.
                if state.review_required || state.goal_required || state.reject_reason_seen {
                    let _ = save_state(repo_root, event, &mut state);
                } else {
                    let mut reset = empty_state();
                    let _ = save_state(repo_root, event, &mut reset);
                }
                (json!({}), false)
            }
        }
    };
    // Advisory: lint review output format (compact envelope checks)
    // Runs on every Stop regardless of gate state; findings go to additional_context as soft hints.
    if !response_text.trim().is_empty() && response_text.contains("[P") {
        let lint_findings = lint_review_output(&response_text);
        if !lint_findings.is_empty() {
            let warning_count = lint_findings
                .iter()
                .filter(|f| f.severity == LintSeverity::Warning)
                .count();
            if warning_count > 0 {
                let msg = format!(
                    "review-output-lint: {} compact envelope warning(s) — check `skills/code-review-deep/SKILL.md` §Compact envelope",
                    warning_count
                );
                crate::autopilot_goal::merge_hook_nudge_paragraph(
                    &mut output,
                    &msg,
                    "review-output-lint",
                    false,
                );
            }
        }
    }
    finalize_stop_hook_outputs(repo_root, &mut output, &frame, skip_continuity_merge);
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
                "router-rs 门控快照：phase={} review={} delegation={} override={} reject={} pre_goal_ok={} subagentStart_n={} subagent_stop={}",
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

fn truncate_cursor_sessionstart_context(text: &str) -> String {
    let max_bytes = crate::router_env_flags::router_rs_cursor_sessionstart_context_max_bytes();
    truncate_cursor_hook_outbound_context(text, max_bytes)
}

fn compact_cursor_sessionstart_context(parts: Vec<String>) -> Option<String> {
    let joined = parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.trim().is_empty() {
        None
    } else {
        Some(truncate_cursor_sessionstart_context(&joined))
    }
}

fn handle_session_start(repo_root: &Path, event: &Value) -> Value {
    maybe_init_session_terminal_ledger(repo_root, event);
    // Align with Codex `handle_codex_session_start`: advisory continuity text must honor
    // `ROUTER_RS_OPERATOR_INJECT` kill-switch (terminal baseline init above is not advisory).
    if !crate::router_env_flags::router_rs_operator_inject_globally_enabled() {
        return json!({ "additional_context": "" });
    }
    let mut sections = Vec::new();
    let task_view = crate::task_state::resolve_task_view(repo_root, None);
    if crate::task_state::task_view_has_active_goal_focus_mismatch_note(&task_view) {
        sections.push(crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH.to_string());
    }
    // Raw SESSION_SUMMARY body (prefix-stable under SessionStart byte cap); see
    // `session_start_additional_context_observes_router_rs_sessionstart_max_env`.
    let session_summary_path = repo_root.join("artifacts/current/SESSION_SUMMARY.md");
    if let Ok(raw) = fs::read_to_string(&session_summary_path) {
        let block = raw.trim();
        if !block.is_empty() {
            sections.push(block.to_string());
        }
    }
    if let Ok(digest) =
        crate::framework_runtime::build_framework_continuity_digest_prompt_ex(repo_root, 4, true)
    {
        let trimmed = digest.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
        }
    }
    sections.push(format!("Repo: {}", repo_root.display()));
    let ctx = compact_cursor_sessionstart_context(sections).unwrap_or_default();
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
fn handle_after_file_edit(repo_root: &Path, event: &Value) -> Value {
    let path = event.get("file_path").and_then(Value::as_str).unwrap_or("");
    let p = PathBuf::from(path);
    if p.extension().and_then(|e| e.to_str()) != Some("rs") {
        return json!({});
    }
    if !p.is_file() {
        return json!({});
    }
    if !crate::path_guard::path_is_within_repo_root(repo_root, &p) {
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
    // **必须先读出 terminal 账本**，再删除本会话 `session-terminals-*.json`：否则账本先被删会导致 `owned_pids` 为空。
    let ledger = load_session_terminal_ledger(repo_root, event);
    let owned_vec = ledger.owned_pids.clone();
    let owned: HashSet<u32> = owned_vec.into_iter().collect();
    // 按本会话 `session_key` 精准删除主状态 / lock / adversarial-loop / terminal 账本。
    let _ = fs::remove_file(state_path(repo_root, event));
    let _ = fs::remove_file(state_lock_path(repo_root, event));
    remove_adversarial_loop(repo_root, event);
    let _ = fs::remove_file(session_terminal_ledger_path(repo_root, event));
    // 原子写入孤儿：始终全局清扫（与 session_key 无关）。
    sweep_hook_state_tmp_orphans(repo_root);
    // 默认不扫其它会话的 review/adversarial/session 文件（同仓库多 Cursor 会话避免互删）。
    // 需清 session_id/cwd 漂移遗留的全目录 stale 时，设 `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1`。
    if crate::router_env_flags::router_rs_cursor_hook_state_legacy_full_sweep_enabled() {
        sweep_review_gate_state_dir(repo_root);
    }
    let owned_filter = if cursor_terminal_kill_use_scoped_ownership() {
        Some(&owned)
    } else {
        None
    };
    // 默认仅回收本会话 shell 账本登记的 terminal；`ROUTER_RS_CURSOR_TERMINAL_KILL_MODE=legacy` 等恢复全仓 stale 扫描。
    let report = terminate_stale_terminal_processes(repo_root, owned_filter);
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
    json!({})
}

/// 仅清理由崩溃残留的原子写入 tmp（与 `session_key` 无关）。
fn sweep_hook_state_tmp_orphans(repo_root: &Path) {
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
        if hook_state_tmp_orphan_filename(name) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// **Legacy / opt-in**：清扫 `.cursor/hook-state/` 下所有由本模块写入的状态文件：
/// 1. review gate 主状态：`review-subagent-<key>.json` / `.lock`；
/// 2. adversarial-loop 主状态：`adversarial-loop-<key>.json`；
/// 3. `session-terminals-<key>.json`；
/// 4. 原子写入孤儿（与 [`sweep_hook_state_tmp_orphans`] 重叠；幂等）。
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

fn hook_state_tmp_orphan_filename(name: &str) -> bool {
    if name.starts_with(".tmp-") && name.contains("review-subagent-") {
        return true;
    }
    if name.starts_with(".tmp-adv-loop-") {
        return true;
    }
    false
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
    hook_state_tmp_orphan_filename(name)
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
/// 在 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 应急关闭时，仍调用各事件的真实 handler，
/// 但需要把可能附加给用户的督促 (`user-visible nags`) 从这几类事件输出里剥离。
/// 与正常模式相比，纯粹是「输出清洁」差异，handler 行为本身不变。
fn dispatch_disabled_should_strip_nags(lowered: &str) -> bool {
    matches!(
        lowered,
        "posttooluse" | "subagentstart" | "subagentstop" | "precompact"
    )
}

pub(crate) fn dispatch_cursor_hook_event(
    repo_root: &Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    let disabled = cursor_review_gate_disabled_by_env();

    // Emergency short-circuit: in disabled mode beforesubmit / userpromptsubmit skip the
    // review-gate-aware handler entirely so the host always sees `continue: true`. Other events
    // share the same handler dispatch with the normal mode; differences live in nag scrubbing.
    if disabled && matches!(lowered, "beforesubmitprompt" | "userpromptsubmit") {
        return json!({ "continue": true });
    }

    let mut out = match lowered {
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
    };

    if disabled && dispatch_disabled_should_strip_nags(lowered) {
        strip_cursor_hook_user_visible_nags(&mut out);
    }

    out
}
