//! L4 transport for `host_id=claude`; review/closeout policy in `core-policy`.
//! Claude（Anthropic）hooks：`router-rs claude hook --event=… --repo-root …`。
//! 历史版本接口快照：`git show 89ece4c^:core/router-rs/src/claude_hooks.rs`（事件：`pre-tool-use`、`user-prompt-submit`、`post-tool-use`、`stop`；CLI 亦接受 `PreToolUse` 等 PascalCase 别名，与 Codex hook 拼写对齐）。
//!
//! **误接 Cursor hook stdin**：仅在 stdin JSON 呈现结构化 Cursor envelope（顶层非空 `cursor_version` 字符串 + `workspace_roots` 数组 + 非空 `hook_event_name` 或 `hookEventName`）时整条静默；
//! 不用路径子串扫描，以免合法 Claude 载荷（例如编辑 `.cursor/` 下文件）被误判为 Cursor 而旁路门禁。
//! stdin 体量上限 4 MiB，与 Codex hook 读取路径对齐，防失控输入撑爆 hook 进程内存。
use core_policy::hook_common::{
    has_override, is_narrow_review_prompt, is_review_prompt, normalize_subagent_type,
    normalize_tool_name,
};
use core_policy::review_gate_engine::{
    fork_context_from_values, review_independent_reviewer_evidence,
};
use core_policy::HookReviewDiskCore;
use serde_json::{Map, Value, json};
use crate::hosts::hook_dispatch::{
    HookEvent, HookOutput, HostHookConfig, HostHookDispatcher,
    add_context, bash_command, closeout_completion_text, collect_path_value,
    collect_payload_paths, compact_repo_relative_segments, extract_output_summary,
    find_numeric_key, first_nonempty_payload_str, hook_output_to_json_value,
    is_framework_guarded_path, is_framework_source_path, is_generated_entrypoint,
    is_host_private_path, is_path_key, is_settings_path, is_verification_command,
    normalize_path_lexical, parse_stdio_agent_hook_stdin_trimmed,
    payload_exit_code, payload_is_successful_bash, payload_is_successful_tool,
    payload_looks_like_foreign_hook_stdin, read_stdio_agent_stdin_limited,
    shared_framework_test_advisory, shared_goal_stop_followup_line,
    shared_settings_validation_advisory, shared_stop_review_output_lint_suppressed,
    silent_success, subagent_tool, tool_name_implies_subagent, value_to_hook_output,
    AgentDiskState, TouchState, deny_pre_tool_use, write_review_state_unlocked,
    read_review_gate_file, load_touch_state_from_path, should_sync_review_gate_on_user_prompt,
    auto_record_verification_evidence, auto_record_research_activity,
};
use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// Hook-state base directory for Claude: `<repo_root>/.claude/hook-state/`
fn hook_state_base(repo_root: &Path) -> PathBuf {
    repo_root.join(".claude").join("hook-state")
}
#[cfg(not(unix))]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLAUDE_HOOK_STATE_UNREADABLE: &str =
    "router-rs CLAUDE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

/// mtime 缓存：`read_active_skill_context` 使用，避免重复 I/O + JSON 解析。
/// `write_active_skill_context` / `clear_active_skill_context` 时主动失效。
static SKILL_CONTEXT_CACHE: Mutex<Option<(std::time::SystemTime, Value)>> = Mutex::new(None);

/// 与 Claude 共享 hook JSON 协议；通过 thread-local 切换 `.claude` 等宿主差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioAgentHookHost {
    Claude,
}

impl StdioAgentHookHost {
    fn state_dir(self) -> &'static str {
        match self {
            Self::Claude => ".claude",
        }
    }

    fn hook_state_unreadable(self) -> &'static str {
        match self {
            Self::Claude => CLAUDE_HOOK_STATE_UNREADABLE,
        }
    }

    fn session_namespace_env(self) -> &'static str {
        match self {
            Self::Claude => "ROUTER_RS_CLAUDE_SESSION_NAMESPACE",
        }
    }

    fn log_label(self) -> &'static str {
        match self {
            Self::Claude => "claude",
        }
    }

    fn settings_guarded_paths(self) -> &'static [&'static str] {
        match self {
            Self::Claude => SETTINGS_GUARDED_PATHS_CLAUDE,
        }
    }

    fn generated_entrypoint_paths(self) -> &'static [&'static str] {
        match self {
            Self::Claude => GENERATED_ENTRYPOINT_PATHS_CLAUDE,
        }
    }

    fn user_config_dir_leaf(self) -> &'static str {
        match self {
            Self::Claude => ".claude",
        }
    }
}

thread_local! {
    static ACTIVE_STDIO_AGENT_HOOK_HOST: Cell<StdioAgentHookHost> =
        const { Cell::new(StdioAgentHookHost::Claude) };
}

fn active_stdio_agent_hook_host() -> StdioAgentHookHost {
    ACTIVE_STDIO_AGENT_HOOK_HOST.with(|c| c.get())
}

fn with_stdio_agent_hook_host<R>(host: StdioAgentHookHost, f: impl FnOnce() -> R) -> R {
    struct Restore(StdioAgentHookHost);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_STDIO_AGENT_HOOK_HOST.with(|c| c.set(self.0));
        }
    }
    let previous = ACTIVE_STDIO_AGENT_HOOK_HOST.with(|c| c.replace(host));
    let _restore = Restore(previous);
    f()
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
            && let Ok(rel) = canon_file.strip_prefix(&canon_repo) {
                return Some(rel.to_string_lossy().replace('\\', "/"));
            }
        let abs_lex = normalize_path_lexical(&candidate);
        if let Ok(rel) = abs_lex.strip_prefix(&repo_lex) {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }
        return Some(abs_lex.to_string_lossy().replace('\\', "/"));
    }

    let rel_only = compact_repo_relative_segments(raw)?;
    let joined = normalize_path_lexical(&repo_root.join(&rel_only));
    if let Ok(rel) = joined.strip_prefix(&repo_lex) {
        return Some(rel.to_string_lossy().replace('\\', "/"));
    }
    Some(joined.to_string_lossy().replace('\\', "/"))
}

