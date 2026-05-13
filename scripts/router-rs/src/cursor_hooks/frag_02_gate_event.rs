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

fn is_framework_entrypoint_prompt(text: &str) -> bool {
    framework_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

fn is_autopilot_entrypoint_prompt(text: &str) -> bool {
    autopilot_entrypoint_re().is_match(&strip_quoted_or_codeblock_or_url(text))
}

/// `/autopilot` 是唯一会拉起 goal 门控的框架入口。
fn is_autopilot_goal_entry_prompt(prompt: &str, signal_text: &str) -> bool {
    let _ = signal_text;
    is_autopilot_entrypoint_prompt(prompt)
}

/// 显式委托/并行入口走 bounded sidecar gate；**框架命令仅认 `/` 前缀**；**`/autopilot` 除外**（只走 goal 机）。
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
