//! Shared host × event harness for cross-host review-gate contract tests.

use serde_json::{Value, json};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cursor_hooks::dispatch_cursor_hook_event;
use crate::hosts::claude_hooks::dispatch_claude_hook_payload_for_test;
use crate::hosts::codex_hooks::run_codex_lifecycle_context_hook_for_state_dir;
use crate::opencode_hooks::dispatch_opencode_hook_event;
use crate::test_env_sync::ProcessEnvLockGuard;

const SPAWN_FIRST_NUDGE_ENV: &str = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
const FORK_INFER_ENV: &str = "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE";
const CURSOR_MODEL_INHERIT_ENV: &str = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";

static MATRIX_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatrixHost {
    Cursor,
    Codex,
    Claude,
    Opencode,
}

pub const DEEP_REVIEW_PROMPT: &str = "全面review这个仓库";
pub const NARROW_REVIEW_PROMPT: &str = "review ./README.md";
pub const MY_LIGHT_STOP_PROMPT: &str = "/implementx 继续";
pub const MY_LIGHT_IMPLEMENT_PROMPT: &str = "/implementx run waves";
pub const SECOND_DEEP_REVIEW_PROMPT: &str = "全面review全仓找bug";
pub const PAPER_PROSE_PROMPT: &str = "polish this abstract for clarity";

/// Hosts with shared Stop closeout enforcement (hard block on completion claim when strict).
pub const CLOSEOUT_MATRIX_HOSTS: &[MatrixHost] =
    &[MatrixHost::Cursor, MatrixHost::Codex, MatrixHost::Claude];

/// Framework repo cwd for Cursor fixtures (matches `cursor_hooks/tests.rs`).
pub fn framework_harness_cwd() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn next_label(prefix: &str) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let seq = MATRIX_SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{prefix}-{seq}-{nonce}")
}

pub fn fresh_matrix_repo(host: MatrixHost, label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "router-rs-hook-contract-{}-{}-{}",
        host_label(host),
        label,
        next_label("repo")
    ));
    let _ = fs::remove_dir_all(&root);
    match host {
        MatrixHost::Cursor => {
            fs::create_dir_all(root.join(".cursor/hooks")).expect("mkdir cursor hooks");
            fs::write(root.join(".cursor/hooks.json"), b"{\"version\":1}\n").expect("hooks.json");
            fs::create_dir_all(root.join(".cursor/hook-state")).expect("hook-state");
        }
        MatrixHost::Codex => {
            fs::create_dir_all(root.join(".codex/hook-state")).expect("codex hook-state");
        }
        MatrixHost::Claude => {
            fs::create_dir_all(root.join(".claude/hook-state")).expect("claude hook-state");
        }
        MatrixHost::Opencode => {
            fs::create_dir_all(root.join(".opencode")).expect("opencode dir");
        }
    }
    root
}

pub fn host_label(host: MatrixHost) -> &'static str {
    match host {
        MatrixHost::Cursor => "cursor",
        MatrixHost::Codex => "codex",
        MatrixHost::Claude => "claude",
        MatrixHost::Opencode => "opencode",
    }
}

const CANONICAL_REVIEW_GATE_DISABLE_ENV: &str = "ROUTER_RS_REVIEW_GATE_DISABLE";

pub struct ReviewGateActiveGuard {
    _lock: ProcessEnvLockGuard,
    restored: Vec<(String, Option<OsString>)>,
}

impl ReviewGateActiveGuard {
    pub fn new(host: MatrixHost) -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let mut restored = Vec::new();
        for key in review_gate_disable_env_keys(host) {
            restored.push((key.to_string(), std::env::var_os(key)));
            unsafe { std::env::remove_var(key) };
        }
        if host == MatrixHost::Cursor {
            crate::cursor_hooks::set_test_review_gate_disable_override(Some(false));
        }
        Self { _lock, restored }
    }
}