const FRAMEWORK_CHANGED_CONTEXT: &str = "Framework routing/runtime files changed; run the targeted Rust contract tests before finishing.";
const SETTINGS_CHANGED_CONTEXT: &str = "Hook/settings files changed; validate JSON and run the agent hook contract tests before finishing.";

fn stop_signal_text(payload: &Value) -> String {
    closeout_completion_text(payload)
}

fn apply_claude_review_gate_user_prompt(
    repo_root: &Path,
    payload: &Value,
    prompt: &str,
) -> Result<HookReviewDiskCore, String> {
    let path = review_state_path(repo_root, payload);
    let interactive = core_policy::hook_common::is_interactive_profile(Some(repo_root), prompt);
    let narrow = is_narrow_review_prompt(prompt);
    let goal_drive = core_policy::hook_common::is_framework_goal_entry_prompt(prompt);
    let review_arms = is_review_prompt(prompt) && !goal_drive;
    let override_now = has_override(prompt);
    with_claude_review_state_lock(&path, || {
        let mut state = match load_review_gate_disk(repo_root, payload) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} review_gate state unreadable: {}",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err("review_gate_unreadable".to_string());
            }
            AgentDiskState::Absent => HookReviewDiskCore::default(),
            AgentDiskState::Ok(s) => s,
        };
        if interactive || goal_drive || narrow {
            state.review_required = false;
            state.independent_reviewer_seen = false;
        } else {
            if review_arms && !override_now {
                state.independent_reviewer_seen = false;
            }
            state.review_required = state.review_required || review_arms;
        }
        state.review_override = state.review_override || override_now;
        write_review_state_unlocked(&path, &state)?;
        Ok(state)
    })
}

/// PreToolUse deny: framework-managed artifacts — generated catalogs/routing data and skill metadata.
const FRAMEWORK_GUARDED_PREFIXES: &[&str] = &[
    "configs/framework/",
    "skills/SKILL_PLUGIN_CATALOG.json",
    "skills/SKILL_ROUTING_METADATA.json",
    "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
    "skills/SKILL_HEALTH_MANIFEST.json",
    "skills/SKILL_APPROVAL_POLICY.json",
    "skills/SKILL_ROUTING_INDEX.md",
    "skills/SKILL_TIERS.json",
];
/// PostToolUse提醒 + Stop门禁: framework source code (overlap with GUARDED is intentional defense-in-depth when skip guard is set).
const FRAMEWORK_SOURCE_PREFIXES: &[&str] = &["core/router-rs/", "configs/framework/"];

const SETTINGS_GUARDED_PATHS_CLAUDE: &[&str] =
    &[".claude/settings.json", ".claude/settings.local.json"];
const GENERATED_ENTRYPOINT_PATHS_CLAUDE: &[&str] = &[".claude/CLAUDE.md"];
/// Cross-host generated surfaces: active in other hosts, Claude should not directly modify.
const CROSS_HOST_SURFACES: &[&str] = &[".codex/hooks.json"];
/// Pre-89ece4c the stdio agent hook accepted kebab-case commands only; CLI adds PascalCase aliases
/// aligned with Codex hook spelling (`PreToolUse`, `Stop`, …)。
pub fn run_claude_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    crate::hooks::ensure_kernel_bootstrap();
    let canonical = canonical_stdio_agent_hook_command(command)?;
    let telemetry_event = canonical.to_string();
    crate::hooks::mark_hook_start();

    // §1.4: 首次 dispatch 时清理孤儿 .lock 文件（exit(0) 不再跳过 Drop，清理历史残留）
    static CLEANED_HOOK_STATE_LOCKS: std::sync::Once = std::sync::Once::new();
    CLEANED_HOOK_STATE_LOCKS.call_once(|| {
        let hook_state_dir = hook_state_base(repo_root);
        if hook_state_dir.is_dir() {
            let cleaned = crate::hooks::sweep_orphan_lock_files(&hook_state_dir);
            if cleaned > 0 {
                eprintln!(
                    "[router-rs] claude hook: cleaned {cleaned} orphan lock file(s) from hook-state"
                );
            }
        }
    });

    let _registry_guard = core_policy::registry_review_gate::HookRegistryRepoGuard::new(repo_root);
    let result = with_stdio_agent_hook_host(StdioAgentHookHost::Claude, || {
        let payload = read_stdin_payload()?;
        let event = HookEvent { repo_root, event_name: canonical, payload: &payload };
        let output = ClaudeHookDispatcher.dispatch(&event);
        Ok(hook_output_to_json_value(canonical, output))
    });
    match &result {
        Ok(output) => crate::hooks::emit_hook_fired(
            &telemetry_event,
            crate::hooks::hook_action_from_output(output),
        ),
        Err(_) => crate::hooks::emit_hook_fired(&telemetry_event, "error"),
    }
    crate::hooks::emit_hook_timing_line(&telemetry_event);
    result
}

