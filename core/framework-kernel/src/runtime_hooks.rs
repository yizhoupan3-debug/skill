//! Runtime core hooks (migrated from framework-runtime-hooks crate, Wave 3b).
//!
//! Owns the `RuntimeCoreHooks` fn-pointer registry for cross-layer communication
//! between runtime-core and its consumers (fr-contracts, framework-extra).
//!
//! Placed in L0 (framework-kernel) to break circular deps — all consumers
//! already depend on framework-kernel.

use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

// ── Hook duplicate check (generic fn-pointer proxy) ──

type HookDuplicateCheckFn = fn(repo_root: &Path) -> Vec<String>;
static HOOK_DUPLICATE_CHECK: OnceLock<HookDuplicateCheckFn> = OnceLock::new();

pub fn register_hook_duplicate_check(f: HookDuplicateCheckFn) {
    HOOK_DUPLICATE_CHECK.set(f).ok();
}

pub fn check_hook_duplicates(repo_root: &Path) -> Vec<String> {
    match HOOK_DUPLICATE_CHECK.get() {
        Some(f) => f(repo_root),
        None => vec![],
    }
}

/// Get a reference to the registered hooks. Returns `None` if not yet registered —
/// callers should fall back to no-op/default behavior.
pub fn try_hooks() -> Option<&'static RuntimeCoreHooks> {
    RUNTIME_CORE_HOOKS.get()
}

static RUNTIME_CORE_HOOKS: OnceLock<RuntimeCoreHooks> = OnceLock::new();

/// Get the registered hooks. Must be called after `register()`.
///
/// # Panics
/// Panics if `register()` has not been called yet. Guaranteed by
/// `runtime_core::init_hooks()` initialization ordering.
pub fn hooks() -> &'static RuntimeCoreHooks {
    #[allow(clippy::expect_used)]
    RUNTIME_CORE_HOOKS
        .get()
        .expect("RuntimeCoreHooks not registered — call register() before use")
}

/// Register hooks. Only the first call takes effect; subsequent calls are silently ignored.
pub fn register(h: RuntimeCoreHooks) {
    RUNTIME_CORE_HOOKS.get_or_init(|| h);
}

// ── Telemetry hook group ──
pub struct TelemetryHooks {
    pub hook_fired: fn(hook_name: &str, action: &str),
    pub tool_call: fn(tool: &str, duration_ms: u64, success: bool),
    pub route_decision: fn(query: &str, decision: &Value, reroute: bool),
    pub prediction_outcome: fn(task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize),
    pub rfv_round: fn(round: u32, verdict: &str),
}

// ── Host provider hook group ──
pub struct HostProviderHooks {
    pub for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub registry: fn() -> Vec<(&'static str, Option<&'static str>)>,
}

// ── Method wrappers for function pointer calls ──
impl RuntimeCoreHooks {
    pub fn emit_hook_fired(&self, name: &str, action: &str) { (self.telemetry.hook_fired)(name, action); }
    pub fn emit_tool_call(&self, tool: &str, duration_ms: u64, success: bool) { (self.telemetry.tool_call)(tool, duration_ms, success); }
    pub fn emit_route_decision(&self, query: &str, decision: &Value, reroute: bool) { (self.telemetry.route_decision)(query, decision, reroute); }
    pub fn emit_prediction_outcome(&self, task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize) { (self.telemetry.prediction_outcome)(task_id, checks_summary, verification_status, checks_count); }
    pub fn emit_rfv_round(&self, round: u32, verdict: &str) { (self.telemetry.rfv_round)(round, verdict); }
    pub fn host_provider_strict_pre_tool_fallback_hint(&self, host_id: &str) -> Option<bool> { (self.host_provider.strict_pre_tool_fallback_hint)(host_id) }
    pub fn host_provider_for_routing_spelling(&self, host_id: Option<&str>) -> Option<&'static str> { (self.host_provider.for_routing_spelling)(host_id) }
    pub fn host_provider_registry(&self) -> Vec<(&'static str, Option<&'static str>)> { (self.host_provider.registry)() }
    pub fn framework_goal_drive(&self, payload: Value) -> Result<Value, String> { (self.framework_goal_drive)(payload) }
    pub fn handle_session_supervisor_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_session_supervisor_operation)(payload) }
    pub fn handle_background_state_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_background_state_operation)(payload) }
    pub fn runtime_concurrency_defaults_payload(&self) -> Value { (self.runtime_concurrency_defaults_payload)() }
    pub fn eval_route_contract(&self) -> Value { (self.eval_route_contract)() }
    pub fn run_eval_route(&self, cases_path: &Path, runtime: Option<&Path>) -> Result<Value, String> { (self.run_eval_route)(cases_path, runtime) }
    pub fn generated_artifacts_status_for_repo(&self, repo_root: &Path) -> Result<String, String> { (self.generated_artifacts_status_for_repo)(repo_root) }
    pub fn ensure_kernel_bootstrap(&self) { (self.ensure_kernel_bootstrap)() }
}

/// All hooks that require callbacks into runtime-core.
///
/// Uses grouped sub-structs to reduce cognitive load (17 flat fields → 5 groups + 8 independent fields).
pub struct RuntimeCoreHooks {
    // ── Telemetry (5 fields → 1 group) ──
    pub telemetry: TelemetryHooks,

    // ── Host (3 fields → 1 group) ──
    pub host_provider: HostProviderHooks,

    // ── Goal / RFV ──
    pub framework_goal_drive: fn(Value) -> Result<Value, String>,

    // ── Session / background ──
    pub handle_session_supervisor_operation: fn(Value) -> Result<Value, String>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, String>,
    pub runtime_concurrency_defaults_payload: fn() -> Value,

    // ── Route evaluation ──
    pub eval_route_contract: fn() -> Value,
    #[allow(clippy::type_complexity)]
    pub run_eval_route: fn(cases_path: &Path, runtime: Option<&Path>) -> Result<Value, String>,

    // ── Diagnostics ──
    pub generated_artifacts_status_for_repo: fn(repo_root: &Path) -> Result<String, String>,

    // ── Kernel bootstrap ──
    pub ensure_kernel_bootstrap: fn(),
}
