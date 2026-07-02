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

/// Builder for `RuntimeCoreHooks` — enforces all fields are set at compile time.
pub struct RuntimeCoreHooksBuilder {
    host_provider: Option<HostProviderHooks>,
    framework_goal_drive: Option<fn(Value) -> Result<Value, FrameworkError>>,
    handle_orchestrator_operation: Option<fn(Value) -> Result<Value, FrameworkError>>,
    handle_background_state_operation: Option<fn(Value) -> Result<Value, FrameworkError>>,
    runtime_concurrency_defaults_payload: Option<fn() -> Result<Value, FrameworkError>>,
    ensure_kernel_bootstrap: Option<fn()>,
    evaluate_quality_gate: Option<fn(serde_json::Value) -> Result<serde_json::Value, FrameworkError>>,
    evaluate_closeout_gate: Option<fn(serde_json::Value) -> Result<serde_json::Value, FrameworkError>>,
}

impl Default for RuntimeCoreHooksBuilder {
    fn default() -> Self { Self::new() }
}

impl RuntimeCoreHooksBuilder {
    pub fn new() -> Self {
        Self {
            host_provider: None,
            framework_goal_drive: None,
            handle_orchestrator_operation: None,
            handle_background_state_operation: None,
            runtime_concurrency_defaults_payload: None,
            ensure_kernel_bootstrap: None,
            evaluate_quality_gate: None,
            evaluate_closeout_gate: None,
        }
    }

    pub fn host_provider(mut self, v: HostProviderHooks) -> Self { self.host_provider = Some(v); self }
    pub fn framework_goal_drive(mut self, v: fn(Value) -> Result<Value, FrameworkError>) -> Self { self.framework_goal_drive = Some(v); self }
    pub fn handle_orchestrator_operation(mut self, v: fn(Value) -> Result<Value, FrameworkError>) -> Self { self.handle_orchestrator_operation = Some(v); self }
    pub fn handle_background_state_operation(mut self, v: fn(Value) -> Result<Value, FrameworkError>) -> Self { self.handle_background_state_operation = Some(v); self }
    pub fn runtime_concurrency_defaults_payload(mut self, v: fn() -> Result<Value, FrameworkError>) -> Self { self.runtime_concurrency_defaults_payload = Some(v); self }
    pub fn ensure_kernel_bootstrap(mut self, v: fn()) -> Self { self.ensure_kernel_bootstrap = Some(v); self }
    pub fn evaluate_quality_gate(mut self, v: fn(serde_json::Value) -> Result<serde_json::Value, FrameworkError>) -> Self { self.evaluate_quality_gate = Some(v); self }
    pub fn evaluate_closeout_gate(mut self, v: fn(serde_json::Value) -> Result<serde_json::Value, FrameworkError>) -> Self { self.evaluate_closeout_gate = Some(v); self }

    /// Pre-filled builder for tests — all fields are stub fns.
    pub fn for_testing() -> Self {
        fn stub(_: Value) -> Result<Value, FrameworkError> { Ok(Value::Null) }
        fn stub2() -> Result<Value, FrameworkError> { Ok(Value::Null) }
        fn stub3() {}
        fn stub4(_: serde_json::Value) -> Result<serde_json::Value, FrameworkError> { Ok(serde_json::Value::Null) }
        Self {
            host_provider: Some(HostProviderHooks {
                for_routing_spelling: |_| None,
                strict_pre_tool_fallback_hint: |_| None,
                registry: || vec![],
            }),
            framework_goal_drive: Some(stub),
            handle_orchestrator_operation: Some(stub),
            handle_background_state_operation: Some(stub),
            runtime_concurrency_defaults_payload: Some(stub2),
            ensure_kernel_bootstrap: Some(stub3),
            evaluate_quality_gate: Some(stub4),
            evaluate_closeout_gate: Some(stub4),
        }
    }

    /// Build — returns Err if any field is unset.
    pub fn build(self) -> Result<RuntimeCoreHooks, String> {
        Ok(RuntimeCoreHooks {
            host_provider: self.host_provider.ok_or("host_provider not set")?,
            framework_goal_drive: self.framework_goal_drive.ok_or("framework_goal_drive not set")?,
            handle_orchestrator_operation: self.handle_orchestrator_operation.ok_or("handle_orchestrator_operation not set")?,
            handle_background_state_operation: self.handle_background_state_operation.ok_or("handle_background_state_operation not set")?,
            runtime_concurrency_defaults_payload: self.runtime_concurrency_defaults_payload.ok_or("runtime_concurrency_defaults_payload not set")?,
            ensure_kernel_bootstrap: self.ensure_kernel_bootstrap.ok_or("ensure_kernel_bootstrap not set")?,
            evaluate_quality_gate: self.evaluate_quality_gate.ok_or("evaluate_quality_gate not set")?,
            evaluate_closeout_gate: self.evaluate_closeout_gate.ok_or("evaluate_closeout_gate not set")?,
        })
    }
}

/// All hooks that require callbacks into runtime-core.
///
/// Uses grouped sub-structs to reduce cognitive load.
#[derive(Clone)]
pub struct RuntimeCoreHooks {
    // ── Host (3 fields → 1 group) ──
    pub host_provider: HostProviderHooks,

    // ── Goal / Quality Gate ──
    pub framework_goal_drive: fn(Value) -> Result<Value, FrameworkError>,

    // ── Session / background ──
    pub handle_orchestrator_operation: fn(Value) -> Result<Value, FrameworkError>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, FrameworkError>,
    pub runtime_concurrency_defaults_payload: fn() -> Result<Value, FrameworkError>,

    // ── Kernel bootstrap ──
    //
    // NOTE [FNH-08/FNH-09]: Two hooks registration centers share this
    // fn-pointer: framework-kernel's static `RuntimeCoreHooks` registry
    // (this struct) and runtime-core's `ensure_kernel_bootstrap()`. Both
    // reference the same underlying `RegisterKernelBootstrap` logic via
    // the hook registration path. Keep the two registration sites
    // consistent — init_hooks() in runtime-core must populate this field.
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