#[cfg_attr(not(test), allow(dead_code))]
fn dispatch_stdio_agent_hook_payload(canonical: &str, repo_root: &Path, payload: &Value) -> Value {
    crate::hooks::ensure_kernel_bootstrap();
    if payload_looks_like_foreign_hook_stdin(payload) {
        return silent_success();
    }
    let response = match canonical {
        "pre-tool-use" => run_pre_tool_use(repo_root, payload),
        "user-prompt-submit" => run_user_prompt_submit(repo_root, payload),
        "post-tool-use" => run_post_tool_use(repo_root, payload),
        "stop" => run_stop(repo_root, payload),
        // Defensive default: host should only dispatch canonical commands from `canonical_stdio_agent_hook_command`.
        _ => Some(silent_success()),
    };
    response.unwrap_or_else(silent_success)
}

fn canonical_stdio_agent_hook_command(command: &str) -> Result<&'static str, String> {
    match command.trim() {
        "pre-tool-use" | "PreToolUse" => Ok("pre-tool-use"),
        "user-prompt-submit" | "UserPromptSubmit" => Ok("user-prompt-submit"),
        "post-tool-use" | "PostToolUse" => Ok("post-tool-use"),
        "stop" | "Stop" => Ok("stop"),
        _ => Err(format!("Unsupported stdio agent hook command: {command}")),
    }
}

// ── HostHookDispatcher implementation (unified with opencode/cursor/codex) ──

pub struct ClaudeHookDispatcher;

impl HostHookConfig for ClaudeHookDispatcher {
    crate::impl_host_config!("claude", "Claude");
}



impl HostHookDispatcher for ClaudeHookDispatcher {
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_pre_tool_use(event.repo_root, event.payload);
        val.and_then(|v| value_to_hook_output(&v))
    }

    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_user_prompt_submit(event.repo_root, event.payload);
        val.and_then(|v| value_to_hook_output(&v))
    }

    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_post_tool_use(event.repo_root, event.payload);
        val.and_then(|v| value_to_hook_output(&v))
    }

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_stop(event.repo_root, event.payload);
        val.and_then(|v| value_to_hook_output(&v))
    }
}

/// Cross-host hook contract matrix: dispatch lifecycle hooks without stdin (test-only).
#[cfg(any(test, feature = "test-support"))]
pub fn dispatch_claude_hook_payload_for_test(
    canonical_event: &str,
    repo_root: &Path,
    payload: &Value,
) -> Value {
    crate::hooks::ensure_kernel_bootstrap();
    with_stdio_agent_hook_host(StdioAgentHookHost::Claude, || {
        let event = HookEvent { repo_root, event_name: canonical_event, payload };
        let output = ClaudeHookDispatcher.dispatch(&event);
        hook_output_to_json_value(canonical_event, output)
    })
}

/// `router-rs claude hook --event=… --repo-root …` — stdin JSON → Claude Code hook response JSON (line-delimited).
pub fn run_claude_hook_cli(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = crate::hooks::resolve_repo_root_arg(cli_repo_root)?;
    let mut output = run_claude_hook(event, &repo_root)?;
    crate::hooks::attach_router_rs_observation(
        &mut output,
        "claude",
    );
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    let mut stdout = std::io::stdout();
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    // DESIGN NOTE: 正常 return（而非 exit(0)）以确保 Drop 语义完整。
    //
    // 2026-06 修复：原先使用 exit(0) 跳过 Drop，导致 FileStateLockGuard::drop 不执行，
    // .lock 文件残留在 hook-state 目录中无法清理。实测 16+ 孤儿锁文件累积。
    //
    // 线程清理开销：hook 进程 ~30ms 生命周期内最多 2 个 detached 线程
    // (LogAggregator + route cache poller)，OS 进程退出时自动回收，
    // Drop 栈展开开销 <1ms，远小于锁泄漏的累积影响。
    Ok(()) // ← 正常返回，Drop 自动运行
}



fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = io::stdin();
    let input = read_stdio_agent_stdin_limited(&mut stdin)?;
    parse_stdio_agent_hook_stdin_trimmed(input.trim())
}


/// Cursor hook stdin 误接到 stdio agent hook 时的结构化识别（顶层字段）。
///
/// 刻意不使用嵌套字符串中的 `/.cursor/` 匹配，否则合法 stdio agent 工具载荷可能被整条静默。
/// 另要求 `hook_event_name` / `hookEventName`，降低仅凭顶造 `cursor_version`+`workspace_roots` 整条静默的面。
#[cfg_attr(not(test), allow(dead_code))]


// ── Active skill context (allowedTools linkage) ──────────────────

fn active_skill_context_path(repo_root: &Path) -> PathBuf {
    hook_state_base(repo_root).join("active-skill-context.json")
}

/// 从 SKILL_ROUTING_RUNTIME.json 读取指定 skill 的 allowedTools 列表。
/// 使用 OnceLock 缓存 JSON 解析结果，避免重复 I/O。
fn skill_allowed_tools_from_runtime(skill_slug: &str) -> Option<Vec<String>> {
    use std::sync::OnceLock;
    static RUNTIME_CACHE: OnceLock<Option<Value>> = OnceLock::new();

    let runtime = RUNTIME_CACHE.get_or_init(|| {
        let candidates = [
            std::path::PathBuf::from("skills/SKILL_ROUTING_RUNTIME.json"),
            std::path::PathBuf::from("SKILL_ROUTING_RUNTIME.json"),
        ];
        for path in &candidates {
            if let Ok(data) = std::fs::read_to_string(path)
                && let Ok(v) = serde_json::from_str(&data) {
                    return Some(v);
                }
        }
        None
    });

    let runtime = runtime.as_ref()?;
    let keys = runtime.get("keys")?.as_array()?;
    let slug_idx = keys.iter().position(|k| k.as_str() == Some("slug"))?;
    let allowed_idx = keys.iter().position(|k| k.as_str() == Some("allowedTools"))?;

    let skills = runtime.get("skills")?.as_array()?;
    for row in skills {
        let arr = row.as_array()?;
        if arr.get(slug_idx).and_then(Value::as_str) == Some(skill_slug) {
            let allowed = arr.get(allowed_idx)?;
            if allowed.is_null() {
                return None;
            }
            let tools = allowed
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>();
            return if tools.is_empty() { None } else { Some(tools) };
        }
    }
    None
}

