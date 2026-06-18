//! L4 transport for `host_id=claude`; review/closeout policy in `core-policy`.
//! Claude（Anthropic）hooks：`router-rs claude hook --event=… --repo-root …`。
//! 历史版本接口快照：`git show 89ece4c^:core/router-rs/src/claude_hooks.rs`（事件：`pre-tool-use`、`user-prompt-submit`、`post-tool-use`、`stop`；CLI 亦接受 `PreToolUse` 等 PascalCase 别名，与 Codex hook 拼写对齐）。
//!
//! **误接 Cursor hook stdin**：仅在 stdin JSON 呈现结构化 Cursor envelope（顶层非空 `cursor_version` 字符串 + `workspace_roots` 数组 + 非空 `hook_event_name` 或 `hookEventName`）时整条静默；
//! 不用路径子串扫描，以免合法 Claude 载荷（例如编辑 `.cursor/` 下文件）被误判为 Cursor 而旁路门禁。
//! stdin 体量上限 4 MiB，与 Codex hook 读取路径对齐，防失控输入撑爆 hook 进程内存。
use core_policy::hook_common::{
    has_override, is_narrow_review_prompt, is_review_prompt, normalize_subagent_type,
    normalize_tool_name, saw_reject_reason,
};
use core_policy::review_gate_engine::{
    fork_context_from_values, review_independent_reviewer_evidence,
};
use core_policy::{HookReviewDiskCore, HookReviewGateFields};
use serde_json::{Map, Value, json};
use super::hook_dispatch::{HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};
use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::path::{Component, Path, PathBuf};
#[cfg(not(unix))]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLAUDE_HOOK_STATE_UNREADABLE: &str =
    "router-rs CLAUDE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

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

    fn validate_settings_stop_reason(self) -> &'static str {
        match self {
            Self::Claude => "Validate Claude hook/settings JSON before ending this turn.",
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

fn hook_state_base(repo_root: &Path) -> PathBuf {
    repo_root
        .join(active_stdio_agent_hook_host().state_dir())
        .join("hook-state")
}

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
/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE` 或 legacy `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`。
fn agent_review_gate_disabled() -> bool {
    core_policy::env_flags::router_rs_review_gate_disabled_for_host("claude")
}

/// Env disable **or** `my-light` profile (advisory-only mode).
fn claude_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if agent_review_gate_disabled() {
        return true;
    }
    core_policy::hook_common::review_gate_hard_block_disabled(Some(repo_root), text)
}

fn claude_user_prompt_text(payload: &Value) -> String {
    payload
        .get("prompt")
        .or_else(|| payload.get("user_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn claude_stop_signal_text(payload: &Value) -> String {
    claude_closeout_completion_text(payload)
}

fn claude_closeout_completion_text(payload: &Value) -> String {
    let prompt = claude_user_prompt_text(payload);
    let response = payload
        .get("response")
        .or_else(|| payload.get("assistant_response"))
        .or_else(|| payload.get("last_assistant_message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if response.trim().is_empty() {
        prompt
    } else {
        format!("{prompt}\n{response}")
    }
}

fn claude_review_gate_incomplete_stop_reason(fields: &HookReviewGateFields) -> Option<String> {
    core_policy::hook_review_stop_advisory_needed(fields, "CLAUDE_REVIEW_GATE")
}

fn should_sync_review_gate_on_user_prompt(repo_root: &Path, prompt: &str) -> bool {
    core_policy::hook_common::my_light_profile_active(Some(repo_root), prompt)
        || core_policy::hook_common::is_framework_goal_entry_prompt(prompt)
        || core_policy::hook_common::is_my_pre_execution_entry_prompt(prompt)
        || is_narrow_review_prompt(prompt)
        || is_review_prompt(prompt)
        || has_override(prompt)
}

fn apply_claude_review_gate_user_prompt(
    repo_root: &Path,
    payload: &Value,
    prompt: &str,
) -> Result<HookReviewDiskCore, String> {
    let path = review_state_path(repo_root, payload);
    let my_light = core_policy::hook_common::my_light_profile_active(Some(repo_root), prompt);
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
        if my_light || goal_drive || narrow {
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
    let _registry_guard = core_policy::registry_review_gate::HookRegistryRepoGuard::new(repo_root);
    let result = with_stdio_agent_hook_host(StdioAgentHookHost::Claude, || {
        let payload = read_stdin_payload()?;
        let event = HookEvent { repo_root, event_name: canonical, payload: &payload };
        let output = ClaudeHookDispatcher.dispatch(&event);
        Ok(hook_output_to_claude_value(canonical, output))
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

#[allow(dead_code)]
fn dispatch_stdio_agent_hook_payload(canonical: &str, repo_root: &Path, payload: &Value) -> Value {
    crate::hooks::ensure_kernel_bootstrap();
    if payload_looks_like_cursor_hook_stdin(payload) {
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
    fn host_id(&self) -> &'static str { "claude" }
    fn state_dir_leaf(&self) -> &'static str { ".claude" }
    fn hook_state_unreadable_tag(&self) -> &'static str { CLAUDE_HOOK_STATE_UNREADABLE }
    fn session_namespace_env(&self) -> &'static str { "ROUTER_RS_CLAUDE_SESSION_NAMESPACE" }
    fn log_label(&self) -> &'static str { "claude" }
}

/// Convert Claude-specific hook JSON to unified HookOutput.
fn claude_value_to_hook_output(val: &Value) -> Option<HookOutput> {
    if val.get("suppressOutput") != Some(&json!(true)) {
        return Some(HookOutput::Raw(val.clone()));
    }
    let Some(hso) = val.get("hookSpecificOutput") else {
        return Some(HookOutput::Raw(val.clone()));
    };
    if hso.get("permissionDecision").and_then(Value::as_str) == Some("deny") {
        let reason = hso
            .get("permissionDecisionReason")
            .and_then(Value::as_str)
            .unwrap_or("denied");
        return Some(HookOutput::Deny { reason: reason.to_string() });
    }
    if let Some(ctx) = hso.get("additionalContext").and_then(Value::as_str)
        && !ctx.is_empty() {
            return Some(HookOutput::AdditionalContext(ctx.to_string()));
        }
    None
}

/// Convert unified HookOutput to Claude-specific hook JSON.
fn hook_output_to_claude_value(event_name: &str, output: Option<HookOutput>) -> Value {
    match output {
        None | Some(HookOutput::None) => silent_success(),
        Some(HookOutput::Deny { reason }) => {
            deny_pre_tool_use(reason).unwrap_or_else(silent_success)
        }
        Some(HookOutput::AdditionalContext(ctx)) => {
            add_context(event_name, &ctx).unwrap_or_else(silent_success)
        }
        Some(HookOutput::Block { reason }) => {
            add_context(event_name, &format!("[advisory] {reason}")).unwrap_or_else(silent_success)
        }
        Some(HookOutput::Advisory { message }) => {
            add_context("Stop", &message).unwrap_or_else(silent_success)
        }
        Some(HookOutput::Warn { message }) => {
            add_context("PreToolUse", &message).unwrap_or_else(silent_success)
        }
        Some(HookOutput::Raw(val)) => val,
    }
}

impl HostHookDispatcher for ClaudeHookDispatcher {
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_pre_tool_use(event.repo_root, event.payload);
        val.and_then(|v| claude_value_to_hook_output(&v))
    }

    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_user_prompt_submit(event.repo_root, event.payload);
        val.and_then(|v| claude_value_to_hook_output(&v))
    }

    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_post_tool_use(event.repo_root, event.payload);
        val.and_then(|v| claude_value_to_hook_output(&v))
    }

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let val = run_stop(event.repo_root, event.payload);
        val.and_then(|v| claude_value_to_hook_output(&v))
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
        hook_output_to_claude_value(canonical_event, output)
    })
}

/// `router-rs claude hook --event=… --repo-root …` — stdin JSON → Claude Code hook response JSON (line-delimited).
pub fn run_claude_hook_cli(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    let repo_root = crate::hooks::resolve_repo_root_arg(cli_repo_root)?;
    let mut output = run_claude_hook(event, &repo_root)?;
    crate::hooks::attach_router_rs_observation(
        &mut output,
        crate::hooks::HookObservationHostType::Claude,
    );
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    let mut stdout = std::io::stdout();
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    // Force immediate exit — skip background thread cleanup (file watcher, telemetry).
    // Hook processes are short-lived fire-and-forget; background threads are not needed.
    std::process::exit(0);
}

fn parse_stdio_agent_hook_stdin_trimmed(trimmed: &str) -> Result<Value, String> {
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).map_err(|err| format!("stdin_json_invalid: {err}"))
}

/// 4 MiB stdin reader — delegates to shared `hooks::read_stdin_limited`.
fn read_stdio_agent_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    crate::hooks::read_stdin_limited(reader)
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = io::stdin();
    let input = read_stdio_agent_stdin_limited(&mut stdin)?;
    parse_stdio_agent_hook_stdin_trimmed(input.trim())
}

fn silent_success() -> Value {
    json!({ "suppressOutput": true })
}

/// Cursor hook stdin 误接到 stdio agent hook 时的结构化识别（顶层字段）。
///
/// 刻意不使用嵌套字符串中的 `/.cursor/` 匹配，否则合法 stdio agent 工具载荷可能被整条静默。
/// 另要求 `hook_event_name` / `hookEventName`，降低仅凭顶造 `cursor_version`+`workspace_roots` 整条静默的面。
#[allow(dead_code)]
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
    
    [map.get("hook_event_name"), map.get("hookEventName")]
        .into_iter()
        .flatten()
        .any(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
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

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();
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
        } else if path == "AGENTS_CLAUDE.md" {
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
    let prompt = claude_user_prompt_text(payload);
    let review_sync = if !agent_review_gate_disabled()
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
        return add_context(
            "UserPromptSubmit",
            core_policy::hook_common::my_goal_drive_hook_nudge_for_prompt(&prompt),
        );
    }
    let mut contexts: Vec<String> = Vec::new();
    if let Some(Ok(state)) = review_sync
        && state.review_required
            && !state.review_override
            && core_policy::hook_common::should_inject_spawn_first_review_nudge(
                Some(repo_root),
                &prompt,
            )
        {
            contexts.push(
                core_policy::registry_review_gate::review_spawn_first_nudge_line(
                    Some(repo_root),
                    "claude",
                ),
            );
        }
    crate::hooks::maybe_append_paper_adversarial_context(
        repo_root,
        &prompt,
        &mut contexts,
        crate::hooks::PaperProseHookHostType::Claude,
    );
    crate::hooks::maybe_append_paper_prose_context(
        repo_root,
        &prompt,
        &mut contexts,
        crate::hooks::PaperProseHookHostType::Claude,
    );
    if contexts.is_empty() {
        return None;
    }
    add_context("UserPromptSubmit", &contexts.join("\n"))
}

fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();
    let tool_name = payload
        .get("tool_name")
        .or(payload.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    crate::hooks::emit_tool_call(
        &tool_name,
        crate::hooks::extract_post_tool_duration_ms(payload).unwrap_or(0),
        crate::hooks::post_tool_call_succeeded(payload),
    );
    if let Err(e) = crate::hooks::record_tool_call(repo_root, &tool_name, None) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
    record_reviewer_evidence(repo_root, payload);
    // §4.4: 自动 evidence 采集 — Bash 验证类命令自动记录到 EVIDENCE_INDEX
    auto_record_verification_evidence(repo_root, payload);
    let paths = payload_relative_paths(repo_root, payload);
    let touched_settings = paths.iter().any(|path| is_settings_path(path));
    let touched_framework = paths.iter().any(|path| is_framework_source_path(path));
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
    crate::hooks::ensure_kernel_bootstrap();
    if let Some(msg) = crate::hooks::closeout_stop_followup_for_completion_text(
        repo_root,
        &claude_closeout_completion_text(payload),
    ) {
        // Advisory only — do not block_stop (causes infinite retry loop)
        return add_context("Stop", &format!("[advisory] {msg}"));
    }

    let review_load = load_review_gate_disk(repo_root, payload);
    let touch_load = load_touch_state_disk(repo_root, payload);
    if matches!(review_load, AgentDiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} review_gate state unreadable on Stop: {}",
            active_stdio_agent_hook_host().log_label(),
            review_state_path(repo_root, payload).display()
        );
        // Advisory — corrupted state is not a reason to block indefinitely
        return add_context(
            "Stop",
            "[advisory] hook-state unreadable; clearing stale files.",
        );
    }
    if matches!(touch_load, AgentDiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} hook_state unreadable on Stop: {}",
            active_stdio_agent_hook_host().log_label(),
            touch_state_path(repo_root, payload).display()
        );
        // Advisory — corrupted state is not a reason to block indefinitely
        clear_touch_state(repo_root, payload);
        return add_context(
            "Stop",
            "[advisory] hook-state unreadable; cleared stale files.",
        );
    }

    let stop_signal = claude_stop_signal_text(payload);
    let prompt = claude_user_prompt_text(payload);
    let reject_now = saw_reject_reason(&stop_signal, &prompt);

    let mut review_state = match review_load {
        AgentDiskState::Absent => HookReviewDiskCore::default(),
        AgentDiskState::Ok(s) => s,
        AgentDiskState::Unreadable => unreachable!("Unreadable already returned above"),
    };
    if reject_now {
        review_state.reject_reason_seen = true;
        let path = review_state_path(repo_root, payload);
        let _ = write_review_state_unlocked(&path, &review_state);
    }
    if !claude_review_gate_suppressed(repo_root, &prompt)
        && let Some(reason) = claude_review_gate_incomplete_stop_reason(&review_state.gate_fields())
        {
            return add_context("Stop", &reason);
        }
    let state = match touch_load {
        AgentDiskState::Absent => TouchState::default(),
        AgentDiskState::Ok(s) => s,
        AgentDiskState::Unreadable => unreachable!("Unreadable already returned above"),
    };
    if state.settings && !state.settings_validated {
        // Advisory — warn but do not block
        clear_touch_state(repo_root, payload);
        return add_context(
            "Stop",
            &format!(
                "[advisory] {}",
                active_stdio_agent_hook_host().validate_settings_stop_reason()
            ),
        );
    }
    if state.framework && !state.framework_tested {
        // Advisory — warn but do not block
        clear_touch_state(repo_root, payload);
        return add_context(
            "Stop",
            "[advisory] Framework source files were modified. Consider running tests.",
        );
    }
    clear_review_state(repo_root, payload);
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
            env_var: active_stdio_agent_hook_host().session_namespace_env(),
        },
        || try_extract_session_string(payload),
        || {
            let cwd = first_nonempty_payload_str(payload, CWD_KEYS);
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

fn read_review_gate_file(path: &Path) -> AgentDiskState<HookReviewDiskCore> {
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(core_policy::migrate_hook_review_disk_core(&value))
}

/// Serialize Claude review_gate JSON under `<state>.lock` (ADR P1-5; mirrors Codex hook-state flock).
#[cfg(unix)]
struct ClaudeReviewStateLock {
    file: std::fs::File,
}

#[cfg(unix)]
impl Drop for ClaudeReviewStateLock {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        // SAFETY: flock(LOCK_UN) releases an advisory lock previously acquired via
        // LOCK_EX in acquire_claude_review_state_lock. The fd comes from a valid
        // File handle that is still open (we are inside Drop for that handle's owner).
        // flock is a simple syscall with no memory safety concerns; its worst case is
        // returning -1 on an already-unlocked fd, which is harmless.
        unsafe {
            let _ = libc::flock(fd, libc::LOCK_UN);
        }
    }
}

#[cfg(not(unix))]
struct ClaudeReviewStateLock {
    path: PathBuf,
}

#[cfg(not(unix))]
impl Drop for ClaudeReviewStateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn acquire_claude_review_state_lock(state_path: &Path) -> Result<ClaudeReviewStateLock, String> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("claude_state_dir_create_failed: {e}"))?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("claude_state_lock_open_failed: {e}"))?;
    let fd = file.as_raw_fd();
    // SAFETY: flock(LOCK_EX) acquires an exclusive advisory lock on the given fd.
    // The fd comes from a freshly opened file (OpenOptions above) and is guaranteed
    // valid. flock is a simple syscall that operates on kernel file table entries;
    // it cannot cause memory unsafety. If the lock is unavailable, it blocks until
    // acquired (LOCK_EX semantics). The return value is checked for errors below.
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if rc != 0 {
        return Err(format!(
            "claude_state_lock_flock_failed: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(ClaudeReviewStateLock { file })
}

#[cfg(not(unix))]
fn acquire_claude_review_state_lock(state_path: &Path) -> Result<ClaudeReviewStateLock, String> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("claude_state_dir_create_failed: {e}"))?;
    }
    let started = SystemTime::now();
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let stamp = format!("pid={} ts={now_ms}\n", std::process::id());
                file.write_all(stamp.as_bytes())
                    .map_err(|e| format!("claude_state_lock_write_failed: {e}"))?;
                return Ok(ClaudeReviewStateLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if started.elapsed().unwrap_or(Duration::ZERO) > Duration::from_secs(20) {
                    // Attempt stale lock cleanup before giving up
                    if let Ok(meta) = fs::metadata(&lock_path) {
                        if let Ok(modified) = meta.modified() {
                            if modified.elapsed().unwrap_or(Duration::ZERO)
                                > Duration::from_secs(60)
                            {
                                let _ = fs::remove_file(&lock_path);
                                continue;
                            }
                        }
                    }
                    return Err("claude_state_lock_timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(format!("claude_state_lock_open_failed: {err}")),
        }
    }
}

fn with_claude_review_state_lock<T, F>(state_path: &Path, f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let _guard = acquire_claude_review_state_lock(state_path)?;
    f()
}

#[derive(Debug, Clone)]
enum AgentDiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
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

fn load_touch_state_disk(repo_root: &Path, payload: &Value) -> AgentDiskState<TouchState> {
    let path = touch_state_path(repo_root, payload);
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let payload_val: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(TouchState {
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

fn write_review_state_unlocked(path: &Path, state: &HookReviewDiskCore) -> Result<(), String> {
    let mut to_write = state.clone();
    to_write.bump_version_for_save();
    let value = serde_json::to_value(&to_write).map_err(|e| e.to_string())?;
    let body = format!("{value}\n");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, &body).map_err(|e| e.to_string())
}

fn clear_review_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(review_state_path(repo_root, payload));
    let _ = fs::remove_file(legacy_review_gate_hook_state_path(repo_root, payload));
    let _ = fs::remove_file(legacy_review_state_path(repo_root, payload));
}

fn agent_tool_input(payload: &Value) -> Value {
    payload
        .as_object()
        .and_then(core_policy::hook_common::tool_input_value_from_map)
        .unwrap_or_else(|| json!({}))
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

fn subagent_tool(payload: &Value) -> bool {
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
    crate::hosts::hook_dispatch::is_subagent_tool(normalized)
}

fn record_reviewer_evidence(repo_root: &Path, payload: &Value) {
    let path = review_state_path(repo_root, payload);
    let tool_input = agent_tool_input(payload);
    let fork = fork_context_from_values(&tool_input, Some(payload));
    if let Err(err) = with_claude_review_state_lock(&path, || {
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
            && review_independent_reviewer_evidence(reviewer_lane(&tool_input, payload), fork)
        {
            state.independent_reviewer_seen = true;
            write_review_state_unlocked(&path, &state)?;
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

/// §4.4: 自动 evidence 采集。当 Bash 运行验证类命令时，自动记录到 EVIDENCE_INDEX。
/// 命令分类规则：
/// - `cargo test*`, `cargo check*`, `cargo build*` → exit_code + 输出摘要
/// - `git diff*`, `git log*` → 变更统计
/// - `npm test*`, `pytest*`, `make test*` → exit_code + 输出摘要
// 同一命令在同一 task 目录下不重复记录（检查最近 5 条）。
fn auto_record_verification_evidence(repo_root: &Path, payload: &Value) {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return;
    }
    let Some(command) = bash_command(payload) else {
        return;
    };
    let cmd_trimmed = command.trim();
    if !is_verification_command(cmd_trimmed) {
        return;
    }
    let exit_code = payload_exit_code(payload);
    let output_summary = extract_output_summary(payload, 500);

    let mut entry = serde_json::Map::new();
    entry.insert("kind".to_string(), json!("auto_evidence"));
    entry.insert("source".to_string(), json!("post_tool_use_auto"));
    entry.insert("tool_name".to_string(), json!("Bash"));
    entry.insert("command_preview".to_string(), json!(cmd_trimmed));
    entry.insert(
        "recorded_at".to_string(),
        json!(crate::hooks::current_local_timestamp()),
    );
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    if let Some(ref text) = output_summary {
        entry.insert("output".to_string(), json!(text));
    }

    if let Err(err) = crate::hooks::append_evidence_index(repo_root, None, entry) {
        eprintln!(
            "[router-rs] {} auto-evidence record failed: {err}",
            active_stdio_agent_hook_host().log_label()
        );
    }
}

/// 判断命令是否为验证类命令。
fn is_verification_command(cmd: &str) -> bool {
    let cmd_lower = cmd.to_ascii_lowercase();
    let patterns = [
        "cargo test",
        "cargo check",
        "cargo build",
        "cargo clippy",
        "cargo fmt",
        "npm test",
        "npm run test",
        "pytest",
        "make test",
        "make check",
        "go test",
        "git diff",
        "git log",
    ];
    patterns.iter().any(|p| cmd_lower.starts_with(p))
}

/// 从 payload 中提取输出摘要，截断到 max_chars。
fn extract_output_summary(payload: &Value, max_chars: usize) -> Option<String> {
    let output = payload
        .get("output")
        .or(payload.get("tool_output"))
        .or(payload.get("result"))
        .and_then(Value::as_str)?;
    let trimmed: String = output.chars().take(max_chars).collect();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn legacy_touch_state_path(repo_root: &Path) -> PathBuf {
    hook_state_base(repo_root).join("hook_state.json")
}

fn touch_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(format!(
        "hook_state_{}.json",
        session_key(repo_root, payload)
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
    let path = touch_state_path(repo_root, session_payload);
    let lock_path = path.clone();
    if let Err(err) = with_claude_review_state_lock(&lock_path, || {
        let current = match load_touch_state_disk(repo_root, session_payload) {
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
        fs::write(&path, format!("{state_payload}\n")).map_err(|e| e.to_string())?;
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

fn clear_touch_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(touch_state_path(repo_root, payload));
    let _ = fs::remove_file(legacy_touch_state_path(repo_root));
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

fn bash_command(payload: &Value) -> Option<&str> {
    payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)
}

fn is_cross_host_or_retired_surface(path: &str) -> bool {
    CROSS_HOST_SURFACES
        .iter()
        .any(|surface| path == *surface || path.starts_with(&format!("{surface}/")))
}

fn is_framework_source_path(path: &str) -> bool {
    FRAMEWORK_SOURCE_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

fn is_generated_entrypoint(path: &str) -> bool {
    active_stdio_agent_hook_host()
        .generated_entrypoint_paths()
        .contains(&path)
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

fn is_host_private_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with(".claude/") && is_repo_claude_hook_state_file(&normalized) {
        return true;
    }
    // `.claude/plans/` is a session scratch area (plan mode), not host-private state.
    if normalized.contains("/.claude/plans/") || normalized.starts_with(".claude/plans/") {
        return false;
    }
    let leaf = active_stdio_agent_hook_host().user_config_dir_leaf();
    let tilde_prefix = format!("~/{}/", leaf.trim_start_matches('/'));
    if normalized.starts_with(&tilde_prefix) {
        return true;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let prefix = PathBuf::from(home)
            .join(leaf)
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
    active_stdio_agent_hook_host()
        .settings_guarded_paths()
        .contains(&path)
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
    payload_is_successful_tool(payload)
}

fn payload_is_successful_tool(payload: &Value) -> bool {
    if payload
        .get("is_error")
        .and_then(Value::as_bool)
        .is_some_and(|v| v)
    {
        return false;
    }
    if payload.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    match payload_exit_code(payload) {
        Some(0) => true,
        Some(_) => false,
        None => true,
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
#[path = "claude_hooks_tests.rs"]
mod tests;
