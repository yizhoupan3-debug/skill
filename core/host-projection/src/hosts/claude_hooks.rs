//! Claude Code（Anthropic CLI）hooks：`router-rs claude hook --event=… --repo-root …`。
//! 历史版本接口快照：`git show 89ece4c^:core/router-rs/src/claude_hooks.rs`（事件：`pre-tool-use`、`user-prompt-submit`、`post-tool-use`、`stop`；CLI 亦接受 `PreToolUse` 等 PascalCase 别名，与 Codex hook 拼写对齐）。
//!
//! **误接 Cursor hook stdin**：仅在 stdin JSON 呈现结构化 Cursor envelope（顶层非空 `cursor_version` 字符串 + `workspace_roots` 数组 + 非空 `hook_event_name` 或 `hookEventName`）时整条静默；
//! 不用路径子串扫描，以免合法 Claude 载荷（例如编辑 `.cursor/` 下文件）被误判为 Cursor 而旁路门禁。
//! stdin 体量上限 4 MiB，与 Codex hook 读取路径对齐，防失控输入撑爆 hook 进程内存。
#[path = "claude_hooks/pre_tool.rs"]
mod pre_tool;
#[path = "claude_hooks/session.rs"]
mod session;
#[path = "claude_hooks/post_tool.rs"]
mod post_tool;
#[path = "claude_hooks/stop.rs"]
mod stop;
#[path = "claude_hooks/user_prompt.rs"]
mod user_prompt;

pub use pre_tool::evaluate_claude_pre_tool_use;
pub use post_tool::evaluate_claude_post_tool_use;
pub use stop::evaluate_claude_stop;
pub use user_prompt::evaluate_claude_user_prompt_submit;


#[allow(unused_imports)]
pub use session::{
    clear_touch_state, legacy_touch_state_path, load_review_gate_disk, persist_touch_state,
    review_state_path, session_key, touch_state_path, AgentDiskState, ReviewGateState, TouchState,
};

use router_rs::framework_error::HookExitExt;
use router_rs::hook_common::read_stdin_payload;
use serde_json::{json, Value};
use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};

const CLAUDE_HOOK_STATE_UNREADABLE: &str =
    "hook_state JSON 读取失败 — 检查 JSON 语法或删除文件重置: rm -f .claude/hook-state/hook_state*.json";

/// 与 Claude Code 共享 hook JSON 协议；通过 thread-local 切换 `.claude` 等宿主差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioAgentHookHost {
    ClaudeCode,
}

impl StdioAgentHookHost {
    fn state_dir(self) -> &'static str {
        match self {
            Self::ClaudeCode => ".claude",
        }
    }

    fn hook_state_unreadable(self) -> &'static str {
        match self {
            Self::ClaudeCode => CLAUDE_HOOK_STATE_UNREADABLE,
        }
    }

    fn review_gate_disable_env(self) -> &'static str {
        match self {
            Self::ClaudeCode => "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
        }
    }

    fn session_namespace_env(self) -> &'static str {
        match self {
            Self::ClaudeCode => "ROUTER_RS_CLAUDE_SESSION_NAMESPACE",
        }
    }

    fn log_label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
        }
    }

    fn settings_guarded_paths(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => SETTINGS_GUARDED_PATHS_CLAUDE,
        }
    }

    fn generated_entrypoint_paths(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => GENERATED_ENTRYPOINT_PATHS_CLAUDE,
        }
    }

    fn user_config_dir_leaf(self) -> &'static str {
        match self {
            Self::ClaudeCode => ".claude",
        }
    }

    fn review_gate_incomplete_stop_reason(self) -> &'static str {
        match self {
            Self::ClaudeCode => "router-rs CLAUDE_REVIEW_GATE incomplete fix=Task subagent_type=general-purpose fork_context=false prompt=\"深度 review（只读 findings）\"",

        }
    }

    fn validate_settings_stop_reason(self) -> &'static str {
        match self {
            Self::ClaudeCode => "cargo build -p router-rs && cargo test -p router-rs --lib",
        }
    }
}

thread_local! {
    static ACTIVE_STDIO_AGENT_HOOK_HOST: Cell<StdioAgentHookHost> =
        const { Cell::new(StdioAgentHookHost::ClaudeCode) };
}

pub fn active_stdio_agent_hook_host() -> StdioAgentHookHost {
    ACTIVE_STDIO_AGENT_HOOK_HOST.with(|c| c.get())
}

#[allow(dead_code)] // legacy stdio agent hook CLI; HostHook is canonical entry
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

