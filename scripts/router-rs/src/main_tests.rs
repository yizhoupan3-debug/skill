use super::*;
use crate::integration_test_prelude::*;

use serde_json::{json, Map, Value};

use crate::cli::args::*;
use crate::cli::runtime_ops::LiveExecuteResult;
use crate::route::RouteDecision;
use crate::route::{
    evaluate_routing_cases, load_records_cached_for_stdio_with_default_runtime_path,
    load_routing_eval_cases, read_json, value_to_string,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_route_fixtures.json")
}

fn routing_eval_case_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/routing_eval_cases.json")
}

fn assert_routing_eval_cases_match<F>(label: &str, mut route_case: F)
where
    F: FnMut(&str, &str, bool) -> Result<RouteDecision, String>,
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
        let decision = route_case(task, &format!("routing-eval::{label}::{id}"), first_turn)
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

fn sample_execute_request() -> ExecuteRequestPayload {
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

fn temp_trace_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.jsonl"))
}

fn execute_allowlist_env_lock() -> &'static Mutex<()> {
    static EXECUTE_ALLOWLIST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    EXECUTE_ALLOWLIST_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

fn with_execute_allowlist_env<F>(value: Option<&str>, test_fn: F)
where
    F: FnOnce(),
{
    let _guard = execute_allowlist_env_lock()
        .lock()
        .expect("execute allowlist env lock poisoned");
    let previous = std::env::var_os(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV);

    match value {
        Some(raw) => std::env::set_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV, raw),
        None => std::env::remove_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV),
    }

    let outcome = catch_unwind(AssertUnwindSafe(test_fn));

    match previous {
        Some(raw) => std::env::set_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV, raw),
        None => std::env::remove_var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV),
    }

    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}

fn closeout_enforcement_env_lock() -> &'static Mutex<()> {
    static CLOSEOUT_ENFORCEMENT_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    CLOSEOUT_ENFORCEMENT_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

/// 测试进程内 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 为全局环境变量；并行测试会互相干扰，需串行。
struct CloseoutStrictEnvGuard {
    prior: Option<String>,
}

impl CloseoutStrictEnvGuard {
    fn new() -> Self {
        let prior = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        // 显式开启硬门禁：本地默认已改为「未设置则软」，测试必须不依赖全局 CI 变量。
        std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");
        Self { prior }
    }
}

impl Drop for CloseoutStrictEnvGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }
}

/// 模拟「CI 检测为真 + 未设置 ROUTER_RS_CLOSEOUT_ENFORCEMENT」：应走硬门禁（与显式 `=1` 路径回归等价）。
/// 故意 `remove_var("GITHUB_ACTIONS")`，单独覆盖实现里 `CI` 分支（`in_ci_like_environment` 为 OR）。
struct CiHardUnsetCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl CiHardUnsetCloseoutEnvGuard {
    fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        std::env::set_var("CI", "true");
        std::env::remove_var("GITHUB_ACTIONS");
        std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
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
            Some(v) => std::env::set_var("CI", v),
            None => std::env::remove_var("CI"),
        }
        match &self.prior_github_actions {
            Some(v) => std::env::set_var("GITHUB_ACTIONS", v),
            None => std::env::remove_var("GITHUB_ACTIONS"),
        }
        match &self.prior_closeout {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }
}

/// 模拟「GitHub Actions 检测为真 + 未设置 ROUTER_RS_CLOSEOUT_ENFORCEMENT」：覆盖 `GITHUB_ACTIONS=true` 分支。
/// 故意清除 `CI`，单独验证 Actions 路径。
struct GithubActionsHardUnsetCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl GithubActionsHardUnsetCloseoutEnvGuard {
    fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        std::env::remove_var("CI");
        std::env::set_var("GITHUB_ACTIONS", "true");
        std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
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
            Some(v) => std::env::set_var("CI", v),
            None => std::env::remove_var("CI"),
        }
        match &self.prior_github_actions {
            Some(v) => std::env::set_var("GITHUB_ACTIONS", v),
            None => std::env::remove_var("GITHUB_ACTIONS"),
        }
        match &self.prior_closeout {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }
}

/// `CI=true` 且显式 `ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`：程序化门禁关闭应优先于 CI 检测。
struct CiWithCloseoutDisabledEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl CiWithCloseoutDisabledEnvGuard {
    fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        std::env::set_var("CI", "true");
        std::env::remove_var("GITHUB_ACTIONS");
        std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "0");
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
            Some(v) => std::env::set_var("CI", v),
            None => std::env::remove_var("CI"),
        }
        match &self.prior_github_actions {
            Some(v) => std::env::set_var("GITHUB_ACTIONS", v),
            None => std::env::remove_var("GITHUB_ACTIONS"),
        }
        match &self.prior_closeout {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }
}

/// 本地非 CI，但 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 设为空字符串：视为「已设置」且非软关断 token → 硬门禁。
struct LocalNonCiEmptyCloseoutEnvGuard {
    prior_ci: Option<String>,
    prior_github_actions: Option<String>,
    prior_closeout: Option<String>,
}

impl LocalNonCiEmptyCloseoutEnvGuard {
    fn new() -> Self {
        let prior_ci = std::env::var("CI").ok();
        let prior_github_actions = std::env::var("GITHUB_ACTIONS").ok();
        let prior_closeout = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
        std::env::remove_var("CI");
        std::env::remove_var("GITHUB_ACTIONS");
        std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "");
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
            Some(v) => std::env::set_var("CI", v),
            None => std::env::remove_var("CI"),
        }
        match &self.prior_github_actions {
            Some(v) => std::env::set_var("GITHUB_ACTIONS", v),
            None => std::env::remove_var("GITHUB_ACTIONS"),
        }
        match &self.prior_closeout {
            Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
            None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
        }
    }
}

fn temp_json_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.json"))
}

fn temp_dir_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-{name}-{nonce}"))
}

fn write_text_fixture(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::write(path, content).expect("write text fixture");
}

