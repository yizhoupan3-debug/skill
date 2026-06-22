//! Runtime Core 钩子注册系统。
//!
//! 为了从 `runtime-core` 提取 `framework-runtime` 独立 crate，所有仍然依赖
//! `runtime-core` 特有模块的功能通过回调钩子解耦。这与 `host-projection` crate
//! 已有的 `register_host_projection_hooks()` 模式一致。
//!
//! 使用方式：
//! ```ignore
//! let hooks = RuntimeCoreHooks { telemetry, host_provider, ... };
//! runtime_core_hooks::register(hooks);
//! ```
//!
//! ## 设计：分组 fn pointer
//!
//! 原始设计有 17 个扁平 fn pointer 字段。重构为 5 个逻辑组，降低认知负荷：
//! - `TelemetryHooks`: 遥测事件发射 (5 fields)
//! - `HostProviderHooks`: 宿主提供者查询 (4 fields)
//! - 其余为独立功能钩子 (8 fields)

use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

static RUNTIME_CORE_HOOKS: OnceLock<RuntimeCoreHooks> = OnceLock::new();

/// 获取已注册的钩子引用。需在调用其他 framework-runtime 函数前注册。
/// 返回 `None` 表示尚未注册——调用方应走 fallback/no-op 路径。
pub fn try_hooks() -> Option<&'static RuntimeCoreHooks> {
    RUNTIME_CORE_HOOKS.get()
}

/// 获取已注册的钩子。需在调用其他 framework-runtime 函数前注册。
///
/// # Panics
/// 仅在 `register()` 尚未调用时 panic。生产环境由 `#[ctor::ctor]` 保证初始化顺序。
pub fn hooks() -> &'static RuntimeCoreHooks {
    RUNTIME_CORE_HOOKS
        .get()
        .expect("RuntimeCoreHooks not registered — call register() before use")
}

/// 注册钩子。只接受第一次调用；后续调用静默忽略。
pub fn register(h: RuntimeCoreHooks) {
    RUNTIME_CORE_HOOKS.get_or_init(|| h);
}

// ── 遥测钩子组 ──
pub struct TelemetryHooks {
    pub hook_fired: fn(hook_name: &str, action: &str),
    pub tool_call: fn(tool: &str, count: u32, blocked: bool),
    pub route_decision: fn(query: &str, decision: &Value, reroute: bool),
    pub prediction_outcome: fn(task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize),
    pub rfv_round: fn(round: u32, verdict: &str),
}

// ── 宿主提供者钩子组 ──
pub struct HostProviderHooks {
    pub for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub default_id: fn() -> &'static str,
    pub strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub registry: fn() -> Vec<(&'static str, Option<&'static str>)>,
}

// ── Method wrappers for hook function pointer calls ──
impl RuntimeCoreHooks {
    pub fn emit_hook_fired(&self, name: &str, action: &str) { (self.telemetry.hook_fired)(name, action); }
    pub fn emit_tool_call(&self, tool: &str, count: u32, blocked: bool) { (self.telemetry.tool_call)(tool, count, blocked); }
    pub fn emit_route_decision(&self, query: &str, decision: &Value, reroute: bool) { (self.telemetry.route_decision)(query, decision, reroute); }
    pub fn emit_prediction_outcome(&self, task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize) { (self.telemetry.prediction_outcome)(task_id, checks_summary, verification_status, checks_count); }
    pub fn emit_rfv_round(&self, round: u32, verdict: &str) { (self.telemetry.rfv_round)(round, verdict); }
    pub fn host_provider_strict_pre_tool_fallback_hint(&self, host_id: &str) -> Option<bool> { (self.host_provider.strict_pre_tool_fallback_hint)(host_id) }
    pub fn host_provider_for_routing_spelling(&self, host_id: Option<&str>) -> Option<&'static str> { (self.host_provider.for_routing_spelling)(host_id) }
    pub fn default_host_id(&self) -> &'static str { (self.host_provider.default_id)() }
    pub fn host_provider_registry(&self) -> Vec<(&'static str, Option<&'static str>)> { (self.host_provider.registry)() }
    pub fn framework_goal_drive(&self, payload: Value) -> Result<Value, String> { (self.framework_goal_drive)(payload) }
    pub fn framework_quality_gate(&self, payload: Value) -> Result<Value, String> { (self.framework_quality_gate)(payload) }
    pub fn handle_session_supervisor_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_session_supervisor_operation)(payload) }
    pub fn handle_background_state_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_background_state_operation)(payload) }
    pub fn runtime_concurrency_defaults_payload(&self) -> Value { (self.runtime_concurrency_defaults_payload)() }
    pub fn eval_route_contract(&self) -> Value { (self.eval_route_contract)() }
    pub fn run_eval_route(&self, cases_path: &Path, runtime: Option<&Path>, manifest: Option<&Path>) -> Result<Value, String> { (self.run_eval_route)(cases_path, runtime, manifest) }
    pub fn generated_artifacts_status_for_repo(&self, repo_root: &Path) -> Result<String, String> { (self.generated_artifacts_status_for_repo)(repo_root) }
    pub fn ensure_kernel_bootstrap(&self) { (self.ensure_kernel_bootstrap)() }
}

/// 所有需要回调到 `runtime-core` 的钩子。
///
/// 使用分组子结构体降低认知负荷（原 17 个扁平字段 → 5 组 + 8 独立字段）。
pub struct RuntimeCoreHooks {
    // ── 遥测 (5 fields → 1 group) ──
    pub telemetry: TelemetryHooks,

    // ── 宿主 (4 fields → 1 group) ──
    pub host_provider: HostProviderHooks,

    // ── Goal / RFV ──
    pub framework_goal_drive: fn(Value) -> Result<Value, String>,
    pub framework_quality_gate: fn(Value) -> Result<Value, String>,

    // ── Session / 后台 ──
    pub handle_session_supervisor_operation: fn(Value) -> Result<Value, String>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, String>,
    pub runtime_concurrency_defaults_payload: fn() -> Value,

    // ── 路由评估 ──
    pub eval_route_contract: fn() -> Value,
    #[allow(clippy::type_complexity)]
    pub run_eval_route: fn(cases_path: &Path, runtime: Option<&Path>, manifest: Option<&Path>) -> Result<Value, String>,

    // ── 诊断 ──
    pub generated_artifacts_status_for_repo: fn(repo_root: &Path) -> Result<String, String>,

    // ── 内核引导 ──
    pub ensure_kernel_bootstrap: fn(),
}
