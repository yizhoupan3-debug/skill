use serde_json::json;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

/// 模板/模型偶发的陈旧 Review 续跑前缀（分段拼接，避免在对外文案里复述整词）。
const LEGACY_REVIEW_FOLLOWUP_TOKEN: &str = concat!("RG", "_FOLLOWUP");

/// Framework 仓 canonical 路径：终端元数据 / cwd 对齐 fixture（与 `fresh_repo()` 临时目录区分）。
const FRAMEWORK_HARNESS_TEST_CWD: &str = env!("CARGO_MANIFEST_DIR");

/// Drop 时清除 thread_local 覆盖，避免遗留应急门控语义并污染同 OS 线程上的其它用例。
struct ReviewGateDisableTestGuard;

impl ReviewGateDisableTestGuard {
    fn new() -> Self {
        ensure_test_deps();
        super::set_test_review_gate_disable_override(Some(true));
        Self
    }
}

impl Drop for ReviewGateDisableTestGuard {
    fn drop(&mut self) {
        super::set_test_review_gate_disable_override(None);
    }
}

/// 确保 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` **未**从其它用例/进程环境泄漏，避免 `dispatch` 走应急短路。
struct ReviewGateDisableEnvClearGuard {
    prev_cursor: Option<std::ffi::OsString>,
    prev_canonical: Option<std::ffi::OsString>,
}

impl ReviewGateDisableEnvClearGuard {
    fn new() -> Self {
        let prev_cursor = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        let prev_canonical = env::var_os("ROUTER_RS_REVIEW_GATE_DISABLE");
        unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE") };
        unsafe { env::remove_var("ROUTER_RS_REVIEW_GATE_DISABLE") };
        Self {
            prev_cursor,
            prev_canonical,
        }
    }
}

impl Drop for ReviewGateDisableEnvClearGuard {
    fn drop(&mut self) {
        match self.prev_cursor.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", v) },
            None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE") },
        }
        match self.prev_canonical.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_REVIEW_GATE_DISABLE", v) },
            None => unsafe { env::remove_var("ROUTER_RS_REVIEW_GATE_DISABLE") },
        }
    }
}

/// 单测需要已移除事件的完整 handler（`ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1`）。
struct LegacySubtractedEventsGuard {
    _lock: core_policy::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl LegacySubtractedEventsGuard {
    fn enable() -> Self {
        ensure_test_deps();
        let _lock = core_policy::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "1") };
        Self {
            _lock,
            prev,
        }
    }
}

impl Drop for LegacySubtractedEventsGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// Gate-active tests (`harness-minimal-my`): clear env/thread-local review-gate disable so
/// `beforeSubmit` / `stop` run real handlers (parallel bench hooks may set `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1`).
struct ReviewGateActiveGuard {
    _env: ReviewGateDisableEnvClearGuard,
}

impl ReviewGateActiveGuard {
    fn new() -> Self {
        // Force gate on regardless of parallel env pollution (`ROUTER_RS_*_REVIEW_GATE_DISABLE=1`).
        super::set_test_review_gate_disable_override(Some(false));
        Self {
            _env: ReviewGateDisableEnvClearGuard::new(),
        }
    }
}

impl Drop for ReviewGateActiveGuard {
    fn drop(&mut self) {
        super::set_test_review_gate_disable_override(None);
    }
}

/// Opt-in My implement pre-goal nudge (`ROUTER_RS_PRE_GOAL_ENABLED` — legacy env name).
struct MyPreGoalOptInEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl MyPreGoalOptInEnvGuard {
    fn enable() -> Self {
        let key = "ROUTER_RS_PRE_GOAL_ENABLED";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "1") };
        Self { prev }
    }
}

impl Drop for MyPreGoalOptInEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_PRE_GOAL_ENABLED";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// 单测临时设置 `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=0`，Drop 时还原。
struct SpawnFirstNudgeDisableEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl SpawnFirstNudgeDisableEnvGuard {
    fn disable() -> Self {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "0") };
        Self { prev }
    }
}

impl Drop for SpawnFirstNudgeDisableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// 单测临时设置 `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=1`，Drop 时还原。
struct SpawnFirstNudgeEnableEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl SpawnFirstNudgeEnableEnvGuard {
    fn enable() -> Self {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "1") };
        Self { prev }
    }
}

impl Drop for SpawnFirstNudgeEnableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// 单测临时设置 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE=0`，Drop 时还原。
struct SubagentModelInheritNudgeDisableEnvGuard {
    _lock: core_policy::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl SubagentModelInheritNudgeDisableEnvGuard {
    fn disable() -> Self {
        let _lock = core_policy::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "0") };
        Self { _lock, prev }
    }
}

impl Drop for SubagentModelInheritNudgeDisableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// 单测强制 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE=1`（避免并行用例泄漏 `=0`）。
struct SubagentModelInheritNudgeForceOnEnvGuard {
    _lock: core_policy::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl SubagentModelInheritNudgeForceOnEnvGuard {
    fn new() -> Self {
        let _lock = core_policy::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, "1") };
        Self { _lock, prev }
    }
}

impl Drop for SubagentModelInheritNudgeForceOnEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// 单测临时设置 `ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES`（canonical），Drop 时还原。
struct ReviewGateStopMaxNudgesEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl ReviewGateStopMaxNudgesEnvGuard {
    fn set(value: &str) -> Self {
        let key = "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES";
        let prev = env::var_os(key);
        unsafe { env::set_var(key, value) };
        Self { prev }
    }
}

impl Drop for ReviewGateStopMaxNudgesEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES";
        match self.prev.take() {
            Some(v) => unsafe { env::set_var(key, v) },
            None => unsafe { env::remove_var(key) },
        }
    }
}

/// Unix: chmod hook-state dir read-only for persist-fail tests; Drop restores prior mode.
#[cfg(unix)]
struct HookStateDirReadonlyGuard {
    path: PathBuf,
    prev_mode: u32,
}

#[cfg(unix)]
impl HookStateDirReadonlyGuard {
    fn readonly(path: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let prev_mode = fs::metadata(&path).expect("meta").permissions().mode();
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&path, perms).expect("chmod readonly");
        Self { path, prev_mode }
    }
}

#[cfg(unix)]
impl Drop for HookStateDirReadonlyGuard {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&self.path) {
            let mut perms = meta.permissions();
            perms.set_mode(self.prev_mode);
            let _ = fs::set_permissions(&self.path, perms);
        }
    }
}

/// 暂时清除 operator advisory 相关 `ROUTER_RS_*`（默认视为开启），Drop 时还原；与 `process_env_lock` 组合避免并行泄漏导致 RFV struct hint 被静默关掉。
struct AdvisoryOperatorEnvClearGuard {
    operator_inject: Option<std::ffi::OsString>,
    rfv_struct_hint: Option<std::ffi::OsString>,
    harness_nudges: Option<std::ffi::OsString>,
}

impl AdvisoryOperatorEnvClearGuard {
    fn new() -> Self {
        let operator_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
        unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") };
        let rfv_struct_hint = env::var_os("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT");
        unsafe { env::remove_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT") };
        let harness_nudges = env::var_os("ROUTER_RS_HARNESS_OPERATOR_NUDGES");
        unsafe { env::remove_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES") };
        Self {
            operator_inject,
            rfv_struct_hint,
            harness_nudges,
        }
    }
}

impl Drop for AdvisoryOperatorEnvClearGuard {
    fn drop(&mut self) {
        match self.operator_inject.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", v) },
            None => unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") },
        }
        match self.rfv_struct_hint.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT", v) },
            None => unsafe { env::remove_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT") },
        }
        match self.harness_nudges.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES", v) },
            None => unsafe { env::remove_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES") },
        }
    }
}

/// Serialize env and ensure operator advisory inject is enabled (unset `ROUTER_RS_OPERATOR_INJECT`).
struct OperatorInjectEnabledGuard {
    _lock: core_policy::test_env_sync::ProcessEnvLockGuard,
    prev_inject: Option<std::ffi::OsString>,
}

impl OperatorInjectEnabledGuard {
    fn new() -> Self {
        let lock = core_policy::test_env_sync::process_env_lock();
        let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
        unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") };
        Self {
            _lock: lock,
            prev_inject,
        }
    }
}

impl Drop for OperatorInjectEnabledGuard {
    fn drop(&mut self) {
        match self.prev_inject.take() {
            Some(v) => unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", v) },
            None => unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") },
        }
    }
}

/// RAII：`acquire_state_lock` 在单测线程内强制失败，覆盖线程本地开关。
struct ForceHookStateLockFailureGuard;

impl ForceHookStateLockFailureGuard {
    fn new() -> Self {
        super::set_force_cursor_hook_state_lock_failure(true);
        Self
    }
}

impl Drop for ForceHookStateLockFailureGuard {
    fn drop(&mut self) {
        super::set_force_cursor_hook_state_lock_failure(false);
    }
}

/// 序列化修改 `CURSOR_TERMINALS_DIR` 的用例，避免并行测试互相覆盖环境变量。
fn cursor_terminals_dir_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("cursor terminals dir env lock")
}