fn review_gate_disable_env_keys(host: MatrixHost) -> [&'static str; 2] {
    match host {
        MatrixHost::Cursor => [
            CANONICAL_REVIEW_GATE_DISABLE_ENV,
            "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
        ],
        MatrixHost::Codex => [
            CANONICAL_REVIEW_GATE_DISABLE_ENV,
            "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
        ],
        MatrixHost::Claude => [
            CANONICAL_REVIEW_GATE_DISABLE_ENV,
            "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
        ],
        MatrixHost::Opencode => [
            CANONICAL_REVIEW_GATE_DISABLE_ENV,
            "ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE",
        ],
    }
}

/// Sets canonical `ROUTER_RS_REVIEW_GATE_DISABLE=1` (clears legacy per-host disable vars).
pub struct CanonicalReviewGateDisableGuard {
    _lock: ProcessEnvLockGuard,
    restored: Vec<(String, Option<OsString>)>,
}

impl CanonicalReviewGateDisableGuard {
    pub fn new(host: MatrixHost) -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let mut restored = Vec::new();
        for key in review_gate_disable_env_keys(host) {
            restored.push((key.to_string(), std::env::var_os(key)));
            unsafe { std::env::remove_var(key) };
        }
        unsafe { std::env::set_var(CANONICAL_REVIEW_GATE_DISABLE_ENV, "1") };
        if host == MatrixHost::Cursor {
            crate::cursor_hooks::set_test_review_gate_disable_override(Some(true));
        }
        Self { _lock, restored }
    }
}

/// Legacy per-host `ROUTER_RS_*_REVIEW_GATE_DISABLE=1` (canonical env cleared).
pub struct LegacyReviewGateDisableGuard {
    _lock: ProcessEnvLockGuard,
    restored: Vec<(String, Option<OsString>)>,
}

impl LegacyReviewGateDisableGuard {
    pub fn new(host: MatrixHost) -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let mut restored = Vec::new();
        for key in review_gate_disable_env_keys(host) {
            restored.push((key.to_string(), std::env::var_os(key)));
            unsafe { std::env::remove_var(key) };
        }
        restored.push((
            CANONICAL_REVIEW_GATE_DISABLE_ENV.to_string(),
            std::env::var_os(CANONICAL_REVIEW_GATE_DISABLE_ENV),
        ));
        unsafe { std::env::remove_var(CANONICAL_REVIEW_GATE_DISABLE_ENV) };
        let legacy = review_gate_disable_env_keys(host)[1];
        unsafe { std::env::set_var(legacy, "1") };
        if host == MatrixHost::Cursor {
            crate::cursor_hooks::set_test_review_gate_disable_override(None);
        }
        Self { _lock, restored }
    }
}

impl Drop for LegacyReviewGateDisableGuard {
    fn drop(&mut self) {
        for (key, prev) in self.restored.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
        crate::cursor_hooks::set_test_review_gate_disable_override(None);
        crate::hook_common::set_test_my_light_override(None);
    }
}

impl Drop for CanonicalReviewGateDisableGuard {
    fn drop(&mut self) {
        for (key, prev) in self.restored.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
        crate::cursor_hooks::set_test_review_gate_disable_override(None);
        crate::hook_common::set_test_my_light_override(None);
    }
}

impl Drop for ReviewGateActiveGuard {
    fn drop(&mut self) {
        for (key, prev) in self.restored.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
        crate::cursor_hooks::set_test_review_gate_disable_override(None);
        crate::hook_common::set_test_my_light_override(None);
    }
}

pub struct MyLightOverrideGuard;

impl MyLightOverrideGuard {
    pub fn clear_stale_override() -> Self {
        crate::hook_common::set_test_my_light_override(None);
        Self
    }
}

impl Drop for MyLightOverrideGuard {
    fn drop(&mut self) {
        crate::hook_common::set_test_my_light_override(None);
    }
}

/// Restore a single env var on drop (caller holds `process_env_lock` when needed).
pub struct EnvVarGuard {
    key: String,
    prev: Option<OsString>,
}

impl EnvVarGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let prev = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self {
            key: key.to_string(),
            prev,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => unsafe { std::env::set_var(&self.key, v) },
            None => unsafe { std::env::remove_var(&self.key) },
        }
    }
}