/// 写入 active skill context（UserPromptSubmit 时调用）。
/// 写入后清除 mtime 缓存，确保下次 PreToolUse 读取到最新值。
fn write_active_skill_context(repo_root: &Path, skill_slug: &str, allowed_tools: &[String]) {
    let path = active_skill_context_path(repo_root);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ctx = json!({
        "skill_slug": skill_slug,
        "allowed_tools": allowed_tools,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    let _ = std::fs::write(&path, ctx.to_string());
    // 失效 mtime 缓存
    if let Ok(mut guard) = SKILL_CONTEXT_CACHE.lock() {
        *guard = None;
    }
}

/// 读取 active skill context（PreToolUse 时调用）。
/// 使用 mtime 缓存：如果文件 mtime 未变则返回缓存值，避免每次 PreToolUse 都读文件+解析 JSON。
fn read_active_skill_context(repo_root: &Path) -> Option<Value> {
    let path = active_skill_context_path(repo_root);
    let meta = fs::metadata(&path).ok()?;
    let mtime = meta.modified().ok()?;

    // 检查缓存：mtime 未变直接返回
    if let Ok(guard) = SKILL_CONTEXT_CACHE.lock()
        && let Some((cached_mtime, ref cached_val)) = *guard
        && cached_mtime == mtime
    {
        return Some(cached_val.clone());
    }

    // mtime 变了或缓存为空，重新读取
    let data = fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    if let Ok(mut guard) = SKILL_CONTEXT_CACHE.lock() {
        *guard = Some((mtime, value.clone()));
    }
    Some(value)
}

/// 清理 active skill context（Stop 时调用）。
/// 同时失效 mtime 缓存。
fn clear_active_skill_context(repo_root: &Path) {
    let path = active_skill_context_path(repo_root);
    let _ = std::fs::remove_file(&path);
    // 失效 mtime 缓存
    if let Ok(mut guard) = SKILL_CONTEXT_CACHE.lock() {
        *guard = None;
    }
}

/// 从用户 prompt 中提取生命周期 skill slug。
fn detect_lifecycle_skill_slug(prompt: &str) -> Option<&'static str> {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("/implementx") {
        Some("implementx")
    } else if lower.contains("/verifyx") {
        Some("verifyx")
    } else if lower.contains("/planx") {
        Some("planx")
    } else if lower.contains("/discussx") {
        Some("discussx")
    } else {
        None
    }
}

pub(crate) fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();
    let tool_name_raw = payload.get("tool_name").or(payload.get("tool")).and_then(serde_json::Value::as_str).unwrap_or_default();
    let tool_origin = core_policy::hook_common::classify_tool_origin(tool_name_raw);
    // allowedTools 联动：检查当前激活 skill 是否允许此工具
    if let Some(ctx) = read_active_skill_context(repo_root)
        && let (Some(slug), Some(allowed)) = (
            ctx.get("skill_slug").and_then(Value::as_str),
            ctx.get("allowed_tools").and_then(Value::as_array),
        ) && !allowed.is_empty()
    {
            let tool_in_list = allowed.iter().any(|v| {
                v.as_str().is_some_and(|a| {
                    // 支持前缀匹配：mcp__mcp-codegraph__* 匹配所有 codegraph 工具
                    a == tool_name_raw
                        || (a.ends_with('*') && tool_name_raw.starts_with(&a[..a.len() - 1]))
                })
            });
            if !tool_in_list {
                return Some(json!({
                    "hookEventName": "PreToolUse",
                    "additionalContext": format!(
                        "⚠️ Tool '{}' not in active skill '{}' allowedTools. Proceed with caution.",
                        tool_name_raw, slug
                    ),
                }));
            }
    }
    // MCP 工具：安全检查 + 跳过文件路径保护
    if tool_origin.is_mcp() {
        // mcp-tool-safety: 检查高风险工具名和参数模式
        let tool_args_str = payload
            .get("tool_input")
            .or(payload.get("input"))
            .or(payload.get("arguments"))
            .map(|v| v.to_string())
            .unwrap_or_default();
        if let Some(reason) = core_policy::hook_policy::dangerous_mcp_tool_reason(
            tool_name_raw,
            &tool_args_str,
        ) {
            return deny_pre_tool_use(format!(
                "Blocked MCP tool '{tool_name_raw}': {reason}"
            ));
        }
        return None;
    }
    let mut warn_contexts: Vec<String> = Vec::new();
    for path in payload_relative_paths(repo_root, payload) {
        if is_cross_host_or_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of cross-host or retired surface {path}; use the Rust host-entrypoint sync path instead."
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
                "Blocked direct mutation of host-private agent state {path}; project policy must live in repo settings or Rust runtime code."
            ));
        }
        // Warn hints for files that can be edited but need care
        if is_settings_path(&path) {
            warn_contexts.push(format!(
                "Modifying {path} — ensure JSON validity before finishing (jq . or python -m json.tool)."
            ));
        } else if path == "AGENTS.md" {
            warn_contexts.push(format!(
                "Modifying {path} — this is a cross-host strategy document; ensure consistency across all hosts."
            ));
        } else if path == "skills/SKILL_ROUTING_RUNTIME.json"
            || path == "skills/SKILL_MANIFEST.json"
        {
            warn_contexts.push(format!(
                "Modifying {path} — framework routing core data source; run `framework skills refresh --validate` after changes."
            ));
        }
    }

    // §CodeGraph: soft warning when modifying files that contain indexed symbols
    // Only triggers on write operations (Write/Edit/Bash) to avoid noise from reads.
    #[cfg(feature = "codegraph")]
    {
        let tool_name = payload
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or("");
        let is_write_op = matches!(tool_name, "Write" | "Edit" | "Bash");
        if is_write_op {
            let affected_paths: Vec<String> = payload_relative_paths(repo_root, payload);
            for p in &affected_paths {
                if let Some(warning) = codegraph_pre_modify_warning(repo_root, p) {
                    warn_contexts.push(warning);
                }
            }
        }
    }
    if !warn_contexts.is_empty() {
        return Some(json!({
            "suppressOutput": true,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": warn_contexts.join("\n"),
            },
        }));
    }
    None
}

