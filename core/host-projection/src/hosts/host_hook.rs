//! 宿主钩子抽象 trait（HostHook）。
//!
//! 每个宿主（Claude, Codex, Cursor, Antigravity-CLI, Desktop）实现此 trait，
//! 将「读取 stdin → 规范化事件名 → 分派到 handler → 输出 JSON」统一到框架层。
//!
//! 宿主特有逻辑（如 Cursor 的 subagent 并发计数、Codex 的 review gate 阶段机）
//! 保留在各自的 `impl HostHook for XxxHook` 中；共性部分（stdin 读取、JSON 输出、
//! 事件规范化的 fallback）由 trait 默认方法（`run_cli_hook` / `dispatch`）提供。
//!
//! **E7 迁移约束**：新宿主不得覆盖 `run_cli_hook` / `dispatch` / `read_stdin_payload`，
//! 除非其 `host_id()` 在 [`router_rs::hosts::host_hook_contract`] 的 legacy allowlist 中。
//! 测试见 `host_hook_contract` 与 `host_hook_dispatch_tests`。

use router_rs::framework_error::FrameworkResult;
use serde_json::Value;
use std::path::Path;

/// 钩子分派决策。
#[derive(Debug, Clone)]
pub enum HookDecision {
    /// 允许操作继续（Claude: `{"suppressOutput":true}`）。
    Allow,
    /// 阻止操作（Claude: `{"decision":"block","reason":"…"}`）。
    Block { reason: String },
    /// 自定义 JSON 响应（Cursor/Codex 各有不同的 schema）。
    Custom(Value),
}

impl HookDecision {
    /// Claude Code allow/no-op shape (legacy `silent_success` parity).
    pub fn allow_value() -> Value {
        serde_json::json!({"suppressOutput": true})
    }

    pub fn block_value(reason: &str) -> Value {
        serde_json::json!({"decision": "block", "reason": reason, "suppressOutput": true})
    }

    pub fn into_value(self) -> Value {
        match self {
            Self::Allow => Self::allow_value(),
            Self::Block { reason } => Self::block_value(&reason),
            Self::Custom(v) => v,
        }
    }
}

/// 宿主钩子公共 trait。
///
/// # 约定
///
/// - `host_id()` 返回稳定的短标识（`"claude"`, `"codex"`, `"cursor"`, `"antigravity"`, `"desktop"`）。
/// - `canonical_event()` 将宿主原始事件名映射到规范 kebab-case 名；无法识别时返回 `Err`。
/// - 各 handler 接收已解析的 JSON payload 和 repo root；返回 `HookDecision`。
/// - `dispatch()` 提供默认实现：规范化事件 → 路由到对应 handler。
/// - **E7**：新宿主应实现 handler 方法并使用默认 `dispatch` / `run_cli_hook`；
///   仅 legacy allowlist 中的宿主可临时覆盖 `run_cli_hook`（见 [`super::host_hook_contract`]）。
/// - `read_stdin_payload()` 可由宿主覆盖（如 Cursor 的特殊 stdin 解析）；新覆盖须入 allowlist。
pub trait HostHook: Send + Sync {
    /// 稳定宿主标识。
    #[allow(dead_code)] // trait surface; exercised via host impls / dispatch tests
    fn host_id(&self) -> &str;

    /// 将宿主原始事件名映射到规范 kebab-case 名。
    ///
    /// 例如：`"PreToolUse"` / `"pre-tool-use"` → `"pre-tool-use"`；
    /// `"BeforeSubmitPrompt"` → `"before-submit-prompt"`。
    fn canonical_event(&self, raw: &str) -> FrameworkResult<&'static str>;

    /// 该宿主关心的关键事件列表（用于 fail-closed 判断）。
    #[allow(dead_code)]
    fn critical_events(&self) -> &[&str];

    /// 读取 stdin payload。默认实现读取 stdin 并解析为 JSON。
    /// Cursor 宿主可覆盖以处理其特殊 envelope 格式。
    fn read_stdin_payload(&self) -> FrameworkResult<Value> {
        router_rs::hook_common::read_stdin_payload()
    }

    /// 静默成功响应（不输出任何内容给宿主）。
    fn silent_success(&self) -> Value {
        HookDecision::allow_value()
    }

    /// Optional observation host for outbound hook JSON (`router_rs_observation`).
    fn hook_observation_host(&self) -> Option<router_rs::router_rs_observation::HookObservationHost> {
        None
    }

    /// Short-circuit misrouted stdin (e.g. Cursor envelope on Claude hook).
    fn misrouted_stdin_short_circuit(&self, _payload: &Value) -> Option<Value> {
        None
    }

    fn finalize_cli_output(&self, output: &mut Value) {
        if let Some(host) = self.hook_observation_host() {
            router_rs::router_rs_observation::attach_router_rs_observation(output, host);
        }
    }

    // ── 事件 handlers ───────────────────────────────────────────────

    fn handle_pre_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision;
    fn handle_post_tool_use(&self, repo_root: &Path, payload: &Value) -> HookDecision;
    fn handle_stop(&self, repo_root: &Path, payload: &Value) -> HookDecision;

    /// 可选：处理 user-prompt-submit 事件。默认返回 Allow。
    fn handle_user_prompt_submit(&self, _repo_root: &Path, _payload: &Value) -> HookDecision {
        HookDecision::Allow
    }

    /// 可选：处理宿主特有事件（Cursor 的 subagent start/stop、before-submit-prompt 等）。
    /// 默认返回 Allow。
    fn handle_custom_event(
        &self,
        _event: &str,
        _repo_root: &Path,
        _payload: &Value,
    ) -> HookDecision {
        HookDecision::Allow
    }

    // ── 分派 ────────────────────────────────────────────────────────

    /// Shared PreToolUse path guard (hook_policy protected paths). Returns `Some(Block)` when denied.
    fn evaluate_pre_tool_path_guard(&self, repo_root: &Path, payload: &Value) -> Option<HookDecision> {
        if router_rs::router_env_flags::router_rs_skip_pre_tool_use_guard() {
            return None;
        }
        router_rs::hook_common::path_guard::pre_tool_protected_path_deny_reason(repo_root, payload)
            .map(|reason| HookDecision::Block { reason })
    }

    /// 默认分派实现：规范化事件名 → 路由到对应 handler。
    fn dispatch(&self, repo_root: &Path, event: &str, payload: &Value) -> Value {
        if let Some(v) = self.misrouted_stdin_short_circuit(payload) {
            return v;
        }
        let canonical = match self.canonical_event(event) {
            Ok(c) => c,
            Err(_) => return self.silent_success(),
        };
        let decision = match canonical {
            "pre-tool-use" => self
                .evaluate_pre_tool_path_guard(repo_root, payload)
                .unwrap_or_else(|| self.handle_pre_tool_use(repo_root, payload)),
            "post-tool-use" => self.handle_post_tool_use(repo_root, payload),
            "stop" => self.handle_stop(repo_root, payload),
            "user-prompt-submit" => self.handle_user_prompt_submit(repo_root, payload),
            other => self.handle_custom_event(other, repo_root, payload),
        };
        decision.into_value()
    }

    /// 完整的 CLI hook 执行流程：读取 stdin → 分派 → 返回 JSON。
    fn run_cli_hook(&self, event: &str, repo_root: &Path) -> FrameworkResult<Value> {
        let _registry_guard =
            router_rs::runtime_registry::HookRegistryRepoGuard::new(repo_root);
        let payload = self.read_stdin_payload()?;
        let mut output = self.dispatch(repo_root, event, &payload);
        self.finalize_cli_output(&mut output);
        Ok(output)
    }
}