fn write_runtime_fixture(path: &Path, slug: &str) {
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

fn write_manifest_fixture(path: &Path, slug: &str, priority: &str) {
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

#[test]
fn stdio_request_dispatches_route_policy_payload() {
    let response =
        handle_stdio_json_line(r#"{"id":1,"op":"route_policy","payload":{"mode":"verify"}}"#);
    assert!(response.ok);
    assert_eq!(response.id, json!(1));
    assert_eq!(
        response.payload.expect("payload")["policy_schema_version"],
        json!(ROUTE_POLICY_SCHEMA_VERSION)
    );
}

#[test]
fn stdio_request_dispatches_hook_policy_payload() {
    let response = handle_stdio_json_line(
        r#"{"id":1,"op":"hook_policy","payload":{"operation":"validation-categories","command":"python3 -m json.tool .codex/config.toml"}}"#,
    );
    assert!(response.ok, "{}", response.error.unwrap_or_default());
    let payload = response.payload.expect("payload");
    assert_eq!(payload["categories"], json!(["config", "json"]));
}

#[test]
fn stdio_request_dispatches_concurrency_defaults_payload() {
    let response = handle_stdio_json_line(r#"{"id":1,"op":"concurrency_defaults","payload":{}}"#);
    assert!(response.ok);
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["router_stdio"]["default_pool_size"],
        json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["router_stdio"]["max_pool_size"],
        json!(MAX_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["max_background_jobs"],
        json!(DEFAULT_MAX_BACKGROUND_JOBS)
    );
    assert_eq!(
        payload["max_concurrent_subagents"],
        json!(DEFAULT_MAX_CONCURRENT_SUBAGENTS)
    );
}

#[test]
fn stdio_request_dispatches_execute_payload() {
    let payload =
        serde_json::to_string(&sample_execute_request()).expect("serialize execute payload");
    let response = handle_stdio_json_line(&format!(
        "{{\"id\":3,\"op\":\"execute\",\"payload\":{payload}}}"
    ));
    assert!(response.ok);
    assert_eq!(response.id, json!(3));
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["execution_schema_version"],
        json!(EXECUTION_SCHEMA_VERSION)
    );
    assert_eq!(payload["authority"], json!(EXECUTION_AUTHORITY));
    assert_eq!(payload["live_run"], json!(false));
}

#[test]
fn framework_refresh_copies_compact_prompt_to_configured_file() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-refresh-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
            task_root.join("TRACE_METADATA.json"),
            r#"{"task":"active bootstrap repair","matched_skills":["autopilot","skill-framework-developer"]}"#,
        )
        .expect("write trace metadata");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
        repo_root.join(".supervisor_state.json"),
        r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "primary_owner":"skill-framework-developer",
                "execution_contract":{
                    "goal":"Repair stale bootstrap injection",
                    "scope":["scripts/router-rs/src/framework_runtime.rs"],
                    "acceptance_criteria":["completed tasks never appear as current execution"]
                },
                "blockers":{"open_blockers":["Need regression coverage"]}
            }"#,
    )
    .expect("write supervisor state");
    let clipboard_path = repo_root.join("clipboard.txt");
    std::env::set_var("ROUTER_RS_CLIPBOARD_PATH", &clipboard_path);
    let refresh =
        build_framework_refresh_payload(&repo_root, 6, false).expect("build refresh payload");
    let prompt = refresh
        .get("prompt")
        .and_then(Value::as_str)
        .expect("refresh prompt");
    let clipboard = copy_text_to_clipboard(prompt).expect("copy prompt");
    std::env::remove_var("ROUTER_RS_CLIPBOARD_PATH");

    let copied = fs::read_to_string(&clipboard_path).expect("read clipboard file");
    assert_eq!(clipboard["backend"], json!("file"));
    assert!(copied.contains("继续当前仓库，先看这些恢复锚点："));
    assert!(copied.contains("先做："));
    assert!(copied.contains("按既定串并行分工直接开始执行。"));
    assert!(!copied.contains("当前上下文："));
    assert!(!copied.contains("必须先做的下一步："));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_refresh_completed_task_uses_plain_closeout_wording() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-refresh-completed-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("completed-rerun-20260423");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: bounded rerun\n- phase: closeout\n- status: completed\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":[]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"bounded rerun","verification_status":"completed"}"#,
    )
    .expect("write trace metadata");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"completed-rerun-20260423","task":"bounded rerun"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"completed-rerun-20260423",
                "task_summary":"bounded rerun",
                "active_phase":"closeout",
                "verification":{"verification_status":"completed","last_verification_summary":"262 passed"},
                "continuity":{"story_state":"completed","resume_allowed":false},
                "next_actions":[],
                "execution_contract":{"goal":"Re-run bounded verification"}
            }"#,
        )
        .expect("write supervisor state");
    let refresh =
        build_framework_refresh_payload(&repo_root, 6, false).expect("build refresh payload");
    let prompt = refresh
        .get("prompt")
        .and_then(Value::as_str)
        .expect("refresh prompt");

    assert!(prompt.contains("最近一轮已经收尾："));
    assert!(prompt.contains("- bounded rerun"));
    assert!(prompt.contains("- 结果已经稳定，可以直接按已完成上下文来看。"));
    assert!(prompt.contains("- 如果还要继续相关工作，先新开一个 standalone task"));
    assert!(prompt.contains("先看这些恢复锚点："));
    assert!(!prompt.contains("剩余："));
    assert!(!prompt.contains("先做："));
    assert!(!prompt.contains("按既定串并行分工直接开始执行。"));
    assert!(!prompt.contains("Keep this task only as recent-completed context"));
    assert!(!prompt.contains("Start a new standalone task before resuming related work"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_refresh_includes_goal_state_attachment() {
    let repo_root = temp_dir_path("framework-refresh-goal");
    let task_id = "goal-task-refresh";
    let task_root = repo_root.join("artifacts/current").join(task_id);
    fs::create_dir_all(&task_root).expect("mkdir task");
    fs::create_dir_all(repo_root.join("artifacts/current")).expect("mkdir current");
    fs::write(
        repo_root.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{task_id}"}}"#),
    )
    .expect("active_task");
    fs::write(
        task_root.join("GOAL_STATE.json"),
        r#"{
            "schema_version": "router-rs-autopilot-goal-v1",
            "goal": "Integration goal text",
            "status": "running",
            "drive_until_done": true,
            "done_when": ["cargo test passes"],
            "validation_commands": ["cargo test -q"],
            "non_goals": [],
            "checkpoints": [],
            "updated_at": "2026-01-01T00:00:00Z"
        }"#,
    )
    .expect("goal state");
    let refresh = build_framework_refresh_payload(&repo_root, 6, false).expect("refresh");
    let prompt = refresh["prompt"].as_str().expect("prompt");
    assert!(
        prompt.contains("深度信号") && prompt.contains("d0/3"),
        "depth hint should surface in refresh prompt; prompt={prompt:?}"
    );
    assert!(
        prompt.contains("Active goal") || prompt.contains("GOAL_STATE（router-rs"),
        "goal section missing (compact vs verbose); prompt={prompt:?}"
    );
    assert!(prompt.contains("Integration goal text"));
    assert!(prompt.contains("cargo test -q"));
    assert_eq!(
        refresh["goal_state"]["goal"],
        json!("Integration goal text")
    );
    assert_eq!(refresh["depth_compliance"]["depth_score"], json!(0));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_refresh_goal_prompt_verbose_restores_long_section() {
    let repo_root = temp_dir_path("framework-refresh-goal-verbose");
    let task_id = "goal-task-verbose";
    let task_root = repo_root.join("artifacts/current").join(task_id);
    fs::create_dir_all(&task_root).expect("mkdir task");
    fs::create_dir_all(repo_root.join("artifacts/current")).expect("mkdir current");
    fs::write(
        repo_root.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{task_id}"}}"#),
    )
    .expect("active_task");
    fs::write(
        task_root.join("GOAL_STATE.json"),
        r#"{
            "schema_version": "router-rs-autopilot-goal-v1",
            "goal": "Verbose fixture",
            "status": "running",
            "drive_until_done": true,
            "done_when": ["pass"],
            "validation_commands": ["cargo test -q"],
            "non_goals": [],
            "checkpoints": [],
            "updated_at": "2026-01-01T00:00:00Z"
        }"#,
    )
    .expect("goal state");
    let prior = std::env::var("ROUTER_RS_GOAL_PROMPT_VERBOSE").ok();
    std::env::set_var("ROUTER_RS_GOAL_PROMPT_VERBOSE", "1");
    let refresh = build_framework_refresh_payload(&repo_root, 6, false).expect("refresh");
    let prompt = refresh["prompt"].as_str().expect("prompt");
    assert!(
        prompt.contains("GOAL_STATE（router-rs"),
        "verbose env should restore long heading; prompt={prompt:?}"
    );
    match prior {
        Some(v) => std::env::set_var("ROUTER_RS_GOAL_PROMPT_VERBOSE", v),
        None => std::env::remove_var("ROUTER_RS_GOAL_PROMPT_VERBOSE"),
    }
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_refresh_payload_always_exports_goal_state_key() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let refresh = build_framework_refresh_payload(&repo_root, 6, false).expect("refresh");
    let keys: Vec<_> = refresh
        .as_object()
        .expect("refresh payload must be object")
        .keys()
        .cloned()
        .collect();
    assert!(
        keys.contains(&"goal_state".to_string()),
        "refresh JSON must include goal_state (null when no GOAL_STATE.json); keys={keys:?}"
    );
    assert!(
        keys.contains(&"depth_compliance".to_string()),
        "refresh JSON must include depth_compliance; keys={keys:?}"
    );
}

#[test]
fn framework_statusline_uses_rust_runtime_view() {
    let repo_root = temp_dir_path("framework-statusline");
    let task_id = "statusline-task-20260424120000";
    let task_root = repo_root.join("artifacts").join("current").join(task_id);
    write_text_fixture(
            &task_root.join("SESSION_SUMMARY.md"),
            "# SESSION_SUMMARY\n\n- task: Validate status line\n- phase: integration\n- status: in_progress\n",
        );
    write_text_fixture(
        &task_root.join("NEXT_ACTIONS.json"),
        &json!({"next_actions": ["Ship it"]}).to_string(),
    );
    write_text_fixture(
        &task_root.join("EVIDENCE_INDEX.json"),
        &json!({"artifacts": []}).to_string(),
    );
    write_text_fixture(
        &task_root.join("TRACE_METADATA.json"),
        &json!({"matched_skills": ["autopilot", "skill-framework-developer"]}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("focus_task.json"),
        &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("task_registry.json"),
        &json!({
            "schema_version": "task-registry-v1",
            "focus_task_id": task_id,
            "tasks": [
                {
                    "task_id": task_id,
                    "task": "Validate status line",
                    "phase": "integration",
                    "status": "in_progress",
                    "resume_allowed": true
                }
            ]
        })
        .to_string(),
    );
    write_text_fixture(
        &repo_root.join(".supervisor_state.json"),
        &json!({
            "task_id": task_id,
            "task_summary": "Validate status line",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": true}
        })
        .to_string(),
    );

    let statusline = build_framework_statusline(&repo_root).expect("build statusline");

    assert!(statusline.contains("task=Validate status line"));
    assert!(statusline.contains("next=/refresh"));
    assert!(statusline.contains("integration/in_progress"));
    assert!(statusline.contains("route=autopilot+1"));
    assert!(statusline.contains("others=0"));
    assert!(statusline.contains("resumable=0"));
    assert!(
        statusline.contains("depth=d0 | "),
        "statusline should surface depth rollup; got {statusline:?}"
    );
    assert!(statusline.contains("git=nogit"));
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_snapshot_missing_recovery_anchors_is_not_resumable() {
    let repo_root = temp_dir_path("framework-missing-recovery-anchors");
    let current_root = repo_root.join("artifacts").join("current");
    write_text_fixture(
        &current_root.join("EVIDENCE_INDEX.json"),
        &json!({"artifacts": []}).to_string(),
    );

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let continuity = &snapshot["runtime_snapshot"]["continuity"];
    let missing_anchors = continuity["missing_recovery_anchors"]
        .as_array()
        .expect("missing anchors array");

    assert_eq!(continuity["state"], json!("missing"));
    assert_eq!(continuity["can_resume"], json!(false));
    assert_eq!(continuity["current_execution"], Value::Null);
    assert!(missing_anchors.contains(&json!("SESSION_SUMMARY")));
    assert!(missing_anchors.contains(&json!("NEXT_ACTIONS")));
    assert!(missing_anchors.contains(&json!("TRACE_METADATA")));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_writer_materializes_complete_focus_continuity() {
    let repo_root = temp_dir_path("framework-session-writer-continuity");
    let output_dir = repo_root.join("artifacts").join("current");
    let payload = json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "continuity-polish-20260424120000",
        "task": "continuity polish",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Make Rust continuity recoverable without manual mirror repair.",
        "focus": true,
        "next_actions": ["Run targeted tests"],
        "matched_skills": ["autopilot"],
        "execution_contract": {
            "goal": "Improve continuity artifacts",
            "acceptance_criteria": ["writer emits all recovery anchors"]
        },
        "blockers": ["none"]
    });

    let result = write_framework_session_artifacts(payload).expect("write artifacts");
    let task_id = result["task_id"].as_str().expect("task id");
    let task_root = repo_root.join("artifacts").join("current").join(task_id);

    for path in [
        task_root.join("SESSION_SUMMARY.md"),
        task_root.join("NEXT_ACTIONS.json"),
        task_root.join("EVIDENCE_INDEX.json"),
        task_root.join("TRACE_METADATA.json"),
        task_root.join("CONTINUITY_JOURNAL.json"),
        repo_root.join(".supervisor_state.json"),
        repo_root.join("artifacts/current/active_task.json"),
        repo_root.join("artifacts/current/focus_task.json"),
        repo_root.join("artifacts/current/task_registry.json"),
    ] {
        assert!(path.is_file(), "missing {}", path.display());
    }

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let runtime = &snapshot["runtime_snapshot"];
    assert_eq!(runtime["active_task_id"], json!(task_id));
    assert_eq!(runtime["continuity"]["state"], json!("active"));
    assert_eq!(runtime["continuity"]["can_resume"], json!(true));
    assert_eq!(runtime["continuity"]["missing_recovery_anchors"], json!([]));

    let supervisor = serde_json::from_str::<Value>(
        &fs::read_to_string(repo_root.join(".supervisor_state.json")).expect("read supervisor"),
    )
    .expect("parse supervisor");
    assert_eq!(supervisor["continuity"]["resume_allowed"], json!(true));
    assert_eq!(
        supervisor["verification"]["verification_status"],
        json!("in_progress")
    );
    assert_eq!(
        supervisor["trace_metadata"]["matched_skills"],
        json!(["autopilot"])
    );
    assert_eq!(
        supervisor["artifact_refs"]["task_root"],
        json!(task_root.display().to_string())
    );
    let active_pointer = serde_json::from_str::<Value>(
        &fs::read_to_string(repo_root.join("artifacts/current/active_task.json"))
            .expect("read active pointer"),
    )
    .expect("parse active pointer");
    assert_eq!(
        active_pointer["session_summary"],
        json!(task_root.join("SESSION_SUMMARY.md").display().to_string())
    );
    for path in [
        repo_root.join("SESSION_SUMMARY.md"),
        repo_root.join("NEXT_ACTIONS.json"),
        repo_root.join("EVIDENCE_INDEX.json"),
        repo_root.join("TRACE_METADATA.json"),
        repo_root.join("CONTINUITY_JOURNAL.json"),
        repo_root.join("artifacts/current/SESSION_SUMMARY.md"),
        repo_root.join("artifacts/current/NEXT_ACTIONS.json"),
        repo_root.join("artifacts/current/EVIDENCE_INDEX.json"),
        repo_root.join("artifacts/current/TRACE_METADATA.json"),
        repo_root.join("artifacts/current/CONTINUITY_JOURNAL.json"),
    ] {
        assert!(!path.exists(), "unexpected mirror {}", path.display());
    }
    let journal = serde_json::from_str::<Value>(
        &fs::read_to_string(task_root.join("CONTINUITY_JOURNAL.json")).expect("read journal"),
    )
    .expect("parse journal");
    assert_eq!(journal["schema_version"], json!("continuity-journal-v1"));
    assert_eq!(journal["checkpoint_count"], json!(1));
    assert!(journal["latest_checkpoint_id"]
        .as_str()
        .is_some_and(|value| value.len() == 64));
    assert!(
        journal["checkpoints"][0]["artifact_hashes"]["supervisor_state"]
            .as_str()
            .is_some_and(|value| value.len() == 64)
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn post_tool_evidence_appends_cargo_test_after_continuity_seed() {
    let repo_root = temp_dir_path("post-tool-evidence-append");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "evidence-task",
        "task": "Verify evidence append",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed continuity",
        "focus": true,
        "next_actions": ["Run tests"]
    }))
    .expect("seed artifacts");

    let event = json!({
        "tool_name": "Bash",
        "tool_input": { "command": "cd scripts/router-rs && cargo test -q" },
        "session_id": "sess-post-tool-1",
        "tool_output": { "exit_code": 0 },
    });
    crate::framework_runtime::try_append_codex_post_tool_evidence(&repo_root, &event)
        .expect("append");

    let evidence_path = repo_root
        .join("artifacts/current/evidence-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read evidence"))
            .expect("parse evidence");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("codex_post_tool_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(0));
    assert_eq!(artifacts[0]["success"], json!(true));
    assert!(artifacts[0]["command_preview"]
        .as_str()
        .unwrap()
        .contains("cargo test"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn cursor_post_tool_evidence_appends_cargo_test_after_continuity_seed() {
    let repo_root = temp_dir_path("cursor-post-tool-evidence-append");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cursor-evidence-task",
        "task": "Cursor shell evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed continuity",
        "focus": true,
        "next_actions": ["Run tests"]
    }))
    .expect("seed artifacts");

    let event = json!({
        "tool_name": "run_terminal_cmd",
        "tool_input": { "command": "cd scripts/router-rs && cargo test -q" },
        "session_id": "sess-cursor-post-tool-1",
        "tool_output": { "exit_code": 0 },
    });
    crate::framework_runtime::try_append_cursor_post_tool_evidence(&repo_root, &event)
        .expect("append");

    let evidence_path = repo_root
        .join("artifacts/current/cursor-evidence-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read evidence"))
            .expect("parse evidence");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("cursor_post_tool_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(0));
    assert_eq!(artifacts[0]["success"], json!(true));
    assert!(artifacts[0]["command_preview"]
        .as_str()
        .unwrap()
        .contains("cargo test"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn hook_evidence_append_cli_writes_cursor_cargo_check() {
    let repo_root = temp_dir_path("hook-evidence-cursor");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cursor-ev-task",
        "task": "cursor hook evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed");

    let payload = json!({
        "repo_root": repo_root,
        "command_preview": "(cd scripts/router-rs && cargo check --message-format=short)",
        "exit_code": 1,
        "source": "cursor_rust_lint",
    });
    let out = framework_hook_evidence_append(payload).expect("append");
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["skipped"], json!(false));

    let evidence_path = repo_root
        .join("artifacts/current/cursor-ev-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read")).expect("parse");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("external_hook_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(1));
    assert_eq!(artifacts[0]["success"], json!(false));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn stdio_framework_hook_evidence_append_dispatches() {
    let repo_root = temp_dir_path("stdio-framework-hook-evidence");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "stdio-he-task",
        "task": "stdio hook evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": []
    }))
    .expect("seed");

    let rr = repo_root.display().to_string();
    let req = json!({
        "id": "stdio-he-1",
        "op": "framework_hook_evidence_append",
        "payload": {
            "repo_root": rr,
            "command_preview": "cargo test -q",
            "exit_code": 0,
            "source": "stdio_integration_test",
        }
    });
    let line = serde_json::to_string(&req).expect("serialize stdio line");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["ok"], json!(true));

    let evidence_path = repo_root
        .join("artifacts/current/stdio-he-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read")).expect("parse");
    assert_eq!(
        evidence["artifacts"][0]["kind"],
        json!("external_hook_verification")
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn post_tool_evidence_no_ops_without_continuity_seed() {
    let repo_root = temp_dir_path("post-tool-evidence-skip");
    let event = json!({
        "tool_name": "Bash",
        "tool_input": { "command": "cargo test" },
    });
    crate::framework_runtime::try_append_codex_post_tool_evidence(&repo_root, &event)
        .expect("noop");
    assert!(
        !repo_root
            .join("artifacts/current/EVIDENCE_INDEX.json")
            .exists(),
        "evidence file should not be created without continuity anchors"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_rejects_corrupt_continuity_journal() {
    let repo_root = temp_dir_path("framework-session-corrupt-journal");
    let output_dir = repo_root.join("artifacts").join("current");
    let first = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "journal-corrupt",
        "task": "Journal corrupt",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Seed journal.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("first write");
    let task_id = first["task_id"].as_str().expect("task id");
    let journal_path = repo_root
        .join("artifacts/current")
        .join(task_id)
        .join("CONTINUITY_JOURNAL.json");
    fs::write(&journal_path, "{not valid json").expect("corrupt journal");

    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": task_id,
        "task": "Journal corrupt",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Second write.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect_err("corrupt journal should fail before overwrite");
    assert!(
        err.contains("parse json failed") || err.contains("CONTINUITY_JOURNAL"),
        "unexpected error: {err}"
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_rejects_stale_focus_update() {
    let repo_root = temp_dir_path("framework-session-cas");
    let output_dir = repo_root.join("artifacts").join("current");
    let first = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-task",
        "task": "CAS task",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Initial write.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("first write");
    assert_eq!(first["task_id"], json!("cas-task"));

    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    let stale_hash = crate::framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
    write_text_fixture(
        &focus_path,
        r#"{"task_id":"other-task","task":"Other task","updated_at":"2026-04-25T00:00:00+08:00"}"#,
    );

    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-task",
        "task": "CAS task",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Stale write.",
        "focus": true,
        "expected_focus_task_hash": stale_hash,
        "next_actions": ["Continue"]
    }))
    .expect_err("stale focus update should fail");
    assert!(err.contains("stale focus task pointer update rejected"));

    let focus = read_json(&focus_path).expect("read focus");
    assert_eq!(focus["task_id"], json!("other-task"));
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_preserves_existing_roundtrip() {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-cas-roundtrip");
    let output_dir = repo_root.join("artifacts").join("current");
    let first = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-roundtrip",
        "task": "CAS roundtrip",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Initial write.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("first write");
    assert_eq!(first["task_id"], json!("cas-roundtrip"));

    let active_path = repo_root.join("artifacts/current/active_task.json");
    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    let supervisor_path = repo_root.join(".supervisor_state.json");
    let active_hash =
        crate::framework_runtime::hash_file_for_test(&active_path).expect("active hash");
    let focus_hash = crate::framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
    let supervisor_hash =
        crate::framework_runtime::hash_file_for_test(&supervisor_path).expect("supervisor hash");

    let second = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-roundtrip",
        "task": "CAS roundtrip",
        "phase": "validation",
        "status": "passed",
        "summary": "Validated write.",
        "focus": true,
        "expected_active_task_hash": active_hash,
        "expected_focus_task_hash": focus_hash,
        "expected_supervisor_state_hash": supervisor_hash,
        "next_actions": [],
        // Completion claims (status in CLOSEOUT_COMPLETION_STATUSES)
        // require a closeout record so closeout_enforcement can verify
        // evidence. Provide a minimal passed record here.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "cas-roundtrip",
            "verification_status": "passed",
            "summary": "Validated write.",
            "commands_run": [
                {"command": "cargo test --manifest-path scripts/router-rs/Cargo.toml", "exit_code": 0}
            ],
            "artifacts_checked": [
                {"path": "README.md", "exists": true}
            ]
        }
    }))
    .expect("roundtrip write");
    assert_eq!(second["task_id"], json!("cas-roundtrip"));
    assert_eq!(
        second["closeout_evaluation"]["closeout_allowed"],
        json!(true)
    );

    let supervisor = read_json(&supervisor_path).expect("read supervisor");
    assert_eq!(supervisor["active_phase"], json!("validation"));
    assert_eq!(
        supervisor["verification"]["verification_status"],
        json!("passed")
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_record() {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-missing");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-missing",
        "task": "Closeout missing",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("missing closeout_record must block completion claim");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_ci_unsets_env() {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _ci = CiHardUnsetCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-ci-unset-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-ci-unset",
        "task": "CI unset closeout env",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record under CI",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("missing closeout_record must block completion claim when CI without explicit env");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_github_actions_unsets_env(
) {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _ga = GithubActionsHardUnsetCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-gha-unset-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-gha-unset",
        "task": "GHA unset closeout env",
        "phase": "validation",
        "status": "completed",
        "summary": "claimed done with no closeout record under GITHUB_ACTIONS",
        "focus": true,
        "next_actions": []
    }))
    .expect_err(
        "missing closeout_record must block completion claim when GITHUB_ACTIONS without explicit env",
    );
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_allows_completion_without_closeout_when_ci_and_closeout_env_off(
) {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _ci_off = CiWithCloseoutDisabledEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-ci-with-env-off");
    let output_dir = repo_root.join("artifacts").join("current");
    let written = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-ci-env-off",
        "task": "CI but closeout enforcement off",
        "phase": "validation",
        "status": "completed",
        "summary": "CI with ROUTER_RS_CLOSEOUT_ENFORCEMENT=0, no closeout_record",
        "focus": true,
        "next_actions": []
    }))
    .expect(
        "completion write should succeed when CI but explicit closeout env disables enforcement",
    );
    assert!(
        written.get("closeout_evaluation").is_none(),
        "expected no closeout_evaluation when enforcement skipped, got: {written}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_blocks_completion_without_closeout_when_closeout_env_empty_string(
) {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _empty = LocalNonCiEmptyCloseoutEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-empty-env");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-empty-env",
        "task": "Empty ROUTER_RS_CLOSEOUT_ENFORCEMENT",
        "phase": "validation",
        "status": "completed",
        "summary": "non-CI with empty string closeout env",
        "focus": true,
        "next_actions": []
    }))
    .expect_err("empty ROUTER_RS_CLOSEOUT_ENFORCEMENT must not be treated as unset/local-soft");
    assert!(
        err.contains("closeout_record"),
        "error must reference missing closeout_record, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_allows_completion_without_closeout_when_env_disables() {
    struct EnvCloseoutGuard {
        prior: Option<String>,
    }
    impl EnvCloseoutGuard {
        fn set(value: &str) -> Self {
            let prior = std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").ok();
            std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", value);
            Self { prior }
        }
    }
    impl Drop for EnvCloseoutGuard {
        fn drop(&mut self) {
            match &self.prior {
                Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
                None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
            }
        }
    }

    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _guard = EnvCloseoutGuard::set("0");
    let repo_root = temp_dir_path("framework-session-closeout-env-off");
    let output_dir = repo_root.join("artifacts").join("current");
    let written = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-env-off",
        "task": "Closeout env off",
        "phase": "validation",
        "status": "completed",
        "summary": "personal mode without closeout_record",
        "focus": true,
        "next_actions": []
    }))
    .expect("completion write should succeed when closeout enforcement is disabled by env");
    assert!(
        written.get("closeout_evaluation").is_none(),
        "expected no closeout_evaluation when enforcement skipped"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_in_progress_ignores_malformed_closeout_record() {
    // In-progress checkpoints often carry partial / draft closeout records
    // that would fail strict deny_unknown_fields parsing. The artifact
    // write path must NOT block in-progress writes on incidental record
    // malformation: pre-completion validation is the caller's job.
    let repo_root = temp_dir_path("framework-session-closeout-inprogress");
    let output_dir = repo_root.join("artifacts").join("current");
    let response = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-inprog",
        "task": "Closeout in-progress",
        "phase": "execution",
        "status": "in_progress",
        "summary": "still working",
        "focus": true,
        "next_actions": [],
        // Unknown field would normally trip deny_unknown_fields, but the
        // record must be ignored when status is not a completion claim.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "co-inprog",
            "verification_status": "not_run",
            "summary": "draft",
            "unexpected_extension_field": "ignored on in-progress"
        }
    }))
    .expect("in-progress write must succeed even with malformed record");
    assert!(
        response.get("closeout_evaluation").is_none(),
        "in-progress writes must not attach closeout_evaluation, got: {response}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_session_artifact_write_blocks_completion_with_failed_command() {
    let _lock = closeout_enforcement_env_lock()
        .lock()
        .expect("closeout env lock poisoned");
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-closeout-bad");
    let output_dir = repo_root.join("artifacts").join("current");
    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "co-bad",
        "task": "Closeout bad",
        "phase": "validation",
        "status": "passed",
        "summary": "Done with failing command",
        "focus": true,
        "next_actions": [],
        // verification_status=passed but a recorded command exited 1.
        // closeout_enforcement R3 must block this claim.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "co-bad",
            "verification_status": "passed",
            "summary": "Done with failing command",
            "commands_run": [
                {"command": "cargo test", "exit_code": 1}
            ]
        }
    }))
    .expect_err("failed command in passed record must block completion");
    assert!(
        err.contains("closeout_enforcement blocked"),
        "error must reference closeout enforcement block, got: {err}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn task_registry_normalization_dedupes_and_limits_old_tasks() {
    let repo_root = temp_dir_path("framework-registry-compact");
    let current_root = repo_root.join("artifacts").join("current");
    let mut tasks = Vec::new();
    for index in 0..140 {
        tasks.push(json!({
            "task_id": format!("task-{index:03}"),
            "task": format!("Task {index:03}"),
            "updated_at": format!("2026-04-24T12:{:02}:00+08:00", index % 60),
            "status": "completed",
            "phase": "closeout",
            "resume_allowed": false
        }));
    }
    tasks.push(json!({
        "task_id": "focus-task",
        "task": "Focus task",
        "updated_at": "2026-04-24T13:00:00+08:00",
        "status": "in_progress",
        "phase": "implementation",
        "resume_allowed": true
    }));
    write_text_fixture(
        &current_root.join("task_registry.json"),
        &json!({
            "schema_version": "task-registry-v1",
            "focus_task_id": "focus-task",
            "tasks": tasks
        })
        .to_string(),
    );

    let changed = crate::framework_runtime::write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": current_root,
        "task_id": "focus-task",
        "task": "Focus task",
        "phase": "implementation",
        "status": "in_progress",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("write focused task");
    assert_eq!(changed["task_id"], json!("focus-task"));

    let registry = serde_json::from_str::<Value>(
        &fs::read_to_string(current_root.join("task_registry.json")).expect("read registry"),
    )
    .expect("parse registry");
    let tasks = registry["tasks"].as_array().expect("tasks");
    assert_eq!(tasks.len(), 128);
    assert_eq!(registry["truncated"], json!(true));
    assert_eq!(registry["focus_task_id"], json!("focus-task"));
    assert_eq!(tasks[0]["task_id"], json!("focus-task"));
    assert_eq!(registry["recoverable_task_count"], json!(1));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_alias_builds_compact_autopilot_payload() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["autopilot"]}"#,
    )
    .expect("write trace metadata");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "autopilot",
        FrameworkAliasBuildOptions {
            max_lines: 4,
            compact: false,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");
    let prompt = alias
        .get("entry_prompt")
        .and_then(Value::as_str)
        .expect("entry prompt");

    assert_eq!(
        payload["schema_version"],
        json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
    );
    assert_eq!(alias["name"], json!("autopilot"));
    assert_eq!(alias["host_entrypoint"], json!("/autopilot"));
    assert_eq!(alias["compact"], json!(false));
    assert!(prompt.contains("进入 autopilot"));
    assert!(prompt.contains("本地 Rust"));
    assert!(prompt.contains("路由："));
    assert_eq!(
        alias["state_machine"]["current_state"],
        json!("resume_requires_repair")
    );
    assert_eq!(
        alias["state_machine"]["recommended_action"],
        json!("repair_continuity_then_resume")
    );
    assert_eq!(alias["state_machine"]["evidence_missing"], json!(true));
    assert_eq!(
        alias["entry_contract"]["context"]["execution_readiness"],
        json!("needs_recovery")
    );
    assert_eq!(
        alias["entry_contract"]["route_rules"][0],
        json!("模糊需求 -> `deepinterview`")
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_alias_builds_compact_deepinterview_payload() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-deepinterview-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["deepinterview"]}"#,
    )
    .expect("write trace metadata");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "deepinterview",
        FrameworkAliasBuildOptions {
            max_lines: 5,
            compact: false,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");
    let prompt = alias
        .get("entry_prompt")
        .and_then(Value::as_str)
        .expect("entry prompt");

    assert_eq!(
        payload["schema_version"],
        json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
    );
    assert_eq!(alias["name"], json!("deepinterview"));
    assert_eq!(alias["host_entrypoint"], json!("/deepinterview"));
    assert_eq!(alias["compact"], json!(false));
    assert_eq!(alias["canonical_owner"], json!("deepinterview"));
    assert_eq!(
        alias["state_machine"]["handoff"]["rules"][1]["target"],
        json!("autopilot")
    );
    assert_eq!(
        alias["entry_contract"]["route_rules"][0],
        json!("主 owner -> `deepinterview`")
    );
    assert!(prompt.contains("进入 deepinterview"));
    assert!(prompt.contains("每轮只问一个问题"));
    assert!(prompt.contains("review lanes ->"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_alias_builds_compact_team_payload() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-team-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["team"]}"#,
    )
    .expect("write trace metadata");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "team",
        FrameworkAliasBuildOptions {
            max_lines: 5,
            compact: false,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");
    let prompt = alias
        .get("entry_prompt")
        .and_then(Value::as_str)
        .expect("entry prompt");

    assert_eq!(
        payload["schema_version"],
        json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
    );
    assert_eq!(alias["name"], json!("team"));
    assert_eq!(alias["host_entrypoint"], json!("/team"));
    assert_eq!(alias["compact"], json!(false));
    assert_eq!(alias["canonical_owner"], json!("team"));
    assert_eq!(
        alias["state_machine"]["handoff"]["rules"][1]["target"],
        json!("agent-swarm-orchestration")
    );
    assert_eq!(
        alias["entry_contract"]["route_rules"][0],
        json!("主 owner -> `team`")
    );
    assert!(prompt.contains("进入 team"));
    assert!(prompt.contains("bounded subagent lane -> `agent-swarm-orchestration`"));
    assert!(prompt.contains("worker write scope -> `lane-local-delta-only`"));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_alias_compact_payload_omits_duplicate_prompt_fields() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-compact-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["team"]}"#,
    )
    .expect("write trace metadata");
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "autopilot",
        FrameworkAliasBuildOptions {
            max_lines: 3,
            compact: true,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");

    assert_eq!(alias["compact"], json!(true));
    assert!(alias.get("entry_prompt").is_none());
    assert!(alias.get("entry_prompt_token_estimate").is_none());
    assert!(alias.get("upstream_source").is_none());
    assert_eq!(alias["state_machine"]["evidence_missing"], json!(true));
    assert_eq!(
        alias["entry_contract"]["context"]["execution_readiness"],
        json!("needs_recovery")
    );
    assert_eq!(
        alias["state_machine"]["required_anchors"],
        json!([
            "SESSION_SUMMARY",
            "NEXT_ACTIONS",
            "TRACE_METADATA",
            "SUPERVISOR_STATE"
        ])
    );
    assert!(alias["state_machine"]["resume"].get("task").is_none());

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_snapshot_reconciles_stale_supervisor_against_current_pointers() {
    let repo_root = temp_dir_path("runtime-anchor-reconcile");
    let artifacts_root = repo_root.join("artifacts");
    let current_root = artifacts_root.join("current");
    let fresh_root = current_root.join("fresh-task");
    let stale_root = current_root.join("stale-task");
    fs::create_dir_all(&fresh_root).expect("create fresh task root");
    fs::create_dir_all(&stale_root).expect("create stale task root");
    write_text_fixture(
        &fresh_root.join("SESSION_SUMMARY.md"),
        "- task: fresh task\n- phase: implementation\n- status: in_progress\n",
    );
    write_text_fixture(
        &fresh_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Continue fresh task"]}"#,
    );
    write_text_fixture(
        &fresh_root.join("EVIDENCE_INDEX.json"),
        r#"{"artifacts":[]}"#,
    );
    write_text_fixture(
        &fresh_root.join("TRACE_METADATA.json"),
        r#"{"task":"fresh task","matched_skills":["autopilot"]}"#,
    );
    write_text_fixture(
        &stale_root.join("SESSION_SUMMARY.md"),
        "- task: stale task\n- phase: implementation\n- status: in_progress\n",
    );
    write_text_fixture(
        &current_root.join("active_task.json"),
        r#"{"task_id":"fresh-task"}"#,
    );
    write_text_fixture(
        &current_root.join("focus_task.json"),
        r#"{"task_id":"fresh-task"}"#,
    );
    write_text_fixture(
        &current_root.join("task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"fresh-task","tasks":[{"task_id":"fresh-task"}]}"#,
    );
    write_text_fixture(
        &repo_root.join(".supervisor_state.json"),
        r#"{
                "task_id":"stale-task",
                "task_summary":"stale task",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true}
            }"#,
    );

    let payload =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("build snapshot");
    let snapshot = &payload["runtime_snapshot"];
    assert_eq!(snapshot["active_task_id"], json!("fresh-task"));
    assert_eq!(snapshot["continuity"]["state"], json!("inconsistent"));
    let reasons = snapshot["continuity"]["inconsistency_reasons"]
        .as_array()
        .expect("inconsistency reasons")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("supervisor task_id 'stale-task' disagrees")));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn framework_runtime_snapshot_surfaces_invalid_task_registry_json() {
    let repo_root = temp_dir_path("framework-runtime-invalid-registry");
    let current_root = repo_root.join("artifacts/current");
    fs::create_dir_all(&current_root).unwrap();
    fs::write(current_root.join("task_registry.json"), "{truncated").unwrap();
    let payload =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let reasons = payload["runtime_snapshot"]["control_plane_inconsistency_reasons"]
        .as_array()
        .expect("control plane reasons");
    let joined = reasons
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(joined.contains("invalid control-plane json"));
    assert!(joined.contains("task_registry.json"));
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn stdio_request_rejects_unknown_operations() {
    let response = handle_stdio_json_line(r#"{"id":"req-1","op":"not-supported","payload":{}}"#);
    assert!(!response.ok);
    assert_eq!(response.id, json!("req-1"));
    assert!(response
        .error
        .expect("error")
        .contains("unsupported stdio operation"));
}

#[test]
fn stdio_dispatch_domain_classification_covers_known_ops() {
    assert!(is_routing_stdio_op("route_report"));
    assert!(is_runtime_stdio_op("runtime_storage"));
    assert!(is_trace_stdio_op("trace_stream_replay"));
    assert!(is_framework_stdio_op("framework_prompt_compression"));
    assert!(is_framework_stdio_op("framework_hook_evidence_append"));
    assert!(is_framework_stdio_op("framework_autopilot_goal"));
    assert!(is_framework_stdio_op("framework_rfv_loop"));
    assert!(!is_routing_stdio_op("framework_prompt_compression"));
    assert!(!is_runtime_stdio_op("trace_record_event"));
    assert!(matches!(
        classify_stdio_op("route_report"),
        Some(StdioOpDomain::Routing)
    ));
    assert!(matches!(
        classify_stdio_op("runtime_storage"),
        Some(StdioOpDomain::Runtime)
    ));
    assert!(matches!(
        classify_stdio_op("trace_stream_replay"),
        Some(StdioOpDomain::Trace)
    ));
    assert!(matches!(
        classify_stdio_op("framework_prompt_compression"),
        Some(StdioOpDomain::Framework)
    ));
    assert!(matches!(
        classify_stdio_op("framework_autopilot_goal"),
        Some(StdioOpDomain::Framework)
    ));
    assert!(matches!(
        classify_stdio_op("framework_rfv_loop"),
        Some(StdioOpDomain::Framework)
    ));
}

#[test]
fn stdio_framework_autopilot_goal_roundtrip() {
    let repo_root = temp_dir_path("stdio-autopilot-goal");
    let _ = fs::remove_dir_all(&repo_root);
    let output_dir = repo_root.join("artifacts").join("current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "ag-stdio-task",
        "task": "autopilot goal stdio",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed session");

    let rr = repo_root.display().to_string();
    let req = json!({
        "id": "ag-1",
        "op": "framework_autopilot_goal",
        "payload": {
            "repo_root": rr,
            "operation": "start",
            "goal": "finish macro task",
            "done_when": ["ci green"],
            "drive_until_done": true
        }
    });
    let line = serde_json::to_string(&req).expect("serialize stdio line");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["rfv_loop_superseded"], json!(false));

    let path = repo_root.join("artifacts/current/ag-stdio-task/GOAL_STATE.json");
    assert!(path.is_file(), "missing {}", path.display());

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn stdio_framework_rfv_loop_roundtrip() {
    let repo_root = temp_dir_path("stdio-rfv-loop");
    let _ = fs::remove_dir_all(&repo_root);
    let output_dir = repo_root.join("artifacts").join("current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "rfv-stdio-task",
        "task": "rfv stdio",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed session");

    let rr = repo_root.display().to_string();
    let start = json!({
        "id": "rfv-1",
        "op": "framework_rfv_loop",
        "payload": {
            "repo_root": rr,
            "operation": "start",
            "goal": "deepen RFV",
            "max_rounds": 100u64,
            "allow_external_research": true,
            "verify_commands": ["cargo test -q"],
        }
    });
    let line = serde_json::to_string(&start).expect("serialize");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["goal_state_cleared"], json!(false));

    let path = repo_root.join("artifacts/current/rfv-stdio-task/RFV_LOOP_STATE.json");
    assert!(path.is_file(), "missing {}", path.display());

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn stdio_request_routes_common_ops_to_expected_domains() {
    let routing = dispatch_stdio_json_request("concurrency_defaults", json!({}))
        .expect("routing op should resolve");
    assert!(routing.get("router_stdio").is_some());

    let runtime_error = dispatch_stdio_json_request("runtime_storage", json!({}))
        .expect_err("runtime op should parse runtime storage payload");
    assert!(runtime_error.contains("parse runtime storage input failed"));

    let trace_error = dispatch_stdio_json_request("trace_compact", json!({}))
        .expect_err("trace op should parse trace compact payload");
    assert!(trace_error.contains("parse trace compact input failed"));

    let framework_error = dispatch_stdio_json_request("framework_runtime_snapshot", json!({}))
        .expect_err("framework op should require repo_root");
    assert!(framework_error.contains("repo_root"));
}

#[test]
fn stdio_request_dispatches_route_snapshot_payload() {
    let response = handle_stdio_json_line(
        r#"{"id":2,"op":"route_snapshot","payload":{"engine":"rust","selected_skill":"router","overlay_skill":null,"layer":"L2","score":42.0,"reasons":["matched"]}}"#,
    );
    assert!(response.ok);
    assert_eq!(response.id, json!(2));
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["snapshot_schema_version"],
        json!(ROUTE_SNAPSHOT_SCHEMA_VERSION)
    );
    assert_eq!(payload["route_snapshot"]["selected_skill"], json!("router"));
}

#[test]
fn stdio_route_supports_inline_skill_catalog_and_token_budget_bias() {
    let response = handle_stdio_json_line(
        r#"{"id":4,"op":"route","payload":{"query":"这是多阶段任务，但只要 bounded sidecar，保留主线程集成，降低 token 开销，不要 team orchestration","session_id":"inline-route","allow_overlay":true,"first_turn":true,"skills":[{"name":"agent-swarm-orchestration","description":"Decide whether work should stay local, use bounded sidecars, or escalate to team orchestration.","routing_layer":"L0","routing_owner":"gate","routing_gate":"delegation","routing_priority":"P1","trigger_hints":["subagent","sidecar","delegation"]},{"name":"team","description":"Supervisor-led worker lifecycle with integration qa cleanup and resume phases.","routing_layer":"L0","routing_owner":"owner","routing_gate":"none","routing_priority":"P1","trigger_hints":["team orchestration","supervisor","worker lifecycle","integration","qa","cleanup"]},{"name":"deepinterview","description":"Evidence-first clarification and convergence review.","routing_layer":"L1","routing_owner":"owner","routing_gate":"none","routing_priority":"P1","trigger_hints":["deepinterview","review"]}]}}"#,
    );
    assert!(response.ok, "{:?}", response.error);
    let payload = response.payload.expect("payload");
    assert_eq!(
        payload["selected_skill"],
        json!("agent-swarm-orchestration")
    );
    assert_eq!(payload["overlay_skill"], Value::Null);
    let reasons = payload["reasons"]
        .as_array()
        .expect("route reasons array")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("Token-budget boost applied")));
}

#[test]
fn runtime_storage_operation_round_trips_filesystem_payload() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let storage_root = std::env::temp_dir();
    let storage_root_text = storage_root.display().to_string();
    let path = storage_root.join(format!("router-rs-runtime-storage-{nonce}.txt"));
    let _ = fs::remove_file(&path);

    let write = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: Some("alpha".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("write payload");
    assert_eq!(write.schema_version, RUNTIME_STORAGE_SCHEMA_VERSION);
    assert_eq!(write.authority, RUNTIME_STORAGE_AUTHORITY);
    assert!(write.exists);
    assert_eq!(write.bytes_written, Some(5));
    assert_eq!(
        write.backend_capabilities["supports_atomic_replace"],
        json!(true)
    );
    assert_eq!(
        write.payload_sha256.as_deref(),
        Some("8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8")
    );

    let append = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "append_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: Some("-beta".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("append payload");
    assert!(append.exists);
    assert_eq!(append.bytes_written, Some(5));
    assert_eq!(
        append.payload_sha256.as_deref(),
        Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
    );

    let read = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text.clone()),
        payload_text: None,
        expected_sha256: Some(
            "a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432".to_string(),
        ),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("read payload");
    assert_eq!(read.payload_text.as_deref(), Some("alpha-beta"));
    assert_eq!(read.verified, Some(true));
    assert_eq!(
        read.payload_sha256.as_deref(),
        Some("a8b405ab6f00d98196baf634c9d1cb02b03a801770775effca822c7abe8cf432")
    );

    let verify = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "verify_text".to_string(),
        path: path.display().to_string(),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        storage_root: Some(storage_root_text),
        payload_text: None,
        expected_sha256: Some(
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        ),
        max_bytes: None,
        tail_lines: None,
    })
    .expect("verify payload");
    assert_eq!(verify.verified, Some(false));

    let _ = fs::remove_file(path);
}

