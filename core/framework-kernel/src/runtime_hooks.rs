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

// ── Host provider hook group ──
pub struct HostProviderHooks {
    pub for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub registry: fn() -> Vec<(&'static str, Option<&'static str>)>,
}

// ── Method wrappers for function pointer calls ──
impl RuntimeCoreHooks {
    pub fn host_provider_strict_pre_tool_fallback_hint(&self, host_id: &str) -> Option<bool> { (self.host_provider.strict_pre_tool_fallback_hint)(host_id) }
    pub fn host_provider_for_routing_spelling(&self, host_id: Option<&str>) -> Option<&'static str> { (self.host_provider.for_routing_spelling)(host_id) }
    pub fn host_provider_registry(&self) -> Vec<(&'static str, Option<&'static str>)> { (self.host_provider.registry)() }
    pub fn framework_goal_drive(&self, payload: Value) -> Result<Value, String> { (self.framework_goal_drive)(payload) }
    pub fn handle_orchestrator_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_orchestrator_operation)(payload) }
    pub fn handle_background_state_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_background_state_operation)(payload) }
    pub fn runtime_concurrency_defaults_payload(&self) -> Value { (self.runtime_concurrency_defaults_payload)() }
    pub fn eval_route_contract(&self) -> Value { (self.eval_route_contract)() }
    pub fn run_eval_route(&self, cases_path: &Path, runtime: Option<&Path>) -> Result<Value, String> { (self.run_eval_route)(cases_path, runtime) }
    pub fn generated_artifacts_status_for_repo(&self, repo_root: &Path) -> Result<String, String> { (self.generated_artifacts_status_for_repo)(repo_root) }
    pub fn ensure_kernel_bootstrap(&self) { (self.ensure_kernel_bootstrap)() }
}

/// All hooks that require callbacks into runtime-core.
///
/// Uses grouped sub-structs to reduce cognitive load.
pub struct RuntimeCoreHooks {
    // ── Host (3 fields → 1 group) ──
    pub host_provider: HostProviderHooks,

    // ── Goal / RFV ──
    pub framework_goal_drive: fn(Value) -> Result<Value, String>,

    // ── Session / background ──
    pub handle_orchestrator_operation: fn(Value) -> Result<Value, String>,
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
