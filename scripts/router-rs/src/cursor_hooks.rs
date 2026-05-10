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

use crate::autopilot_goal::merge_autopilot_drive_followup;
use crate::rfv_loop::merge_rfv_loop_followup;

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
        Regex::new(r"(?i)(^|\s)([/$])(autopilot|team|gitx|loop)\b").expect("invalid regex")
    })
}

fn autopilot_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)([/$])autopilot\b").expect("invalid regex"))
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
fn fork_context_from_tool(event: &Value, tool_input: &Value) -> Option<bool> {
    tool_input
        .get("fork_context")
        .or_else(|| tool_input.get("forkContext"))
        .or_else(|| event.get("fork_context"))
        .or_else(|| event.get("forkContext"))
        .and_then(Value::as_bool)
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
    reject_reason_patterns().iter().any(|p| p.is_match(text))
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

fn prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input", "text"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn agent_response_text(event: &Value) -> String {
    for key in [
        "response",
        "agent_response",
        "content",
        "text",
        "message",
        "output",
    ] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn tool_name_of(event: &Value) -> String {
    event
        .get("tool_name")
        .or_else(|| event.get("tool"))
        .or_else(|| event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn tool_input_of(event: &Value) -> Value {
    let value = event
        .get("tool_input")
        .or_else(|| event.get("input"))
        .or_else(|| event.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn loop_count_of(event: &Value) -> i64 {
    event
        .get("loop_count")
        .or_else(|| event.get("loopCount"))
        .or_else(|| event.get("loop"))
        .and_then(|v| {
            if let Some(n) = v.as_i64() {
                Some(n)
            } else {
                v.as_str().and_then(|s| s.parse::<i64>().ok())
            }
        })
        .unwrap_or(0)
}

/// 从 stdin JSON 提取会话标识。
///
/// **优先级（先到先用）**：顶层 `session_id`、`conversation_id`、…、`sessionId` 等依次扫描；
/// 再读 `metadata.{sessionId,conversationId,chatId,threadId}`。
/// 若同一 payload 中多套字段彼此冲突，**仅第一个非空值生效**（宿主应对齐字段）。
///
/// **仅 cwd、无会话 id**：`session_key` 对 `cwd` 做稳定哈希；同一文件系统路径上并行多会话会**共用**
/// 一份状态，除非设置环境变量 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`。
fn extract_first_session_string(event: &Value) -> Option<String> {
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
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(obj) = event.get("metadata").and_then(Value::as_object) {
        for key in ["sessionId", "conversationId", "chatId", "threadId"] {
            if let Some(value) = obj.get(key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// 派生 `.cursor/hook-state/review-subagent-<key>.json` 文件名组件。
/// 顺序：`extract_first_session_string` → `ROUTER_RS_CURSOR_SESSION_NAMESPACE` → `cwd` → 常量 fallback。
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
    if let Some(cwd) = event.get("cwd").and_then(Value::as_str) {
        if !cwd.trim().is_empty() {
            // 与 Cursor「每 hook 新进程」兼容：cwd fallback 必须跨调用稳定，否则状态永不累积。
            return short_hash(&format!("cwd::{cwd}"));
        }
    }
    short_hash("router-rs-cursor-session-fallback")
}

fn hook_lock_unavailable_notice_json() -> Value {
    json!({
        "additional_context": "router-rs: .cursor/hook-state lock unavailable; this hook did not persist review-gate state (permissions or contention). Fix hook-state or retry."
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
    RE.get_or_init(|| Regex::new(r"(?i)^\s*([/$])loop\s+clear\b").expect("invalid regex"))
}

fn loop_line_next_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*([/$])loop\s+next\b").expect("invalid regex"))
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

fn is_satisfied(state: &ReviewGateState) -> bool {
    if !is_armed(state) {
        return true;
    }
    if is_overridden(state) {
        return true;
    }
    if state.reject_reason_seen {
        return true;
    }
    state.phase >= 2
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

fn goal_followup_message() -> &'static str {
    "Autopilot goal mode: before goal_start, spawn an independent-context subagent (Task/subagent; fork_context=false or omitted) for pre-goal review, or emit one reject_reason token if you will not spawn. Then provide closeout evidence: goal contract (Goal/Non-goals/Done when/Validation commands), checkpoint progress + next step, and verification result or explicit blocker."
}

fn autopilot_pre_goal_followup_message() -> &'static str {
    "Autopilot (/autopilot): spawn an independent-context reviewer subagent now (fresh lane; not fork_context=true), then publish the goal contract. If you will not spawn, output one explicit reject reason token (small_task, shared_context_heavy, …) before proceeding."
}

fn review_followup_message() -> &'static str {
    "Broad/deep review (or independent parallel lanes) was requested, but no independent subagent/sidecar was observed. Spawn a suitable subagent lane now, or explicitly state why spawning is rejected."
}

fn review_missing_parts(_state: &ReviewGateState) -> String {
    "independent_subagent_or_reject_reason".to_string()
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
    "Cursor review gate state lock is unavailable under .cursor/hook-state, so enforcement is fail-closed/degraded for this turn. Do not finalize until an independent subagent lane is observed or an explicit reject reason is stated in assistant output."
}

fn lock_failure_followup_for_before_submit(prompt: &str) -> (bool, String) {
    let review = is_review_prompt(prompt);
    let framework_entrypoint = is_framework_entrypoint_prompt(prompt);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(prompt);
    let delegation = is_parallel_delegation_prompt(prompt) || framework_entrypoint;
    let overridden =
        has_review_override(prompt) || has_delegation_override(prompt) || has_override(prompt);

    let strong_constraint = (review || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return (
            false,
            "Cursor review gate state lock is unavailable under .cursor/hook-state, and this prompt requires strict review/delegation enforcement. Submission is blocked for this turn. Resolve hook-state lock/permission issue, then retry with independent subagent lanes or an explicit reject reason."
                .to_string(),
        );
    }

    (
        true,
        "Cursor review gate state lock is unavailable under .cursor/hook-state. Enforcement is degraded for this turn; non-strict prompts are allowed to continue."
            .to_string(),
    )
}

fn lock_failure_followup_for_stop(event: &Value) -> String {
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let combined = format!("{text}\n{response_text}");
    let review = is_review_prompt(&text);
    let framework_entrypoint = is_framework_entrypoint_prompt(&text);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(&text);
    let delegation = is_parallel_delegation_prompt(&text) || framework_entrypoint;
    let overridden = has_review_override(&combined)
        || has_delegation_override(&combined)
        || has_override(&combined)
        || saw_reject_reason(&combined);

    let strong_constraint = (review || delegation || autopilot_entrypoint) && !overridden;
    if strong_constraint {
        return "Cursor review gate state lock is unavailable under .cursor/hook-state, and this turn requires strict review/delegation/autopilot evidence. Do not treat the response as merge-ready until you fix hook-state (permissions or stale lock) and re-run with independent subagent lanes or an explicit reject reason.".to_string();
    }
    state_lock_degraded_followup().to_string()
}

fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let text = prompt_text(event);
        let (allow_continue, followup) = lock_failure_followup_for_before_submit(&text);
        return json!({
            "continue": allow_continue,
            "followup_message": followup
        });
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let text = prompt_text(event);
    let review = is_review_prompt(&text);
    let framework_entrypoint = is_framework_entrypoint_prompt(&text);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(&text);
    let delegation = is_parallel_delegation_prompt(&text) || framework_entrypoint;
    let review_override = has_review_override(&text) || has_override(&text);
    let delegation_override = has_delegation_override(&text) || has_override(&text);

    state.review_required = state.review_required || review;
    state.delegation_required = state.delegation_required || delegation;
    state.review_override = state.review_override || review_override;
    state.delegation_override = state.delegation_override || delegation_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
    state.goal_contract_seen = state.goal_contract_seen || has_goal_contract_signal(&text);
    state.goal_progress_seen = state.goal_progress_seen || has_goal_progress_signal(&text);
    state.goal_verify_or_block_seen =
        state.goal_verify_or_block_seen || has_goal_verify_or_block_signal(&text);
    if review || delegation {
        state.last_prompt = Some(text.chars().take(500).collect());
    }
    if is_armed(&state) && !is_overridden(&state) && !state.reject_reason_seen {
        bump_phase(&mut state, 1);
    }

    let persisted = save_state(repo_root, event, &mut state);

    let needs_followup =
        is_armed(&state) && !is_overridden(&state) && !state.reject_reason_seen && state.phase < 2;
    let needs_autopilot_pre_goal = state.goal_required
        && !state.pre_goal_review_satisfied
        && !is_overridden(&state)
        && !state.reject_reason_seen;
    let mut output = json!({ "continue": true });
    let mut followup_parts: Vec<String> = Vec::new();
    if needs_followup {
        let is_first_followup = state.review_followup_count == 0;
        state.followup_count += 1;
        state.review_followup_count += 1;
        let msg = if is_first_followup {
            if state.review_required {
                "Broad/deep review detected. Spawn an independent reviewer subagent lane now; if you will not spawn, provide one explicit reject reason before finalizing.".to_string()
            } else {
                "Parallel lane request detected. Spawn bounded subagent lanes now; if you will not spawn, provide one explicit reject reason before finalizing.".to_string()
            }
        } else {
            format!("RG_FOLLOWUP missing_parts={}", review_missing_parts(&state))
        };
        followup_parts.push(msg);
    }
    if needs_autopilot_pre_goal {
        // 仅计入总 follow-up 次数；不要把 goal_followup_count 算进去，否则首次 stop 会误判成「非首条」而跳过完整 goal 提示。
        state.followup_count += 1;
        followup_parts.push(autopilot_pre_goal_followup_message().to_string());
    }
    if let Some(msg) = crate::autopilot_goal::build_autopilot_drive_followup_message(repo_root) {
        if !followup_parts.iter().any(|p| p.contains("AUTOPILOT_DRIVE")) {
            followup_parts.push(msg);
        }
    }
    if let Some(msg) = crate::rfv_loop::build_rfv_loop_followup_message(repo_root) {
        if !followup_parts
            .iter()
            .any(|p| p.contains("RFV_LOOP_CONTINUE"))
        {
            followup_parts.push(msg);
        }
    }
    if !followup_parts.is_empty() {
        output["followup_message"] = Value::String(followup_parts.join("\n\n"));
    }
    if let Some(loop_ctx) = adversarial_loop_process_prompt(repo_root, event, &text) {
        merge_additional_context(&mut output, &loop_ctx);
    }
    let persisted_after_followup = if needs_followup || needs_autopilot_pre_goal {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    release_state_lock(lock);
    if !persisted || !persisted_after_followup {
        let warning = "Cursor review gate state could not be persisted under .cursor/hook-state. Review/delegation enforcement may be degraded for this turn.";
        let merged = output
            .get("followup_message")
            .and_then(Value::as_str)
            .map(|s| format!("{s} {warning}"))
            .unwrap_or_else(|| warning.to_string());
        output["followup_message"] = Value::String(merged.trim().to_string());
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
    let typed_subagent = typed_subagent_in_allowlist(&sub_type, &agent_type);
    let armed = is_armed(&state);
    let mut mutated = false;
    // 与 PostToolUse 对齐：pre-goal 仅在有合法 subagent 类型且独立上下文 fork 时满足。
    if state.goal_required && typed_subagent && independent_fork {
        state.pre_goal_review_satisfied = true;
        mutated = true;
    }
    if armed {
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
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
    let typed_subagent = typed_subagent_in_allowlist(&sub_type, &agent_type);
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let mut mutated = false;
    if subagent_tool_names().contains(&name.as_str())
        && typed_subagent
        && state.goal_required
        && independent_fork
    {
        state.pre_goal_review_satisfied = true;
        mutated = true;
    }
    if subagent_tool_names().contains(&name.as_str()) && typed_subagent && armed {
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
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
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
    let text = agent_response_text(event);
    adversarial_loop_on_response_complete(repo_root, event, &text);
    let mut dirty = false;
    if saw_reject_reason(&text) {
        state.reject_reason_seen = true;
        if state.goal_required {
            state.pre_goal_review_satisfied = true;
        }
        dirty = true;
    }
    if track_goal && has_goal_contract_signal(&text) {
        state.goal_contract_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_progress_signal(&text) {
        state.goal_progress_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_verify_or_block_signal(&text) {
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
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "followup_message": lock_failure_followup_for_stop(event)
        });
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let combined_text = format!("{text}\n{response_text}");
    let inferred_required = is_review_prompt(&text)
        || is_parallel_delegation_prompt(&text)
        || is_framework_entrypoint_prompt(&text);
    let inferred_overridden = has_review_override(&combined_text)
        || has_delegation_override(&combined_text)
        || has_override(&combined_text)
        || saw_reject_reason(&combined_text);
    let loop_count = loop_count_of(event);
    let mut output = match loaded {
        Ok(None) => {
            if inferred_required && !inferred_overridden {
                json!({
                    "followup_message": "Cursor review gate state is missing under .cursor/hook-state, so enforcement cannot be verified for this turn. Re-run with subagent lanes or an explicit reject reason before finalizing."
                })
            } else {
                json!({})
            }
        }
        Err(io_error) => json!({
            "followup_message": format!(
                "Cursor review gate state is unreadable under .cursor/hook-state ({}). Enforcement degraded; check hook-state permissions and JSON integrity.",
                io_error
            )
        }),
        Ok(Some(mut state)) => {
            if has_review_override(&combined_text) || has_override(&combined_text) {
                state.review_override = true;
            }
            if has_delegation_override(&combined_text) || has_override(&combined_text) {
                state.delegation_override = true;
            }
            if has_goal_contract_signal(&combined_text) {
                state.goal_contract_seen = true;
            }
            if has_goal_progress_signal(&combined_text) {
                state.goal_progress_seen = true;
            }
            if has_goal_verify_or_block_signal(&combined_text) {
                state.goal_verify_or_block_seen = true;
            }
            if saw_reject_reason(&combined_text) {
                state.reject_reason_seen = true;
                if state.goal_required {
                    state.pre_goal_review_satisfied = true;
                }
            }
            if !is_satisfied(&state) {
                let is_first_followup = state.review_followup_count == 0;
                state.followup_count += 1;
                state.review_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let escalation = if loop_count >= 3 || state.followup_count >= 3 {
                    "This has already looped multiple times; do not silently continue. "
                } else {
                    ""
                };
                let message = if is_first_followup {
                    format!("{} {}", review_followup_message(), escalation)
                        .trim()
                        .to_string()
                } else {
                    format!(
                        "RG_FOLLOWUP missing_parts={}{}",
                        review_missing_parts(&state),
                        if escalation.is_empty() {
                            String::new()
                        } else {
                            format!(" escalation={}", escalation.trim())
                        }
                    )
                };
                json!({
                    "followup_message": message
                })
            } else if !goal_is_satisfied(&state) {
                let is_first_followup = state.goal_followup_count == 0;
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let message = if is_first_followup {
                    format!(
                        "{} Missing: {}.",
                        goal_followup_message(),
                        goal_missing_parts(&state)
                    )
                } else {
                    format!("AG_FOLLOWUP missing_parts={}", goal_missing_parts(&state))
                };
                json!({
                    "followup_message": message
                })
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
    merge_autopilot_drive_followup(repo_root, &mut output);
    merge_rfv_loop_followup(repo_root, &mut output);
    release_state_lock(lock);
    output
}

fn handle_pre_compact(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "additional_context": "router-rs: hook-state lock unavailable; preCompact snapshot could not read persisted gate state."
        });
    }
    let out = match load_state(repo_root, event) {
        Ok(Some(state)) => {
            let mut summary = format!(
                "Cursor review gate state (preserved across compaction): phase={} review_required={} delegation_required={} override={} rejected={} pre_goal_ok={} subagent_starts={} subagent_stops={}",
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
    let _ = fs::remove_file(state_path(repo_root, event));
    let _ = fs::remove_file(state_lock_path(repo_root, event));
    remove_adversarial_loop(repo_root, event);
    json!({})
}

fn dispatch_event(repo_root: &Path, event_name: &str, payload: &Value) -> Value {
    match event_name.trim().to_lowercase().as_str() {
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
    let output = dispatch_event(&repo_root, event, &payload);
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
        assert!(out.get("followup_message").is_some());
        let state = load_state_for(&repo, "s1");
        assert_eq!(state.phase, 1);
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
        assert_eq!(state.phase, 1);
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
        assert!(out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("state lock is unavailable"));
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
        assert!(out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("Enforcement is degraded"));
    }

    #[test]
    fn stop_lock_failure_reports_degraded_followup() {
        let repo = fresh_repo();
        let payload = event("s15", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_event(&repo, "stop", &payload);
        assert!(out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("state lock is unavailable"));
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
        assert_eq!(state.phase, 1);
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
    fn stop_without_subagent_emits_followup() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s7", "全面review这个仓库"),
        );
        let out = dispatch_event(&repo, "stop", &event("s7", "继续"));
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(msg.contains("Broad/deep review") || msg.starts_with("RG_FOLLOWUP missing_parts="));
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
            .contains("phase=1"));
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
    fn review_followup_is_detailed_then_short_code() {
        let repo = fresh_repo();
        let first = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s16", "全面review这个仓库"),
        );
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(first_msg.contains("Broad/deep review"));
        let second = dispatch_event(&repo, "stop", &event("s16", "继续"));
        let second_msg = second
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(second_msg.starts_with("RG_FOLLOWUP missing_parts="));
    }

    #[test]
    fn goal_followup_is_detailed_then_short_code() {
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
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(first_msg.contains("Autopilot goal mode:"));
        assert!(first_msg.contains("before goal_start"));
        let second = dispatch_event(&repo, "stop", &event("s17", "继续"));
        let second_msg = second
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(second_msg.starts_with("AG_FOLLOWUP missing_parts="));
    }

    #[test]
    fn autopilot_before_submit_prompts_pre_goal_review() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s17b", "/autopilot 完成任务"),
        );
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            msg.contains("Autopilot (/autopilot)") || msg.contains("independent-context"),
            "followup={msg:?}"
        );
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
        assert_eq!(load_state_for(&repo, "s24").phase, 1);
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
}
