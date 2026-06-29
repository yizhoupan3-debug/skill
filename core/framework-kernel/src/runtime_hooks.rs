//! Runtime core hooks (migrated from framework-runtime-hooks crate, Wave 3b).
//!
//! Owns the `RuntimeCoreHooks` fn-pointer registry for cross-layer communication
//! between runtime-core and its consumers (fr-contracts, framework-extra).
//!
//! Placed in L0 (framework-kernel) to break circular deps — all consumers
//! already depend on framework-kernel.

use core_errors::FrameworkError;
use serde_json::Value;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

// ── Hook duplicate check ──

static HOOK_DUPLICATE_CHECK: OnceLock<bool> = OnceLock::new();

pub fn check_hook_duplicates(_repo_root: &Path) -> Vec<String> {
    let found = HOOK_DUPLICATE_CHECK.get().copied().unwrap_or(false);
    if found {
        vec!["RuntimeCoreHooks: register() called more than once".to_string()]
    } else {
        vec![]
    }
}

static RUNTIME_CORE_HOOKS: RwLock<Option<RuntimeCoreHooks>> = RwLock::new(None);

/// Get the registered RuntimeCoreHooks, panicking if not initialized.
/// Prefer `try_hooks()` which returns `Option` for graceful handling.
///
/// # Panics
/// Panics if `init_hooks()` (or `register()`) has not been called before this function is invoked.
/// There is no compile-time check enforcing this ordering — incorrect initialization will cause a panic.
#[deprecated(note = "use try_hooks() instead for non-panicking access")]
#[track_caller]
pub fn hooks() -> RuntimeCoreHooks {
    RUNTIME_CORE_HOOKS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| panic!("RuntimeCoreHooks not registered (call register() before use)"))
}

/// Try to get registered hooks without panicking.
/// Returns `None` if `register()` has not been called yet.
pub fn try_hooks() -> Option<RuntimeCoreHooks> {
    RUNTIME_CORE_HOOKS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Register hooks. Only the first call takes effect; subsequent calls are logged as warnings.
pub fn register(h: RuntimeCoreHooks) {
    let mut guard = RUNTIME_CORE_HOOKS
        .write()
        .unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        let _ = HOOK_DUPLICATE_CHECK.set(true);
        tracing::warn!("RuntimeCoreHooks already registered — overwriting");
    }
    *guard = Some(h);
}

/// Reset hook state for test isolation.
///
/// Thread-safe via `RwLock` — no `unsafe` required.
#[cfg(test)]
pub fn unregister_hooks() {
    *RUNTIME_CORE_HOOKS
        .write()
        .unwrap_or_else(|e| e.into_inner()) = None;
}

// ── Host provider hook group ──
#[derive(Clone)]
pub struct HostProviderHooks {
    pub for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub registry: fn() -> Vec<(&'static str, Option<&'static str>)>,
}

// ── Method wrappers for function pointer calls ──
impl RuntimeCoreHooks {
    pub fn host_provider_strict_pre_tool_fallback_hint(&self, host_id: &str) -> Option<bool> {
        (self.host_provider.strict_pre_tool_fallback_hint)(host_id)
    }
    pub fn host_provider_for_routing_spelling(
        &self,
        host_id: Option<&str>,
    ) -> Option<&'static str> {
        (self.host_provider.for_routing_spelling)(host_id)
    }
    pub fn host_provider_registry(&self) -> Vec<(&'static str, Option<&'static str>)> {
        (self.host_provider.registry)()
    }
    pub fn framework_goal_drive(&self, payload: Value) -> Result<Value, FrameworkError> {
        (self.framework_goal_drive)(payload)
    }
    pub fn handle_orchestrator_operation(&self, payload: Value) -> Result<Value, FrameworkError> {
        (self.handle_orchestrator_operation)(payload)
    }
    pub fn handle_background_state_operation(
        &self,
        payload: Value,
    ) -> Result<Value, FrameworkError> {
        (self.handle_background_state_operation)(payload)
    }
    pub fn runtime_concurrency_defaults_payload(&self) -> Result<Value, FrameworkError> {
        (self.runtime_concurrency_defaults_payload)()
    }
    pub fn eval_route_contract(&self) -> Value {
        (self.eval_route_contract)()
    }
    pub fn run_eval_route(
        &self,
        cases_path: &Path,
        runtime: Option<&Path>,
    ) -> Result<Value, FrameworkError> {
        (self.run_eval_route)(cases_path, runtime)
    }
    pub fn generated_artifacts_status_for_repo(
        &self,
        repo_root: &Path,
    ) -> Result<String, FrameworkError> {
        (self.generated_artifacts_status_for_repo)(repo_root)
    }
    pub fn ensure_kernel_bootstrap(&self) {
        (self.ensure_kernel_bootstrap)()
    }

    pub fn evaluate_quality_gate(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, core_errors::FrameworkError> {
        (self.evaluate_quality_gate)(payload)
    }

    pub fn evaluate_closeout_gate(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, core_errors::FrameworkError> {
        (self.evaluate_closeout_gate)(payload)
    }
}

/// All hooks that require callbacks into runtime-core.
///
/// Uses grouped sub-structs to reduce cognitive load.
#[derive(Clone)]
pub struct RuntimeCoreHooks {
    // ── Host (3 fields → 1 group) ──
    pub host_provider: HostProviderHooks,

    // ── Goal / RFV ──
    pub framework_goal_drive: fn(Value) -> Result<Value, FrameworkError>,

    // ── Session / background ──
    pub handle_orchestrator_operation: fn(Value) -> Result<Value, FrameworkError>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, FrameworkError>,
    pub runtime_concurrency_defaults_payload: fn() -> Result<Value, FrameworkError>,

    // ── Route evaluation ──
    pub eval_route_contract: fn() -> Value,
    #[allow(clippy::type_complexity)]
    pub run_eval_route:
        fn(cases_path: &Path, runtime: Option<&Path>) -> Result<Value, FrameworkError>,

    // ── Diagnostics ──
    pub generated_artifacts_status_for_repo: fn(repo_root: &Path) -> Result<String, FrameworkError>,

    // ── Kernel bootstrap ──
    pub ensure_kernel_bootstrap: fn(),

    // ── Quality Gate evaluation (wraps qg_entry::trigger) ──
    /// Payload: { repo_root: String, task_id: String, scene: String, goal: String,
    ///            sub_scene: Option<String>, round: u64, output_data: Option<Value> }
    /// Returns: GateVerdict as Value { passed: bool, blockers: [...], advisories: [...] }
    pub evaluate_quality_gate:
        fn(serde_json::Value) -> Result<serde_json::Value, core_errors::FrameworkError>,

    // ── Closeout Gate evaluation (wraps closeout_gate_evaluate) ──
    /// Payload: { repo_root: String, task_id: String, host_id: String }
    /// Returns: { result: String, passed: bool, findings: Vec<String> }
    pub evaluate_closeout_gate:
        fn(serde_json::Value) -> Result<serde_json::Value, core_errors::FrameworkError>,
}
