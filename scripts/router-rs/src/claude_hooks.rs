//! Claude Code（Anthropic CLI）hooks：`router-rs claude hook --event=… --repo-root …`。
//! 历史版本接口快照：`git show 89ece4c^:scripts/router-rs/src/claude_hooks.rs`（事件：`pre-tool-use`、`user-prompt-submit`、`post-tool-use`、`stop`；CLI 亦接受 `PreToolUse` 等 PascalCase 别名，与 Codex hook 拼写对齐）。
//!
//! **误接 Cursor hook stdin**：仅在 stdin JSON 呈现结构化 Cursor envelope（顶层非空 `cursor_version` 字符串 + `workspace_roots` 数组 + 非空 `hook_event_name` 或 `hookEventName`）时整条静默；
//! 不用路径子串扫描，以免合法 Claude 载荷（例如编辑 `.cursor/` 下文件）被误判为 Cursor 而旁路门禁。
//! stdin 体量上限 4 MiB，与 Codex hook 读取路径对齐，防失控输入撑爆 hook 进程内存。
use crate::hook_common::{
    has_override, has_review_override, is_review_prompt, normalize_subagent_type,
    normalize_tool_name,
};
use crate::review_gate_engine::{
    fork_context_from_values, independent_reviewer_evidence, review_gate_blocks_stop,
    ReviewGateFacts,
};
use regex::Regex;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

const CLAUDE_HOOK_STATE_UNREADABLE: &str =
    "router-rs CLAUDE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

/// Lexically normalize `.` / `..` segments (no filesystem access). Prefix/Root handling matches
/// `PathBuf` push semantics so repo-root joins stay absolute on POSIX.
fn normalize_path_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                out.push(comp);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if out.file_name().is_some() {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            Component::Normal(c) => out.push(c),
        }
    }
    out
}