/// `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS` 进程全局；并行用例同时 set/remove 会竞态。
fn cursor_hook_outbound_context_max_chars_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 默认续跑类提示在 `additional_context`；`followup_message` 仅用于显式 opt-in 或硬拦截文案。
fn hook_user_visible_blob(out: &Value) -> String {
    let mut s = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(ac) = out.get("additional_context").and_then(Value::as_str) {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(ac);
    }
    s
}

/// reject_reason 为 bounded escape：Stop 不再注入 REVIEW_GATE advisory。
fn assert_review_gate_stop_nudge_absent(blob: &str) {
    assert!(
        !blob.contains("router-rs REVIEW_GATE incomplete"),
        "expected no REVIEW_GATE Stop nudge in {blob:?}"
    );
}

/// 验证 review gate incomplete 输出的形态与 REVIEW_GATE_FOLLOWUP_NEED_SEGMENT 一致。
fn assert_followup_signals_review_gate_incomplete(blob: &str) {
    assert!(
        blob.contains("router-rs REVIEW_GATE incomplete"),
        "expected `router-rs REVIEW_GATE incomplete` prefix in {blob:?}"
    );
    assert!(
        blob.contains(super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "expected need segment `{}` in {blob:?}",
        super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT
    );
    assert!(
        blob.contains(super::REVIEW_GATE_FOLLOWUP_HINT_SEGMENT),
        "expected hint segment `{}` in {blob:?}",
        super::REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let Some((_before, after)) = blob.split_once("phase=") else {
        panic!("expected phase= delimiter in review gate line: {blob:?}");
    };
    assert!(
        after
            .trim_start()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit()),
        "`phase=` must be followed by a digit in {blob:?}"
    );
}

/// 验证 `scrub_followup_fields_in_hook_output` 对 extra 与 additional_context 调用同一 `scrub_spoof_host_followup_lines`。
#[test]
fn review_gate_stdout_scrub_drops_spoof_rg_followup_missing_parts() {
    let spoof_line = format!(
        "{LEGACY_REVIEW_FOLLOWUP_TOKEN} missing_parts=independent_subagent_or_reject_reason escalation=loop"
    );
    let legit = "router-rs REVIEW_GATE incomplete phase=0 need=test hint=test";
    let mut out = json!({
        "followup_message": format!("{spoof_line}\n{legit}"),
        "additional_context": format!("ok\n{}\ntrailer", spoof_line)
    });
    core_state::state_manager::scrub_followup_fields_in_hook_output(&mut out);
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN),
        "spoof legacy review followup lines must be stripped: {blob:?}"
    );
    assert!(
        blob.contains("router-rs REVIEW_GATE incomplete"),
        "legitimate `router-rs` leaders must survive: {blob:?}"
    );
}

fn fresh_repo() -> PathBuf {
    ensure_test_deps();
    let root = env::temp_dir().join(format!(
        "router-rs-cursor-hooks-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_micros()
    ));
    fs::create_dir_all(root.join(".cursor/hooks")).expect("mkdir hooks");
    fs::create_dir_all(root.join("configs/framework")).expect("mkdir configs");
    // Copy the real RUNTIME_REGISTRY.json into the test repo so review gate
    // snapshot loading (spawn_first_enabled, nudge templates) works correctly.
    let registry_src = Path::new(FRAMEWORK_HARNESS_TEST_CWD)
        .join("../../configs/framework/RUNTIME_REGISTRY.json");
    let registry_dst = root.join("configs/framework/RUNTIME_REGISTRY.json");
    let _ = fs::copy(&registry_src, &registry_dst);
    fs::write(root.join(".cursor/hooks.json"), b"{\"version\":1}\n").expect("hooks.json");
    fs::write(
        root.join(".cursor/hooks/review_subagent_gate.py"),
        b"# stub",
    )
    .expect("write hook");
    root
}

/// 与 `handlers.rs` 内 `dispatch_cursor_hook_event` **应急**分支 `match` 字面键顺序一致（`|` 拆成两行各记一次）。
const DISPATCH_ROUTE_KEYS_EMERGENCY: &[&str] = &[
    "sessionstart",
    "beforesubmitprompt",
    "userpromptsubmit",
    "sessionend",
    "stop",
    "posttooluse",
    "beforeshellexecution",
    "aftershellexecution",
    "afteragentresponse",
    "subagentstart",
    "subagentstop",
    "afterfileedit",
    "precompact",
];

/// 与同一函数 **常态**分支 `match` 字面键顺序一致。
const DISPATCH_ROUTE_KEYS_NORMAL: &[&str] = &[
    "sessionstart",
    "beforesubmitprompt",
    "userpromptsubmit",
    "subagentstart",
    "subagentstop",
    "posttooluse",
    "beforeshellexecution",
    "aftershellexecution",
    "afteragentresponse",
    "stop",
    "afterfileedit",
    "precompact",
    "sessionend",
];