#[test]
fn runtime_storage_operation_round_trips_sqlite_payload() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("router-rs-runtime-storage-root-{nonce}"));
    let db_path = root.join("runtime_checkpoint_store.sqlite3");
    let artifact_path = root.join("runtime-data").join("TRACE_RESUME_MANIFEST.json");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create sqlite root");

    let write = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "write_text".to_string(),
        path: artifact_path.display().to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: Some("{\"status\":\"ok\"}".to_string()),
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("sqlite write payload");
    assert_eq!(write.backend_family, "sqlite");
    assert_eq!(
        write.backend_capabilities["supports_sqlite_wal"],
        json!(true)
    );
    assert_eq!(
        write.sqlite_db_path.as_deref(),
        Some(db_path.display().to_string().as_str())
    );
    assert_eq!(
        write.storage_root.as_deref(),
        Some(root.display().to_string().as_str())
    );
    assert!(db_path.exists());

    let read = runtime_storage_operation(RuntimeStorageRequestPayload {
        operation: "read_text".to_string(),
        path: artifact_path.display().to_string(),
        backend_family: "sqlite".to_string(),
        sqlite_db_path: Some(db_path.display().to_string()),
        storage_root: Some(root.display().to_string()),
        payload_text: None,
        expected_sha256: None,
        max_bytes: None,
        tail_lines: None,
    })
    .expect("sqlite read payload");
    assert_eq!(read.payload_text.as_deref(), Some("{\"status\":\"ok\"}"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_checkpoint_control_plane_normalizes_backend_family_catalog() {
    let root = temp_dir_path("checkpoint-control-plane");
    let response = build_checkpoint_control_plane_compiler_payload(json!({
            "control_plane_descriptor": {
                "schema_version": "router-rs-runtime-control-plane-v1",
                "authority": "rust-runtime-control-plane",
                "services": {
                    "trace": {
                        "authority": "rust-runtime-control-plane",
                        "role": "trace-and-handoff",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-trace-store"
                    },
                    "state": {
                        "authority": "rust-runtime-control-plane",
                        "role": "durable-background-state",
                        "projection": "rust-native-projection",
                        "delegate_kind": "filesystem-state-store"
                    }
                }
            },
            "capabilities": {
                "backend_family": "sqlite3",
                "store_backend_family": "sqlite",
                "trace_backend_family": "sqlite",
                "state_backend_family": "sqlite"
            },
            "paths": {
                "trace_output_path": root.join("TRACE_METADATA.json").display().to_string(),
                "event_stream_path": root.join("TRACE_EVENTS.jsonl").display().to_string(),
                "resume_manifest_path": root.join("TRACE_RESUME_MANIFEST.json").display().to_string(),
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect("checkpoint control plane");
    let control_plane = &response["checkpoint_control_plane"];

    assert_eq!(control_plane["backend_family"], json!("sqlite"));
    assert_eq!(
        control_plane["trace_service"]["delegate_kind"],
        json!("filesystem-trace-store")
    );
    assert_eq!(
        control_plane["state_service"]["delegate_kind"],
        json!("filesystem-state-store")
    );
    assert_eq!(control_plane["supports_compaction"], json!(true));
    assert_eq!(control_plane["supports_snapshot_delta"], json!(true));
    assert_eq!(control_plane["supports_consistent_append"], json!(true));
    assert_eq!(control_plane["supports_sqlite_wal"], json!(true));
    assert_eq!(
        control_plane["backend_family_catalog"]["strongest_local_backend_family"],
        json!("sqlite")
    );
    assert_eq!(
        control_plane["backend_family_parity"]["aligned"],
        json!(true)
    );
    assert_eq!(
        control_plane["backend_family_parity"]["compaction_eligible"],
        json!(true)
    );
}

#[test]
fn runtime_checkpoint_control_plane_rejects_mixed_backend_families() {
    let root = temp_dir_path("checkpoint-control-plane-mismatch");
    let err = build_checkpoint_control_plane_compiler_payload(json!({
            "capabilities": {
                "backend_family": "sqlite",
                "store_backend_family": "filesystem"
            },
            "paths": {
                "background_state_path": root.join("runtime_background_jobs.json").display().to_string(),
                "event_transport_dir": root.join("runtime_event_transports").display().to_string()
            }
        }))
        .expect_err("mixed backend families should fail closed");

    assert!(err.contains("backend family mismatch"));
}

#[test]
fn stdio_route_cache_reuses_records_until_runtime_changes() {
    let runtime_path = temp_json_path("routing-runtime");
    let manifest_path = temp_json_path("routing-manifest");
    write_runtime_fixture(&runtime_path, "alpha");
    write_manifest_fixture(&manifest_path, "alpha", "P1");

    let first = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("first cache load");
    let second = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("second cache load");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first[0].slug, "alpha");

    sleep(Duration::from_millis(20));
    write_runtime_fixture(&runtime_path, "beta");

    let third = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))
        .expect("reload after runtime change");
    assert!(!Arc::ptr_eq(&second, &third));
    assert_eq!(third[0].slug, "beta");

    let _ = fs::remove_file(runtime_path);
    let _ = fs::remove_file(manifest_path);
}

#[test]
fn route_records_cache_refreshes_default_runtime_path() {
    let repo_root = temp_dir_path("routing-default-runtime");
    let skills_dir = repo_root.join("skills");
    fs::create_dir_all(&skills_dir).expect("create skills dir");
    let runtime_path = skills_dir.join("SKILL_ROUTING_RUNTIME.json");
    write_runtime_fixture(&runtime_path, "default-alpha");

    let first = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("first default load");
    let second = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("second default load");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first[0].slug, "default-alpha");

    sleep(Duration::from_millis(20));
    write_runtime_fixture(&runtime_path, "default-beta");

    let third = load_records_cached_for_stdio_with_default_runtime_path(&runtime_path, None)
        .expect("refreshed default load");
    assert!(!Arc::ptr_eq(&second, &third));
    assert_eq!(third[0].slug, "default-beta");

    let _ = fs::remove_dir_all(repo_root);
}

#[test]
fn route_decision_fixture_expectations_hold() {
    let fixture = fixture_path();
    let records = load_records_from_manifest(&fixture).expect("load fixture records");
    let payload = read_json(&fixture).expect("read fixture");
    let cases = payload
        .get("cases")
        .and_then(Value::as_array)
        .expect("cases array");

    for case in cases {
        let case_name = case
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>");
        let query = case
            .get("query")
            .and_then(Value::as_str)
            .expect("case query");
        let expected = case.get("expected").expect("case expected");
        let allow_overlay = case
            .get("allow_overlay")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let first_turn = case
            .get("first_turn")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let decision = route_task(
            &records,
            query,
            "fixture-session",
            allow_overlay,
            first_turn,
        )
        .expect("route task");

        assert_eq!(
            decision.selected_skill,
            expected
                .get("selected_skill")
                .and_then(Value::as_str)
                .expect("selected_skill"),
            "selected_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.overlay_skill,
            expected
                .get("overlay_skill")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
            "overlay_skill mismatch for {case_name}: {:?}",
            decision.reasons
        );
        assert_eq!(
            decision.layer,
            expected
                .get("layer")
                .and_then(Value::as_str)
                .expect("expected layer"),
            "layer mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.selected_skill, decision.selected_skill,
            "snapshot selected_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.overlay_skill, decision.overlay_skill,
            "snapshot overlay_skill mismatch for {case_name}"
        );
        assert_eq!(
            decision.route_snapshot.layer, decision.layer,
            "snapshot layer mismatch for {case_name}"
        );
        if let Some(expected_route_context) = expected.get("route_context") {
            assert_eq!(
                serde_json::to_value(&decision.route_context).expect("serialize route context"),
                expected_route_context.clone(),
                "route_context mismatch for {case_name}"
            );
        }
    }
}

#[test]
fn routing_eval_report_matches_expected_baseline() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records =
        load_records(Some(&runtime_path), Some(&manifest_path)).expect("load routing records");
    let cases =
        load_routing_eval_cases(&routing_eval_case_path()).expect("load routing eval cases");
    let report = evaluate_routing_cases(&records, cases).expect("evaluate routing cases");

    assert_eq!(report.schema_version, "routing-eval-v1");
    let expected_case_count = read_json(&routing_eval_case_path())
        .expect("read routing eval cases")["cases"]
        .as_array()
        .expect("routing eval case array")
        .len();
    assert_eq!(report.metrics.case_count, expected_case_count);
    assert_eq!(report.metrics.overtrigger, 0);
    assert_routing_eval_cases_match("runtime+manifest", |task, session_id, first_turn| {
        route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            Some(&manifest_path),
            task,
            session_id,
            true,
            first_turn,
        )
    });
}