/// Collapse `.` / `..` in a path string interpreted **relative to repo root**. Extra `..` at the
/// virtual root are ignored so `a/../../AGENTS.md` resolves like `AGENTS.md`, never above `repo_root`.
fn compact_repo_relative_segments(rel_raw: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in Path::new(rel_raw).components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(s) => out.push(s),
            Component::ParentDir => {
                if out.file_name().is_some() {
                    out.pop();
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

/// Repo-relative forward-slash path when `raw` resolves under `repo_root`. Host-private paths pass
/// through unchanged. Escaped or unresolvable paths return `None` (guards do not apply).
fn repo_relative_slash_path(repo_root: &Path, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if is_host_private_path(raw) {
        return Some(raw.replace('\\', "/"));
    }
    let candidate = PathBuf::from(raw);
    let repo_lex = normalize_path_lexical(repo_root);

    if candidate.is_absolute() {
        if let (Ok(canon_file), Ok(canon_repo)) =
            (candidate.canonicalize(), repo_root.canonicalize())
        {
            if let Ok(rel) = canon_file.strip_prefix(&canon_repo) {
                return Some(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        let abs_lex = normalize_path_lexical(&candidate);
        if let Ok(rel) = abs_lex.strip_prefix(&repo_lex) {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }
        return None;
    }

    let rel_only = compact_repo_relative_segments(raw)?;
    let joined = normalize_path_lexical(&repo_root.join(&rel_only));
    if let Ok(rel) = joined.strip_prefix(&repo_lex) {
        return Some(rel.to_string_lossy().replace('\\', "/"));
    }
    None
}

const FRAMEWORK_CHANGED_CONTEXT: &str =
    "Framework routing/runtime files changed; run the targeted Rust contract tests before finishing.";
const SETTINGS_CHANGED_CONTEXT: &str =
    "Claude hook/settings files changed; validate JSON and run the Claude hook contract tests before finishing.";
const AUTOMATION_CONTEXT: &str =
    "Automation requests such as 'from now on', 'whenever', or 'before/after' must be implemented through settings hooks, not memory alone.";

/// **仅当** `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` 为 `1` / `true` / `yes` / `on`（大小写不敏感）时跳过
/// Claude Code review gate；unset、空串及其它值保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` **同形**：`router_rs_env_enabled_default_false`）。
fn claude_review_gate_disabled() -> bool {
    crate::router_env_flags::router_rs_env_enabled_default_false(
        "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
    )
}

const FRAMEWORK_GUARDED_PREFIXES: &[&str] = &[
    "scripts/router-rs/",
    "configs/framework/",
    "skills/SKILL_",
    "skills/SKILL_ROUTING_RUNTIME.json",
    "skills/SKILL_MANIFEST.json",
];

const SETTINGS_GUARDED_PATHS: &[&str] = &[".claude/settings.json", ".claude/settings.local.json"];
const GENERATED_ENTRYPOINT_PATHS: &[&str] = &["AGENTS.md", "CLAUDE.md", ".claude/CLAUDE.md"];
const RETIRED_SURFACE_PATHS: &[&str] = &[
    ".codex/hooks.json",
    ".agents",
    "plugins/skill-framework-native/.mcp.json",
];
/// Bash segment hints that indicate writes targeting user-global Claude config (not repo `.claude/` policy).
const CLAUDE_HOME_GUARD_HINTS: &[&str] = &["~/.claude/"];
/// Pre-89ece4c `claude_hooks.rs` accepted kebab-case commands only; CLI adds PascalCase aliases
/// aligned with Codex hook spelling (`PreToolUse`, `Stop`, …).
pub fn run_claude_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    let canonical = canonical_claude_hook_command(command)?;
    let payload = read_stdin_payload()?;
    Ok(dispatch_claude_hook_payload(canonical, repo_root, &payload))
}

fn dispatch_claude_hook_payload(canonical: &str, repo_root: &Path, payload: &Value) -> Value {
    if payload_looks_like_cursor_hook_stdin(payload) {
        return silent_success();
    }
    let response = match canonical {
        "pre-tool-use" => run_pre_tool_use(repo_root, payload),
        "user-prompt-submit" => run_user_prompt_submit(repo_root, payload),
        "post-tool-use" => run_post_tool_use(repo_root, payload),
        "stop" => run_stop(repo_root, payload),
        // Defensive default: host should only dispatch canonical commands from `canonical_claude_hook_command`.
        _ => Some(silent_success()),
    };
    response.unwrap_or_else(silent_success)
}

fn canonical_claude_hook_command(command: &str) -> Result<&'static str, String> {
    match command.trim() {
        "pre-tool-use" | "PreToolUse" => Ok("pre-tool-use"),
        "user-prompt-submit" | "UserPromptSubmit" => Ok("user-prompt-submit"),
        "post-tool-use" | "PostToolUse" => Ok("post-tool-use"),
        "stop" | "Stop" => Ok("stop"),
        _ => Err(format!("Unsupported Claude hook command: {command}")),
    }
}

/// `router-rs claude hook --event=… --repo-root …` — stdin JSON → Claude Code hook response JSON (line-delimited).
pub fn run_claude_hook_cli(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = crate::framework_runtime::resolve_repo_root_arg(cli_repo_root)?;
    let output = run_claude_hook(event, &repo_root)?;
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    let mut stdout = std::io::stdout();
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_claude_hook_stdin_trimmed(trimmed: &str) -> Result<Value, String> {
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).map_err(|err| format!("stdin_json_invalid: {err}"))
}

/// 与 Codex hook `read_codex_stdin_limited` 同款上限与 UTF-8 错误归一。
fn read_claude_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    const LIMIT: u64 = 4 * 1024 * 1024;
    let mut input = String::new();
    let mut limited = reader.take(LIMIT);
    limited.read_to_string(&mut input).map_err(|err| {
        let msg = err.to_string();
        let lower = msg.to_ascii_lowercase();
        if matches!(err.kind(), std::io::ErrorKind::InvalidData)
            || lower.contains("utf-8")
            || lower.contains("utf8")
            || lower.contains("utf")
        {
            return "stdin_invalid_utf8".to_string();
        }
        msg
    })?;
    if limited.limit() == 0 {
        let inner = limited.into_inner();
        let mut probe = [0u8; 1];
        if inner.read(&mut probe).map_err(|err| err.to_string())? > 0 {
            return Err("stdin payload exceeds 4 MiB limit".to_string());
        }
    }
    Ok(input)
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = io::stdin();
    let input = read_claude_stdin_limited(&mut stdin)?;
    parse_claude_hook_stdin_trimmed(input.trim())
}

fn silent_success() -> Value {
    json!({ "suppressOutput": true })
}

/// Cursor hook stdin 误接到 `claude hook` 时的结构化识别（顶层字段）。
///
/// 刻意不使用嵌套字符串中的 `/.cursor/` 匹配，否则合法 Claude 工具载荷可能被整条静默。
/// 另要求 `hook_event_name` / `hookEventName`，降低仅凭顶造 `cursor_version`+`workspace_roots` 整条静默的面。
fn payload_looks_like_cursor_hook_stdin(payload: &Value) -> bool {
    let Value::Object(map) = payload else {
        return false;
    };
    let Some(Value::String(cv)) = map.get("cursor_version") else {
        return false;
    };
    if cv.trim().is_empty() {
        return false;
    }
    if !matches!(map.get("workspace_roots"), Some(Value::Array(_))) {
        return false;
    }
    let hook_ok = [map.get("hook_event_name"), map.get("hookEventName")]
        .into_iter()
        .flatten()
        .any(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()));
    hook_ok
}

fn deny_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn add_context(event: &str, context: &str) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": event,
            "additionalContext": context,
        },
    }))
}

fn block_stop(reason: &str) -> Option<Value> {
    Some(json!({
        "continue": false,
        "stopReason": reason,
        "decision": "block",
        "reason": reason,
        "suppressOutput": true,
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    if let Some(reason) = dangerous_bash_reason(payload) {
        return deny_pre_tool_use(reason);
    }
    for path in payload_relative_paths(repo_root, payload) {
        if is_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked restoring retired generated surface {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_generated_entrypoint(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of generated host entrypoint {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_framework_guarded_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of framework routing/runtime file {path}; use the Rust host-entrypoint sync or routing path instead."
            ));
        }
        if is_host_private_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of host-private Claude state {path}; project policy must live in repo settings or Rust runtime code."
            ));
        }
    }
    if let Some(path) = bash_write_target(payload) {
        if is_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of retired generated surface {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_generated_entrypoint(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of generated host entrypoint {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_framework_guarded_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of framework routing/runtime file {path}; use the Rust host-entrypoint sync or routing path instead."
            ));
        }
        if is_host_private_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of host-private Claude state {path}; keep shared policy in project settings."
            ));
        }
    }
    None
}