pub struct SpawnFirstNudgeEnableGuard {
    _lock: ProcessEnvLockGuard,
    _nudge: EnvVarGuard,
}

impl SpawnFirstNudgeEnableGuard {
    pub fn enable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        Self {
            _lock,
            _nudge: EnvVarGuard::set(SPAWN_FIRST_NUDGE_ENV, "1"),
        }
    }
}

pub struct SpawnFirstNudgeDisableGuard {
    _lock: ProcessEnvLockGuard,
    _nudge: EnvVarGuard,
}

impl SpawnFirstNudgeDisableGuard {
    pub fn disable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        Self {
            _lock,
            _nudge: EnvVarGuard::set(SPAWN_FIRST_NUDGE_ENV, "0"),
        }
    }
}

/// Cursor spawn-first tests: disable model-inherit line so assertions stay on spawn-first only.
pub struct CursorModelInheritDisableGuard {
    _lock: ProcessEnvLockGuard,
    _inherit: EnvVarGuard,
}

impl CursorModelInheritDisableGuard {
    pub fn disable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        Self {
            _lock,
            _inherit: EnvVarGuard::set(CURSOR_MODEL_INHERIT_ENV, "0"),
        }
    }
}

/// Canonical `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=1` (Epic D).
pub struct ForkInferEnableGuard {
    _lock: ProcessEnvLockGuard,
    _infer: EnvVarGuard,
    restored_legacy: Vec<(String, Option<OsString>)>,
}

impl ForkInferEnableGuard {
    pub fn enable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let mut restored_legacy = Vec::new();
        for key in [
            "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
        ] {
            restored_legacy.push((key.to_string(), std::env::var_os(key)));
            unsafe { std::env::remove_var(key) };
        }
        Self {
            _lock,
            _infer: EnvVarGuard::set(FORK_INFER_ENV, "1"),
            restored_legacy,
        }
    }
}

impl Drop for ForkInferEnableGuard {
    fn drop(&mut self) {
        for (key, prev) in self.restored_legacy.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }
}

pub struct CloseoutEnforcementGuard {
    _lock: ProcessEnvLockGuard,
    _closeout: EnvVarGuard,
}

impl CloseoutEnforcementGuard {
    pub fn strict() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        Self {
            _lock,
            _closeout: EnvVarGuard::set("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1"),
        }
    }
}

/// Paper prose hook default-on: clear per-host off switch and operator inject kill-switch.
pub struct PaperProseDefaultGuard {
    _nudge_lock: std::sync::MutexGuard<'static, ()>,
    _lock: ProcessEnvLockGuard,
    restored: Vec<(String, Option<OsString>)>,
}

impl PaperProseDefaultGuard {
    pub fn enable(host: MatrixHost) -> Self {
        let _nudge_lock = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let _lock = crate::test_env_sync::process_env_lock();
        let mut restored = Vec::new();
        for key in [paper_prose_hook_env(host), "ROUTER_RS_OPERATOR_INJECT"] {
            restored.push((key.to_string(), std::env::var_os(key)));
            unsafe { std::env::remove_var(key) };
        }
        Self {
            _nudge_lock,
            _lock,
            restored,
        }
    }
}

impl Drop for PaperProseDefaultGuard {
    fn drop(&mut self) {
        for (key, prev) in self.restored.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }
}

fn paper_prose_hook_env(host: MatrixHost) -> &'static str {
    match host {
        MatrixHost::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
        MatrixHost::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
        MatrixHost::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
        MatrixHost::Opencode => "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK",
    }
}

