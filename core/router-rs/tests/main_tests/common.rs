use super::*;
pub(super) use crate::integration_test_prelude::*;

pub(super) use serde_json::{json, Map, Value};

pub(super) use std::collections::HashSet;
pub(super) use std::fs;
pub(super) use std::panic::{catch_unwind, AssertUnwindSafe};
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::sync::{Arc, Mutex, OnceLock};
pub(super) use std::thread::{sleep, spawn};
pub(super) use std::time::{Duration, SystemTime, UNIX_EPOCH};


// Re-exports from lib.rs test-only scope (these were available via `use super::*`
// when this file lived at the crate-root test module level).
pub(super) use crate::cli::route_task_with_manifest_fallback;
pub(super) use crate::cli::args::{
    BackgroundControlRequestPayload, SandboxControlRequestPayload,
    TraceStreamInspectRequestPayload, TraceStreamReplayRequestPayload,
    RouterCommand, HostCommand, CodexSubcommand,
};
pub(super) use crate::cli::Cli;
pub(super) use crate::execution_contract::{
    EXECUTION_KERNEL_AUTHORITY, EXECUTION_KERNEL_FALLBACK_POLICY, EXECUTION_KERNEL_KIND,
    EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION, EXECUTION_METADATA_SCHEMA_VERSION,
    EXECUTION_PROMPT_PREVIEW_OWNER,
};
pub(super) use crate::framework_runtime::FRAMEWORK_ALIAS_SCHEMA_VERSION;
pub(super) use crate::route::ROUTE_REPORT_SCHEMA_VERSION;
pub(super) use crate::hook_status;

pub(super) fn execution_kernel_contract_shape_fields(shape: &Value) -> Vec<String> {
    let object = shape.as_object().expect("contract shape object");
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort_unstable();
    keys
}

pub(super) fn background_control_request_defaults() -> BackgroundControlRequestPayload {
    BackgroundControlRequestPayload {
        schema_version: String::new(),
        operation: String::new(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        requested_parallel_group_id: None,
        request_parallel_group_ids: None,
        request_lane_ids: None,
        lane_id_prefix: None,
        batch_size: None,
    }
}

pub(super) fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_route_fixtures.json")
}

pub(super) fn routing_eval_case_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_eval_cases.json")
}

pub(super) fn assert_routing_eval_cases_match<F>(label: &str, mut route_case: F)
where
    F: FnMut(&str, &str, bool, Option<&str>) -> Result<RouteDecision, String>,
{
    let payload = read_json(&routing_eval_case_path()).expect("read routing eval fixture");
    let cases = payload
        .get("cases")
        .and_then(Value::as_array)
        .expect("routing eval cases array");
    let mut failures = Vec::new();

    for (index, case) in cases.iter().enumerate() {
        let id = case
            .get("id")
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| (index + 1).to_string());
        let task = case
            .get("task")
            .and_then(Value::as_str)
            .expect("routing eval task");
        let first_turn = case
            .get("first_turn")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let host_id = case
            .get("host_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let decision = route_case(
            task,
            &format!("routing-eval::{label}::{id}"),
            first_turn,
            host_id,
        )
        .unwrap_or_else(|err| panic!("route eval {label}/{id} failed: {err}"));

        if let Some(expected_owner) = case.get("expected_owner").and_then(Value::as_str) {
            if decision.selected_skill != expected_owner {
                failures.push(format!(
                    "{id}: expected owner {expected_owner}, got {} (score {})",
                    decision.selected_skill, decision.score
                ));
            }
        }

        let expected_overlay = case
            .get("expected_overlay")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
        if decision.overlay_skill != expected_overlay {
            failures.push(format!(
                "{id}: expected overlay {:?}, got {:?} (owner {}, score {})",
                expected_overlay, decision.overlay_skill, decision.selected_skill, decision.score
            ));
        }

        if case
            .get("forbidden_owners")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|forbidden| forbidden == decision.selected_skill)
            })
            .unwrap_or(false)
        {
            failures.push(format!(
                "{id}: selected forbidden owner {} (score {})",
                decision.selected_skill, decision.score
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{label} routing eval strict failures:\n{}",
        failures.join("\n")
    );
}

pub(super) fn sample_execute_request() -> ExecuteRequestPayload {
    ExecuteRequestPayload {
        schema_version: "router-rs-execute-request-v1".to_string(),
        task: "帮我继续推进 Rust kernel".to_string(),
        session_id: "execute-session".to_string(),
        user_id: "tester".to_string(),
        selected_skill: "autopilot".to_string(),
        overlay_skill: None,
        layer: "L2".to_string(),
        route_engine: Some("rust".to_string()),
        diagnostic_route_mode: Some("none".to_string()),
        reasons: vec!["Trigger phrase matched: 直接做代码.".to_string()],
        prompt_preview: Some("Keep the kernel Rust-first.".to_string()),
        dry_run: true,
        trace_event_count: 6,
        trace_output_path: Some("/tmp/TRACE_METADATA.json".to_string()),
        default_output_tokens: 512,
        research_mode: None,
        execution_protocol: None,
        verification_required: None,
        evidence_required: None,
        model_id: "gpt-5.4".to_string(),
        aggregator_base_url: "http://127.0.0.1:20128/v1".to_string(),
        aggregator_api_key: "test-key".to_string(),
    }
}

pub(super) fn temp_trace_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.jsonl"))
}