#[test]
fn routing_eval_runtime_fallback_matches_expected_baseline() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    assert_routing_eval_cases_match("runtime-fallback", |task, session_id, first_turn| {
        route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            task,
            session_id,
            true,
            first_turn,
        )
    });
}

#[test]
fn runtime_fallback_prefers_framework_manifest_owner_over_low_score_hot_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "review framework snapshot route continuity integration risk",
        "framework-low-hot-gate",
        true,
        true,
    )
    .expect("route framework review query");

    assert_eq!(decision.selected_skill, "skill-framework-developer");
}

#[test]
fn confident_hot_route_does_not_parse_implicit_malformed_manifest() {
    let repo_root = temp_dir_path("malformed-implicit-manifest");
    let skills_root = repo_root.join("skills");
    fs::create_dir_all(&skills_root).expect("create skills root");
    let runtime_path = skills_root.join("SKILL_ROUTING_RUNTIME.json");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json"),
        &runtime_path,
    )
    .expect("copy hot runtime");
    write_text_fixture(
        &skills_root.join("SKILL_MANIFEST.json"),
        "{ not valid json\n",
    );
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "inspect sentry production errors",
        "confident-hot-route",
        true,
        true,
    )
    .expect("confident hot route should not parse implicit malformed manifest");

    assert_eq!(decision.selected_skill, "sentry");
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Manifest fallback unavailable")),
        "degraded fallback should stay observable in routing reasons"
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn runtime_declared_manifest_fallback_resolves_relative_to_runtime_directory() {
    let repo_root = temp_dir_path("runtime-relative-fallback");
    let skills_root = repo_root.join("skills");
    fs::create_dir_all(&skills_root).expect("create skills root");
    let runtime_path = skills_root.join("SKILL_ROUTING_RUNTIME.json");
    let fallback_path = skills_root.join("nested/SKILL_MANIFEST.json");
    fs::create_dir_all(fallback_path.parent().expect("fallback parent"))
        .expect("create fallback parent");
    fs::write(
        &runtime_path,
        serde_json::to_string(&json!({
            "scope": {
                "fallback_manifest": "nested/SKILL_MANIFEST.json"
            }
        }))
        .expect("serialize runtime payload"),
    )
    .expect("write runtime payload");
    fs::write(
        &fallback_path,
        serde_json::to_string(&json!({"skills": []})).expect("serialize fallback payload"),
    )
    .expect("write fallback payload");

    let resolved = resolve_runtime_declared_manifest_fallback(&runtime_path)
        .expect("resolve fallback path")
        .expect("declared fallback path should exist");
    assert_eq!(resolved, fallback_path);

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn pr_triage_summary_routes_to_github_source_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "pull request summary",
        "reviewer feedback digest",
        "changed-file digest",
        "PR triage changed file digest",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            query,
            &format!("pr-triage::{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route PR triage query {query}: {err}"));

        assert_eq!(
            decision.selected_skill, "gh-address-comments",
            "PR triage query should stay on GitHub source gate: {query}; reasons: {:?}",
            decision.reasons
        );
    }
}

#[test]
fn pr_summary_ci_context_routes_to_ci_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "pull request summary CI failure",
        "github actions pull request summary failing checks",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            query,
            &format!("pr-summary-ci::{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route PR summary CI query {query}: {err}"));

        assert_eq!(
            decision.selected_skill, "gh-fix-ci",
            "PR summary mixed with CI failure should use CI gate: {query}; reasons: {:?}",
            decision.reasons
        );
    }
}

#[test]
fn framework_command_aliases_require_literal_entrypoints() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");
    assert!(records.iter().any(|record| record.slug == "autopilot"));
    assert!(records.iter().any(|record| record.slug == "deepinterview"));
    assert!(records.iter().any(|record| record.slug == "gitx"));
    assert!(records.iter().any(|record| record.slug == "team"));

    let autopilot = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "/autopilot",
        "alias-autopilot",
        true,
        true,
    )
    .expect("route explicit autopilot alias");
    assert_eq!(autopilot.selected_skill, "autopilot");

    let team = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "/team",
        "alias-team",
        true,
        true,
    )
    .expect("route explicit team alias");
    assert_eq!(team.selected_skill, "team");

    let deepinterview = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "/deepinterview",
        "alias-deepinterview",
        true,
        true,
    )
    .expect("route explicit deepinterview alias");
    assert_eq!(deepinterview.selected_skill, "deepinterview");

    let gitx = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "gitx",
        "alias-gitx",
        true,
        true,
    )
    .expect("route explicit gitx alias");
    assert_eq!(gitx.selected_skill, "gitx");

    let natural_language_team = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "需要 team orchestration 多 agent 执行",
        "natural-language-team",
        true,
        true,
    )
    .expect("route natural language team ask");
    assert_eq!(
        natural_language_team.selected_skill,
        "agent-swarm-orchestration"
    );

    for (query, forbidden) in [("autopilot", "autopilot"), ("team", "team")] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            query,
            &format!("negative-{forbidden}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route negative case {query}: {err}"));
        assert_ne!(
            decision.selected_skill, forbidden,
            "generic query {query:?} should not select {forbidden}"
        );
    }

    for query in ["make a plan", "write a small helper function"] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            query,
            &format!("native-runtime-{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route native runtime case {query}: {err}"));
        assert_eq!(decision.selected_skill, "none");
        assert_eq!(decision.overlay_skill, None);
    }
}

#[test]
fn manifest_fallback_preserves_runtime_visual_review_gate() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let records = load_records(Some(&runtime_path), None).expect("load hot runtime records");

    for query in [
        "review this screenshot UI",
        "audit this rendered chart screenshot",
    ] {
        let decision = route_task_with_manifest_fallback(
            &records,
            Some(&runtime_path),
            None,
            query,
            &format!("visual-review-{query}"),
            true,
            true,
        )
        .unwrap_or_else(|err| panic!("route visual review case {query}: {err}"));
        assert_eq!(decision.selected_skill, "visual-review");
    }

    let capture = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        "take a screenshot",
        "screenshot-capture",
        true,
        true,
    )
    .expect("route screenshot capture case");
    assert_eq!(capture.selected_skill, "screenshot");
}

#[test]
fn explicit_manifest_preserves_native_runtime_for_low_confidence_hits() {
    let runtime_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records(Some(&runtime_path), Some(&manifest_path))
        .expect("load hot runtime records with manifest metadata");

    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        Some(&manifest_path),
        "帮我写一个 Python 脚本，并补 pytest 回归测试",
        "explicit-manifest-native-runtime",
        true,
        true,
    )
    .expect("route explicit manifest native runtime case");

    assert_eq!(decision.selected_skill, "none");
    assert_eq!(decision.overlay_skill, None);
    assert_eq!(decision.layer, "runtime");
}

#[test]
fn search_uses_route_scorer_for_framework_review() {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records_from_manifest(&manifest_path).expect("load routing records");

    let rows = search_skills(&records, "DESIGN.md 设计规范 token", 5);

    assert_eq!(rows.first().map(|row| row.slug.as_str()), Some("design-md"));
    assert!(!rows.iter().any(|row| row.slug == "css-pro"));
}

#[test]
fn generic_xlsx_intake_hits_spreadsheet_gate_first() {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    let records = load_records_from_manifest(&manifest_path).expect("load routing records");

    let decision = route_task(
        &records,
        "整理这个 xlsx 表格",
        "artifact-gate-test",
        true,
        true,
    )
    .expect("route task");

    assert_eq!(decision.selected_skill, "spreadsheets");
}

#[test]
fn route_diff_report_matches_shadow_compare_contract() {
    let rust_snapshot = build_route_snapshot(
        "rust",
        "autopilot",
        Some("deepinterview"),
        "L2",
        39.0,
        &["Trigger phrase matched: 直接做代码.".to_string()],
    );

    let report = build_route_diff_report("shadow", rust_snapshot, None).expect("shadow report");

    assert_eq!(report.report_schema_version, ROUTE_REPORT_SCHEMA_VERSION);
    assert_eq!(report.authority, ROUTE_AUTHORITY);
    assert_eq!(report.mode, "shadow");
    assert_eq!(report.primary_engine, "rust");
    assert_eq!(report.evidence_kind, "rust-owned-snapshot");
    assert!(!report.strict_verification);
    assert!(report.verification_passed);
    assert!(report.verified_contract_fields.is_empty());
    assert!(report.contract_mismatch_fields.is_empty());
    assert_eq!(report.route_snapshot.engine, "rust");
}

#[test]
fn route_policy_matches_mode_matrix() {
    let shadow = build_route_policy("shadow").expect("shadow policy");
    assert_eq!(shadow.diagnostic_route_mode, "shadow");
    assert_eq!(shadow.primary_authority, "rust");
    assert_eq!(shadow.route_result_engine, "rust");
    assert!(shadow.diagnostic_report_required);
    assert!(!shadow.strict_verification_required);

    let verify = build_route_policy("verify").expect("verify policy");
    assert_eq!(verify.diagnostic_route_mode, "verify");
    assert_eq!(verify.primary_authority, "rust");
    assert_eq!(verify.route_result_engine, "rust");
    assert!(verify.diagnostic_report_required);
    assert!(verify.strict_verification_required);

    let rust = build_route_policy("rust").expect("rust policy");
    assert_eq!(rust.diagnostic_route_mode, "none");
    assert_eq!(rust.primary_authority, "rust");
    assert_eq!(rust.route_result_engine, "rust");
    assert!(!rust.diagnostic_report_required);
    assert!(!rust.strict_verification_required);

    let unsupported = build_route_policy("python").expect_err("unsupported route mode");
    assert!(unsupported.contains("unsupported route policy mode"));
}