fn run_user_prompt_submit(repo_root: &Path, payload: &Value) -> Option<Value> {
    let prompt = payload
        .get("prompt")
        .or_else(|| payload.get("user_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !claude_review_gate_disabled()
        && (is_review_prompt(prompt) || has_review_override(prompt) || has_override(prompt))
    {
        let mut state = match load_claude_review_gate_disk(repo_root, payload) {
            ClaudeDiskState::Unreadable => {
                let path = claude_review_state_path(repo_root, payload);
                eprintln!(
                    "[router-rs] claude review_gate state unreadable: {}",
                    path.display()
                );
                return add_context(
                    "UserPromptSubmit",
                    &format!(
                        "{CLAUDE_HOOK_STATE_UNREADABLE} (path {}). Repair JSON or permissions before continuing.",
                        path.display()
                    ),
                );
            }
            ClaudeDiskState::Absent => ClaudeReviewGateState::default(),
            ClaudeDiskState::Ok(s) => s,
        };
        state.review_required = state.review_required || is_review_prompt(prompt);
        state.review_override =
            state.review_override || has_review_override(prompt) || has_override(prompt);
        write_claude_review_state(repo_root, payload, &state);
        if state.review_required && !state.review_override {
            return add_context(
                "UserPromptSubmit",
                "Review gate: start an observed independent reviewer lane first (`fork_context=false`), then synthesize findings locally. Claude records independent reviewer evidence when the hook payload proves the lane and fork setting.",
            );
        }
    }
    if prompt_mentions_automation(prompt) {
        return add_context("UserPromptSubmit", AUTOMATION_CONTEXT);
    }
    None
}

fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    record_claude_reviewer_evidence(repo_root, payload);
    let paths = payload_relative_paths(repo_root, payload);
    let touched_settings = paths.iter().any(|path| is_settings_path(path));
    let touched_framework = paths.iter().any(|path| is_framework_guarded_path(path));
    let settings_validated =
        payload_is_successful_bash(payload) && payload_runs_settings_validation(payload);
    let framework_tested =
        payload_is_successful_bash(payload) && payload_runs_framework_tests(payload);
    if touched_settings || touched_framework || settings_validated || framework_tested {
        persist_touch_state(
            repo_root,
            payload,
            touched_settings,
            touched_framework,
            settings_validated,
            framework_tested,
        );
    }
    match (touched_settings, touched_framework) {
        (true, true) => add_context(
            "PostToolUse",
            &format!("{SETTINGS_CHANGED_CONTEXT}\n{FRAMEWORK_CHANGED_CONTEXT}"),
        ),
        (true, false) => add_context("PostToolUse", SETTINGS_CHANGED_CONTEXT),
        (false, true) => add_context("PostToolUse", FRAMEWORK_CHANGED_CONTEXT),
        (false, false) => None,
    }
}

fn run_stop(repo_root: &Path, payload: &Value) -> Option<Value> {
    let review_load = load_claude_review_gate_disk(repo_root, payload);
    let touch_load = load_claude_touch_state_disk(repo_root, payload);
    if matches!(review_load, ClaudeDiskState::Unreadable) {
        eprintln!(
            "[router-rs] claude review_gate state unreadable on Stop: {}",
            claude_review_state_path(repo_root, payload).display()
        );
        return block_stop(CLAUDE_HOOK_STATE_UNREADABLE);
    }
    if matches!(touch_load, ClaudeDiskState::Unreadable) {
        eprintln!(
            "[router-rs] claude hook_state unreadable on Stop: {}",
            touch_state_path(repo_root, payload).display()
        );
        return block_stop(CLAUDE_HOOK_STATE_UNREADABLE);
    }

    let review_state = match review_load {
        ClaudeDiskState::Absent => ClaudeReviewGateState::default(),
        ClaudeDiskState::Ok(s) => s,
        ClaudeDiskState::Unreadable => {
            return block_stop(CLAUDE_HOOK_STATE_UNREADABLE);
        }
    };
    if !claude_review_gate_disabled()
        && review_gate_blocks_stop(ReviewGateFacts {
            review_required: review_state.review_required,
            review_override: review_state.review_override,
            independent_reviewer_seen: review_state.independent_reviewer_seen,
        })
    {
        return block_stop(
            "CLAUDE_REVIEW_GATE incomplete: run an observed independent reviewer lane with explicit fork_context=false before closing this review turn.",
        );
    }
    let state = match touch_load {
        ClaudeDiskState::Absent => TouchState::default(),
        ClaudeDiskState::Ok(s) => s,
        ClaudeDiskState::Unreadable => {
            return block_stop(CLAUDE_HOOK_STATE_UNREADABLE);
        }
    };
    if state.settings && !state.settings_validated {
        return block_stop("Validate Claude hook/settings JSON before ending this turn.");
    }
    if state.framework && !state.framework_tested {
        return block_stop("Run targeted Rust contract tests for framework routing/runtime changes before ending this turn.");
    }
    clear_claude_review_state(repo_root, payload);
    clear_touch_state(repo_root, payload);
    None
}

#[derive(Default)]
struct TouchState {
    settings: bool,
    framework: bool,
    settings_validated: bool,
    framework_tested: bool,
}

#[derive(Default)]
struct ClaudeReviewGateState {
    review_required: bool,
    review_override: bool,
    independent_reviewer_seen: bool,
}

fn try_extract_claude_session_string(payload: &Value) -> Option<String> {
    let map = payload.as_object()?;
    try_session_ids_from_object(map)
}

fn try_session_ids_from_object(map: &Map<String, Value>) -> Option<String> {
    for key in [
        "session_id",
        "conversation_id",
        "thread_id",
        "chat_id",
        "transcript_path",
        "conversationId",
        "threadId",
        "sessionId",
    ] {
        if let Some(value) = map.get(key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(meta) = map.get("metadata").and_then(Value::as_object) {
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

fn first_nonempty_payload_str(payload: &Value, keys: &[&str]) -> String {
    let Some(map) = payload.as_object() else {
        return String::new();
    };
    for key in keys {
        if let Some(s) = map.get(*key).and_then(Value::as_str) {
            let t = s.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    String::new()
}

fn claude_repo_fallback_token(repo_root: &Path) -> String {
    let resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    format!(
        "claude-repo::{}",
        resolved.to_string_lossy().replace('\\', "/")
    )
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
        let _ = FmtWrite::write_fmt(&mut s, format_args!("{:02x}", byte));
    }
    s
}

/// 与 Cursor `session_key` 同类：**显式会话串** → **`ROUTER_RS_CLAUDE_SESSION_NAMESPACE`** → **`cwd` 类字段** → **repo 稳定 token**。
/// 同仓多会话在无 id 时仍可能共用状态文件；需并行分流时与 Cursor 一样设 namespace。
fn claude_session_key(repo_root: &Path, payload: &Value) -> String {
    if let Some(raw) = try_extract_claude_session_string(payload) {
        return short_hash(&raw);
    }
    if let Ok(ns) = std::env::var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE") {
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
    let cwd = first_nonempty_payload_str(payload, CWD_KEYS);
    if !cwd.is_empty() {
        return short_hash(&format!("cwd::{cwd}"));
    }
    short_hash(&claude_repo_fallback_token(repo_root))
}

fn claude_review_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    repo_root.join(".claude").join(format!(
        "review_gate_{}.json",
        claude_session_key(repo_root, payload)
    ))
}

#[derive(Debug, Clone)]
enum ClaudeDiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
}

fn load_claude_review_gate_disk(
    repo_root: &Path,
    payload: &Value,
) -> ClaudeDiskState<ClaudeReviewGateState> {
    let path = claude_review_state_path(repo_root, payload);
    if !path.is_file() {
        return ClaudeDiskState::Absent;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return ClaudeDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return ClaudeDiskState::Unreadable;
    }
    let value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return ClaudeDiskState::Unreadable,
    };
    ClaudeDiskState::Ok(ClaudeReviewGateState {
        review_required: value
            .get("review_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        review_override: value
            .get("review_override")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        independent_reviewer_seen: value
            .get("independent_reviewer_seen")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn load_claude_touch_state_disk(repo_root: &Path, payload: &Value) -> ClaudeDiskState<TouchState> {
    let path = touch_state_path(repo_root, payload);
    if !path.is_file() {
        return ClaudeDiskState::Absent;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return ClaudeDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return ClaudeDiskState::Unreadable;
    }
    let payload_val: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return ClaudeDiskState::Unreadable,
    };
    ClaudeDiskState::Ok(TouchState {
        settings: payload_val
            .get("settings")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        framework: payload_val
            .get("framework")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        settings_validated: payload_val
            .get("settings_validated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        framework_tested: payload_val
            .get("framework_tested")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn write_claude_review_state(repo_root: &Path, payload: &Value, state: &ClaudeReviewGateState) {
    let path = claude_review_state_path(repo_root, payload);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let value = json!({
        "review_required": state.review_required,
        "review_override": state.review_override,
        "independent_reviewer_seen": state.independent_reviewer_seen,
    });
    if let Err(err) = fs::write(path, format!("{value}\n")) {
        eprintln!("[router-rs] claude hook state write failed (review_gate): {err}");
    }
}

fn clear_claude_review_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(claude_review_state_path(repo_root, payload));
}

fn claude_tool_input(payload: &Value) -> Value {
    payload
        .as_object()
        .and_then(crate::hook_common::tool_input_value_from_map)
        .unwrap_or_else(|| json!({}))
}

fn claude_reviewer_lane(tool_input: &Value, payload: &Value) -> bool {
    let subagent_type = normalize_subagent_type(
        tool_input
            .get("subagent_type")
            .or_else(|| tool_input.get("agent_type"))
            .or_else(|| tool_input.get("type"))
            .or_else(|| payload.get("subagent_type"))
            .or_else(|| payload.get("agent_type"))
            .and_then(Value::as_str),
    );
    matches!(
        subagent_type.as_str(),
        "general-purpose"
            | "generalpurpose"
            | "best-of-n-runner"
            | "bestofnrunner"
            | "review"
            | "reviewer"
            | "critic"
            | "code-review"
    )
}

fn claude_subagent_tool(payload: &Value) -> bool {
    let name = normalize_tool_name(
        payload
            .get("tool_name")
            .or_else(|| payload.get("tool"))
            .or_else(|| payload.get("name"))
            .and_then(Value::as_str),
    );
    tool_name_implies_subagent(&name)
}

fn tool_name_implies_subagent(normalized: &str) -> bool {
    if matches!(
        normalized,
        "task"
            | "functions.task"
            | "functions.subagent"
            | "functions.spawn_agent"
            | "subagent"
            | "spawn_agent"
    ) {
        return true;
    }
    if normalized.ends_with("_subagent")
        || normalized.ends_with("_spawn_agent")
        || normalized.ends_with(".subagent")
        || normalized.ends_with(".spawn_agent")
    {
        return true;
    }
    normalized
        .split('.')
        .any(|seg| seg == "subagent" || seg == "spawn_agent")
}

fn record_claude_reviewer_evidence(repo_root: &Path, payload: &Value) {
    let mut state = match load_claude_review_gate_disk(repo_root, payload) {
        ClaudeDiskState::Unreadable => return,
        ClaudeDiskState::Absent => ClaudeReviewGateState::default(),
        ClaudeDiskState::Ok(s) => s,
    };
    if !state.review_required || state.review_override {
        return;
    }
    let tool_input = claude_tool_input(payload);
    let fork = fork_context_from_values(&tool_input, Some(payload));
    if claude_subagent_tool(payload)
        && independent_reviewer_evidence(claude_reviewer_lane(&tool_input, payload), fork)
    {
        state.independent_reviewer_seen = true;
        write_claude_review_state(repo_root, payload, &state);
    }
}

fn legacy_touch_state_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".claude/hook_state.json")
}

fn touch_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    repo_root.join(".claude").join(format!(
        "hook_state_{}.json",
        claude_session_key(repo_root, payload)
    ))
}

fn persist_touch_state(
    repo_root: &Path,
    session_payload: &Value,
    settings: bool,
    framework: bool,
    settings_validated: bool,
    framework_tested: bool,
) {
    let current = match load_claude_touch_state_disk(repo_root, session_payload) {
        ClaudeDiskState::Unreadable => {
            eprintln!(
                "[router-rs] claude hook_state unreadable; skip merge (path {}): repair JSON or remove file",
                touch_state_path(repo_root, session_payload).display()
            );
            return;
        }
        ClaudeDiskState::Absent => TouchState::default(),
        ClaudeDiskState::Ok(s) => s,
    };
    let state_payload = json!({
        "settings": current.settings || settings,
        "framework": current.framework || framework,
        "settings_validated": current.settings_validated || settings_validated,
        "framework_tested": current.framework_tested || framework_tested,
    });
    let path = touch_state_path(repo_root, session_payload);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(legacy_touch_state_path(repo_root));
    if let Err(err) = fs::write(path, format!("{}\n", state_payload)) {
        eprintln!("[router-rs] claude hook state write failed (hook_state): {err}");
    }
}

fn clear_touch_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(touch_state_path(repo_root, payload));
    let _ = fs::remove_file(legacy_touch_state_path(repo_root));
}

fn prompt_mentions_automation(prompt: &str) -> bool {
    let lowered = prompt.to_ascii_lowercase();
    [
        "from now on",
        "whenever",
        "every time",
        "each time",
        "before ",
        "after ",
        "每次",
        "以后",
        "从现在起",
        " whenever ",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn dangerous_bash_reason(payload: &Value) -> Option<String> {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let command = bash_command(payload)?;
    if let Some(reason) = crate::hook_policy::dangerous_bash_reason(command) {
        return Some(reason);
    }
    let lowered = command.to_ascii_lowercase();
    let supplemental: &[(&str, &str)] = &[
        (r"\bmkfs\.", "filesystem formatting command"),
        (r":\(\)\s*\{\s*:\|:&\s*\};:", "fork bomb"),
        (r"\bgit\s+branch\s+-d\b", "git branch deletion"),
    ];
    for (pattern, label) in supplemental {
        if Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(&lowered))
            .unwrap_or(false)
        {
            return Some(format!("Blocked dangerous shell command: {label}."));
        }
    }
    None
}

fn payload_relative_paths(repo_root: &Path, payload: &Value) -> Vec<String> {
    let mut paths = HashSet::new();
    collect_payload_paths(payload, &mut paths);
    paths
        .into_iter()
        .filter_map(|path| repo_relative_slash_path(repo_root, &path))
        .collect()
}

fn collect_payload_paths(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if is_path_key(key) {
                    collect_path_value(child, paths);
                }
                collect_payload_paths(child, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_payload_paths(item, paths);
            }
        }
        _ => {}
    }
}

fn collect_path_value(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::String(text) => {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                paths.insert(normalized);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_path_value(item, paths);
            }
        }
        _ => {}
    }
}

fn is_path_key(key: &str) -> bool {
    matches!(
        key,
        "file_path"
            | "changed_path"
            | "path"
            | "config_path"
            | "target_path"
            | "changed_files"
            | "file_paths"
            | "paths"
    )
}

fn bash_write_target(payload: &Value) -> Option<String> {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let command = bash_command(payload)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in RETIRED_SURFACE_PATHS
            .iter()
            .chain(GENERATED_ENTRYPOINT_PATHS.iter())
            .chain(CLAUDE_HOME_GUARD_HINTS.iter())
        {
            if !segment.contains(hint) {
                continue;
            }
            if looks_mutating || bash_segment_redirects_to_hint(&segment, hint) {
                return Some((*hint).to_string());
            }
        }
    }
    None
}

fn bash_command(payload: &Value) -> Option<&str> {
    payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)
}

