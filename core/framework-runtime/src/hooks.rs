//! Runtime Core 钩子注册系统。
//!
//! 为了从 `runtime-core` 提取 `framework-runtime` 独立 crate，所有仍然依赖
//! `runtime-core` 特有模块的功能通过回调钩子解耦。这与 `host-projection` crate
//! 已有的 `register_host_projection_hooks()` 模式一致。
//!
//! 使用方式：
//! ```ignore
//! let hooks = RuntimeCoreHooks { ... };
//! runtime_core_hooks::register(hooks);
//! ```

#![allow(clippy::type_complexity)]
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

static RUNTIME_CORE_HOOKS: OnceLock<RuntimeCoreHooks> = OnceLock::new();

/// 获取已注册的钩子。需在调用其他 framework-runtime 函数前注册。
pub fn hooks() -> &'static RuntimeCoreHooks {
    RUNTIME_CORE_HOOKS
        .get()
        .expect("RuntimeCoreHooks not registered — call register() before use")
}

/// 注册钩子。只接受第一次调用；后续调用静默忽略。
pub fn register(h: RuntimeCoreHooks) {
    RUNTIME_CORE_HOOKS.get_or_init(|| h);
}

// ── Method wrappers for hook function pointer calls ──
impl RuntimeCoreHooks {
    pub fn emit_hook_fired(&self, name: &str, action: &str) { (self.emit_hook_fired)(name, action); }
    pub fn emit_tool_call(&self, tool: &str, count: u32, blocked: bool) { (self.emit_tool_call)(tool, count, blocked); }
    pub fn emit_route_decision(&self, query: &str, decision: &Value, reroute: bool) { (self.emit_route_decision)(query, decision, reroute); }
    pub fn framework_goal_drive(&self, payload: Value) -> Result<Value, String> { (self.framework_goal_drive)(payload) }
    pub fn framework_rfv_loop(&self, payload: Value) -> Result<Value, String> { (self.framework_rfv_loop)(payload) }
    pub fn emit_prediction_outcome(&self, task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize) { (self.emit_prediction_outcome)(task_id, checks_summary, verification_status, checks_count); }
    pub fn emit_rfv_round(&self, round: u32, verdict: &str) { (self.emit_rfv_round)(round, verdict); }
    pub fn host_provider_strict_pre_tool_fallback_hint(&self, host_id: &str) -> Option<bool> { (self.host_provider_strict_pre_tool_fallback_hint)(host_id) }
    pub fn host_provider_for_routing_spelling(&self, host_id: Option<&str>) -> Option<&'static str> { (self.host_provider_for_routing_spelling)(host_id) }
    pub fn default_host_id(&self) -> &'static str { (self.default_host_id)() }
    pub fn host_provider_registry(&self) -> Vec<(&'static str, Option<&'static str>)> { (self.host_provider_registry)() }
    pub fn handle_session_supervisor_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_session_supervisor_operation)(payload) }
    pub fn handle_background_state_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_background_state_operation)(payload) }
    pub fn runtime_concurrency_defaults_payload(&self) -> Value { (self.runtime_concurrency_defaults_payload)() }
    pub fn eval_route_contract(&self) -> Value { (self.eval_route_contract)() }
    pub fn run_eval_route(&self, cases_path: &Path, runtime: Option<&Path>, manifest: Option<&Path>) -> Result<Value, String> { (self.run_eval_route)(cases_path, runtime, manifest) }
    pub fn generated_artifacts_status_for_repo(&self, repo_root: &Path) -> Result<String, String> { (self.generated_artifacts_status_for_repo)(repo_root) }
    pub fn ensure_kernel_bootstrap(&self) { (self.ensure_kernel_bootstrap)() }
}

/// 所有需要回调到 `runtime-core` 的钩子。
pub struct RuntimeCoreHooks {
    // ── 遥测 ──
    pub emit_hook_fired: fn(hook_name: &str, action: &str),
    pub emit_tool_call: fn(tool: &str, count: u32, blocked: bool),
    pub emit_route_decision: fn(query: &str, decision: &Value, reroute: bool),
    pub framework_goal_drive: fn(Value) -> Result<Value, String>,
    pub framework_rfv_loop: fn(Value) -> Result<Value, String>,
    pub emit_prediction_outcome: fn(task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize),
    pub emit_rfv_round: fn(round: u32, verdict: &str),

    // ── 宿主 ──
    pub host_provider_for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub default_host_id: fn() -> &'static str,
    pub host_provider_strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub host_provider_registry: fn() -> Vec<(&'static str, Option<&'static str>)>,

    // ── Session / 后台 ──
    pub handle_session_supervisor_operation: fn(Value) -> Result<Value, String>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, String>,
    pub runtime_concurrency_defaults_payload: fn() -> Value,

    // ── 路由评估 ──
    pub eval_route_contract: fn() -> Value,
    pub run_eval_route: fn(cases_path: &Path, runtime: Option<&Path>, manifest: Option<&Path>) -> Result<Value, String>,

    // ── 诊断 ──
    pub generated_artifacts_status_for_repo: fn(repo_root: &Path) -> Result<String, String>,

    // ── 内核引导 ──
    pub ensure_kernel_bootstrap: fn(),
}