pub fn hook_state_base(repo_root: &Path) -> PathBuf {
    repo_root
        .join(active_stdio_agent_hook_host().state_dir())
        .join("hook-state")
}

/// Repo-relative forward-slash path when `raw` resolves under `repo_root`.
#[allow(dead_code)]
fn repo_relative_slash_path(repo_root: &Path, raw: &str) -> Option<String> {
    router_rs::hook_common::path_guard::repo_relative_slash_path(repo_root, raw, is_host_private_path)
}

pub const SETTINGS_CHANGED_CONTEXT: &str =
    "Hook/settings files changed; validate JSON and run the agent hook contract tests before finishing.";
const FRAMEWORK_SOURCE_PREFIXES: &[&str] = &[
    "core/router-rs/",
    "configs/framework/",
];
/// **仅当** 对应宿主 `ROUTER_RS_*_REVIEW_GATE_DISABLE` 为 `1` / `true` / `yes` / `on`（大小写不敏感）时跳过
/// review gate；unset、空串及其它值保持启用（与 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` **同形**：`router_rs_env_enabled_default_false`）。
pub fn agent_review_gate_disabled() -> bool {
    let host = active_stdio_agent_hook_host();
    router_rs::router_env_flags::router_rs_env_enabled_default_false(host.review_gate_disable_env())
}

/// Env disable **or** `my-light` profile (advisory-only mode).
pub fn claude_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if agent_review_gate_disabled() {
        return true;
    }
    router_rs::hook_common::review_gate_hard_block_disabled(Some(repo_root), text)
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

const SETTINGS_GUARDED_PATHS_CLAUDE: &[&str] =
    &[".claude/settings.json", ".claude/settings.local.json"];
const GENERATED_ENTRYPOINT_PATHS_CLAUDE: &[&str] = &[".claude/CLAUDE.md"];
/// Cross-host generated surfaces: active in other hosts, Claude should not directly modify.
const CROSS_HOST_SURFACES: &[&str] = &[".codex/hooks.json"];
/// Truly retired surfaces: defense-in-depth against accidental restoration.
const RETIRED_SURFACES: &[&str] = &[
    ".agents",
    "plugins/skill-framework-native/.mcp.json",
];

/// Pre-89ece4c the stdio agent hook accepted kebab-case commands only; CLI adds PascalCase aliases
/// aligned with Codex hook spelling (`PreToolUse`, `Stop`, …)。
#[allow(dead_code)]
pub fn run_claude_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    run_claude_hook_inner(command, repo_root).map_hook_exit()
}

#[allow(dead_code)]
fn run_claude_hook_inner(
    command: &str,
    repo_root: &Path,
) -> framework_core::error::Result<Value> {
    let _registry_guard = router_rs::runtime_registry::HookRegistryRepoGuard::new(repo_root);
    with_stdio_agent_hook_host(StdioAgentHookHost::ClaudeCode, || {
        let canonical = canonical_stdio_agent_hook_command(command)?;
        let payload = read_stdin_payload()?;
        Ok(dispatch_stdio_agent_hook_payload(
            canonical, repo_root, &payload,
        ))
    })
}

#[allow(dead_code)]
fn dispatch_stdio_agent_hook_payload(canonical: &str, repo_root: &Path, payload: &Value) -> Value {
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

#[allow(dead_code)]
fn canonical_stdio_agent_hook_command(
    command: &str,
) -> framework_core::error::Result<&'static str> {
    match command.trim() {
        "pre-tool-use" | "PreToolUse" => Ok("pre-tool-use"),
        "user-prompt-submit" | "UserPromptSubmit" => Ok("user-prompt-submit"),
        "post-tool-use" | "PostToolUse" => Ok("post-tool-use"),
        "stop" | "Stop" => Ok("stop"),
        _ => Err(framework_core::error::FrameworkError::unsupported(format!(
            "Unsupported stdio agent hook command: {command}"
        ))),
    }
}

/// `router-rs claude hook --event=… --repo-root …` — stdin JSON → Claude Code hook response JSON (line-delimited).
#[allow(dead_code)]
pub fn run_claude_hook_cli(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    use router_rs::hosts::host_hook::HostHook;

    let repo_root = router_rs::framework_runtime::resolve_repo_root_arg(cli_repo_root)
        .map_hook_exit()?;
    let host = router_rs::hosts::claude_hook_host::ClaudeHookHost;
    let output = host
        .run_cli_hook(event, &repo_root)
        .map_hook_exit()?;
    let serialized =
        serde_json::to_string(&output).map_err(|err| format!("serialize hook output failed: {err}"))?;
    let mut stdout = std::io::stdout();
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|err| format!("write hook stdout failed: {err}"))?;
    Ok(())
}