fn run_user_prompt_submit(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();
    let prompt = crate::hosts::hook_dispatch::extract_prompt_text(payload);
    let review_sync = if !core_policy::env_flags::router_rs_review_gate_disabled_for_host("claude")
        && should_sync_review_gate_on_user_prompt(repo_root, &prompt)
    {
        Some(apply_claude_review_gate_user_prompt(
            repo_root, payload, &prompt,
        ))
    } else {
        None
    };
    if let Some(Err(_)) = review_sync {
        let path = review_state_path(repo_root, payload);
        return add_context(
            "UserPromptSubmit",
            &format!(
                "{} (path {}). Repair JSON or permissions before continuing.",
                active_stdio_agent_hook_host().hook_state_unreadable(),
                path.display()
            ),
        );
    }
    if core_policy::hook_common::is_my_pre_execution_entry_prompt(&prompt) {
        return add_context(
            "UserPromptSubmit",
            core_policy::hook_common::MY_PRE_EXECUTION_HOOK_NUDGE,
        );
    }
    if core_policy::hook_common::is_framework_goal_entry_prompt(&prompt) {
        // Write active skill context for allowedTools linkage
        if let Some(slug) = detect_lifecycle_skill_slug(&prompt)
            && let Some(allowed) = skill_allowed_tools_from_runtime(slug) {
                write_active_skill_context(repo_root, slug, &allowed);
            }
        return add_context(
            "UserPromptSubmit",
            core_policy::hook_common::my_goal_drive_hook_nudge_for_prompt(&prompt),
        );
    }
    // Write active skill context for pre-execution lifecycle commands too
    if core_policy::hook_common::is_my_lifecycle_entry_prompt(&prompt)
        && let Some(slug) = detect_lifecycle_skill_slug(&prompt)
        && let Some(allowed) = skill_allowed_tools_from_runtime(slug) {
            write_active_skill_context(repo_root, slug, &allowed);
        }
    let mut contexts: Vec<String> = Vec::new();
    if let Some(Ok(state)) = review_sync {
        // Shared context injection (4-host unified)
        contexts.extend(crate::hosts::hook_dispatch::build_user_prompt_context_injection(
            repo_root,
            &prompt,
            "claude",
            "claude",
            state.review_required,
            state.review_override,
        ));
    }
    if contexts.is_empty() {
        return None;
    }
    add_context("UserPromptSubmit", &contexts.join("\n"))
}

/// Pre-computed context for a single PostToolUse hook invocation.
struct PostToolContext {
    review_path: PathBuf,
    touch_path: PathBuf,
}

impl PostToolContext {
    fn new(repo_root: &Path, payload: &Value) -> Self {
        let key = session_key(repo_root, payload);
        let base = hook_state_base(repo_root);
        let review_path = base.join(core_policy::hook_review_subagent_state_basename(&key));
        let touch_path = base.join(format!("hook_state_{key}.json"));
        Self { review_path, touch_path }
    }
}

/// 合并 PostToolUse 中 reviewer evidence 记录和 touch state 持久化。
/// 共享 `PostToolContext`，避免 `session_key` 重复计算和路径重复构建。
fn record_evidence_and_persist_touch_state(
    repo_root: &Path,
    payload: &Value,
    touched_settings: bool,
    touched_framework: bool,
    settings_validated: bool,
    framework_tested: bool,
) {
    let ctx = PostToolContext::new(repo_root, payload);

    // record_reviewer_evidence (使用 ctx.review_path)
    record_reviewer_evidence_with_ctx(repo_root, payload, &ctx);

    // persist_touch_state (仅在条件满足时执行，使用 ctx.touch_path)
    if touched_settings || touched_framework || settings_validated || framework_tested {
        persist_touch_state_with_ctx(repo_root, &ctx, touched_settings, touched_framework, settings_validated, framework_tested);
    }
}

/// Backwards-compatible wrapper for tests: persists touch state without reviewer evidence.
#[cfg(test)]
pub(crate) fn persist_touch_state(
    repo_root: &Path,
    payload: &Value,
    touched_settings: bool,
    touched_framework: bool,
    settings_validated: bool,
    framework_tested: bool,
) {
    record_evidence_and_persist_touch_state(
        repo_root,
        payload,
        touched_settings,
        touched_framework,
        settings_validated,
        framework_tested,
    );
}

fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();
    let tool_name = payload
        .get("tool_name")
        .or(payload.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tool_origin = core_policy::hook_common::classify_tool_origin(&tool_name);
    let _ = &tool_origin;

    // Shared tool call telemetry (4-host unified)
    crate::hosts::hook_dispatch::record_tool_call_emission(
        repo_root,
        &tool_name,
        crate::hooks::extract_post_tool_duration_ms(payload).unwrap_or(0),
        crate::hooks::post_tool_call_succeeded(payload),
    );
    // §4.4: 自动 evidence 采集 — Bash 验证类命令自动记录到 EVIDENCE_INDEX
    auto_record_verification_evidence(repo_root, payload);
    // §19.5: 科研活动内联日志 — 在科研工作空间中自动记录关键操作
    auto_record_research_activity(repo_root, payload);
    let paths = payload_relative_paths(repo_root, payload);
    let touched_settings = paths.iter().any(|path| is_settings_path(path));
    let touched_framework = paths.iter().any(|path| is_framework_source_path(path));
    let settings_validated =
        payload_is_successful_bash(payload) && payload_runs_settings_validation(payload);
    let framework_tested =
        payload_is_successful_bash(payload) && payload_runs_framework_tests(payload);
    // 合并 reviewer evidence 记录和 touch state 持久化，共享 session_key 计算
    record_evidence_and_persist_touch_state(
        repo_root, payload,
        touched_settings, touched_framework,
        settings_validated, framework_tested,
    );
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


/// Pre-computed context for a single Stop hook invocation.
/// Avoids repeated path building and canonicalize syscalls.
struct StopContext {
    review_path: PathBuf,
    touch_path: PathBuf,
    legacy_review_gate_path: PathBuf,
    legacy_review_flat_path: PathBuf,
    legacy_touch_path: PathBuf,
}

impl StopContext {
    fn new(repo_root: &Path, payload: &Value) -> Self {
        let key = session_key(repo_root, payload);
        let base = hook_state_base(repo_root);
        let review_path = base.join(core_policy::hook_review_subagent_state_basename(&key));
        let legacy_review_gate_path = base.join(core_policy::hook_review_gate_legacy_state_basename(&key));
        let legacy_review_flat_path = repo_root.join(".claude").join(core_policy::hook_review_gate_legacy_state_basename(&key));
        let touch_path = base.join(format!("hook_state_{key}.json"));
        let legacy_touch_path = base.join("hook_state.json");
        Self {
            review_path,
            touch_path,
            legacy_review_gate_path,
            legacy_review_flat_path,
            legacy_touch_path,
        }
    }
}

pub(crate) fn run_stop(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hosts::stop_dispatch::run_unified_stop(repo_root, payload, &ClaudeStopOps)
}

/// Claude-specific Stop operations (implements StopHostOps trait).
struct ClaudeStopOps;

impl crate::hosts::stop_dispatch::StopHostOps for ClaudeStopOps {
    fn host_id(&self) -> &'static str { "claude" }
    fn log_label(&self) -> &'static str { "Claude" }

    fn hook_state_base(&self, repo_root: &Path) -> PathBuf {
        hook_state_base(repo_root)
    }

    fn session_key(&self, repo_root: &Path, payload: &Value) -> String {
        session_key(repo_root, payload)
    }

    fn stop_signal_text(&self, payload: &Value) -> String {
        stop_signal_text(payload)
    }

    fn pre_stop_cleanup(&self, repo_root: &Path) -> Option<()> {
        clear_active_skill_context(repo_root);
        Some(())
    }

    fn hydrate_goal_gate_from_disk(
        &self,
        repo_root: &Path,
        state: &mut HookReviewDiskCore,
        goal_drive_entrypoint: bool,
    ) {
        hydrate_goal_gate_from_disk_for_claude(repo_root, state, goal_drive_entrypoint);
    }
}

/// Original Claude Stop implementation — now delegates to unified pipeline.

/// Hydrate goal gate from disk (GOAL_STATE.json + EVIDENCE_INDEX.json).
/// Uses the same function-pointer proxy as Cursor; active when runtime-core registers
/// `evaluate_goal_readiness_from_disk` (no-op in standalone mode).
/// Falls back to direct GOAL_STATE.json read when runtime-core is not registered.
fn hydrate_goal_gate_from_disk_for_claude(
    repo_root: &Path,
    state: &mut HookReviewDiskCore,
    goal_drive_entrypoint: bool,
) {
    if !state.goal_drive_entry_active && !goal_drive_entrypoint {
        return;
    }
    let frame = core_state::task_state::resolve_continuity_frame(repo_root);
    let Some((goal, task_id)) = frame.hydration_goal.as_ref() else {
        if goal_drive_entrypoint {
            state.goal_drive_entry_active = false;
        }
        return;
    };
    let readiness = crate::hooks::evaluate_goal_readiness_from_disk(
        repo_root,
        goal,
        task_id.as_str(),
    );
    // If runtime-core registered a real evaluator, use its result.
    if readiness.contract || readiness.progress || readiness.verification {
        if readiness.contract {
            state.goal_contract_seen = true;
        }
        if readiness.progress {
            state.goal_progress_seen = true;
        }
        if readiness.verification {
            state.goal_verify_or_block_seen = true;
        }
        return;
    }
    // Standalone fallback: read GOAL_STATE.json directly.
    let goal_path = repo_root
        .join("artifacts/current")
        .join(task_id.as_str())
        .join("GOAL_STATE.json");
    let Ok(raw) = fs::read_to_string(&goal_path) else {
        return;
    };
    let Ok(goal_json) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    // Check for goal contract fields (done_when, validation_commands, non_goals)
    if goal_json.get("done_when").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty())
        || goal_json.get("validation_commands").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty())
    {
        state.goal_contract_seen = true;
    }
    // Check for progress/milestone markers
    if goal_json.get("progress").is_some()
        || goal_json.get("checkpoints").is_some()
        || goal_json.get("milestone").is_some()
    {
        state.goal_progress_seen = true;
    }
    // Check for verification/blocker markers
    if goal_json.get("verification").is_some()
        || goal_json.get("verified").and_then(Value::as_bool).is_some_and(|b| b)
        || goal_json.get("blockers").is_some()
    {
        state.goal_verify_or_block_seen = true;
    }
}


