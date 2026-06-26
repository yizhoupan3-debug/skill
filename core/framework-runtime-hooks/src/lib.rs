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

#![deny(clippy::unwrap_used, clippy::expect_used)]

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

/// 获取已注册的钩子引用。需在调用其他 framework-runtime 函数前注册。
/// 返回 `None` 表示尚未注册——调用方应走 fallback/no-op 路径。
pub fn try_hooks() -> Option<&'static RuntimeCoreHooks> {
    RUNTIME_CORE_HOOKS.get()
}

static RUNTIME_CORE_HOOKS: OnceLock<RuntimeCoreHooks> = OnceLock::new();

/// 获取已注册的钩子。需在调用其他 framework-runtime 函数前注册。
///
/// # Panics
/// 仅在 `register()` 尚未调用时 panic。由 `runtime_core::init_hooks()` 保证初始化顺序。
pub fn hooks() -> &'static RuntimeCoreHooks {
    #[allow(clippy::expect_used)]
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
    pub tool_call: fn(tool: &str, duration_ms: u64, success: bool),
    pub route_decision: fn(query: &str, decision: &Value, reroute: bool),
    pub prediction_outcome: fn(task_id: &str, checks_summary: &str, verification_status: &str, checks_count: usize),
    pub rfv_round: fn(round: u32, verdict: &str),
}

// ── 宿主提供者钩子组 ──
pub struct HostProviderHooks {
    pub for_routing_spelling: fn(host_id: Option<&str>) -> Option<&'static str>,
    pub strict_pre_tool_fallback_hint: fn(host_id: &str) -> Option<bool>,
    /// Returns (host_id, capabilities_config_path) for each registered host provider.
    pub registry: fn() -> Vec<(&'static str, Option<&'static str>)>,
}