pub(super) fn execute_allowlist_env_lock() -> &'static Mutex<()> {
    static EXECUTE_ALLOWLIST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    EXECUTE_ALLOWLIST_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn with_execute_allowlist_env<F>(value: Option<&str>, test_fn: F)
where
    F: FnOnce(),
{
    let _guard = execute_allowlist_env_lock()
        .lock()
        .expect("execute allowlist env lock poisoned");
    let previous = std::env::var_os(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV);

    match value {
        Some(raw) => unsafe { std::env::set_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV, raw) },
        None => unsafe { std::env::remove_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV) },
    }

    let outcome = catch_unwind(AssertUnwindSafe(test_fn));

    match previous {
        Some(raw) => unsafe { std::env::set_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV, raw) },
        None => unsafe { std::env::remove_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV) },
    }

    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}

/// 测试进程内 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 为全局环境变量；并行测试会互相干扰，需串行。
pub(super) struct CloseoutStrictEnvGuard {
    prior: Option<String>,
}

impl CloseoutStrictEnvGuard {
    pub(super) fn new() -> Self {
        let prior = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        // 显式开启硬门禁：本地默认已改为「未设置则软」，测试必须不依赖全局 CI 变量。
        unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1") };
        Self { prior }
    }
}

impl Drop for CloseoutStrictEnvGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
        }
    }
}

/// 模拟「CI 检测为真 + 未设置 ROUTER_RS_CLOSEOUT_ENFORCEMENT」：应走硬门禁（与显式 `=1` 路径回归等价）。
/// 故意 `remove_var("GITHUB_ACTIONS")`，单独覆盖实现里 `CI` 分支（`in_ci_like_environment` 为 OR）。
pub(super) struct CiHardUnsetCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl CiHardUnsetCloseoutEnvGuard {
    pub(super) fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        unsafe { std::env::set_var("CI", "true") };
        unsafe { std::env::remove_var("GITHUB_ACTIONS") };
        unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") };
        Self {
            prior_ci,
            prior_github_actions,
            prior_closeout,
        }
    }
}

impl Drop for CiHardUnsetCloseoutEnvGuard {
    fn drop(&mut self) {
        match &self.prior_ci {
            Some(v) => unsafe { std::env::set_var("CI", v) },
            None => unsafe { std::env::remove_var("CI") },
        }
        match &self.prior_github_actions {
            Some(v) => unsafe { std::env::set_var("GITHUB_ACTIONS", v) },
            None => unsafe { std::env::remove_var("GITHUB_ACTIONS") },
        }
        match &self.prior_closeout {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
        }
    }
}