#[test]
fn runtime_control_plane_payload_is_rust_owned() {
    let payload = build_runtime_control_plane_payload();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_CONTROL_PLANE_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["default_route_mode"],
        Value::String("rust".to_string())
    );
    assert_eq!(
        payload["default_route_authority"],
        Value::String(ROUTE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["runtime_status"]["runtime_primary_owner"],
        Value::String("rust-control-plane".to_string())
    );
    assert_eq!(
        payload["runtime_status"]["hot_path_projection_mode"],
        Value::String("descriptor-driven".to_string())
    );
    assert!(payload["runtime_status"]
        .get("framework_runtime_package_status")
        .is_none());
    assert_eq!(
        payload["runtime_status"]["framework_runtime_replacement"],
        Value::String("router-rs::framework_runtime".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["role"],
        Value::String("runtime-orchestration".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["startup_order"][0],
        Value::String("router".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_default_size"],
        json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_max_size"],
        json!(MAX_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["services"]["middleware"]["subagent_limit_contract"]
            ["max_concurrent_subagents_limit"],
        json!(MAX_CONCURRENT_SUBAGENTS_LIMIT)
    );
    assert_eq!(
        payload["runtime_host"]["shutdown_order"][0],
        Value::String("background".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["delegate_kind"],
        Value::String("rust-execution-kernel-slice".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_live_backend_impl"],
        Value::String("router-rs".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_delegate_impl"],
        Value::String("router-rs".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]
            ["execution_kernel_metadata_schema_version"],
        Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_fallback_policy"],
        Value::String(EXECUTION_KERNEL_FALLBACK_POLICY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract_by_mode"]
            [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract_by_mode"]
            [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["schema_version"],
        Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["authority"],
        Value::String(EXECUTION_KERNEL_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
            ["live_primary_required"][2],
        Value::String("execution_mode".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
            ["live_primary_passthrough"][1],
        Value::String("diagnostic_route_mode".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["defaults"]
            ["live_primary_model_id_source"],
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_live_delegate_authority"],
        Value::String("rust-execution-cli".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["schema_version"],
        Value::String("runtime-sandbox-lifecycle-v1".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["cleanup_mode"],
        Value::String("async-drain-and-recycle".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][1],
        Value::String("cleanup".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][2],
        Value::String("admit".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_schema_version"],
        Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_tracing"]
            ["response_flag"],
        Value::String("event_written".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["delegate_kind"],
        Value::String("filesystem-checkpointer".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_catalog"]
            ["strongest_local_backend_family"],
        Value::String("sqlite".to_string())
    );
    assert!(
        !payload["services"]["checkpoint"]["backend_family_catalog"]["families"]
            .as_array()
            .expect("backend family catalog")
            .iter()
            .any(|family| family["backend_family"] == "memory")
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_catalog"]["test_only_backend_families"]
            [0],
        Value::String("memory".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_parity"]["aligned"],
        Value::Bool(true)
    );
    assert_eq!(
        payload["services"]["background"]["authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["services"]["background"]["delegate_kind"],
        Value::String("rust-background-control-policy".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_schema_version"],
        Value::String(BACKGROUND_CONTROL_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["active_statuses"][4],
        Value::String("retry_claimed".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_operations"][0],
        Value::String("batch-plan".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_operations"][5],
        Value::String("retry".to_string())
    );
    assert!(payload["services"].get("agent_factory").is_none());
}

fn execution_kernel_contract_shape_fields(shape: &Value) -> Vec<String> {
    let object = shape.as_object().expect("contract shape object");
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort_unstable();
    keys
}

#[test]
fn execution_kernel_metadata_shape_consistency_regression_for_primary_and_dry_run() {
    let contracts = build_execution_kernel_contracts_by_mode();
    let live_primary = contracts
        .get(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY)
        .expect("live primary contract");
    let dry_run = contracts
        .get(EXECUTION_RESPONSE_SHAPE_DRY_RUN)
        .expect("dry run contract");
    let base_fields = execution_kernel_contract_shape_fields(live_primary);
    assert_eq!(base_fields, execution_kernel_contract_shape_fields(dry_run));
    assert_eq!(
        live_primary["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
    );
    assert_eq!(
        dry_run["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
    );
    assert_eq!(
        live_primary["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(
        dry_run["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(contracts.len(), 2);
}

#[test]
fn execution_kernel_metadata_contract_is_rust_owned() {
    let contract = build_execution_kernel_metadata_contract();

    assert_eq!(
        contract["schema_version"],
        Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        contract["steady_state_fields"][0],
        Value::String("execution_kernel_metadata_schema_version".to_string())
    );
    assert_eq!(
        contract["runtime_fields"]["shared"],
        json!(["trace_event_count", "trace_output_path"])
    );
    assert_eq!(
        contract["defaults"]["supported_response_shapes"],
        json!([
            EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
            EXECUTION_RESPONSE_SHAPE_DRY_RUN,
        ])
    );
}

#[test]
fn runtime_observability_exporter_descriptor_is_rust_owned() {
    let payload = build_runtime_observability_exporter_descriptor();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["producer_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["export_path"],
        Value::String("jsonl-plus-otel".to_string())
    );
}

#[test]
fn runtime_observability_dashboard_and_metric_record_follow_contract() {
    let catalog = build_runtime_observability_metric_catalog_payload();
    let metrics = catalog["metrics"].as_array().expect("metric catalog array");
    assert_eq!(
        catalog["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        catalog["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert!(metrics
        .iter()
        .all(|metric| metric.get("dimensions").is_some()));
    assert!(metrics
        .iter()
        .all(|metric| metric.get("base_dimensions").is_none()));

    let dashboard = runtime_observability_dashboard_schema();
    let resource_dimensions = dashboard["resource_dimensions"]
        .as_array()
        .expect("resource dimensions array");
    assert_eq!(
        dashboard["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "service.name"));
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "runtime.generation"));
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "runtime.schema_version"));

    let record = build_runtime_metric_record(json!({
        "metric_name": "runtime.route_mismatch_total",
        "value": 3,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": 2,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect("metric record");
    assert_eq!(
        record["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(record["metric_type"], Value::String("counter".to_string()));
    assert_eq!(record["unit"], Value::String("1".to_string()));
    assert_eq!(
        record["dimensions"]["runtime.stage"],
        Value::String("runtime.metric".to_string())
    );
    assert_eq!(
        record["dimensions"]["runtime.status"],
        Value::String("ok".to_string())
    );
    assert_eq!(
        record["dimensions"]["runtime.schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        record["ownership"]["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );

    let err = build_runtime_metric_record(json!({
        "metric_name": "runtime.unknown_total",
        "value": 1,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": 1,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect_err("unknown metric should fail closed");
    assert_eq!(err, "unsupported runtime metric: runtime.unknown_total");

    let err = build_runtime_metric_record(json!({
        "metric_name": "runtime.route_mismatch_total",
        "value": 1,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": -1,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect_err("negative attempts should fail closed");
    assert_eq!(
        err,
        "runtime metric record requires non-negative integer field attempt"
    );
}

#[test]
fn runtime_observability_health_snapshot_is_rust_owned() {
    let payload = build_runtime_observability_health_snapshot();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_panel_count"],
        Value::Number(serde_json::Number::from(6))
    );
    assert_eq!(
        payload["dashboard_alert_count"],
        Value::Number(serde_json::Number::from(3))
    );
    let metric_names = payload["metric_names"].as_array().expect("metric names");
    assert_eq!(metric_names.len(), 6);
    assert!(metric_names
        .iter()
        .any(|value| value == "runtime.route_mismatch_total"));
    assert_eq!(
        payload["exporter"]["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
}

#[test]
fn sandbox_control_accepts_known_edges_and_rejects_invalid_edges() {
    let accepted = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "transition".to_string(),
        current_state: Some("warm".to_string()),
        next_state: Some("busy".to_string()),
        ..sandbox_control_request_defaults()
    })
    .expect("accepted transition");
    assert_eq!(accepted.authority, SANDBOX_CONTROL_AUTHORITY);
    assert!(accepted.allowed);
    assert_eq!(accepted.reason, "transition-accepted");
    assert_eq!(accepted.resolved_state.as_deref(), Some("busy"));

    let rejected = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "transition".to_string(),
        current_state: Some("busy".to_string()),
        next_state: Some("warm".to_string()),
        ..sandbox_control_request_defaults()
    })
    .expect("rejected transition");
    assert!(!rejected.allowed);
    assert_eq!(rejected.reason, "invalid-transition");
    assert_eq!(
        rejected.error.as_deref(),
        Some("invalid sandbox transition: \"busy\" -> \"warm\"")
    );
}

#[test]
fn sandbox_control_cleanup_resolves_recycled_and_failed_targets() {
    let recycled = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "cleanup".to_string(),
        current_state: Some("draining".to_string()),
        cleanup_failed: Some(false),
        ..sandbox_control_request_defaults()
    })
    .expect("cleanup recycled response");
    assert!(recycled.allowed);
    assert_eq!(recycled.reason, "cleanup-completed");
    assert_eq!(recycled.resolved_state.as_deref(), Some("recycled"));

    let failed = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "cleanup".to_string(),
        current_state: Some("draining".to_string()),
        cleanup_failed: Some(true),
        ..sandbox_control_request_defaults()
    })
    .expect("cleanup failed response");
    assert!(failed.allowed);
    assert_eq!(failed.reason, "cleanup-failed");
    assert_eq!(failed.resolved_state.as_deref(), Some("failed"));
}

#[test]
fn sandbox_control_records_durable_event_when_requested() {
    let path = temp_trace_path("sandbox-events");
    let response = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "admit".to_string(),
        sandbox_id: Some("sandbox-1".to_string()),
        profile_id: Some("workspace".to_string()),
        current_state: Some("warm".to_string()),
        tool_category: Some("workspace_mutating".to_string()),
        capability_categories: Some(vec![
            "read_only".to_string(),
            "workspace_mutating".to_string(),
        ]),
        budget_cpu: Some(1.0),
        budget_memory: Some(1024),
        budget_wall_clock: Some(5.0),
        budget_output_size: Some(4096),
        event_log_path: Some(path.display().to_string()),
        trace_event: Some(true),
        ..sandbox_control_request_defaults()
    })
    .expect("sandbox event response");

    assert!(response.allowed);
    assert!(response.event_written);
    assert_eq!(
        response.event_schema_version.as_deref(),
        Some(SANDBOX_EVENT_SCHEMA_VERSION)
    );
    assert_eq!(
        response.effective_capabilities,
        Some(vec![
            "read_only".to_string(),
            "workspace_mutating".to_string()
        ])
    );

    let line = fs::read_to_string(&path).expect("sandbox event log");
    let event: Value = serde_json::from_str(line.trim()).expect("sandbox event json");
    assert_eq!(
        event["schema_version"],
        Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        event["kind"],
        Value::String("sandbox.execution_started".to_string())
    );
    assert_eq!(event["sandbox_id"], Value::String("sandbox-1".to_string()));
    assert_eq!(
        event["effective_capabilities"][1],
        Value::String("workspace_mutating".to_string())
    );
}

#[test]
fn sandbox_event_append_preserves_jsonl_records_under_concurrency() {
    let event_path = temp_trace_path("sandbox-events-concurrent");
    let mut workers = Vec::new();
    for seq in 0..32 {
        let path = event_path.clone();
        workers.push(spawn(move || {
            build_sandbox_control_response(SandboxControlRequestPayload {
                schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
                operation: "admit".to_string(),
                sandbox_id: Some(format!("sandbox-{seq}")),
                profile_id: Some("workspace".to_string()),
                current_state: Some("warm".to_string()),
                tool_category: Some("read_only".to_string()),
                capability_categories: Some(vec!["read_only".to_string()]),
                budget_cpu: Some(1.0),
                budget_memory: Some(1024),
                budget_wall_clock: Some(5.0),
                budget_output_size: Some(4096),
                event_log_path: Some(path.display().to_string()),
                trace_event: Some(true),
                ..sandbox_control_request_defaults()
            })
            .expect("sandbox event response");
        }));
    }
    for worker in workers {
        worker.join().expect("join sandbox worker");
    }

    let persisted = fs::read_to_string(&event_path).expect("read sandbox jsonl");
    let lines = persisted.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 32);
    let mut seen = HashSet::new();
    for line in lines {
        let event = serde_json::from_str::<Value>(line).expect("parse sandbox jsonl");
        seen.insert(
            event["sandbox_id"]
                .as_str()
                .expect("sandbox id")
                .to_string(),
        );
    }
    assert_eq!(seen.len(), 32);

    fs::remove_file(&event_path).expect("cleanup sandbox path");
}

#[test]
fn sandbox_control_rejects_admission_from_invalid_state() {
    let response = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "admit".to_string(),
        current_state: Some("failed".to_string()),
        tool_category: Some("read_only".to_string()),
        capability_categories: Some(vec!["read_only".to_string()]),
        budget_cpu: Some(1.0),
        budget_memory: Some(1024),
        budget_wall_clock: Some(5.0),
        budget_output_size: Some(4096),
        ..sandbox_control_request_defaults()
    })
    .expect("sandbox invalid admission response");

    assert!(!response.allowed);
    assert_eq!(response.reason, "admission-rejected");
    assert_eq!(response.resolved_state.as_deref(), Some("failed"));
    assert_eq!(response.quarantined, Some(true));
    assert_eq!(
        response.failure_reason.as_deref(),
        Some("invalid sandbox admission state: \"failed\" -> \"busy\"")
    );
}

fn sandbox_control_request_defaults() -> SandboxControlRequestPayload {
    SandboxControlRequestPayload {
        schema_version: String::new(),
        operation: String::new(),
        sandbox_id: None,
        profile_id: None,
        current_state: None,
        next_state: None,
        cleanup_failed: None,
        tool_category: None,
        capability_categories: None,
        dedicated_profile: None,
        budget_cpu: None,
        budget_memory: None,
        budget_wall_clock: None,
        budget_output_size: None,
        probe_cpu: None,
        probe_memory: None,
        probe_wall_clock: None,
        probe_output_size: None,
        error_kind: None,
        event_log_path: None,
        trace_event: None,
    }
}

fn background_control_request_defaults() -> BackgroundControlRequestPayload {
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

#[test]
fn background_control_enqueue_rejects_invalid_strategy_and_capacity() {
    let invalid = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "enqueue".to_string(),
        multitask_strategy: Some("pause".to_string()),
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: Some(0),
        capacity_limit: Some(4),
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("invalid strategy response");
    assert_eq!(invalid.authority, BACKGROUND_CONTROL_AUTHORITY);
    assert!(!invalid.strategy_supported);
    assert_eq!(invalid.accepted, Some(false));

    let capacity = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "enqueue".to_string(),
        multitask_strategy: Some("interrupt".to_string()),
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: Some(2),
        capacity_limit: Some(2),
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("capacity response");
    assert!(capacity.strategy_supported);
    assert_eq!(capacity.accepted, Some(false));
    assert_eq!(capacity.requires_takeover, Some(true));
    assert_eq!(capacity.reason, "capacity-rejected");
}

#[test]
fn background_control_batch_plan_resolves_group_and_lane_assignments() {
    let planned = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "batch-plan".to_string(),
        requested_parallel_group_id: Some("pgroup-contract".to_string()),
        request_parallel_group_ids: Some(vec![
            Some("pgroup-contract".to_string()),
            Some("pgroup-contract".to_string()),
        ]),
        request_lane_ids: Some(vec![Some("lane-a".to_string()), None]),
        lane_id_prefix: Some("lane".to_string()),
        batch_size: Some(2),
        ..background_control_request_defaults()
    })
    .expect("batch plan response");
    assert_eq!(planned.accepted, Some(true));
    assert_eq!(
        planned.resolved_parallel_group_id.as_deref(),
        Some("pgroup-contract")
    );
    assert_eq!(
        planned.lane_ids,
        Some(vec!["lane-a".to_string(), "lane-2".to_string()])
    );
    assert_eq!(planned.reason, "batch-plan-resolved");
    assert_eq!(planned.effect_plan.next_step, "plan_batch");

    let rejected = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "batch-plan".to_string(),
        request_parallel_group_ids: Some(vec![
            Some("pgroup-a".to_string()),
            Some("pgroup-b".to_string()),
        ]),
        batch_size: Some(2),
        ..background_control_request_defaults()
    })
    .expect("rejected batch plan response");
    assert_eq!(rejected.accepted, Some(false));
    assert_eq!(rejected.reason, "batch-plan-misaligned-parallel-group");
    assert_eq!(
            rejected.error.as_deref(),
            Some(
                "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
            )
        );
}

#[test]
fn background_control_retry_computes_backoff_and_terminal_status() {
    let retry = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry".to_string(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: Some(1),
        retry_count: Some(0),
        max_attempts: Some(2),
        backoff_base_seconds: Some(0.5),
        backoff_multiplier: Some(2.0),
        max_backoff_seconds: Some(1.0),
        ..background_control_request_defaults()
    })
    .expect("retry response");
    assert_eq!(retry.should_retry, Some(true));
    assert_eq!(retry.next_retry_count, Some(1));
    assert_eq!(retry.backoff_seconds, Some(0.5));
    assert_eq!(retry.terminal_status.as_deref(), Some("retry_scheduled"));
    assert_eq!(retry.effect_plan.next_step, "schedule_retry");
    assert_eq!(retry.effect_plan.next_retry_count, Some(1));
    assert_eq!(retry.effect_plan.backoff_seconds, Some(0.5));

    let exhausted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry".to_string(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: Some(2),
        retry_count: Some(1),
        max_attempts: Some(2),
        backoff_base_seconds: Some(0.5),
        backoff_multiplier: Some(2.0),
        max_backoff_seconds: Some(1.0),
        ..background_control_request_defaults()
    })
    .expect("retry exhausted response");
    assert_eq!(exhausted.should_retry, Some(false));
    assert_eq!(
        exhausted.terminal_status.as_deref(),
        Some("retry_exhausted")
    );
    assert_eq!(exhausted.effect_plan.next_step, "finalize_terminal");
}

#[test]
fn background_control_interrupt_resolves_finalize_and_cancel_paths() {
    let queued = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt".to_string(),
        multitask_strategy: None,
        current_status: Some("queued".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("queued interrupt response");
    assert_eq!(
        queued.resolved_status.as_deref(),
        Some("interrupt_requested")
    );
    assert_eq!(queued.finalize_immediately, Some(true));
    assert_eq!(queued.cancel_running_task, Some(false));
    assert_eq!(queued.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(queued.effect_plan.next_step, "finalize_interrupted");

    let running = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(true),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("running interrupt response");
    assert_eq!(running.finalize_immediately, Some(false));
    assert_eq!(running.cancel_running_task, Some(true));
    assert_eq!(
        running.terminal_status.as_deref(),
        Some("interrupt_requested")
    );
    assert_eq!(running.effect_plan.next_step, "request_interrupt");
}

#[test]
fn background_control_claim_resolves_running_and_suppressed_paths() {
    let queued = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "claim".to_string(),
        multitask_strategy: None,
        current_status: Some("queued".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("queued claim response");
    assert_eq!(queued.resolved_status.as_deref(), Some("running"));
    assert_eq!(queued.reason, "claim-running");
    assert_eq!(queued.effect_plan.next_step, "claim_execution");

    let interrupted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "claim".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("interrupt claim response");
    assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.reason, "claim-suppressed-interrupted");
    assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_complete_and_completion_race_resolve_terminal_status() {
    let complete = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "complete".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("complete response");
    assert_eq!(complete.terminal_status.as_deref(), Some("completed"));
    assert_eq!(complete.resolved_status.as_deref(), Some("completed"));
    assert_eq!(complete.effect_plan.next_step, "finalize_completed");

    let race_won = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "completion-race".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("completion race won response");
    assert_eq!(race_won.terminal_status.as_deref(), Some("completed"));
    assert_eq!(race_won.reason, "completion-race-won");
    assert_eq!(race_won.effect_plan.next_step, "finalize_completed");

    let race_lost = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "completion-race".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("completion race lost response");
    assert_eq!(race_lost.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(race_lost.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(race_lost.reason, "completion-race-lost");
    assert_eq!(race_lost.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_retry_claim_and_interrupt_finalize_cover_retry_lifecycle() {
    let claimed = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry-claim".to_string(),
        multitask_strategy: None,
        current_status: Some("retry_scheduled".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("retry claim response");
    assert_eq!(claimed.terminal_status.as_deref(), Some("retry_claimed"));
    assert_eq!(claimed.resolved_status.as_deref(), Some("retry_claimed"));
    assert_eq!(claimed.finalize_immediately, Some(false));
    assert_eq!(claimed.effect_plan.next_step, "claim_retry");

    let interrupted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry-claim".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("retry claim interrupted response");
    assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.reason, "retry-claim-interrupted");
    assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");

    let finalize = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt-finalize".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("interrupt finalize response");
    assert_eq!(finalize.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(finalize.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(finalize.reason, "interrupt-finalized");
    assert_eq!(finalize.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_session_release_exposes_wait_plan() {
    let release = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "session-release".to_string(),
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
        ..background_control_request_defaults()
    })
    .expect("session release response");
    assert_eq!(release.reason, "session-release-wait");
    assert_eq!(release.effect_plan.next_step, "wait_for_release");
    assert_eq!(release.effect_plan.wait_timeout_seconds, Some(5.0));
    assert_eq!(release.effect_plan.wait_poll_interval_seconds, Some(0.02));

    let backed_off = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "session-release".to_string(),
        retry_count: Some(3),
        ..background_control_request_defaults()
    })
    .expect("session release backoff response");
    assert_eq!(
        backed_off.effect_plan.wait_poll_interval_seconds,
        Some(0.0675)
    );
}

#[test]
fn route_snapshot_builder_normalizes_score_bucket_and_reasons_class() {
    let snapshot = RouteSnapshotEnvelopePayload {
        snapshot_schema_version: ROUTE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        authority: ROUTE_AUTHORITY.to_string(),
        route_snapshot: build_route_snapshot(
            "rust",
            "autopilot",
            Some("deepinterview"),
            "L2",
            39.4,
            &[
                " Trigger phrase matched: 直接做代码. ".to_string(),
                "trigger phrase matched: 直接做代码.".to_string(),
            ],
        ),
    };

    assert_eq!(
        snapshot.snapshot_schema_version,
        ROUTE_SNAPSHOT_SCHEMA_VERSION
    );
    assert_eq!(snapshot.authority, ROUTE_AUTHORITY);
    assert_eq!(snapshot.route_snapshot.engine, "rust");
    assert_eq!(snapshot.route_snapshot.selected_skill, "autopilot");
    assert_eq!(
        snapshot.route_snapshot.overlay_skill.as_deref(),
        Some("deepinterview")
    );
    assert_eq!(snapshot.route_snapshot.score_bucket, "30-39");
    assert_eq!(
        snapshot.route_snapshot.reasons_class,
        "trigger phrase matched: 直接做代码."
    );
}

#[test]
fn execute_request_dry_run_returns_rust_owned_contract() {
    let response = execute_request(sample_execute_request()).expect("execute response");

    assert_eq!(response.execution_schema_version, EXECUTION_SCHEMA_VERSION);
    assert_eq!(response.authority, EXECUTION_AUTHORITY);
    assert!(!response.live_run);
    assert_eq!(response.skill, "autopilot");
    assert_eq!(response.overlay, None);
    assert_eq!(response.usage.mode, "estimated");
    assert_eq!(response.model_id, None);
    assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
    assert_eq!(
        response.metadata["execution_kernel_metadata_schema_version"],
        EXECUTION_METADATA_SCHEMA_VERSION
    );
    assert_eq!(
        response.metadata["execution_kernel_authority"],
        EXECUTION_KERNEL_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_response_shape"],
        EXECUTION_RESPONSE_SHAPE_DRY_RUN
    );
    assert_eq!(
        response.metadata["execution_kernel_prompt_preview_owner"],
        EXECUTION_PROMPT_PREVIEW_OWNER
    );
    assert_eq!(
        response.metadata["diagnostic_route_mode"],
        Value::String("none".to_string())
    );
}

#[test]
fn live_execute_prompt_builder_produces_rust_owned_contract_prompt() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("Help with the user's request directly."));
    assert!(prompt.contains("Primary focus: autopilot"));
    assert!(!prompt.contains("Extra guidance:"));
    assert!(prompt.contains("How to reply:"));
    assert!(prompt.contains("Lead with the answer or result."));
    assert!(prompt.contains(
        "Use plain Chinese unless the user asks otherwise, and keep the wording natural."
    ));
    assert!(prompt.contains(
        "Keep the default reply short; only use a list when the content is naturally list-shaped."
    ));
    assert!(prompt.contains("Trigger phrase matched: 直接做代码."));
    assert!(prompt.contains("Execution mode: quick."));
}

#[test]
fn live_execute_prompt_builder_treats_none_as_native_runtime() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.selected_skill = "none".to_string();
    payload.overlay_skill = None;
    payload.reasons = vec![
        "No explicit skill hit; native runtime should proceed without loading a skill.".to_string(),
    ];

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("Primary focus: native runtime instructions"));
    assert!(prompt.contains("No skill body is required"));
    assert!(!prompt.contains("Primary focus: none"));
    assert!(!prompt.contains("Use the selected skill"));
}

#[test]
fn live_execute_prompt_builder_caps_task_cues_to_five_lines() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.reasons = vec![
        "cue-1".to_string(),
        "cue-2".to_string(),
        "cue-3".to_string(),
        "cue-4".to_string(),
        "cue-5".to_string(),
        "cue-6".to_string(),
    ];

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("- cue-1"));
    assert!(prompt.contains("- cue-2"));
    assert!(prompt.contains("- cue-3"));
    assert!(prompt.contains("- cue-4"));
    assert!(prompt.contains("- cue-5"));
    assert!(!prompt.contains("- cue-6"));
}

#[test]
fn live_execute_prompt_builder_does_not_add_removed_planning_contract() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.selected_skill = "deepinterview".to_string();
    payload.overlay_skill = None;
    payload.layer = "L-1".to_string();
    payload.reasons = vec!["Trigger hint matched: 先探索现状再提方案.".to_string()];

    let prompt = build_live_execute_prompt(&payload);

    assert!(!prompt.contains("Planning output:"));
    assert!(prompt.contains("Primary focus: deepinterview"));
    assert!(!prompt.contains("READ-ONLY planning route"));
    assert!(!prompt.contains("<proposed_plan>"));
}

#[test]
fn live_execute_prompt_builder_uses_deep_mode_contract_when_requested() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = None;
    payload.task = "/autopilot deep 深度调研联网能力".to_string();

    let prompt = build_live_execute_prompt(&payload);

    assert!(prompt.contains("Execution mode: deep."));
    assert!(prompt.contains("Use a deep-research structure"));
    assert!(prompt.contains("at least two independent evidence anchors"));
    assert!(!prompt.contains("Keep the default reply short;"));
}

#[test]
fn live_execute_ignores_caller_supplied_prompt_preview() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.prompt_preview = Some("Native supplied live prompt".to_string());

    let prompt = build_live_execute_prompt(&payload);
    let response = build_live_execute_response(
        &payload,
        Some(prompt.clone()),
        LiveExecuteResult {
            content: "router-rs content".to_string(),
            model_id: Some("gpt-5.4".to_string()),
            run_id: Some("run-1".to_string()),
            status: Some("stop".to_string()),
            input_tokens: 21,
            output_tokens: 13,
            total_tokens: 34,
            finish_reason: Some("stop".to_string()),
            continuation_attempted: false,
            continuation_status: None,
            continuation_error: None,
        },
    );

    assert_eq!(response.prompt_preview.as_deref(), Some(prompt.as_str()));
    assert_ne!(
        response.prompt_preview.as_deref(),
        Some("Native supplied live prompt")
    );
    assert_eq!(response.metadata["execution_kernel"], EXECUTION_KERNEL_KIND);
    assert_eq!(
        response.metadata["execution_kernel_authority"],
        EXECUTION_KERNEL_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_metadata_schema_version"],
        EXECUTION_METADATA_SCHEMA_VERSION
    );
    assert_eq!(response.metadata["research_mode"], json!("quick"));
    assert_eq!(response.metadata["finish_reason"], json!("stop"));
    assert_eq!(
        response.metadata["execution_kernel_delegate_family"],
        "rust-cli"
    );
    assert_eq!(
        response.metadata["execution_kernel_delegate_impl"],
        "router-rs"
    );
    assert_eq!(
        response.metadata["execution_kernel_live_primary"],
        "router-rs"
    );
    assert_eq!(
        response.metadata["execution_kernel_live_primary_authority"],
        EXECUTION_AUTHORITY
    );
    assert_eq!(
        response.metadata["execution_kernel_response_shape"],
        EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY
    );
    assert_eq!(
        response.metadata["execution_kernel_prompt_preview_owner"],
        EXECUTION_PROMPT_PREVIEW_OWNER
    );
    assert_eq!(
        response.metadata["execution_kernel_model_id_source"],
        EXECUTION_MODEL_ID_SOURCE
    );
}

#[test]
fn extract_chat_completion_content_accepts_string_and_part_arrays() {
    let string_payload = serde_json::json!({
        "choices": [{"message": {"content": "hello from router-rs"}}]
    });
    let parts_payload = serde_json::json!({
        "choices": [{
            "message": {
                "content": [
                    {"text": "hello "},
                    {"content": "from "},
                    {"text": "router-rs"}
                ]
            }
        }]
    });

    assert_eq!(
        extract_chat_completion_content(&string_payload).expect("string content"),
        "hello from router-rs"
    );
    assert_eq!(
        extract_chat_completion_content(&parts_payload).expect("parts content"),
        "hello from router-rs"
    );
}

#[test]
fn validate_live_execute_aggregator_base_url_accepts_public_https_domain() {
    with_execute_allowlist_env(None, || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("public https domain should be allowed");
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_http_scheme() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("http://api.openai.com/v1")
            .expect_err("http scheme should be rejected");
        assert!(err.contains("requires https"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_localhost() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("https://localhost:8443/v1")
            .expect_err("localhost should be rejected");
        assert!(err.contains("blocks localhost"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_rejects_private_ip_literal() {
    with_execute_allowlist_env(None, || {
        let err = validate_live_execute_aggregator_base_url("https://10.10.10.2/v1")
            .expect_err("private IP literal should be rejected");
        assert!(err.contains("unsafe aggregator_base_url host IP"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_allowlist_match_passes() {
    with_execute_allowlist_env(Some("api.openai.com,example.com"), || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("allowlisted host should pass");
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_allowlist_miss_rejects() {
    with_execute_allowlist_env(Some("allowed.example.com"), || {
        let err = validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect_err("non-allowlisted host should be rejected");
        assert!(err.contains("not in allowlist"));
    });
}

#[test]
fn validate_live_execute_aggregator_base_url_without_allowlist_preserves_behavior() {
    with_execute_allowlist_env(None, || {
        validate_live_execute_aggregator_base_url("https://api.openai.com/v1")
            .expect("public https domain should remain allowed without allowlist");
    });
}

#[test]
fn live_execute_deep_length_continuation_success_accumulates_usage_and_metadata() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.research_mode = Some("deep-research".to_string());
    let mut call_index = 0usize;
    let first_content = "A".repeat(DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS + 120);
    let mut captured_requests = Vec::new();
    let live_result = perform_live_execute_with_sender(&payload, "deep prompt", |body| {
        captured_requests.push(body.clone());
        call_index += 1;
        if call_index == 1 {
            return Ok((
                200,
                json!({
                    "id": "run-1",
                    "model": "gpt-5.4",
                    "choices": [{
                        "finish_reason": "length",
                        "message": {"content": first_content}
                    }],
                    "usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 20,
                        "total_tokens": 30
                    }
                })
                .to_string(),
            ));
        }
        Ok((
            200,
            json!({
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"content": "second-part"}
                }],
                "usage": {
                    "prompt_tokens": 3,
                    "completion_tokens": 7,
                    "total_tokens": 10
                }
            })
            .to_string(),
        ))
    })
    .expect("live execute should succeed");
    assert_eq!(call_index, 2);
    assert!(live_result.content.contains(&first_content));
    assert!(live_result.content.contains("second-part"));
    assert_eq!(live_result.input_tokens, 13);
    assert_eq!(live_result.output_tokens, 27);
    assert_eq!(live_result.total_tokens, 40);
    assert_eq!(live_result.finish_reason.as_deref(), Some("stop"));
    assert_eq!(live_result.continuation_status.as_deref(), Some("success"));
    assert!(live_result.continuation_error.is_none());
    let continuation_messages = captured_requests
        .get(1)
        .and_then(|body| body.get("messages"))
        .and_then(Value::as_array)
        .expect("continuation request should include messages");
    assert_eq!(continuation_messages.len(), 3);
    let assistant_message = continuation_messages
        .iter()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .expect("continuation request should include assistant tail");
    assert!(assistant_message.starts_with("[...omitted "));
    assert!(assistant_message.len() < first_content.len());
    assert!(!assistant_message.contains(&first_content));
    let response = build_live_execute_response(&payload, None, live_result);
    assert_eq!(response.metadata["research_mode"], json!("deep"));
    assert_eq!(response.metadata["continuation_attempted"], json!(true));
    assert_eq!(response.metadata["continuation_status"], json!("success"));
    assert_eq!(response.metadata["continuation_error"], Value::Null);
}

#[test]
fn live_execute_deep_length_continuation_failure_fails_open() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.research_mode = Some("deep".to_string());
    let mut call_index = 0usize;
    let live_result = perform_live_execute_with_sender(&payload, "deep prompt", |_body| {
        call_index += 1;
        if call_index == 1 {
            return Ok((
                200,
                json!({
                    "id": "run-1",
                    "model": "gpt-5.4",
                    "choices": [{
                        "finish_reason": "length",
                        "message": {"content": "first-part-only"}
                    }],
                    "usage": {
                        "prompt_tokens": 8,
                        "completion_tokens": 5,
                        "total_tokens": 13
                    }
                })
                .to_string(),
            ));
        }
        Ok((502, "{\"error\":\"bad gateway\"}".to_string()))
    })
    .expect("continuation failure should fail-open");
    assert_eq!(call_index, 2);
    assert_eq!(live_result.content, "first-part-only");
    assert_eq!(live_result.input_tokens, 8);
    assert_eq!(live_result.output_tokens, 5);
    assert_eq!(live_result.total_tokens, 13);
    assert_eq!(live_result.continuation_status.as_deref(), Some("http_502"));
    assert!(live_result
        .continuation_error
        .as_deref()
        .unwrap_or_default()
        .contains("HTTP 502"));
    let response = build_live_execute_response(&payload, None, live_result);
    assert_eq!(response.metadata["continuation_attempted"], json!(true));
    assert_eq!(response.metadata["continuation_status"], json!("http_502"));
    assert!(response.metadata["continuation_error"]
        .as_str()
        .unwrap_or_default()
        .contains("HTTP 502"));
}

#[test]
fn live_execute_retries_first_round_before_success() {
    let mut payload = sample_execute_request();
    payload.dry_run = false;
    payload.research_mode = Some("quick".to_string());
    let mut call_index = 0usize;
    let live_result = perform_live_execute_with_sender(&payload, "quick prompt", |_body| {
        call_index += 1;
        if call_index == 1 {
            return Ok((500, "{\"error\":\"transient\"}".to_string()));
        }
        Ok((
            200,
            json!({
                "id": "run-retry",
                "model": "gpt-5.4",
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"content": "retry-success"}
                }],
                "usage": {
                    "prompt_tokens": 4,
                    "completion_tokens": 6,
                    "total_tokens": 10
                }
            })
            .to_string(),
        ))
    })
    .expect("second attempt should pass");
    assert_eq!(call_index, 2);
    assert_eq!(live_result.content, "retry-success");
    assert!(!live_result.continuation_attempted);
}

#[test]
fn normalize_chat_completions_endpoint_keeps_existing_path() {
    assert_eq!(
        normalize_chat_completions_endpoint("https://api.openai.com/v1/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(
        normalize_chat_completions_endpoint("https://api.openai.com/v1"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn live_execute_http_client_is_process_cached() {
    let first = live_execute_http_client().expect("first client");
    let second = live_execute_http_client().expect("second client");

    assert!(std::ptr::eq(first, second));
}

#[test]
fn trace_stream_replay_unwraps_wrapped_events_and_supports_resume() {
    let trace_path = temp_trace_path("trace-replay");
    fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
        after_event_id: Some("evt-1".to_string()),
        limit: Some(10),
    })
    .expect("replay trace stream");

    assert_eq!(replay.schema_version, TRACE_STREAM_REPLAY_SCHEMA_VERSION);
    assert_eq!(replay.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(replay.event_count, 2);
    assert_eq!(replay.source_kind, "trace_stream");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-2".to_string())
    );
    assert_eq!(
        replay.events[0]["kind"],
        Value::String("job.completed".to_string())
    );
    assert!(!replay.has_more);
    assert_eq!(
        replay.next_cursor.expect("next cursor").event_id.as_deref(),
        Some("evt-2")
    );
    assert!(replay.latest_cursor.is_none());

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn trace_stream_inspect_reports_latest_event_metadata() {
    let trace_path = temp_trace_path("trace-inspect");
    fs::write(
            &trace_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-22T10:00:00.000Z\"}\n",
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"ts\":\"2026-04-22T10:00:01.000Z\"}}\n"
            ),
        )
        .expect("write trace stream");

    let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
    })
    .expect("inspect trace stream");

    assert_eq!(summary.schema_version, TRACE_STREAM_INSPECT_SCHEMA_VERSION);
    assert_eq!(summary.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(summary.source_kind, "trace_stream");
    assert_eq!(summary.event_count, 2);
    assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
    assert_eq!(summary.latest_event_kind.as_deref(), Some("job.completed"));
    assert_eq!(
        summary.latest_event_timestamp.as_deref(),
        Some("2026-04-22T10:00:01.000Z")
    );
    assert!(summary.latest_cursor.is_none());

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn trace_stream_replay_filters_by_scope_and_hydrates_cursor_fields() {
    let trace_path = temp_trace_path("trace-scope");
    fs::write(
            &trace_path,
            concat!(
                "{\"sink_schema_version\":\"runtime-trace-sink-v2\",\"event\":{\"session_id\":\"session-1\",\"job_id\":\"job-1\",\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:00.000Z\"}}\n",
                "{\"session_id\":\"session-1\",\"job_id\":\"job-2\",\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"stage\":\"background\",\"ts\":\"2026-04-22T10:00:01.000Z\"}\n"
            ),
        )
        .expect("write trace stream");

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-1".to_string()),
        job_id: Some("job-1".to_string()),
        stream_scope_fields: None,
        after_event_id: None,
        limit: Some(10),
    })
    .expect("replay scoped trace stream");

    assert_eq!(replay.event_count, 1);
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-1".to_string())
    );
    assert_eq!(replay.events[0]["seq"], json!(1));
    assert_eq!(replay.events[0]["generation"], json!(0));
    assert_eq!(
        replay.events[0]["cursor"],
        Value::String("g0:s1:evt-1".to_string())
    );
    assert_eq!(
        replay.latest_cursor.expect("latest cursor")["cursor"],
        Value::String("g0:s1:evt-1".to_string())
    );

    fs::remove_file(&trace_path).expect("cleanup trace stream");
}

#[test]
fn attach_runtime_event_transport_preserves_resume_manifest_resolution_on_descriptor_roundtrip() {
    let binding_artifact_path = temp_json_path("attach-transport");
    let resume_manifest_path = temp_json_path("attach-resume-manifest");
    let trace_stream_path = temp_trace_path("attach-trace-stream");

    fs::write(
        &binding_artifact_path,
        serde_json::to_string_pretty(&json!({
            "stream_id": "stream-attach-roundtrip",
            "session_id": "session-attach-roundtrip",
            "job_id": "job-attach-roundtrip",
            "binding_backend_family": "filesystem",
            "resume_mode": "after_event_id"
        }))
        .expect("serialize binding artifact"),
    )
    .expect("write binding artifact");
    fs::write(&trace_stream_path, "").expect("write empty trace stream");
    fs::write(
        &resume_manifest_path,
        serde_json::to_string_pretty(&json!({
            "session_id": "session-attach-roundtrip",
            "job_id": "job-attach-roundtrip",
            "event_transport_path": binding_artifact_path.display().to_string(),
            "trace_stream_path": trace_stream_path.display().to_string()
        }))
        .expect("serialize resume manifest"),
    )
    .expect("write resume manifest");

    let attached = attach_runtime_event_transport(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string()
    }))
    .expect("attach via resume manifest");
    let attach_descriptor = attached
        .get("attach_descriptor")
        .cloned()
        .expect("attach descriptor");
    assert_eq!(
        attach_descriptor["resolution"]["binding_artifact_path"],
        Value::String("resume_manifest".to_string())
    );
    assert_eq!(
        attach_descriptor["resolution"]["resume_manifest_path"],
        Value::String("explicit_request".to_string())
    );

    let roundtrip = attach_runtime_event_transport(json!({
        "attach_descriptor": attach_descriptor
    }))
    .expect("attach via descriptor roundtrip");
    assert_eq!(
        roundtrip["attach_descriptor"]["resolution"]["binding_artifact_path"],
        Value::String("resume_manifest".to_string())
    );
    assert_eq!(
        roundtrip["attach_descriptor"]["resolution"]["resume_manifest_path"],
        Value::String("explicit_request".to_string())
    );
    assert_eq!(
        roundtrip["binding_artifact_path"],
        Value::String(binding_artifact_path.display().to_string())
    );
    assert_eq!(
        roundtrip["resume_manifest_path"],
        Value::String(resume_manifest_path.display().to_string())
    );

    fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
    fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
    fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
}

#[test]
fn attach_runtime_event_transport_reads_sqlite_resume_manifest_trace_stream() {
    let root = temp_json_path("attach-sqlite-root")
        .with_extension("")
        .join("runtime-data");
    let db_path = root.join("runtime_checkpoint_store.sqlite3");
    let binding_artifact_path = root
        .join("runtime_event_transports")
        .join("session-sqlite__job-sqlite.json");
    let resume_manifest_path = root.join("TRACE_RESUME_MANIFEST.json");
    let trace_stream_path = root.join("TRACE_EVENTS.jsonl");

    fs::create_dir_all(binding_artifact_path.parent().expect("binding parent"))
        .expect("create sqlite fixture dir");
    let conn = rusqlite::Connection::open(&db_path).expect("open sqlite fixture");
    conn.execute(
            "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)",
            [],
        )
        .expect("create runtime storage payload table");
    for (path, payload) in [
            (
                binding_artifact_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-event-transport-v1",
                    "stream_id": "stream::job-sqlite",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "binding_backend_family": "sqlite",
                    "resume_mode": "after_event_id",
                    "cleanup_preserves_replay": true
                }))
                .expect("serialize binding"),
            ),
            (
                resume_manifest_path.clone(),
                serde_json::to_string_pretty(&json!({
                    "schema_version": "runtime-resume-manifest-v1",
                    "session_id": "session-sqlite",
                    "job_id": "job-sqlite",
                    "event_transport_path": binding_artifact_path.display().to_string(),
                    "trace_stream_path": trace_stream_path.display().to_string(),
                    "updated_at": "2026-04-23T00:00:01+00:00"
                }))
                .expect("serialize resume"),
            ),
            (
                trace_stream_path.clone(),
                "{\"event_id\":\"evt-sqlite-1\",\"kind\":\"job.started\",\"ts\":\"2026-04-23T00:00:00.000Z\"}\n".to_string(),
            ),
        ] {
            let relative_key = path
                .strip_prefix(&root)
                .expect("path under sqlite root")
                .to_string_lossy()
                .replace('\\', "/");
            let stable_key = format!(
                "{}::{}",
                root.display().to_string().replace('\\', "/"),
                relative_key
            );
            conn.execute(
                "INSERT OR REPLACE INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)",
                rusqlite::params![stable_key, payload],
            )
            .expect("insert sqlite fixture payload");
        }
    drop(conn);

    let attached = attach_runtime_event_transport(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string()
    }))
    .expect("attach via sqlite resume manifest");
    assert_eq!(
        attached["artifact_backend_family"],
        Value::String("sqlite".to_string())
    );
    assert_eq!(
        attached["trace_stream_path"],
        Value::String(trace_stream_path.display().to_string())
    );

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: Some(trace_stream_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: None,
        job_id: None,
        stream_scope_fields: None,
        after_event_id: None,
        limit: Some(10),
    })
    .expect("replay sqlite trace stream");
    assert_eq!(replay.event_count, 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-sqlite-1".to_string())
    );

    fs::remove_dir_all(root.parent().expect("fixture parent")).expect("cleanup sqlite fixture");
}

#[test]
fn background_state_operation_persists_control_plane_projection_and_health() {
    let state_path = temp_json_path("background-state-filesystem");
    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "control_plane_descriptor": {
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "state": {
                    "authority": "rust-runtime-control-plane",
                    "role": "durable-background-state",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-state-store"
                },
                "trace": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-trace-store"
                }
            }
        },
        "job_id": "job-filesystem-1",
        "mutation": {
            "status": "queued",
            "session_id": "session-filesystem-1"
        }
    }))
    .expect("filesystem background state response");

    assert_eq!(
        response["schema_version"],
        Value::String("router-rs-background-state-store-v1".to_string())
    );
    assert_eq!(
        response["authority"],
        Value::String("rust-background-state-store".to_string())
    );
    assert_eq!(
        response["health"]["runtime_control_plane_authority"],
        Value::String("rust-runtime-control-plane".to_string())
    );
    assert_eq!(
        response["health"]["runtime_control_plane_schema_version"],
        Value::String("router-rs-runtime-control-plane-v1".to_string())
    );
    assert_eq!(
        response["health"]["control_plane_projection"],
        Value::String("rust-native-projection".to_string())
    );
    assert_eq!(
        response["health"]["control_plane_delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        response["health"]["backend_family"],
        Value::String("filesystem".to_string())
    );
    assert_eq!(
        response["health"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_compaction"],
        Value::Bool(false)
    );
    assert_eq!(
        response["health"]["supports_snapshot_delta"],
        Value::Bool(false)
    );
    assert_eq!(
        response["health"]["supports_remote_event_transport"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_sqlite_wal"],
        Value::Bool(false)
    );

    let persisted = read_json(&state_path).expect("read persisted state");
    assert_eq!(
        persisted["control_plane"]["authority"],
        Value::String("rust-runtime-control-plane".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["projection"],
        Value::String("rust-native-projection".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(
        persisted["control_plane"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(
        persisted["jobs"][0]["status"],
        Value::String("queued".to_string())
    );

    let recovered = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem"
    }))
    .expect("recovered background state snapshot");
    assert_eq!(
        recovered["health"]["control_plane_delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        recovered["state"]["jobs"][0]["job_id"],
        Value::String("job-filesystem-1".to_string())
    );

    fs::remove_file(&state_path).expect("cleanup filesystem background state");
}

#[test]
fn background_state_operation_compacts_terminal_jobs_over_capacity() {
    let state_path = temp_json_path("background-state-capacity");
    for (job_id, status) in [
        ("job-1", "completed"),
        ("job-2", "failed"),
        ("job-3", "queued"),
    ] {
        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "job_id": job_id,
            "mutation": {"status": status}
        }))
        .expect("write background state fixture");
    }

    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "capacity_limit": 2
    }))
    .expect("capacity-compacted snapshot");
    let jobs = response["state"]["jobs"].as_array().expect("jobs");
    assert_eq!(jobs.len(), 2);
    assert!(jobs
        .iter()
        .any(|job| job["job_id"] == Value::String("job-3".to_string())));
    assert_eq!(response["health"]["max_background_jobs"], json!(16));
    assert_eq!(response["health"]["max_background_jobs_limit"], json!(64));

    fs::remove_file(&state_path).expect("cleanup capacity background state");
}

#[test]
fn background_state_operation_reports_sqlite_backend_capabilities() {
    let temp_dir = temp_json_path("background-state-sqlite-root")
        .parent()
        .expect("temp root parent")
        .join(format!(
            "router-rs-bg-sqlite-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
    fs::create_dir_all(&temp_dir).expect("create sqlite temp dir");
    let canonical_temp_dir = temp_dir
        .canonicalize()
        .expect("canonicalize sqlite temp dir");
    let state_path = canonical_temp_dir.join("runtime_background_jobs.json");
    let sqlite_db_path = canonical_temp_dir.join("runtime_background_jobs.sqlite");

    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "sqlite",
        "sqlite_db_path": sqlite_db_path.display().to_string(),
        "control_plane_descriptor": {
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "state": {
                    "authority": "rust-runtime-control-plane",
                    "role": "durable-background-state",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-state-store"
                }
            }
        },
        "job_id": "job-sqlite-1",
        "mutation": {
            "status": "completed",
            "session_id": "session-sqlite-1"
        }
    }))
    .expect("sqlite background state response");

    assert_eq!(
        response["health"]["control_plane_delegate_kind"],
        Value::String("sqlite-state-store".to_string())
    );
    assert_eq!(
        response["health"]["backend_family"],
        Value::String("sqlite".to_string())
    );
    assert_eq!(
        response["health"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(response["health"]["supports_compaction"], Value::Bool(true));
    assert_eq!(
        response["health"]["supports_snapshot_delta"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_remote_event_transport"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(response["health"]["supports_sqlite_wal"], Value::Bool(true));
    assert!(!state_path.exists());
    assert!(sqlite_db_path.exists());

    let recovered = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "sqlite",
        "sqlite_db_path": sqlite_db_path.display().to_string()
    }))
    .expect("recovered sqlite background state snapshot");
    assert_eq!(
        recovered["state"]["jobs"][0]["job_id"],
        Value::String("job-sqlite-1".to_string())
    );
    assert_eq!(
        recovered["health"]["control_plane_delegate_kind"],
        Value::String("sqlite-state-store".to_string())
    );

    fs::remove_dir_all(&canonical_temp_dir).expect("cleanup sqlite background state dir");
}

#[test]
fn background_state_arbitration_dispatch_requires_explicit_operation() {
    let state_path = temp_json_path("background-state-arbitration-dispatch");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session"
        }
    }))
    .expect("seed active owner");

    let reserved = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "arbitrate_session_takeover",
        "arbitration_operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("dispatch reserve arbitration");
    assert_eq!(reserved["takeover"]["operation"], json!("reserve"));
    assert_eq!(reserved["takeover"]["outcome"], json!("pending"));

    let missing = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "arbitrate_session_takeover",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-3"
    }))
    .expect_err("missing arbitration operation should fail closed");
    assert_eq!(
        missing,
        "Background state arbitration is missing arbitration_operation."
    );

    fs::remove_file(&state_path).expect("cleanup arbitration dispatch state");
}

#[test]
fn background_state_operation_arbitrates_takeover_across_persisted_roundtrip() {
    let state_path = temp_json_path("background-state-takeover");
    let control_plane_descriptor = json!({
        "schema_version": "router-rs-runtime-control-plane-v1",
        "authority": "rust-runtime-control-plane",
        "services": {
            "state": {
                "authority": "rust-runtime-control-plane",
                "role": "durable-background-state",
                "projection": "rust-native-projection",
                "delegate_kind": "filesystem-state-store"
            }
        }
    });

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "control_plane_descriptor": control_plane_descriptor,
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session",
            "claimed_by": "job-1"
        }
    }))
    .expect("seed active owner");

    let reserved = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("reserve takeover");
    assert_eq!(
        reserved["takeover"]["outcome"],
        Value::String("pending".to_string())
    );
    assert_eq!(reserved["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        reserved["takeover"]["previous_active_job_id"],
        Value::String("job-1".to_string())
    );
    assert_eq!(
        reserved["takeover"]["pending_job_id"],
        Value::String("job-2".to_string())
    );
    assert_eq!(reserved["health"]["pending_session_takeovers"], json!(1));

    let completed = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "completed",
            "session_id": "shared-session",
            "claimed_by": "job-1"
        }
    }))
    .expect("complete previous owner");
    assert_eq!(completed["health"]["active_job_count"], json!(0));

    let claimed = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "claim",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("claim takeover");
    assert_eq!(
        claimed["takeover"]["outcome"],
        Value::String("claimed".to_string())
    );
    assert_eq!(claimed["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        claimed["takeover"]["active_job_id"],
        Value::String("job-2".to_string())
    );
    assert_eq!(claimed["takeover"]["pending_job_id"], Value::Null);
    assert_eq!(claimed["health"]["pending_session_takeovers"], json!(0));

    let active = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "get_active_job",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session"
    }))
    .expect("get active job after claim");
    assert_eq!(active["active_job_id"], Value::String("job-2".to_string()));

    let persisted = read_json(&state_path).expect("read persisted takeover state");
    assert_eq!(persisted["pending_session_takeovers"], Value::Array(vec![]));
    assert_eq!(
        persisted["active_sessions"],
        Value::Array(vec![json!({
            "session_id": "shared-session",
            "job_id": "job-2"
        })])
    );

    fs::remove_file(&state_path).expect("cleanup takeover background state");
}