fn split_bash_segments(command: &str) -> Vec<String> {
    Regex::new(r"\s*(?:&&|\|\||;|\|)\s*")
        .ok()
        .map(|regex| {
            regex
                .split(command)
                .filter_map(|segment| {
                    let trimmed = segment.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![command.trim().to_string()])
}

fn bash_command_looks_mutating(command: &str) -> bool {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate|mkdir)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(command))
            .unwrap_or(false)
    })
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    })
}

fn is_retired_surface(path: &str) -> bool {
    RETIRED_SURFACE_PATHS
        .iter()
        .any(|retired| path == *retired || path.starts_with(&format!("{retired}/")))
}

fn is_generated_entrypoint(path: &str) -> bool {
    GENERATED_ENTRYPOINT_PATHS.contains(&path)
}

fn is_host_private_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with("~/.claude/") {
        return true;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let prefix = PathBuf::from(home)
            .join(".claude")
            .to_string_lossy()
            .replace('\\', "/")
            + "/";
        if normalized.starts_with(&prefix) {
            return true;
        }
    }
    false
}

fn is_settings_path(path: &str) -> bool {
    SETTINGS_GUARDED_PATHS.contains(&path)
}

fn is_framework_guarded_path(path: &str) -> bool {
    FRAMEWORK_GUARDED_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

fn payload_is_successful_bash(payload: &Value) -> bool {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return false;
    }
    match payload_exit_code(payload) {
        Some(0) => true,
        Some(_) => false,
        None => !payload_text(payload).contains("\"error\""),
    }
}