fn try_extract_session_string(payload: &Value) -> Option<String> {
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


fn repo_fallback_token(repo_root: &Path) -> String {
    let resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let label = active_stdio_agent_hook_host().log_label();
    format!(
        "{label}-repo::{}",
        resolved.to_string_lossy().replace('\\', "/")
    )
}

/// 与 Cursor `session_key` 同类：**显式会话串** → **宿主 `ROUTER_RS_*_SESSION_NAMESPACE`** → **`cwd` 类字段** → **repo 稳定 token**。
/// 同仓多会话在无 id 时仍可能共用状态文件；需并行分流时与 Cursor 一样设 namespace。
fn session_key(repo_root: &Path, payload: &Value) -> String {
    core_policy::session_key::session_key_core(
        &core_policy::session_key::SessionKeyConfig {
            env_var: active_stdio_agent_hook_host().session_namespace_env(),
            scan_tool_input: false,
        },
        || try_extract_session_string(payload),
        || {
            let cwd = first_nonempty_payload_str(payload, core_policy::session_key::SESSION_KEY_CWD_FIELDS);
            if cwd.is_empty() { None } else { Some(cwd) }
        },
        &repo_fallback_token(repo_root),
    )
}

fn review_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(core_policy::hook_review_subagent_state_basename(
        &session_key(repo_root, payload),
    ))
}

/// `.claude/hook-state/review_gate_<key>.json` (pre–phase-3 canonical).
fn legacy_review_gate_hook_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(core_policy::hook_review_gate_legacy_state_basename(
        &session_key(repo_root, payload),
    ))
}

/// `.claude/review_gate_<key>.json` (flat legacy).
fn legacy_review_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    repo_root
        .join(".claude")
        .join(core_policy::hook_review_gate_legacy_state_basename(
            &session_key(repo_root, payload),
        ))
}

/// Acquire a file lock and execute a closure (Claude-specific config).
///
/// Uses the shared `FileStateLockGuard` with Claude's timeout/stale-lock config.
/// Replaces the former `ClaudeReviewStateLock` + `acquire_claude_review_state_lock`.
fn with_claude_review_state_lock<T, F>(state_path: &Path, f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    let _guard = crate::hosts::file_state_lock::acquire_file_lock_with_config(
        &lock_path,
        &crate::hosts::file_state_lock::LockConfig::short_timeout(),
    )?;
    f()
}

fn migrate_claude_review_gate_state_to_canonical(
    canonical_path: &Path,
    state: &HookReviewDiskCore,
) -> AgentDiskState<HookReviewDiskCore> {
    if let Err(err) = with_claude_review_state_lock(canonical_path, || {
        write_review_state_unlocked(canonical_path, state)
    }) {
        eprintln!(
            "[router-rs] claude review_gate legacy migrate failed (using in-memory state): {err}"
        );
    }
    AgentDiskState::Ok(state.clone())
}

fn load_review_gate_disk(repo_root: &Path, payload: &Value) -> AgentDiskState<HookReviewDiskCore> {
    let path = review_state_path(repo_root, payload);
    match read_review_gate_file(&path) {
        AgentDiskState::Ok(state) => return AgentDiskState::Ok(state),
        AgentDiskState::Unreadable => return AgentDiskState::Unreadable,
        AgentDiskState::Absent => {}
    }
    for legacy_path in [
        legacy_review_gate_hook_state_path(repo_root, payload),
        legacy_review_state_path(repo_root, payload),
    ] {
        match read_review_gate_file(&legacy_path) {
            AgentDiskState::Ok(state) => {
                return migrate_claude_review_gate_state_to_canonical(&path, &state);
            }
            AgentDiskState::Unreadable => return AgentDiskState::Unreadable,
            AgentDiskState::Absent => {}
        }
    }
    AgentDiskState::Absent
}


fn reviewer_lane(tool_input: &Value, payload: &Value) -> bool {
    let subagent_type = normalize_subagent_type(
        tool_input
            .get("subagent_type")
            .or_else(|| tool_input.get("agent_type"))
            .or_else(|| tool_input.get("type"))
            .or_else(|| payload.get("subagent_type"))
            .or_else(|| payload.get("agent_type"))
            .and_then(Value::as_str),
    );
    !subagent_type.is_empty()
        && core_policy::registry_review_gate::is_reviewer_lane_from_registry(&subagent_type, None)
}



fn record_reviewer_evidence_with_ctx(repo_root: &Path, payload: &Value, ctx: &PostToolContext) {
    let path = &ctx.review_path;
    let tool_input = crate::hosts::hook_dispatch::extract_tool_input(payload);
    let fork = fork_context_from_values(&tool_input, Some(payload));
    if let Err(err) = with_claude_review_state_lock(path, || {
        let mut state = match load_review_gate_disk(repo_root, payload) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} review_gate state unreadable on PostToolUse: {}",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err("review_gate_unreadable".to_string());
            }
            AgentDiskState::Absent => HookReviewDiskCore::default(),
            AgentDiskState::Ok(s) => s,
        };
        if !state.review_required || state.review_override {
            return Ok(());
        }
        if !payload_is_successful_tool(payload) {
            return Ok(());
        }
        if subagent_tool(payload)
            && review_independent_reviewer_evidence(fork, reviewer_lane(&tool_input, payload))
        {
            state.independent_reviewer_seen = true;
            write_review_state_unlocked(path, &state)?;
        }
        Ok(())
    })
        && err != "review_gate_unreadable" {
            eprintln!(
                "[router-rs] {} review_gate evidence record failed: {err}",
                active_stdio_agent_hook_host().log_label()
            );
        }
}