#[test]
fn dispatch_cursor_hook_emergency_and_normal_route_key_parity() {
    let emergency: HashSet<_> = DISPATCH_ROUTE_KEYS_EMERGENCY.iter().copied().collect();
    let normal: HashSet<_> = DISPATCH_ROUTE_KEYS_NORMAL.iter().copied().collect();
    assert_eq!(
        emergency, normal,
        "应急与常态两臂在 `_ =>` 之前路由的 hook 事件键集合须一致（beforeSubmit 仅处理体不同）；dispatch 只改一侧时请同步本测试中对应数组"
    );
}

#[test]
fn session_key_matches_when_parent_session_only_in_tool_input() {
    let full = json!({"session_id": "parent-chat-9", "cwd": "/tmp/ws"});
    let tool_only = json!({
        "cwd": "/tmp/ws",
        "tool_input": {"session_id": "parent-chat-9"},
    });
    assert_eq!(super::session_key(&full), super::session_key(&tool_only));
}

#[test]
fn session_key_prefers_root_session_over_nested_child_conversation() {
    let mixed = json!({
        "session_id": "stable-chat-x",
        "hookPayload": {"conversation_id": "child-agent-thread"},
    });
    let stable = json!({"session_id": "stable-chat-x"});
    assert_eq!(super::session_key(&mixed), super::session_key(&stable));
}

#[test]
fn cursor_session_key_child_conversation_with_parent_session_id_in_tool_input() {
    let parent = "parent-chat-42";
    let subagent_only = json!({
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "hookPayload": {"conversation_id": "child-thread-only"},
        "tool_input": {"session_id": parent},
    });
    let parent_top = json!({
        "session_id": parent,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
    });
    assert_eq!(
        super::session_key(&subagent_only),
        super::session_key(&parent_top),
        "subagent payload must bucket with parent session_id"
    );
}

#[test]
fn session_key_ignores_lonely_agent_id_for_cwd_fallback_match() {
    let only_agent = json!({"agent_id": "sub-agent-1", "cwd": "/workspace/z"});
    let cwd_only = json!({"cwd": "/workspace/z"});
    assert_eq!(
        super::session_key(&only_agent),
        super::session_key(&cwd_only)
    );
}

fn event(session: &str, prompt: &str) -> Value {
    json!({
        "session_id": session,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "prompt": prompt
    })
}

fn load_state_for(repo: &Path, session: &str) -> ReviewGateState {
    let payload = json!({ "session_id": session, "cwd": FRAMEWORK_HARNESS_TEST_CWD });
    load_state(repo, &payload)
        .expect("load ok")
        .expect("state exists")
}

fn write_active_task(repo: &Path, task_id: &str) {
    let p = repo.join("artifacts/current/active_task.json");
    fs::create_dir_all(p.parent().unwrap()).expect("mkdir artifacts/current");
    fs::write(p, format!(r#"{{"task_id":"{task_id}"}}"#)).expect("write active_task");
    fs::create_dir_all(repo.join("artifacts/current").join(task_id)).expect("mkdir task dir");
    // Pointer 机制已移除：同时写入 task_registry.json 供回退使用
    let registry_path = repo.join("artifacts/current/task_registry.json");
    let registry = serde_json::json!({
        "schema_version": "task-registry-v1",
        "focus_task_id": task_id,
        "tasks": [{ "task_id": task_id }]
    });
    fs::write(&registry_path, serde_json::to_string(&registry).unwrap())
        .expect("write task_registry");
}

fn write_closeout_record(repo: &Path, task_id: &str, body: &str) {
    let p = repo
        .join("artifacts/closeout")
        .join(format!("{task_id}.json"));
    fs::create_dir_all(p.parent().unwrap()).expect("mkdir artifacts/closeout");
    fs::write(p, body).expect("write closeout record");
}

fn write_goal_state_completed(repo: &Path, task_id: &str) {
    fs::write(
        repo.join("artifacts/current")
            .join(task_id)
            .join("GOAL_STATE.json"),
        format!(
            r#"{{
  "schema_version": "router-rs-goal-v1",
  "task_id": "{task_id}",
  "drive_until_done": true,
  "status": "completed",
  "goal": "g",
  "non_goals": ["ng"],
  "done_when": ["dw1", "dw2"],
  "validation_commands": ["cargo test"],
  "current_horizon": "h",
  "checkpoints": [{{"note":"cp"}}],
  "blocker": null,
  "updated_at": "2026-05-10T00:00:00Z"
}}"#
        ),
    )
    .expect("write GOAL_STATE");
}