pub fn write_matrix_active_task(repo: &Path, task_id: &str) {
    let p = repo.join("artifacts/current/active_task.json");
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir artifacts/current");
    fs::write(p, format!(r#"{{"task_id":"{task_id}"}}"#)).expect("write active_task");
    fs::create_dir_all(repo.join("artifacts/current").join(task_id)).expect("mkdir task dir");
    // Pointer 机制已移除：同时写入 task_registry.json 供回退使用
    let registry_path = repo.join("artifacts/current/task_registry.json");
    let registry = serde_json::json!({
        "schema_version": "task-registry-v1",
        "focus_task_id": task_id,
        "tasks": [{ "task_id": task_id }]
    });
    fs::write(
        &registry_path,
        serde_json::to_string(&registry).expect("serialize registry"),
    )
    .expect("write task_registry");
}

fn session_payload(host: MatrixHost, repo: &Path, session_id: &str, prompt: &str) -> Value {
    match host {
        MatrixHost::Cursor => json!({
            "session_id": session_id,
            "cwd": framework_harness_cwd(),
            "prompt": prompt,
        }),
        MatrixHost::Codex => json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": repo.to_string_lossy(),
            "prompt": prompt,
        }),
        MatrixHost::Claude => json!({
            "session_id": session_id,
            "prompt": prompt,
        }),
        MatrixHost::Opencode => json!({
            "session_id": session_id,
            "prompt": prompt,
        }),
    }
}

fn stop_payload(
    host: MatrixHost,
    repo: &Path,
    session_id: &str,
    prompt: &str,
    response: Option<&str>,
) -> Value {
    let mut payload = match host {
        MatrixHost::Cursor => json!({
            "session_id": session_id,
            "cwd": framework_harness_cwd(),
            "prompt": prompt,
        }),
        MatrixHost::Codex => json!({
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": repo.to_string_lossy(),
            "prompt": prompt,
        }),
        MatrixHost::Claude => json!({
            "session_id": session_id,
            "prompt": prompt,
        }),
        MatrixHost::Opencode => json!({
            "session_id": session_id,
            "prompt": prompt,
        }),
    };
    if let Some(response) = response {
        payload["response"] = json!(response);
    }
    payload
}

pub fn dispatch_user_prompt_submit(
    host: MatrixHost,
    repo: &Path,
    session_id: &str,
    prompt: &str,
) -> Value {
    let payload = session_payload(host, repo, session_id, prompt);
    dispatch_hook(host, repo, "UserPromptSubmit", &payload)
}

pub fn dispatch_stop(
    host: MatrixHost,
    repo: &Path,
    session_id: &str,
    prompt: &str,
    response: Option<&str>,
) -> Value {
    let payload = stop_payload(host, repo, session_id, prompt, response);
    dispatch_hook(host, repo, "Stop", &payload)
}