#[test]
fn background_state_operation_release_keeps_current_owner_when_only_pending_takeover_exists() {
    let state_path = temp_json_path("background-state-release");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session"
        }
    }))
    .expect("seed release owner");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("seed pending takeover");

    let released = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "release",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("release pending takeover");
    assert_eq!(
        released["takeover"]["outcome"],
        Value::String("released".to_string())
    );
    assert_eq!(released["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        released["takeover"]["active_job_id"],
        Value::String("job-1".to_string())
    );
    assert_eq!(released["takeover"]["pending_job_id"], Value::Null);
    assert_eq!(released["health"]["pending_session_takeovers"], json!(0));

    let active = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "get_active_job",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session"
    }))
    .expect("get active job after release");
    assert_eq!(active["active_job_id"], Value::String("job-1".to_string()));

    fs::remove_file(&state_path).expect("cleanup release background state");
}

#[test]
fn trace_compaction_inspect_and_replay_read_snapshot_plus_deltas() {
    let temp_root = temp_trace_path("trace-compaction");
    let trace_root = temp_root.parent().expect("temp root parent").join(
        temp_root
            .file_stem()
            .expect("temp root stem")
            .to_string_lossy()
            .to_string(),
    );
    let manifest_path = trace_root.join("stream.manifest.json");
    let delta_path = trace_root.join("stream.deltas.jsonl");
    let artifact_dir = trace_root.join("artifacts");
    let state_path = artifact_dir.join("stream.state.json");
    let artifact_index_path = artifact_dir.join("stream.artifacts.json");
    fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    let state_text = serde_json::to_string_pretty(&json!({
        "session_id": "session-compact",
        "job_id": "job-compact",
        "latest_cursor": {
            "schema_version": "runtime-trace-cursor-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "generation": 0,
            "seq": 2,
            "event_id": "evt-snapshot",
            "cursor": "g0:s2:evt-snapshot"
        },
        "latest_event": {
            "event_id": "evt-snapshot",
            "kind": "job.progress",
            "stage": "background",
            "status": "ok",
            "ts": "2026-04-22T10:00:00.000Z"
        }
    }))
    .expect("serialize state");
    fs::write(&state_path, &state_text).expect("write state");
    let state_digest = sha256_hex(state_text.as_bytes());
    let artifact_index_text = serde_json::to_string_pretty(&json!([
        {
            "schema_version": "runtime-trace-artifact-ref-v1",
            "artifact_id": "art-state",
            "kind": "state_ref",
            "uri": state_path.display().to_string(),
            "digest": state_digest,
            "size_bytes": state_text.len()
        }
    ]))
    .expect("serialize artifact index");
    fs::write(&artifact_index_path, &artifact_index_text).expect("write artifact index");
    let artifact_index_digest = sha256_hex(artifact_index_text.as_bytes());
    fs::write(
            &delta_path,
            concat!(
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-1\",\"parent_snapshot_id\":\"snap-1\",\"seq\":1,\"ts\":\"2026-04-22T10:00:01.000Z\",\"kind\":\"job.resumed\",\"payload\":{\"event_id\":\"evt-1\",\"cursor\":\"g1:s1:evt-1\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":3}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n",
                "{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"generation\":1,\"delta_id\":\"delta-2\",\"parent_snapshot_id\":\"snap-1\",\"seq\":2,\"ts\":\"2026-04-22T10:00:02.000Z\",\"kind\":\"job.completed\",\"payload\":{\"event_id\":\"evt-2\",\"cursor\":\"g1:s2:evt-2\",\"stage\":\"background\",\"status\":\"ok\",\"payload\":{\"step\":4}},\"artifact_refs\":[],\"applies_to\":{\"session_id\":\"session-compact\",\"job_id\":\"job-compact\"}}\n"
            ),
        )
        .expect("write deltas");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "backend_family": "filesystem",
            "compaction_supported": true,
            "snapshot_delta_supported": true,
            "latest_stable_snapshot": {
                "schema_version": "runtime-trace-compaction-snapshot-v1",
                "generation": 0,
                "snapshot_id": "snap-1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "state_digest": "state-digest",
                "artifact_index_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-index",
                    "kind": "artifact_index_ref",
                    "uri": artifact_index_path.display().to_string(),
                    "digest": artifact_index_digest,
                    "size_bytes": artifact_index_text.len()
                },
                "state_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-state",
                    "kind": "state_ref",
                    "uri": state_path.display().to_string(),
                    "digest": state_digest,
                    "size_bytes": state_text.len()
                }
            },
            "active_generation": 1,
            "active_parent_snapshot_id": "snap-1",
            "manifest_path": manifest_path.display().to_string(),
            "delta_path": delta_path.display().to_string(),
            "artifact_index_path": artifact_index_path.display().to_string(),
            "state_path": state_path.display().to_string()
        }))
        .expect("serialize manifest"),
    )
    .expect("write manifest");

    let summary = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
    })
    .expect("inspect compaction manifest");
    assert_eq!(summary.source_kind, "compaction_manifest");
    assert_eq!(summary.event_count, 2);
    assert_eq!(summary.latest_event_id.as_deref(), Some("evt-2"));
    assert_eq!(
        summary.recovery.expect("recovery")["latest_recoverable_generation"],
        json!(1)
    );

    let replay = replay_trace_stream(TraceStreamReplayRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
        after_event_id: Some("evt-1".to_string()),
        limit: Some(10),
    })
    .expect("replay compaction manifest");
    assert_eq!(replay.source_kind, "compaction_manifest");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(
        replay.events[0]["event_id"],
        Value::String("evt-2".to_string())
    );

    fs::remove_dir_all(&trace_root).expect("cleanup compaction root");
}