fn payload_exit_code(payload: &Value) -> Option<i64> {
    find_numeric_key(payload, &["exit_code", "exitCode", "status"])
}

fn find_numeric_key(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(number) = map.get(*key).and_then(Value::as_i64) {
                    return Some(number);
                }
            }
            map.values().find_map(|child| find_numeric_key(child, keys))
        }
        Value::Array(items) => items.iter().find_map(|child| find_numeric_key(child, keys)),
        _ => None,
    }
}

fn payload_runs_settings_validation(payload: &Value) -> bool {
    let Some(command) = bash_command(payload) else {
        return false;
    };
    let lowered = command.to_ascii_lowercase();
    (lowered.contains("jq") || lowered.contains("python") || lowered.contains("node"))
        && (lowered.contains(".claude/settings.json")
            || lowered.contains(".claude/settings.local.json"))
}

fn payload_runs_framework_tests(payload: &Value) -> bool {
    let Some(command) = bash_command(payload) else {
        return false;
    };
    let lowered = command.to_ascii_lowercase();
    if !lowered.contains("cargo test") {
        return false;
    }
    [
        "--manifest-path scripts/router-rs/cargo.toml",
        "scripts/router-rs/cargo.toml",
        "router-rs",
        "--test policy_contracts",
        "--test documentation_contracts",
        "--test host_integration",
    ]
    .iter()
    .any(|hint| lowered.contains(hint))
}