/// 模拟「GitHub Actions 检测为真 + 未设置 ROUTER_RS_CLOSEOUT_ENFORCEMENT」：覆盖 `GITHUB_ACTIONS=true` 分支。
/// 故意清除 `CI`，单独验证 Actions 路径。
pub(super) struct GithubActionsHardUnsetCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl GithubActionsHardUnsetCloseoutEnvGuard {
    pub(super) fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        unsafe { std::env::remove_var("CI") };
        unsafe { std::env::set_var("GITHUB_ACTIONS", "true") };
        unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") };
        Self {
            prior_ci,
            prior_github_actions,
            prior_closeout,
        }
    }
}

impl Drop for GithubActionsHardUnsetCloseoutEnvGuard {
    fn drop(&mut self) {
        match &self.prior_ci {
            Some(v) => unsafe { std::env::set_var("CI", v) },
            None => unsafe { std::env::remove_var("CI") },
        }
        match &self.prior_github_actions {
            Some(v) => unsafe { std::env::set_var("GITHUB_ACTIONS", v) },
            None => unsafe { std::env::remove_var("GITHUB_ACTIONS") },
        }
        match &self.prior_closeout {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
        }
    }
}

/// `CI=true` 且显式 `ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`：程序化门禁关闭应优先于 CI 检测。
pub(super) struct CiWithCloseoutDisabledEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl CiWithCloseoutDisabledEnvGuard {
    pub(super) fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        unsafe { std::env::set_var("CI", "true") };
        unsafe { std::env::remove_var("GITHUB_ACTIONS") };
        unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "0") };
        Self {
            prior_ci,
            prior_github_actions,
            prior_closeout,
        }
    }
}

impl Drop for CiWithCloseoutDisabledEnvGuard {
    fn drop(&mut self) {
        match &self.prior_ci {
            Some(v) => unsafe { std::env::set_var("CI", v) },
            None => unsafe { std::env::remove_var("CI") },
        }
        match &self.prior_github_actions {
            Some(v) => unsafe { std::env::set_var("GITHUB_ACTIONS", v) },
            None => unsafe { std::env::remove_var("GITHUB_ACTIONS") },
        }
        match &self.prior_closeout {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
        }
    }
}

/// 本地非 CI，但 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 设为空字符串：视为「已设置」且非软关断 token → 硬门禁。
pub(super) struct LocalNonCiEmptyCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl LocalNonCiEmptyCloseoutEnvGuard {
    pub(super) fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        unsafe { std::env::remove_var("CI") };
        unsafe { std::env::remove_var("GITHUB_ACTIONS") };
        unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "") };
        Self {
            prior_ci,
            prior_github_actions,
            prior_closeout,
        }
    }
}

impl Drop for LocalNonCiEmptyCloseoutEnvGuard {
    fn drop(&mut self) {
        match &self.prior_ci {
            Some(v) => unsafe { std::env::set_var("CI", v) },
            None => unsafe { std::env::remove_var("CI") },
        }
        match &self.prior_github_actions {
            Some(v) => unsafe { std::env::set_var("GITHUB_ACTIONS", v) },
            None => unsafe { std::env::remove_var("GITHUB_ACTIONS") },
        }
        match &self.prior_closeout {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
        }
    }
}

pub(super) fn temp_json_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.json"))
}

pub(super) fn temp_dir_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("router-rs-{name}-{nonce}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

pub(super) fn write_text_fixture(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(path, content).expect("write text fixture");
}

pub(super) fn write_runtime_fixture(path: &Path, slug: &str) {
    fs::write(
            path,
            json!({
                "keys": ["slug", "layer", "owner", "gate", "summary", "trigger_hints", "priority", "session_start"],
                "skills": [[slug, "L2", "primary", "none", format!("{slug} summary"), ["trigger"], "P1", "always"]]
            })
            .to_string(),
        )
        .expect("write runtime fixture");
}

pub(super) fn write_manifest_fixture(path: &Path, slug: &str, priority: &str) {
    fs::write(
            path,
            json!({
                "keys": ["slug", "description", "layer", "owner", "gate", "trigger_hints", "priority", "session_start"],
                "skills": [[slug, format!("{slug} manifest"), "L2", "primary", "none", ["trigger"], priority, "always"]]
            })
            .to_string(),
        )
        .expect("write manifest fixture");
}