pub fn user_prompt_additional_context(host: MatrixHost, out: &Value) -> String {
    match host {
        MatrixHost::Cursor | MatrixHost::Opencode => out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        MatrixHost::Codex | MatrixHost::Claude => out
            .get("hookSpecificOutput")
            .and_then(|h| h.get("additionalContext"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

pub fn dispatch_reviewer_with_fork(
    host: MatrixHost,
    repo: &Path,
    session_id: &str,
    fork: Option<bool>,
    subagent_id: &str,
) -> Value {
    match host {
        MatrixHost::Cursor => {
            let mut start = json!({
                "session_id": session_id,
                "cwd": framework_harness_cwd(),
                "subagent_type": "general-purpose",
                "subagent_id": subagent_id,
            });
            if let Some(f) = fork {
                start["fork_context"] = json!(f);
            }
            let _ = dispatch_cursor_hook_event(repo, "subagentStart", &start);
            dispatch_cursor_hook_event(
                repo,
                "subagentStop",
                &json!({
                    "session_id": session_id,
                    "subagent_type": "general-purpose",
                    "subagent_id": subagent_id,
                }),
            )
        }
        MatrixHost::Codex => {
            let mut tool_input = json!({ "subagent_type": "general-purpose" });
            if let Some(f) = fork {
                tool_input["fork_context"] = json!(f);
            }
            let payload = json!({
                "hook_event_name": "PostToolUse",
                "session_id": session_id,
                "cwd": repo.to_string_lossy(),
                "tool_name": "Task",
                "tool_input": tool_input,
            });
            run_codex_lifecycle_context_hook_for_state_dir(repo, &payload, ".codex")
                .unwrap_or_else(|err| panic!("codex reviewer dispatch failed: {err}"))
                .unwrap_or(json!({}))
        }
        MatrixHost::Claude => {
            let mut tool_input = json!({ "agent_type": "general-purpose" });
            if let Some(f) = fork {
                tool_input["fork_context"] = json!(f);
            }
            let payload = json!({
                "session_id": session_id,
                "tool_name": "functions.spawn_agent",
                "tool_input": tool_input,
            });
            dispatch_claude_hook_payload_for_test("post-tool-use", repo, &payload)
        }
        MatrixHost::Opencode => {
            let mut tool_input = json!({ "subagent_type": "general-purpose" });
            if let Some(f) = fork {
                tool_input["fork_context"] = json!(f);
            }
            let payload = json!({
                "session_id": session_id,
                "tool_name": "Task",
                "tool_input": tool_input,
            });
            dispatch_opencode_hook_event(repo, "PostToolUse", &payload)
        }
    }
}

pub fn dispatch_independent_reviewer(host: MatrixHost, repo: &Path, session_id: &str) -> Value {
    match host {
        MatrixHost::Cursor => {
            let base = json!({
                "session_id": session_id,
                "cwd": framework_harness_cwd(),
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "matrix-reviewer-1",
            });
            let _ = dispatch_cursor_hook_event(repo, "subagentStart", &base);
            dispatch_cursor_hook_event(
                repo,
                "subagentStop",
                &json!({
                    "session_id": session_id,
                    "subagent_type": "general-purpose",
                    "subagent_id": "matrix-reviewer-1",
                }),
            )
        }
        MatrixHost::Codex => {
            let payload = json!({
                "hook_event_name": "PostToolUse",
                "session_id": session_id,
                "cwd": repo.to_string_lossy(),
                "tool_name": "Task",
                "tool_input": {
                    "subagent_type": "general-purpose",
                    "fork_context": false,
                },
            });
            run_codex_lifecycle_context_hook_for_state_dir(repo, &payload, ".codex")
                .unwrap_or_else(|err| panic!("codex reviewer dispatch failed: {err}"))
                .unwrap_or(json!({}))
        }
        MatrixHost::Claude => {
            let payload = json!({
                "session_id": session_id,
                "tool_name": "functions.spawn_agent",
                "tool_input": {
                    "agent_type": "general-purpose",
                    "fork_context": false,
                },
            });
            dispatch_claude_hook_payload_for_test("post-tool-use", repo, &payload)
        }
        MatrixHost::Opencode => {
            let payload = json!({
                "session_id": session_id,
                "tool_name": "Task",
                "tool_input": {
                    "subagent_type": "general-purpose",
                    "fork_context": false,
                },
            });
            dispatch_opencode_hook_event(repo, "PostToolUse", &payload)
        }
    }
}

fn dispatch_hook(host: MatrixHost, repo: &Path, canonical: &str, payload: &Value) -> Value {
    match host {
        MatrixHost::Cursor => {
            let native = match canonical {
                "UserPromptSubmit" => "beforeSubmitPrompt",
                "Stop" => "stop",
                other => panic!("unsupported cursor matrix event {other}"),
            };
            dispatch_cursor_hook_event(repo, native, payload)
        }
        MatrixHost::Codex => {
            run_codex_lifecycle_context_hook_for_state_dir(repo, payload, ".codex")
                .unwrap_or_else(|err| panic!("codex dispatch failed: {err}"))
                .unwrap_or(json!({}))
        }
        MatrixHost::Claude => {
            let canonical_event = match canonical {
                "UserPromptSubmit" => "user-prompt-submit",
                "Stop" => "stop",
                "PostToolUse" => "post-tool-use",
                other => panic!("unsupported claude matrix event {other}"),
            };
            dispatch_claude_hook_payload_for_test(canonical_event, repo, payload)
        }
        MatrixHost::Opencode => dispatch_opencode_hook_event(repo, canonical, payload),
    }
}

pub fn user_visible_blob(host: MatrixHost, out: &Value) -> String {
    match host {
        MatrixHost::Claude => {
            // Primary: stopReason / reason (block_stop path)
            let primary = out
                .get("stopReason")
                .or_else(|| out.get("reason"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !primary.is_empty() {
                return primary;
            }
            // Fallback: hookSpecificOutput.additionalContext (advisory add_context path)
            out.get("hookSpecificOutput")
                .and_then(|h| h.get("additionalContext"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }
        MatrixHost::Cursor | MatrixHost::Codex | MatrixHost::Opencode => {
            let mut s = out
                .get("followup_message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if let Some(ac) = out.get("additional_context").and_then(Value::as_str) {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(ac);
            }
            s
        }
    }
}

pub fn stop_review_gate_advisory(host: MatrixHost, out: &Value) -> bool {
    match host {
        MatrixHost::Cursor => user_visible_blob(host, out).contains("REVIEW_GATE incomplete"),
        MatrixHost::Codex => {
            out.get("decision").and_then(Value::as_str) != Some("block")
                && user_visible_blob(host, out).contains("CODEX_REVIEW_GATE")
        }
        MatrixHost::Claude => {
            out.get("decision").and_then(Value::as_str) != Some("block")
                && out
                    .get("hookSpecificOutput")
                    .and_then(|h| h.get("additionalContext"))
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.contains("CLAUDE_REVIEW_GATE"))
        }
        MatrixHost::Opencode => user_visible_blob(host, out).contains("opencode-review-gate"),
    }
}

/// Hard Stop block for review gate only (closeout / corrupt state may still block).
pub fn stop_review_gate_blocked(host: MatrixHost, out: &Value) -> bool {
    match host {
        MatrixHost::Cursor => {
            out.get("continue").and_then(Value::as_bool) == Some(false)
                && user_visible_blob(host, out).contains("REVIEW_GATE")
        }
        MatrixHost::Codex => {
            out.get("decision").and_then(Value::as_str) == Some("block")
                && user_visible_blob(host, out).contains("CODEX_REVIEW_GATE")
        }
        MatrixHost::Claude => {
            out.get("decision").and_then(Value::as_str) == Some("block")
                && user_visible_blob(host, out).contains("CLAUDE_REVIEW_GATE")
        }
        MatrixHost::Opencode => {
            out.get("continue").and_then(Value::as_bool) == Some(false)
                && user_visible_blob(host, out).contains("opencode-review-gate")
        }
    }
}

pub fn stop_allowed(host: MatrixHost, out: &Value) -> bool {
    !stop_review_gate_blocked(host, out)
}

/// Bounded reject / `rg_clear` clears review-gate Stop nudge on all hosts (advisory posture).
pub fn reject_token_clears_stop(_host: MatrixHost) -> bool {
    true
}

pub fn closeout_followup_visible(host: MatrixHost, out: &Value) -> bool {
    let blob = user_visible_blob(host, out);
    blob.contains("CLOSEOUT_FOLLOWUP") && blob.contains("missing_record")
}

/// Dispatch Stop with a completion claim for cross-host closeout matrix rows.
pub fn dispatch_closeout_claim_stop(host: MatrixHost, repo: &Path, session_id: &str) -> Value {
    match host {
        MatrixHost::Cursor => {
            let payload = json!({
                "session_id": session_id,
                "cwd": repo.to_string_lossy(),
                "prompt": "ok",
                "response": "done",
            });
            dispatch_cursor_hook_event(repo, "stop", &payload)
        }
        MatrixHost::Codex => dispatch_stop(host, repo, session_id, "all done, shipped", None),
        MatrixHost::Claude => dispatch_stop(host, repo, session_id, "done", None),
        MatrixHost::Opencode => {
            panic!("Opencode is my-light; closeout tests should not dispatch closeout_claim_stop")
        }
    }
}

/// All matrix hosts in this slice (Cursor + Codex + Claude + Opencode).
pub const MATRIX_HOSTS: &[MatrixHost] = &[
    MatrixHost::Cursor,
    MatrixHost::Codex,
    MatrixHost::Claude,
    MatrixHost::Opencode,
];