fn legacy_touch_state_path(repo_root: &Path) -> PathBuf {
    hook_state_base(repo_root).join("hook_state.json")
}

#[cfg(test)]
pub(crate) fn touch_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(format!(
        "hook_state_{}.json",
        session_key(repo_root, payload)
    ))
}

fn persist_touch_state_with_ctx(
    repo_root: &Path,
    ctx: &PostToolContext,
    settings: bool,
    framework: bool,
    settings_validated: bool,
    framework_tested: bool,
) {
    let path = &ctx.touch_path;
    if let Err(err) = with_claude_review_state_lock(path, || {
        let current = match load_touch_state_from_path(path) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} hook_state unreadable; skip merge (path {}): repair JSON or remove file",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err("hook_state_unreadable".to_string());
            }
            AgentDiskState::Absent => TouchState::default(),
            AgentDiskState::Ok(s) => s,
        };
        let state_payload = json!({
            "settings": current.settings || settings,
            "framework": current.framework || framework,
            "settings_validated": current.settings_validated || settings_validated,
            "framework_tested": current.framework_tested || framework_tested,
        });
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let _ = fs::remove_file(legacy_touch_state_path(repo_root));
        fs::write(path, format!("{state_payload}\n")).map_err(|e| e.to_string())?;
        // §1.3: hook-state 写入后概率性清理过期文件
        if let Some(hook_state_dir) = path.parent() {
            crate::hooks::sweep_stale_hook_state_files(hook_state_dir);
        }
        Ok(())
    })
        && err != "hook_state_unreadable" {
            eprintln!(
                "[router-rs] {} hook state write failed (hook_state): {err}",
                active_stdio_agent_hook_host().log_label()
            );
        }
}







fn payload_relative_paths(repo_root: &Path, payload: &Value) -> Vec<String> {
    let mut paths = HashSet::new();
    collect_payload_paths(payload, &mut paths);
    paths
        .into_iter()
        .filter_map(|path| repo_relative_slash_path(repo_root, &path))
        .collect()
}





fn is_cross_host_or_retired_surface(path: &str) -> bool {
    CROSS_HOST_SURFACES
        .iter()
        .any(|surface| path == *surface || path.starts_with(&format!("{surface}/")))
}



fn is_repo_claude_hook_state_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with(".claude/hook-state/") {
        return true;
    }
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    if !file_name.ends_with(".json") {
        return false;
    }
    file_name.starts_with("review-subagent-")
        || file_name.starts_with("review_gate_")
        || file_name.starts_with("hook_state_")
}



/// Cached codegraph index handle to avoid re-opening SQLite on every PreToolUse.
#[cfg(feature = "codegraph")]
static CODEGRAPH_INDEX: std::sync::OnceLock<std::sync::Mutex<Option<codegraph_rs::CodeGraphIndex>>> =
    std::sync::OnceLock::new();

/// Check codegraph index for a file being modified: if the file contains
/// indexed symbols that have upstream callers, return a soft warning.
/// Only compiled when `feature = "codegraph"` is enabled (default).
///
/// Uses a cached index handle (OnceLock) — first call opens the DB, subsequent
/// calls reuse the same connection to avoid repeated ~30ms SQLite open+check.
#[cfg(feature = "codegraph")]
fn codegraph_pre_modify_warning(repo_root: &Path, file_rel_path: &str) -> Option<String> {
    use std::time::Instant;
    let start = Instant::now();

    // Get or initialize the cached index handle (session-scoped, repo_root stable)
    let mut guard = CODEGRAPH_INDEX
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| {
            eprintln!("[CodeGraph] index mutex poisoned, recovering");
            e.into_inner()
        });
    if guard.is_none() {
        *guard = codegraph_rs::CodeGraphIndex::open(repo_root).ok();
    }
    let index = guard.as_ref()?;

    // Query symbols defined in the modified file
    let symbols = index.find_symbols_by_file(file_rel_path).ok()?;
    if symbols.is_empty() {
        return None;
    }
    // Check if any of these symbols have callers
    // Use both file_path and node_id for precise matching,
    // avoiding inflated counts from cross-file name collisions.
    let mut caller_count = 0usize;
    for sym in &symbols {
        let filter = codegraph_rs::db::node_ops::SymbolFilter::from_options(
            Some(&sym.file_path),
            Some(&sym.id),
        );
        if let Ok(callers) = index.find_callers(&sym.symbol, 1, &filter) {
            caller_count += callers.len();
        }
    }
    if caller_count == 0 {
        return None;
    }
    let elapsed = start.elapsed().as_millis();
    let symbols_str: Vec<&str> = symbols.iter().map(|s| s.symbol.as_str()).collect();
    Some(format!(
        "[CodeGraph] {file_rel_path} symbols ({count}: {names}) have {caller_count} upstream caller(s) — verify all references before deleting/renaming. (analysis took {elapsed}ms)",
        count = symbols.len(),
        names = symbols_str.join(", "),
    ))
}






fn payload_runs_settings_validation(payload: &Value) -> bool {
    let Some(command) = bash_command(payload) else {
        return false;
    };
    let lowered = command.to_ascii_lowercase();
    (lowered.contains("jq") || lowered.contains("python") || lowered.contains("node"))
        && active_stdio_agent_hook_host()
            .settings_guarded_paths()
            .iter()
            .any(|p| lowered.contains(&p.to_ascii_lowercase()))
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
        "--manifest-path core/router-rs/cargo.toml",
        "core/router-rs/cargo.toml",
        "router-rs",
        "--test policy_contracts",
        "--test documentation_contracts",
        "--test host_integration",
    ]
    .iter()
    .any(|hint| lowered.contains(hint))
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