#[allow(dead_code)]
fn silent_success() -> Value {
    router_rs::hosts::host_hook::HookDecision::allow_value()
}

/// Cursor hook stdin 误接到 stdio agent hook 时的结构化识别（顶层字段）。
pub fn payload_looks_like_cursor_hook_stdin(payload: &Value) -> bool {
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

pub fn deny_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

pub fn add_context(event: &str, context: &str) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": event,
            "additionalContext": context,
        },
    }))
}

pub fn block_stop(reason: &str) -> Option<Value> {
    Some(json!({
        "continue": false,
        "stopReason": reason,
        "decision": "block",
        "reason": reason,
        "suppressOutput": true,
    }))
}

#[allow(dead_code)]
fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    evaluate_claude_pre_tool_use(repo_root, payload)
}

#[allow(dead_code)]
fn run_user_prompt_submit(repo_root: &Path, payload: &Value) -> Option<Value> {
    evaluate_claude_user_prompt_submit(repo_root, payload)
}

#[allow(dead_code)]
fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    evaluate_claude_post_tool_use(repo_root, payload)
}

#[allow(dead_code)]
fn run_stop(repo_root: &Path, payload: &Value) -> Option<Value> {
    evaluate_claude_stop(repo_root, payload)
}

pub fn payload_relative_paths(repo_root: &Path, payload: &Value) -> Vec<String> {
    router_rs::hook_common::path_guard::payload_repo_relative_paths(
        repo_root,
        payload,
        is_host_private_path,
    )
}

pub fn is_cross_host_or_retired_surface(path: &str) -> bool {
    CROSS_HOST_SURFACES
        .iter()
        .chain(RETIRED_SURFACES.iter())
        .any(|surface| path == *surface || path.starts_with(&format!("{surface}/")))
}