fn payload_text(payload: &Value) -> String {
    serde_json::to_string(payload)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_dangerous_bash() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git reset --hard HEAD" }
        });
        let output = run_pre_tool_use(Path::new("/repo"), &payload).unwrap();
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
    }

    #[test]
    fn silent_for_safe_read_only_bash() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status --short" }
        });
        assert!(run_pre_tool_use(Path::new("/repo"), &payload).is_none());
    }

    #[test]
    fn claude_stdin_limited_rejects_over_size() {
        let large = vec![b'a'; 5 * 1024 * 1024];
        let mut cursor = std::io::Cursor::new(large);
        let err = read_claude_stdin_limited(&mut cursor).unwrap_err();
        assert!(err.contains("4 MiB"), "unexpected err: [{err}]");
    }

    #[test]
    fn claude_stdin_limited_rejects_invalid_utf8() {
        let mut cursor = std::io::Cursor::new(vec![0xff, 0xfe, 0xfd]);
        let err = read_claude_stdin_limited(&mut cursor).unwrap_err();
        assert_eq!(err, "stdin_invalid_utf8");
    }

    #[test]
    fn subagent_tool_accepts_dotted_subagent_segment() {
        let p = json!({"tool_name": "lane.subagent.run"});
        assert!(claude_subagent_tool(&p));
    }

    #[test]
    fn subagent_tool_rejects_subagent_as_plain_substring() {
        let p = json!({"tool_name": "not_really_subagent_helpers"});
        assert!(!claude_subagent_tool(&p));
    }

    #[test]
    fn stop_does_not_trust_payload_text_for_framework_tests() {
        let repo = unique_test_repo("stop-text-framework");
        let payload = json!({ "session_id": "s-text", "transcript": "cargo test passed" });
        persist_touch_state(&repo, &payload, false, true, false, false);

        let output = run_stop(&repo, &payload).unwrap();

        assert_eq!(output["continue"], false);
        assert_eq!(output["stopReason"], "Run targeted Rust contract tests for framework routing/runtime changes before ending this turn.");
        assert_eq!(output["decision"], "block");
        clear_touch_state(&repo, &payload);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn successful_framework_test_allows_stop() {
        let repo = unique_test_repo("framework-tested");
        let session = json!({ "session_id": "s-framework-ok" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-framework-ok",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --manifest-path scripts/router-rs/Cargo.toml claude_hooks"
            },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        assert!(!touch_state_path(&repo, &session).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn failed_framework_test_keeps_stop_blocked() {
        let repo = unique_test_repo("framework-test-failed");
        let session = json!({ "session_id": "s-framework-fail" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-framework-fail",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --manifest-path scripts/router-rs/Cargo.toml claude_hooks"
            },
            "exit_code": 101
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        let output = run_stop(&repo, &session).unwrap();

        assert_eq!(output["continue"], false);
        assert_eq!(output["stopReason"], "Run targeted Rust contract tests for framework routing/runtime changes before ending this turn.");
        assert_eq!(output["decision"], "block");
        clear_touch_state(&repo, &session);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn automation_prompt_triggers_context() {
        let repo = unique_test_repo("automation-prompt-context");
        for phrase in [
            "from now on always run tests",
            "whenever I save",
            "每次提交之前",
        ] {
            let payload = json!({ "prompt": phrase });
            let output = run_user_prompt_submit(&repo, &payload).unwrap();
            assert_eq!(
                output["hookSpecificOutput"]["hookEventName"], "UserPromptSubmit",
                "phrase={phrase}"
            );
            assert!(
                output["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .unwrap_or("")
                    .contains("hook"),
                "phrase={phrase}"
            );
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn non_automation_prompt_is_silent() {
        let repo = unique_test_repo("non-automation-prompt");
        let payload = json!({ "prompt": "fix the failing test in main.rs" });
        assert!(run_user_prompt_submit(&repo, &payload).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_prompt_blocks_stop_until_independent_reviewer_seen() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-block");
        let payload = json!({ "session_id": "s-review", "prompt": "深度 review 这个 PR" });
        let context = run_user_prompt_submit(&repo, &payload).expect("review context");
        assert!(context["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("")
            .contains("fork_context=false"));
        let stop = run_stop(&repo, &json!({ "session_id": "s-review" })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_requires_explicit_false_fork() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-shared-fork");
        let prompt = json!({ "session_id": "s-shared", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let shared = json!({
            "session_id": "s-shared",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": true}
        });
        assert!(run_post_tool_use(&repo, &shared).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": "s-shared" })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_allows_matching_independent_reviewer() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-pass");
        let prompt = json!({ "session_id": "s-pass", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let reviewer = json!({
            "session_id": "s-pass",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        assert!(run_stop(&repo, &json!({ "session_id": "s-pass" })).is_none());
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_rejects_explore_even_with_fork_false() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-explore-reject");
        let prompt = json!({ "session_id": "s-explore", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let explorer = json!({
            "session_id": "s-explore",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &explorer).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": "s-explore" })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_skipped_when_disable_env_set() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", "1");
        let repo = unique_test_repo("review-gate-disabled-env");
        let payload = json!({ "session_id": "s-off", "prompt": "深度 review 这个 PR" });
        assert!(
            run_user_prompt_submit(&repo, &payload).is_none(),
            "disable env must suppress UserPromptSubmit review nag"
        );
        let stop = run_stop(&repo, &json!({ "session_id": "s-off" }));
        assert!(
            stop.is_none(),
            "disable env must allow Stop without independent reviewer evidence; got {stop:?}"
        );
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_gate_still_blocks_when_disable_env_is_noncanonical_token() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", "maybe");
        let repo = unique_test_repo("review-gate-disable-garbage");
        let payload = json!({ "session_id": "s-garbage", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &payload).expect("review nag");
        let stop = run_stop(&repo, &json!({ "session_id": "s-garbage" })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parse_claude_hook_stdin_trimmed_accepts_empty_and_valid_json() {
        assert_eq!(
            super::parse_claude_hook_stdin_trimmed("").unwrap(),
            json!({})
        );
        assert_eq!(
            super::parse_claude_hook_stdin_trimmed(r#"{"session_id":"x"}"#).unwrap(),
            json!({"session_id":"x"})
        );
    }

    #[test]
    fn parse_claude_hook_stdin_trimmed_rejects_invalid_json() {
        let err = super::parse_claude_hook_stdin_trimmed("not json").unwrap_err();
        assert!(
            err.starts_with("stdin_json_invalid:"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn claude_session_key_metadata_session_id_matches_flat() {
        let repo = unique_test_repo("claude-meta-session");
        let flat = json!({"session_id": "sid-meta", "prompt": "x"});
        let nested = json!({"metadata": {"sessionId": "sid-meta"}, "prompt": "x"});
        assert_eq!(
            claude_session_key(&repo, &flat),
            claude_session_key(&repo, &nested)
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_session_key_namespace_splits_same_repo_empty_payload() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-ns");
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-a");
        let a = claude_session_key(&repo, &json!({}));
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-b");
        let b = claude_session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_ne!(a, b, "namespace must split state for empty payload");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_session_key_repo_fallback_stable_without_id() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-repo-fb");
        let k1 = claude_session_key(&repo, &json!({}));
        let k2 = claude_session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_eq!(k1, k2);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_lexical_traversal_disguised_framework_path() {
        let repo = unique_test_repo("lexical-fw-path");
        fs::create_dir_all(repo.join("nest")).unwrap();
        assert_eq!(
            super::repo_relative_slash_path(&repo, "nest/../../skills/SKILL_ROUTING_RUNTIME.json")
                .as_deref(),
            Some("skills/SKILL_ROUTING_RUNTIME.json")
        );
        let payload = json!({
            "tool_name": "Write",
            "file_path": "nest/../../skills/SKILL_ROUTING_RUNTIME.json"
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_lexical_traversal_to_generated_entrypoint() {
        let repo = unique_test_repo("lexical-entrypoint");
        fs::write(repo.join("AGENTS.md"), b"x").unwrap();
        let payload = json!({
            "tool_name": "Edit",
            "file_path": "a/../../AGENTS.md"
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stop_blocks_when_review_gate_state_corrupt() {
        let repo = unique_test_repo("corrupt-review-gate");
        let session = json!({ "session_id": "s-corrupt-rg" });
        let path = claude_review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_stop(&repo, &session).expect("block");
        assert_eq!(out["decision"], "block");
        let reason = out["stopReason"].as_str().unwrap();
        assert!(
            reason.contains("CLAUDE_HOOK_STATE_UNREADABLE"),
            "unexpected: {reason}"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stop_blocks_when_touch_state_corrupt() {
        let repo = unique_test_repo("corrupt-touch");
        let session = json!({ "session_id": "s-corrupt-touch" });
        let path = touch_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_stop(&repo, &session).expect("block");
        assert_eq!(out["decision"], "block");
        assert!(out["stopReason"]
            .as_str()
            .unwrap()
            .contains("CLAUDE_HOOK_STATE_UNREADABLE"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_returns_context_when_review_gate_corrupt() {
        let _env = crate::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-ups");
        let session = json!({ "session_id": "s-corrupt-ups", "prompt": "深度 review 这个 PR" });
        let path = claude_review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        assert!(out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("CLAUDE_HOOK_STATE_UNREADABLE"));
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn canonical_command_rejects_unknown_event() {
        let err = canonical_claude_hook_command("unknown-event").unwrap_err();
        assert!(err.contains("Unsupported Claude hook command"), "{err}");
    }

    #[test]
    fn successful_settings_validation_allows_stop() {
        let repo = unique_test_repo("settings-validated");
        let session = json!({ "session_id": "s-settings-ok" });
        persist_touch_state(&repo, &session, true, false, false, false);
        let payload = json!({
            "session_id": "s-settings-ok",
            "tool_name": "Bash",
            "tool_input": { "command": "jq empty .claude/settings.json" },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        assert!(!touch_state_path(&repo, &session).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn root_contract_tests_count_as_framework_validation() {
        let repo = unique_test_repo("framework-root-contracts");
        let session = json!({ "session_id": "s-root-contracts" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-root-contracts",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --test policy_contracts --test documentation_contracts"
            },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn legacy_repo_scoped_touch_state_does_not_block_new_session() {
        let repo = unique_test_repo("legacy-touch-state");
        let legacy = legacy_touch_state_path(&repo);
        fs::write(
            &legacy,
            "{\"framework\":true,\"framework_tested\":false,\"settings\":false,\"settings_validated\":false}\n",
        )
        .unwrap();

        assert!(run_stop(&repo, &json!({ "session_id": "fresh-session" })).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_payload_sent_to_claude_hook_is_ignored() {
        let repo = unique_test_repo("cursor-payload-isolation");
        let payload = json!({
            "session_id": "cursor-session",
            "hook_event_name": "postToolUse",
            "cursor_version": "3.3.30",
            "workspace_roots": [repo.to_string_lossy()],
            "transcript_path": "/Users/joe/.cursor/projects/example/session.json",
            "tool_name": "Bash",
            "tool_input": {
                "command": "apply_patch scripts/router-rs/src/claude_hooks.rs"
            },
            "file_path": "scripts/router-rs/src/claude_hooks.rs",
            "exit_code": 0
        });

        let output = dispatch_claude_hook_payload("post-tool-use", &repo, &payload);

        assert_eq!(output, silent_success());
        assert!(!legacy_touch_state_path(&repo).exists());
        assert!(!touch_state_path(&repo, &payload).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_stop_payload_sent_to_claude_hook_does_not_block() {
        let repo = unique_test_repo("cursor-stop-isolation");
        persist_touch_state(
            &repo,
            &json!({ "session_id": "cursor-session" }),
            false,
            true,
            false,
            false,
        );
        let payload = json!({
            "session_id": "cursor-session",
            "hook_event_name": "stop",
            "cursor_version": "3.3.30",
            "workspace_roots": [repo.to_string_lossy()]
        });

        let output = dispatch_claude_hook_payload("stop", &repo, &payload);

        assert_eq!(output, silent_success());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn partial_cursor_envelope_without_hook_event_runs_claude_pre_tool() {
        let repo = unique_test_repo("forge-cursor-envelope-partial");
        let payload = json!({
            "session_id": "forge",
            "cursor_version": "9.9.9",
            "workspace_roots": [repo.to_string_lossy()],
            "tool_name": "Bash",
            "tool_input": { "command": "git reset --hard HEAD" }
        });
        let output = dispatch_claude_hook_payload("pre-tool-use", &repo, &payload);
        assert!(
            output.get("hookSpecificOutput").is_some(),
            "must not silent_success on partial envelope; got {output:?}"
        );
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_payload_with_nested_cursor_path_is_not_silenced_as_cursor_stdin() {
        let repo = unique_test_repo("claude-cursor-path-not-envelope");
        let cursor_plan = repo.join(".cursor").join("plans").join("feature.plan.md");
        fs::create_dir_all(cursor_plan.parent().unwrap()).unwrap();
        let payload = json!({
            "session_id": "claude-session",
            "tool_name": "Bash",
            "tool_input": { "command": "rm -rf /" },
            "file_path": cursor_plan.to_string_lossy(),
        });
        let output = dispatch_claude_hook_payload("pre-tool-use", &repo, &payload);
        assert!(
            output.get("hookSpecificOutput").is_some(),
            "expected PreToolUse decision payload, not bare silent_success; got {output:?}"
        );
        assert_eq!(
            output["hookSpecificOutput"]["permissionDecision"],
            json!("deny")
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_version_without_workspace_roots_is_not_envelope() {
        let repo = unique_test_repo("cursor-version-only-not-envelope");
        let payload = json!({
            "session_id": "mixed",
            "cursor_version": "3.3.30",
            "tool_name": "Bash",
            "tool_input": { "command": "rm -rf /" },
        });
        let output = dispatch_claude_hook_payload("pre-tool-use", &repo, &payload);
        assert_eq!(
            output["hookSpecificOutput"]["permissionDecision"],
            json!("deny")
        );
        let _ = fs::remove_dir_all(repo);
    }

    fn unique_test_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "router-rs-claude-hooks-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".claude")).unwrap();
        path
    }
}