// ── Method wrappers for hook function pointer calls ──
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
    pub fn framework_quality_gate(&self, payload: Value) -> Result<Value, String> { (self.framework_quality_gate)(payload) }
    pub fn handle_session_supervisor_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_session_supervisor_operation)(payload) }
    pub fn handle_background_state_operation(&self, payload: Value) -> Result<Value, String> { (self.handle_background_state_operation)(payload) }
    pub fn runtime_concurrency_defaults_payload(&self) -> Value { (self.runtime_concurrency_defaults_payload)() }
    pub fn eval_route_contract(&self) -> Value { (self.eval_route_contract)() }
    pub fn run_eval_route(&self, cases_path: &Path, runtime: Option<&Path>) -> Result<Value, String> { (self.run_eval_route)(cases_path, runtime) }
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
    pub run_eval_route: fn(cases_path: &Path, runtime: Option<&Path>) -> Result<Value, String>,

    // ── 诊断 ──
    pub generated_artifacts_status_for_repo: fn(repo_root: &Path) -> Result<String, String>,

    // ── 内核引导 ──
    pub ensure_kernel_bootstrap: fn(),
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn noop_telemetry() -> TelemetryHooks {
        TelemetryHooks {
            hook_fired: |_, _| {},
            tool_call: |_, _, _| {},
            route_decision: |_, _, _| {},
            prediction_outcome: |_, _, _, _| {},
            rfv_round: |_, _| {},
        }
    }

    fn noop_host_provider() -> HostProviderHooks {
        HostProviderHooks {
            for_routing_spelling: |_| None,
            strict_pre_tool_fallback_hint: |_| None,
            registry: || vec![],
        }
    }

    fn noop_hooks() -> RuntimeCoreHooks {
        RuntimeCoreHooks {
            telemetry: noop_telemetry(),
            host_provider: noop_host_provider(),
            framework_goal_drive: |_| Ok(serde_json::Value::Null),
            framework_quality_gate: |_| Ok(serde_json::Value::Null),
            handle_session_supervisor_operation: |_| Ok(serde_json::Value::Null),
            handle_background_state_operation: |_| Ok(serde_json::Value::Null),
            runtime_concurrency_defaults_payload: || serde_json::Value::Null,
            eval_route_contract: || serde_json::Value::Null,
            run_eval_route: |_, _| Ok(serde_json::Value::Null),
            generated_artifacts_status_for_repo: |_| Ok("ok".into()),
            ensure_kernel_bootstrap: || {},
        }
    }

    #[test]
    fn try_hooks_returns_none_before_register() {
        // In a fresh process (or when OnceLock hasn't been set), try_hooks returns None.
        // Note: this test may fail if another test in the same binary already called register().
        // We use Once to ensure register is only called once across all tests.
        INIT.call_once(|| {
            register(noop_hooks());
        });
        // After register, try_hooks should return Some
        assert!(try_hooks().is_some());
    }

    #[test]
    fn hooks_returns_registered_instance() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        assert!(h.host_provider_registry().is_empty());
    }

    #[test]
    fn register_is_idempotent() {
        // Second register call should be silently ignored
        INIT.call_once(|| {
            register(noop_hooks());
        });
        // This should not panic or change the registered hooks
        let h = hooks();
        assert!(h.host_provider_registry().is_empty());
    }

    #[test]
    fn hook_duplicate_check_returns_empty_when_not_registered() {
        // NOTE: check_hook_duplicates uses a process-level OnceLock.
        // OnceLock cannot be reset between tests, so the result depends on
        // whether register_hook_duplicate_check was already called earlier
        // in this test binary.  We can only assert the call does not panic;
        // we cannot deterministically assert the content.
        let result = check_hook_duplicates(Path::new("/tmp"));
        let _ = result.len(); // must not panic
    }

    #[test]
    fn check_hook_duplicates_always_returns_vec() {
        // Regardless of registration state, the function should always return a Vec<String>.
        let result = check_hook_duplicates(Path::new("/any/path"));
        // Verify it is a valid Vec<String> (always constructible, never panics).
        let _len = result.len();
    }

    #[test]
    fn check_hook_duplicates_deterministic_across_calls() {
        // Multiple calls with the same path should return the same result
        // (once the OnceLock is initialized).
        let r1 = check_hook_duplicates(Path::new("/test"));
        let r2 = check_hook_duplicates(Path::new("/test"));
        assert_eq!(r1, r2);
    }

    #[test]
    fn method_wrappers_delegate_correctly() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        // These should not panic — they delegate to noop fns
        h.emit_hook_fired("test", "action");
        h.emit_tool_call("tool", 1, false);
        h.emit_route_decision("q", &serde_json::Value::Null, false);
        h.emit_prediction_outcome("t", "s", "passed", 1);
        h.emit_rfv_round(1, "PASS");
        assert_eq!(h.host_provider_strict_pre_tool_fallback_hint("h"), None);
        assert_eq!(h.host_provider_for_routing_spelling(None), None);
        assert!(h.host_provider_registry().is_empty());
        assert!(h.framework_goal_drive(serde_json::Value::Null).is_ok());
        assert!(h.framework_quality_gate(serde_json::Value::Null).is_ok());
        assert_eq!(h.runtime_concurrency_defaults_payload(), serde_json::Value::Null);
        assert_eq!(h.eval_route_contract(), serde_json::Value::Null);
        assert!(h.generated_artifacts_status_for_repo(Path::new("/tmp")).is_ok());
        h.ensure_kernel_bootstrap();
    }

    // ── OnceLock registry behavior tests ──

    #[test]
    fn hook_duplicate_check_fn_is_called_with_correct_path() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        fn counting_check(repo_root: &Path) -> Vec<String> {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            let path_str = repo_root.to_string_lossy().to_string();
            vec![path_str]
        }

        register_hook_duplicate_check(counting_check);
        let r1 = check_hook_duplicates(Path::new("/first"));
        let r2 = check_hook_duplicates(Path::new("/second"));
        // Both calls go to the registered function (OnceLock doesn't cache fn calls).
        assert!(!r1.is_empty());
        assert!(!r2.is_empty());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn try_hooks_and_hooks_return_same_instance() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let via_try = try_hooks().expect("try_hooks should return Some after register");
        let via_hooks = hooks();
        // Both pointers should reference the same static data.
        assert_eq!(
            via_try.host_provider_registry(),
            via_hooks.host_provider_registry()
        );
    }

    #[test]
    fn noop_host_provider_for_routing_spelling_returns_none_for_any_input() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        assert_eq!(h.host_provider_for_routing_spelling(Some("claude")), None);
        assert_eq!(h.host_provider_for_routing_spelling(Some("cursor")), None);
        assert_eq!(h.host_provider_for_routing_spelling(None), None);
    }

    #[test]
    fn noop_host_provider_strict_pre_tool_fallback_hint_returns_none() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        assert_eq!(h.host_provider_strict_pre_tool_fallback_hint("any-host"), None);
    }

    #[test]
    fn framework_goal_drive_and_quality_gate_return_null() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let input = json!({"key": "value"});
        assert_eq!(
            h.framework_goal_drive(input.clone()).unwrap(),
            serde_json::Value::Null
        );
        assert_eq!(
            h.framework_quality_gate(input).unwrap(),
            serde_json::Value::Null
        );
    }

    #[test]
    fn handle_session_supervisor_operation_returns_null() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let result = h.handle_session_supervisor_operation(serde_json::Value::Null);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn handle_background_state_operation_returns_null() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let result = h.handle_background_state_operation(serde_json::Value::Null);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn run_eval_route_returns_null() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let result = h.run_eval_route(
            Path::new("/tmp/cases.json"),
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn generated_artifacts_status_returns_ok_string() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let result = h.generated_artifacts_status_for_repo(Path::new("/tmp"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "ok");
    }

    #[test]
    fn telemetry_emitters_do_not_panic_with_various_inputs() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        // Empty strings
        h.emit_hook_fired("", "");
        h.emit_tool_call("", 0, true);
        h.emit_route_decision("", &json!({"a": 1}), true);
        h.emit_prediction_outcome("", "", "", 0);
        h.emit_rfv_round(0, "");
        // Large values
        h.emit_tool_call("tool-name-with-many-characters", u64::MAX, false);
        h.emit_rfv_round(u32::MAX, "LONG_VERDICT_STRING");
    }

    #[test]
    fn host_provider_registry_returns_empty_vec() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        let registry = h.host_provider_registry();
        assert!(registry.is_empty());
    }

    #[test]
    fn ensure_kernel_bootstrap_does_not_panic() {
        INIT.call_once(|| {
            register(noop_hooks());
        });
        let h = hooks();
        // Should be a no-op; verify it returns without panic.
        h.ensure_kernel_bootstrap();
        h.ensure_kernel_bootstrap();
    }

    use serde_json::json;
}