#[test]
fn trace_compaction_recovery_fails_closed_on_artifact_digest_mismatch() {
    let temp_root = temp_trace_path("trace-compaction-digest-mismatch");
    let trace_root = temp_root.parent().expect("temp root parent").join(
        temp_root
            .file_stem()
            .expect("temp root stem")
            .to_string_lossy()
            .to_string(),
    );
    let manifest_path = trace_root.join("stream.manifest.json");
    let artifact_dir = trace_root.join("artifacts");
    let state_path = artifact_dir.join("stream.state.json");
    let artifact_index_path = artifact_dir.join("stream.artifacts.json");
    fs::create_dir_all(&artifact_dir).expect("create digest mismatch artifact dir");
    let state_text = serde_json::to_string_pretty(&json!({
        "session_id": "session-compact",
        "job_id": "job-compact",
        "latest_cursor": {
            "schema_version": "runtime-trace-cursor-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "generation": 0,
            "seq": 1,
            "event_id": "evt-snapshot",
            "cursor": "g0:s1:evt-snapshot"
        }
    }))
    .expect("serialize digest mismatch state");
    fs::write(&state_path, &state_text).expect("write digest mismatch state");
    let artifact_index_text = "[]";
    fs::write(&artifact_index_path, artifact_index_text)
        .expect("write digest mismatch artifact index");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-trace-compaction-manifest-v1",
            "session_id": "session-compact",
            "job_id": "job-compact",
            "latest_stable_snapshot": {
                "schema_version": "runtime-trace-compaction-snapshot-v1",
                "generation": 0,
                "snapshot_id": "snap-1",
                "session_id": "session-compact",
                "job_id": "job-compact",
                "state_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-state",
                    "kind": "state_ref",
                    "uri": state_path.display().to_string(),
                    "digest": "not-the-real-digest",
                    "size_bytes": state_text.len()
                },
                "artifact_index_ref": {
                    "schema_version": "runtime-trace-artifact-ref-v1",
                    "artifact_id": "art-index",
                    "kind": "artifact_index_ref",
                    "uri": artifact_index_path.display().to_string(),
                    "digest": sha256_hex(artifact_index_text.as_bytes()),
                    "size_bytes": artifact_index_text.len()
                }
            },
            "active_generation": 1,
            "active_parent_snapshot_id": "snap-1",
            "manifest_path": manifest_path.display().to_string(),
            "artifact_index_path": artifact_index_path.display().to_string(),
            "state_path": state_path.display().to_string()
        }))
        .expect("serialize digest mismatch manifest"),
    )
    .expect("write digest mismatch manifest");

    let err = inspect_trace_stream(TraceStreamInspectRequestPayload {
        path: None,
        event_stream_text: None,
        compaction_manifest_path: Some(manifest_path.display().to_string()),
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        session_id: Some("session-compact".to_string()),
        job_id: Some("job-compact".to_string()),
        stream_scope_fields: None,
    })
    .expect_err("digest mismatch must fail closed");

    assert_eq!(
        err,
        "Compaction recovery failed closed because state_ref artifact digest mismatched."
    );

    fs::remove_dir_all(&trace_root).expect("cleanup digest mismatch compaction root");
}

