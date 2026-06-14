include!("handlers/stop_closeout.rs");

/// Release L3 session hook lock before any L1 task-ledger work inside [`finalize_stop_hook_outputs`].
fn release_lock_then_finalize_stop(
    repo_root: &Path,
    output: &mut Value,
    frame: &core_state::task_state::CursorContinuityFrame,
    lock: &mut Option<LockGuard>,
) {
    release_state_lock(lock);
    finalize_stop_hook_outputs(repo_root, output, frame);
}

pub const STATE_VERSION: u32 = 3;

/// MCP / 宿主可能使用 `…subagent…` 等未列入清单的工具名。
fn tool_name_matches_subagent_lane(normalized: &str) -> bool {
    crate::hosts::hook_dispatch::is_subagent_tool(normalized)
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
    core_policy::hook_common::GOAL_CHAT_VERIFY_ZH_PHRASES
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
    if !hooks::router_rs_cursor_review_fork_context_missing_infer_false_enabled()
    {
        return None;
    }
    let lane = if !sub_type.is_empty() {
        sub_type
    } else {
        agent_type
    };
    if core_policy::hook_common::is_deep_review_gate_lane_normalized(lane) {
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

/// My implement pre-goal（`ROUTER_RS_AUTOPILOT_PRE_GOAL_ENABLED`）：常态下与 `review_subagent_kind_ok` 对齐（仅可数深度 lane + 独立 fork 证据链）；
/// `ROUTER_RS_REVIEW_GATE_DISABLE` 应急开启时退化为「任一带名 lane/agent 字段」以免应急路径过严。
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
    // 默认可清除 `REVIEW_GATE` 的深度审稿 lane：**不**含 `explore` / CI / guide / Claude-only `review` 别名。
    (!sub_type.is_empty() && core_policy::hook_common::is_deep_review_gate_lane_normalized(sub_type))
        || (!agent_type.is_empty()
            && core_policy::hook_common::is_deep_review_gate_lane_normalized(agent_type))
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

const STABLE_SUBAGENT_ID_FIELDS: &[&str] = &[
    "subagent_id",
    "subagentId",
    "agent_id",
    "agentId",
    "task_id",
    "taskId",
    "run_id",
    "runId",
];

fn stable_subagent_id(event: &Value, tool_input: &Value) -> String {
    first_nonempty_tool_or_event_str(event, tool_input, STABLE_SUBAGENT_ID_FIELDS)
}

fn review_subagent_cycle_key(
    event: &Value,
    tool_input: &Value,
    sub_type: &str,
    agent_type: &str,
) -> Option<String> {
    let stable = stable_subagent_id(event, tool_input);
    if !stable.is_empty() {
        return Some(format!("id:{stable}"));
    }
    let legacy_id = first_nonempty_tool_or_event_str(event, tool_input, &["id"]);
    if !legacy_id.is_empty() {
        return Some(format!("id:{legacy_id}"));
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
    let response = core_policy::hook_common::hook_assistant_tail_window(
        response,
        core_policy::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
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

pub fn tool_name_of(event: &Value) -> String {
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
    core_policy::hook_common::tool_input_value_from_map(obj)
}

pub fn tool_input_of(event: &Value) -> Value {
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

pub fn extract_first_session_string(event: &Value) -> Option<String> {
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
    const CWD_KEYS: &[&str] = &[
        "cwd",
        "workspaceFolder",
        "workspace_folder",
        "workspaceRoot",
        "workspace_root",
        "root",
    ];
    core_policy::session_key::session_key_core(
        &core_policy::session_key::SessionKeyConfig {
            env_var: "ROUTER_RS_CURSOR_SESSION_NAMESPACE",
        },
        || extract_first_session_string_including_tool_input(event),
        || {
            let cwd = first_nonempty_event_str(event, CWD_KEYS);
            if cwd.is_empty() { None } else { Some(cwd) }
        },
        "router-rs-cursor-session-fallback",
    )
}

fn state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".cursor").join("hook-state")
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
        let _ = core_state::utils::atomic_write::write_atomic_text(&path, &text);
    }
    // §1.3: session end 时清理过期 hook-state 文件
    if let Some(hook_state_dir) = path.parent() {
        crate::hooks::sweep_stale_hook_state_files(hook_state_dir);
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
    let extra = core_state::state_manager::scrub_spoof_host_followup_lines(extra);
    match output.get_mut("additional_context") {
        Some(Value::String(s)) => {
            s.push_str("\n\n");
            s.push_str(&extra);
            *s = core_state::state_manager::scrub_spoof_host_followup_lines(s);
        }
        _ => {
            output["additional_context"] = Value::String(extra);
        }
    }
}


include!("handlers/review_gate.rs");

include!("handlers/outbound.rs");

include!("handlers_parts/handlers_before_submit.inc.rs");

include!("handlers_parts/handlers_subagent.inc.rs");

include!("handlers_parts/handlers_post_tool.inc.rs");

include!("handlers/stop.rs");

include!("handlers_parts/handlers_session.inc.rs");
