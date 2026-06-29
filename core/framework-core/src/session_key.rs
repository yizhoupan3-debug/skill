use crate::crypto_util::short_hash;

/// 跨宿主 `session_key` 的公共配置。
///
/// 每个宿主提供自己的提取逻辑（session id / fallback token），
/// 而 env→cwd→fallback 的顺序与哈希方式统一在此。
pub struct SessionKeyConfig {
    /// 宿主特定的 session namespace 环境变量名（如 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`）。
    pub env_var: &'static str,
    /// 是否扫描 `tool_input` / `input` / `arguments` 中的父会话 id。
    /// Cursor 的 PostToolUse/SubagentStart 需要在 tool_input 内提取
    /// `session_id` / `conversation_id` 等字段；其他宿主通常不需要。
    pub scan_tool_input: bool,
}

/// 跨宿主共享的标准 cwd 字段名（4 宿主超集）。
///
/// 用于 session key fallback 中从事件载荷提取工作目录。
/// 定义在 `core-policy` 以消除各宿主的重复 `const CWD_KEYS`。
pub const SESSION_KEY_CWD_FIELDS: &[&str] = &[
    "cwd",
    "workspaceFolder",
    "workspace_folder",
    "workspaceRoot",
    "workspace_root",
    "root",
];

/// 跨宿主共享的标准 session id 字段名（4 宿主超集）。
///
/// 用于从事件载荷顶层提取显式 session id。
pub const SESSION_ID_FIELDS: &[&str] = &[
    "session_id",
    "sessionId",
    "conversation_id",
    "conversationId",
    "thread_id",
    "chat_id",
];

/// 跨宿主共享的 `tool_input` 内父会话 id 字段名。
///
/// 仅当 `SessionKeyConfig::scan_tool_input` 为 `true` 时使用。
pub const TOOL_INPUT_SESSION_ID_FIELDS: &[&str] = &[
    "session_id",
    "conversation_id",
    "thread_id",
    "chat_id",
    "conversationId",
    "threadId",
    "sessionId",
];

/// 跨宿主共享的 `tool_input.metadata` 内 session id 字段名。
pub const TOOL_INPUT_METADATA_SESSION_ID_FIELDS: &[&str] =
    &["sessionId", "conversationId", "chatId", "threadId"];

/// 跨宿主共享的 `session_key` 核心逻辑。
///
/// 调用方负责提供 `extract_session`（从事件载荷提取稳定 session id）和
/// `cwd_fallback`（当无 session id 且无 namespace env 时的 cwd 派生 token）。
///
/// 顺序：`extract_session()` → `env_var` → `cwd_fallback()` → `default_fallback`。
pub fn session_key_core(
    config: &SessionKeyConfig,
    extract_session: impl FnOnce() -> Option<String>,
    cwd_fallback: impl FnOnce() -> Option<String>,
    default_fallback: &str,
) -> String {
    // 1. 宿主提供的显式 session 串
    if let Some(raw) = extract_session() {
        return short_hash(&raw);
    }
    // 2. 宿主特定的 namespace 环境变量
    if let Ok(ns) = std::env::var(config.env_var) {
        let t = ns.trim();
        if !t.is_empty() {
            return short_hash(&format!("env::{t}"));
        }
    }
    // 3. cwd 派生
    if let Some(cwd) = cwd_fallback() {
        let trimmed = cwd.trim();
        if !trimmed.is_empty() {
            return short_hash(&format!("cwd::{trimmed}"));
        }
    }
    // 4. 常量 fallback
    short_hash(default_fallback)
}