#[test]
fn write_trace_compaction_delta_appends_one_jsonl_line() {
    let delta_path = temp_trace_path("trace-delta-write");
    let response = write_trace_compaction_delta(TraceCompactionDeltaWriteRequestPayload {
        path: delta_path.display().to_string(),
        delta: json!({
            "schema_version": "runtime-trace-compaction-delta-v1",
            "delta_id": "delta-1",
            "seq": 1
        }),
    })
    .expect("write delta");

    assert_eq!(
        response.schema_version,
        TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION
    );
    assert_eq!(response.authority, TRACE_STREAM_IO_AUTHORITY);
    assert_eq!(response.path, delta_path.display().to_string());
    assert!(response.bytes_written > 0);
    let persisted = fs::read_to_string(&delta_path).expect("read delta");
    assert!(persisted.contains("\"delta_id\":\"delta-1\""));

    fs::remove_file(&delta_path).expect("cleanup delta path");
}

#[test]
fn trace_append_preserves_jsonl_records_under_concurrency() {
    let trace_path = temp_trace_path("trace-record-event-concurrent");
    let mut workers = Vec::new();
    for seq in 0..32 {
        let path = trace_path.clone();
        workers.push(spawn(move || {
            record_trace_event(TraceRecordEventRequestPayload {
                path: Some(path.display().to_string()),
                write_outputs: true,
                sink_schema_version: "runtime-trace-sink-v2".to_string(),
                event_schema_version: "runtime-trace-v2".to_string(),
                generation: 1,
                seq,
                session_id: "concurrent-trace".to_string(),
                job_id: None,
                kind: "test.event".to_string(),
                stage: "append".to_string(),
                status: "ok".to_string(),
                payload: Map::new(),
                compaction_manifest_path: None,
                compaction_manifest_text: None,
            })
            .expect("record trace event");
        }));
    }
    for worker in workers {
        worker.join().expect("join trace worker");
    }

    let persisted = fs::read_to_string(&trace_path).expect("read trace jsonl");
    let lines = persisted.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 32);
    let mut seen = HashSet::new();
    for line in lines {
        let record = serde_json::from_str::<Value>(line).expect("parse trace jsonl");
        seen.insert(record["event"]["seq"].as_u64().expect("seq"));
    }
    assert_eq!(seen.len(), 32);

    fs::remove_file(&trace_path).expect("cleanup trace path");
}

#[test]
fn stdio_request_dispatches_write_trace_compaction_delta_payload() {
    let delta_path = temp_trace_path("trace-delta-write-stdio");
    let response = handle_stdio_json_line(&format!(
            "{{\"id\":2,\"op\":\"write_trace_compaction_delta\",\"payload\":{{\"path\":\"{}\",\"delta\":{{\"schema_version\":\"runtime-trace-compaction-delta-v1\",\"delta_id\":\"delta-stdio\",\"seq\":2}}}}}}",
            delta_path.display()
        ));
    assert!(response.ok);
    assert_eq!(response.id, json!(2));
    assert_eq!(
        response.payload.expect("payload")["schema_version"],
        json!(TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION)
    );
    let persisted = fs::read_to_string(&delta_path).expect("read stdio delta");
    assert!(persisted.contains("\"delta_id\":\"delta-stdio\""));
    fs::remove_file(&delta_path).expect("cleanup stdio delta path");
}

#[test]
fn write_trace_metadata_persists_primary_and_mirror_outputs() {
    let output_path = temp_json_path("trace-metadata-write");
    let mirror_path = output_path
        .parent()
        .expect("output parent")
        .join("artifacts")
        .join("current")
        .join("TRACE_METADATA.json");
    let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
        output_path: output_path.display().to_string(),
        mirror_paths: vec![mirror_path.display().to_string()],
        write_outputs: true,
        task: "trace metadata rustification".to_string(),
        matched_skills: vec!["autopilot".to_string()],
        owner: "autopilot".to_string(),
        gate: "none".to_string(),
        overlay: None,
        reroute_count: Some(0),
        retry_count: Some(1),
        artifact_paths: vec!["artifacts/current/SESSION_SUMMARY.md".to_string()],
        verification_status: "passed".to_string(),
        session_id: None,
        job_id: None,
        event_stream_path: None,
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        stream_scope_fields: None,
        framework_version: Some("phase1".to_string()),
        metadata_schema_version: Some("trace-metadata-v2".to_string()),
        routing_runtime_version: Some(9),
        runtime_path: None,
        ts: Some("2026-04-23T00:00:00Z".to_string()),
        trace_event_schema_version: None,
        trace_event_sink_schema_version: None,
        parallel_group: None,
        supervisor_projection: None,
        control_plane: None,
        stream: None,
        events: None,
    })
    .expect("write trace metadata");

    assert_eq!(response.schema_version, TRACE_METADATA_WRITE_SCHEMA_VERSION);
    assert_eq!(response.authority, TRACE_METADATA_WRITE_AUTHORITY);
    assert_eq!(response.output_path, output_path.display().to_string());
    assert_eq!(response.routing_runtime_version, 9);
    assert!(response.payload_text.contains("\"version\": 1"));
    let primary = fs::read_to_string(&output_path).expect("read primary trace metadata");
    let mirror = fs::read_to_string(&mirror_path).expect("read mirror trace metadata");
    assert_eq!(primary, mirror);
    assert!(primary.contains("\"schema_version\": \"trace-metadata-v2\""));
    assert!(primary.contains("\"task\": \"trace metadata rustification\""));

    fs::remove_file(&output_path).expect("cleanup primary trace metadata");
    fs::remove_file(&mirror_path).expect("cleanup mirror trace metadata");
    fs::remove_dir_all(
        mirror_path
            .parent()
            .and_then(Path::parent)
            .expect("cleanup mirror root"),
    )
    .expect("cleanup mirror directories");
}

#[test]
fn stdio_request_dispatches_write_trace_metadata_payload() {
    let output_path = temp_json_path("trace-metadata-write-stdio");
    let response = handle_stdio_json_line(&format!(
            "{{\"id\":3,\"op\":\"write_trace_metadata\",\"payload\":{{\"output_path\":\"{}\",\"task\":\"trace metadata stdio\",\"matched_skills\":[\"autopilot\"],\"owner\":\"autopilot\",\"gate\":\"none\",\"overlay\":null,\"reroute_count\":0,\"retry_count\":0,\"artifact_paths\":[],\"verification_status\":\"passed\",\"metadata_schema_version\":\"trace-metadata-v2\",\"routing_runtime_version\":11}}}}",
            output_path.display()
        ));
    assert!(response.ok);
    assert_eq!(response.id, json!(3));
    assert_eq!(
        response.payload.expect("payload")["schema_version"],
        json!(TRACE_METADATA_WRITE_SCHEMA_VERSION)
    );
    let persisted = fs::read_to_string(&output_path).expect("read stdio trace metadata");
    assert!(persisted.contains("\"routing_runtime_version\": 11"));
    fs::remove_file(&output_path).expect("cleanup stdio trace metadata");
}

#[test]
fn write_trace_metadata_fails_closed_for_explicit_bad_trace_source() {
    let output_path = temp_json_path("trace-metadata-bad-source");
    let missing_trace_path = temp_trace_path("trace-metadata-missing-source");
    let response = write_trace_metadata(TraceMetadataWriteRequestPayload {
        output_path: output_path.display().to_string(),
        mirror_paths: Vec::new(),
        write_outputs: true,
        task: "trace metadata missing source".to_string(),
        matched_skills: Vec::new(),
        owner: "autopilot".to_string(),
        gate: "none".to_string(),
        overlay: None,
        reroute_count: Some(0),
        retry_count: Some(0),
        artifact_paths: Vec::new(),
        verification_status: "passed".to_string(),
        session_id: None,
        job_id: None,
        event_stream_path: Some(missing_trace_path.display().to_string()),
        event_stream_text: None,
        compaction_manifest_path: None,
        compaction_manifest_text: None,
        compaction_state_text: None,
        compaction_artifact_index_text: None,
        compaction_delta_text: None,
        stream_scope_fields: None,
        framework_version: None,
        metadata_schema_version: Some("trace-metadata-v2".to_string()),
        routing_runtime_version: Some(11),
        runtime_path: None,
        ts: Some("2026-04-23T00:00:00Z".to_string()),
        trace_event_schema_version: None,
        trace_event_sink_schema_version: None,
        parallel_group: None,
        supervisor_projection: None,
        control_plane: None,
        stream: None,
        events: Some(Vec::new()),
    });

    assert!(response.is_err());
    assert!(!output_path.exists());
}

#[test]
fn write_text_payload_uses_unique_temp_paths_under_concurrency() {
    let output_path = temp_json_path("atomic-write-concurrent");
    let mut workers = Vec::new();
    for index in 0..32 {
        let path = output_path.clone();
        workers.push(spawn(move || {
            write_text_payload(&path, &format!("payload-{index}"))
                .expect("concurrent atomic write");
        }));
    }
    for worker in workers {
        worker.join().expect("join writer");
    }

    let persisted = fs::read_to_string(&output_path).expect("read final payload");
    assert!(persisted.starts_with("payload-"));
    let tmp_entries = fs::read_dir(output_path.parent().expect("output parent"))
        .expect("read temp dir")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with(
                output_path
                    .file_name()
                    .expect("file name")
                    .to_string_lossy()
                    .as_ref(),
            ) && entry.file_name().to_string_lossy().ends_with(".tmp")
        })
        .count();
    assert_eq!(tmp_entries, 0);

    fs::remove_file(&output_path).expect("cleanup concurrent write output");
}

#[test]
fn subscribe_attached_runtime_events_returns_cursor_not_event_payload() {
    let binding_artifact_path = temp_json_path("subscribe-transport");
    let resume_manifest_path = temp_json_path("subscribe-resume-manifest");
    let trace_stream_path = temp_trace_path("subscribe-trace-stream");

    fs::write(
        &binding_artifact_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-event-transport-v1",
            "stream_id": "stream::job-subscribe",
            "session_id": "session-subscribe",
            "job_id": "job-subscribe",
            "binding_backend_family": "filesystem",
            "resume_mode": "after_event_id"
        }))
        .expect("serialize binding artifact"),
    )
    .expect("write binding artifact");
    fs::write(
        &resume_manifest_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": "runtime-resume-manifest-v1",
            "session_id": "session-subscribe",
            "job_id": "job-subscribe",
            "event_transport_path": binding_artifact_path.display().to_string(),
            "trace_stream_path": trace_stream_path.display().to_string()
        }))
        .expect("serialize resume manifest"),
    )
    .expect("write resume manifest");
    fs::write(
            &trace_stream_path,
            concat!(
                "{\"event_id\":\"evt-1\",\"kind\":\"job.started\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n",
                "{\"event_id\":\"evt-2\",\"kind\":\"job.completed\",\"session_id\":\"session-subscribe\",\"job_id\":\"job-subscribe\"}\n"
            ),
        )
        .expect("write trace stream");

    let response = subscribe_attached_runtime_events(json!({
        "resume_manifest_path": resume_manifest_path.display().to_string(),
        "after_event_id": "evt-1",
        "limit": 1
    }))
    .expect("subscribe attached events");

    assert_eq!(response["events"].as_array().expect("events").len(), 1);
    assert_eq!(
        response["next_cursor"],
        json!({"event_id": "evt-2", "event_index": 1})
    );
    assert_eq!(response["next_cursor"]["kind"], Value::Null);

    fs::remove_file(&binding_artifact_path).expect("cleanup binding artifact");
    fs::remove_file(&resume_manifest_path).expect("cleanup resume manifest");
    fs::remove_file(&trace_stream_path).expect("cleanup trace stream");
}

#[test]
fn cli_parses_codex_hook_with_event_flag() {
    let cli = Cli::try_parse_from(["router-rs", "codex", "hook", "--event", "Stop"])
        .expect("parse codex hook --event");
    let Some(RouterCommand::Codex {
        command: CodexCommand::Hook(command),
    }) = cli.command
    else {
        panic!("expected codex hook command");
    };
    assert_eq!(command.event.as_deref(), Some("Stop"));
    assert_eq!(command.name.as_deref(), None);
}

#[test]
fn cli_parses_codex_hook_with_positional() {
    let cli =
        Cli::try_parse_from(["router-rs", "codex", "hook", "Stop"]).expect("parse codex hook");
    let Some(RouterCommand::Codex {
        command: CodexCommand::Hook(command),
    }) = cli.command
    else {
        panic!("expected codex hook command");
    };
    assert_eq!(command.name.as_deref(), Some("Stop"));
    assert_eq!(command.event.as_deref(), None);
}

#[test]
fn install_hooks_cli_repo_root_optional() {
    let cli = Cli::try_parse_from(["router-rs", "codex", "install-hooks"])
        .expect("parse install-hooks without repo-root");
    let Some(RouterCommand::Codex {
        command: CodexCommand::InstallHooks(command),
    }) = cli.command
    else {
        panic!("expected codex install-hooks command");
    };
    assert!(command.repo_root.is_none());
}

#[test]
fn hook_status_constants_are_stable() {
    assert_eq!(
        hook_status::REVIEW_GATE_CHECKING,
        "Checking review/subagent gate"
    );
    assert_eq!(
        hook_status::REVIEW_GATE_UPDATING,
        "Updating review/subagent gate state"
    );
    assert_eq!(
        hook_status::REVIEW_GATE_ENFORCING,
        "Enforcing review/subagent gate"
    );
}