pub fn is_framework_source_path(path: &str) -> bool {
    FRAMEWORK_SOURCE_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

pub fn is_generated_entrypoint(path: &str) -> bool {
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
    file_name.starts_with("review_gate_") || file_name.starts_with("hook_state_")
}

pub fn is_host_private_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with(".claude/") && is_repo_claude_hook_state_file(&normalized) {
        return true;
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

pub fn is_settings_path(path: &str) -> bool {
    active_stdio_agent_hook_host()
        .settings_guarded_paths()
        .contains(&path)
}

pub fn is_framework_guarded_path(path: &str) -> bool {
    FRAMEWORK_GUARDED_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use router_rs::hosts::claude_hooks::post_tool::subagent_tool;
    use std::fs;

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
        let err = router_rs::hook_common::read_limited_stdin(&mut cursor).unwrap_err();
        assert_eq!(err.to_hook_exit(), "stdin_too_large");
    }

    #[test]
    fn claude_stdin_limited_rejects_invalid_utf8() {
        let mut cursor = std::io::Cursor::new(vec![0xff, 0xfe, 0xfd]);
        let err = router_rs::hook_common::read_limited_stdin(&mut cursor).unwrap_err();
        assert_eq!(err.to_hook_exit(), "stdin_invalid_utf8");
    }

    #[test]
    fn subagent_tool_accepts_dotted_subagent_segment() {
        let p = json!({"tool_name": "lane.subagent.run"});
        assert!(subagent_tool(&p));
    }

    #[test]
    fn subagent_tool_rejects_subagent_as_plain_substring() {
        let p = json!({"tool_name": "not_really_subagent_helpers"});
        assert!(!subagent_tool(&p));
    }

    #[test]
    fn stop_blocks_when_no_exit_code_present() {
        let repo = unique_test_repo("stop-text-framework");
        let payload = json!({ "session_id": "s-text", "transcript": "cargo test passed" });
        persist_touch_state(&repo, &payload, false, true, false, false);

        let output = run_stop(&repo, &payload).unwrap();

        assert_eq!(output["continue"], false);
        assert_eq!(output["stopReason"], "cargo test --lib -p router-rs");
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
                "command": "cargo test --manifest-path core/router-rs/Cargo.toml claude_hooks"
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
                "command": "cargo test --manifest-path core/router-rs/Cargo.toml claude_hooks"
            },
            "exit_code": 101
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        let output = run_stop(&repo, &session).unwrap();

        assert_eq!(output["continue"], false);
        assert_eq!(output["stopReason"], "cargo test --lib -p router-rs");
        assert_eq!(output["decision"], "block");
        clear_touch_state(&repo, &session);
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
    fn claude_review_state_lock_file_created_on_write() {
        let repo = unique_test_repo("claude-flock");
        let payload = json!({ "session_id": "flock-s1", "prompt": "深度 review" });
        let _ = run_user_prompt_submit(&repo, &payload);
        let path = review_state_path(&repo, &payload);
        assert!(path.is_file());
        assert!(
            PathBuf::from(format!("{}.lock", path.display())).is_file(),
            "flock sidecar should exist after locked write"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_prompt_blocks_stop_until_independent_reviewer_seen() {
        let _env = router_rs::test_env_sync::process_env_lock();
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
        let _env = router_rs::test_env_sync::process_env_lock();
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
        let _env = router_rs::test_env_sync::process_env_lock();
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
    fn review_gate_accepts_review_lane_with_fork_false() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-review-lane");
        let prompt = json!({ "session_id": "s-review-lane", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let reviewer = json!({
            "session_id": "s-review-lane",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"subagent_type": "review", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        assert!(
            run_stop(&repo, &json!({ "session_id": "s-review-lane" })).is_none(),
            "Claude claude_reviewer_lanes includes review; independent evidence should clear gate"
        );
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_rejects_explore_even_with_fork_false() {
        let _env = router_rs::test_env_sync::process_env_lock();
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
        let _env = router_rs::test_env_sync::process_env_lock();
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
        let _env = router_rs::test_env_sync::process_env_lock();
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
    fn parse_stdin_json_trimmed_accepts_empty_and_valid_json() {
        use router_rs::framework_error::HookExitExt;
        use router_rs::hook_common::parse_stdin_json_trimmed;
        assert_eq!(
            parse_stdin_json_trimmed("").map_hook_exit().unwrap(),
            json!({})
        );
        assert_eq!(
            parse_stdin_json_trimmed(r#"{"session_id":"x"}"#)
                .map_hook_exit()
                .unwrap(),
            json!({"session_id":"x"})
        );
    }

    #[test]
    fn parse_stdin_json_trimmed_rejects_invalid_json() {
        use router_rs::framework_error::HookExitExt;
        use router_rs::hook_common::parse_stdin_json_trimmed;
        let err = parse_stdin_json_trimmed("not json").map_hook_exit().unwrap_err();
        assert!(
            err.starts_with("stdin_json_invalid:"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn session_key_metadata_session_id_matches_flat() {
        let repo = unique_test_repo("claude-meta-session");
        let flat = json!({"session_id": "sid-meta", "prompt": "x"});
        let nested = json!({"metadata": {"sessionId": "sid-meta"}, "prompt": "x"});
        assert_eq!(session_key(&repo, &flat), session_key(&repo, &nested));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn session_key_namespace_splits_same_repo_empty_payload() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-ns");
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-a");
        let a = session_key(&repo, &json!({}));
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-b");
        let b = session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_ne!(a, b, "namespace must split state for empty payload");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn session_key_repo_fallback_stable_without_id() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-repo-fb");
        let k1 = session_key(&repo, &json!({}));
        let k2 = session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_eq!(k1, k2);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_repo_review_gate_hook_state_path() {
        let repo = unique_test_repo("deny-review-gate-pretool");
        let session = json!({ "session_id": "s-deny-rg" });
        let sk = session_key(&repo, &session);
        let payload = json!({
            "tool_name": "Write",
            "file_path": format!(".claude/hook-state/review_gate_{sk}.json")
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("");
        assert!(
            reason.contains("host-private"),
            "unexpected reason: {reason}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_legacy_flat_review_gate_path() {
        let repo = unique_test_repo("deny-legacy-review-gate");
        let sk = session_key(&repo, &json!({ "session_id": "s-legacy-rg" }));
        let payload = json!({
            "tool_name": "Edit",
            "file_path": format!(".claude/review_gate_{sk}.json")
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn load_review_gate_migrates_legacy_flat_file_to_hook_state() {
        let repo = unique_test_repo("legacy-review-gate-migrate");
        let sid = "s-legacy-load";
        let session = json!({ "session_id": sid });
        let sk = session_key(&repo, &session);
        let legacy = repo.join(format!(".claude/review_gate_{sk}.json"));
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"review_required":true,"review_override":false,"independent_reviewer_seen":false}"#,
        )
        .unwrap();
        let loaded = match load_review_gate_disk(&repo, &session) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected legacy load, got {other:?}"),
        };
        assert!(loaded.review_required);
        assert!(!loaded.independent_reviewer_seen);
        let new_path = review_state_path(&repo, &session);
        assert!(
            new_path.is_file(),
            "legacy load must migrate to hook-state: {}",
            new_path.display()
        );
        let out = run_stop(&repo, &json!({ "session_id": sid, "prompt": "继续" }));
        assert!(
            out.is_some(),
            "armed legacy state must still block Stop until reviewer contract met"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_state_path_lives_under_hook_state_dir() {
        let repo = unique_test_repo("hook-state-dir");
        let session = json!({ "session_id": "s-path" });
        let path = review_state_path(&repo, &session);
        assert!(
            path.to_string_lossy().contains("/.claude/hook-state/review_gate_"),
            "unexpected path: {}",
            path.display()
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_warns_lexical_traversal_to_framework_data_source() {
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
        // SKILL_ROUTING_RUNTIME.json is now warn-only (not deny)
        let out = run_pre_tool_use(&repo, &payload).expect("warn");
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        let ctx = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("SKILL_ROUTING_RUNTIME.json"), "warn should mention the file");
        assert_eq!(out["suppressOutput"], true, "warn should suppress output");
        assert!(
            out.get("decision").is_none(),
            "warn should have no permissionDecision"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_allows_lexical_traversal_to_agents_md() {
        let repo = unique_test_repo("lexical-entrypoint");
        fs::write(repo.join("AGENTS.md"), b"x").unwrap();
        let payload = json!({
            "tool_name": "Edit",
            "file_path": "a/../../AGENTS.md"
        });
        // AGENTS.md is not in GENERATED_ENTRYPOINT_PATHS_CLAUDE and intentionally
        // has no PreToolUse warn — it is a cross-host shared document (not a Claude-specific
        // generated entrypoint), so direct editing is normal and a warn would be noise.
        let out = run_pre_tool_use(&repo, &payload);
        assert!(out.is_none(), "AGENTS.md should not be denied");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stop_blocks_when_review_gate_state_corrupt() {
        let repo = unique_test_repo("corrupt-review-gate");
        let session = json!({ "session_id": "s-corrupt-rg" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_stop(&repo, &session).expect("block");
        assert_eq!(out["decision"], "block");
        let reason = out["stopReason"].as_str().unwrap();
        assert!(
            reason.contains(CLAUDE_HOOK_STATE_UNREADABLE),
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
            .contains(CLAUDE_HOOK_STATE_UNREADABLE));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_returns_context_when_review_gate_corrupt() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-ups");
        let session = json!({ "session_id": "s-corrupt-ups", "prompt": "深度 review 这个 PR" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        assert!(out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(CLAUDE_HOOK_STATE_UNREADABLE));
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_implementx_returns_unreadable_when_review_gate_corrupt() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-implementx");
        let session = json!({ "session_id": "s-corrupt-impl", "prompt": "/implementx" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            ctx.contains(CLAUDE_HOOK_STATE_UNREADABLE),
            "corrupt hook-state must surface unreadable; got {ctx:?}"
        );
        assert!(
            !ctx.contains("ALL waves"),
            "must not mask corrupt state with implement nudge; got {ctx:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_discussx_returns_unreadable_when_review_gate_corrupt() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-discussx");
        let session = json!({ "session_id": "s-corrupt-discuss", "prompt": "/discussx" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            ctx.contains(CLAUDE_HOOK_STATE_UNREADABLE),
            "corrupt hook-state must surface unreadable before pre-exec nudge; got {ctx:?}"
        );
        assert!(
            !ctx.contains("READ-ONLY"),
            "must not mask corrupt state with pre-exec nudge; got {ctx:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_review_and_implementx_suppresses_review_arming() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("claude-dual-review-implementx");
        let sid = "s-claude-dual";
        let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": prompt }),
        );
        let state = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected state, got {other:?}"),
        };
        assert!(
            !state.review_required,
            "goal drive must suppress review arming on Claude UPS; got {state:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn canonical_command_rejects_unknown_event() {
        use router_rs::framework_error::HookExitExt;
        let err = canonical_stdio_agent_hook_command("unknown-event")
            .map_hook_exit()
            .unwrap_err();
        assert!(
            err.contains("Unsupported stdio agent hook command"),
            "{err}"
        );
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
                "command": "apply_patch core/router-rs/src/claude_hooks.rs"
            },
            "file_path": "core/router-rs/src/claude_hooks.rs",
            "exit_code": 0
        });

        let output = dispatch_stdio_agent_hook_payload("post-tool-use", &repo, &payload);

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

        let output = dispatch_stdio_agent_hook_payload("stop", &repo, &payload);

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
            "tool_name": "Edit",
            "file_path": "AGENTS.md"
        });
        // Partial envelope without hook_event_name is not a valid Cursor envelope,
        // so it routes through Claude pre-tool-use path guard (AGENTS.md is allow-listed).
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert_eq!(
            output,
            silent_success(),
            "partial envelope should not be mistaken for full Cursor stdin; got {output:?}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_payload_with_nested_cursor_path_is_not_silenced_as_cursor_stdin() {
        let repo = unique_test_repo("claude-cursor-path-not-envelope");
        let cursor_plan = repo.join(".cursor").join("plans").join("feature.plan.md");
        fs::create_dir_all(cursor_plan.parent().unwrap()).unwrap();
        let payload = json!({
            "session_id": "claude-session",
            "tool_name": "Edit",
            "file_path": "AGENTS.md",
            "context": cursor_plan.to_string_lossy(),
        });
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert_eq!(
            output,
            silent_success(),
            "nested .cursor path in context must not silence Claude PreToolUse; got {output:?}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_version_without_workspace_roots_is_not_envelope() {
        let repo = unique_test_repo("cursor-version-only-not-envelope");
        let payload = json!({
            "session_id": "mixed",
            "cursor_version": "3.3.30",
            "tool_name": "Edit",
            "file_path": "AGENTS.md",
        });
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert_eq!(
            output,
            silent_success(),
            "cursor_version without workspace_roots is not a Cursor envelope; AGENTS.md edit allowed; got {output:?}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn my_light_implementx_stop_suppresses_review_gate_when_review_armed() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("my-light-stop");
        let sid = "s-my-light";
        let armed = json!({ "session_id": sid, "prompt": "全面review" });
        let _ = run_user_prompt_submit(&repo, &armed);
        let stop = json!({ "session_id": sid, "prompt": "/implementx finish" });
        assert!(
            run_stop(&repo, &stop).is_none(),
            "my-light must suppress CLAUDE_REVIEW_GATE on Stop"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn my_light_user_prompt_clears_sticky_review_required() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("my-light-clear");
        let sid = "s-clear";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let armed = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected armed state, got {other:?}"),
        };
        assert!(armed.review_required);
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "/implementx run waves" }),
        );
        let cleared = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected cleared state, got {other:?}"),
        };
        assert!(
            !cleared.review_required,
            "my-light UPS must clear sticky review_required"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn second_review_prompt_in_same_session_requires_fresh_reviewer_evidence() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("rearm-review");
        let sid = "s-rearm";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let reviewer = json!({
            "session_id": sid,
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "Please do another code review of this change." }),
        );
        let stop = run_stop(&repo, &json!({ "session_id": sid })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn narrow_path_review_disarms_sticky_deep_arm() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("narrow-disarm");
        let sid = "s-narrow";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 整个路由系统" }),
        );
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "review ./README.md" }),
        );
        let cleared = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected state, got {other:?}"),
        };
        assert!(!cleared.review_required);
        assert!(
            run_stop(&repo, &json!({ "session_id": sid, "prompt": "review ./README.md" })).is_none()
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn failed_subagent_post_tool_does_not_record_reviewer_evidence() {
        let _env = router_rs::test_env_sync::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("failed-subagent");
        let sid = "s-fail";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let failed = json!({
            "session_id": sid,
            "tool_name": "functions.spawn_agent",
            "exit_code": 1,
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &failed).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": sid })).expect("stop block");
        assert_eq!(stop["decision"], "block");
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_injects_paper_prose_by_default() {
        let _g = router_rs::harness_operator_nudges::harness_nudges_env_test_lock();
        let prior = std::env::var_os("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK");
        std::env::remove_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK");
        let repo = unique_test_repo("prose-ups");
        let payload = json!({
            "prompt": "polish this abstract for clarity",
            "session_id": "claude-prose-1"
        });
        let out = run_user_prompt_submit(&repo, &payload);
        let ctx = out
            .as_ref()
            .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
            .unwrap_or_default();
        assert!(
            ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
            "expected prose hook: {ctx}"
        );
        let _ = fs::remove_dir_all(&repo);
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK"),
        }
    }

    fn unique_test_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "router-rs-claude-hooks-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".claude").join("hook-state")).unwrap();
        path
    }
}
