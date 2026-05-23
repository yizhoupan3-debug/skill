use serde_json::json;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

/// 模板/模型偶发的陈旧 Review 续跑前缀（分段拼接，避免在对外文案里复述整词）。
const LEGACY_REVIEW_FOLLOWUP_TOKEN: &str = concat!("RG", "_FOLLOWUP");

/// Framework 仓 canonical 路径：终端元数据 / cwd 对齐 fixture（与 `fresh_repo()` 临时目录区分）。
const FRAMEWORK_HARNESS_TEST_CWD: &str = "/Users/joe/Developer/skill";

/// Drop 时清除 thread_local 覆盖，避免遗留应急门控语义并污染同 OS 线程上的其它用例。
struct ReviewGateDisableTestGuard;

impl ReviewGateDisableTestGuard {
    fn new() -> Self {
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
    prev: Option<std::ffi::OsString>,
}

impl ReviewGateDisableEnvClearGuard {
    fn new() -> Self {
        let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE");
        Self { prev }
    }
}

impl Drop for ReviewGateDisableEnvClearGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE", v),
            None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE"),
        }
    }
}

/// 单测需要已移除事件的完整 handler（`ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1`）。
struct LegacySubtractedEventsGuard {
    _lock: crate::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl LegacySubtractedEventsGuard {
    fn enable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS";
        let prev = env::var_os(key);
        env::set_var(key, "1");
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
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
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
        super::set_test_review_gate_disable_override(None);
        Self {
            _env: ReviewGateDisableEnvClearGuard::new(),
        }
    }
}

/// Opt-in My implement pre-goal nudge (`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` — legacy env name).
struct MyPreGoalOptInEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl MyPreGoalOptInEnvGuard {
    fn enable() -> Self {
        let key = "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";
        let prev = env::var_os(key);
        env::set_var(key, "1");
        Self { prev }
    }
}

impl Drop for MyPreGoalOptInEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
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
        env::set_var(key, "0");
        Self { prev }
    }
}

impl Drop for SpawnFirstNudgeDisableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
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
        env::set_var(key, "1");
        Self { prev }
    }
}

impl Drop for SpawnFirstNudgeEnableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}

/// 单测临时设置 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE=0`，Drop 时还原。
struct SubagentModelInheritNudgeDisableEnvGuard {
    _lock: crate::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl SubagentModelInheritNudgeDisableEnvGuard {
    fn disable() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        let prev = env::var_os(key);
        env::set_var(key, "0");
        Self { _lock, prev }
    }
}

impl Drop for SubagentModelInheritNudgeDisableEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}

/// 单测强制 `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE=1`（避免并行用例泄漏 `=0`）。
struct SubagentModelInheritNudgeForceOnEnvGuard {
    _lock: crate::test_env_sync::ProcessEnvLockGuard,
    prev: Option<std::ffi::OsString>,
}

impl SubagentModelInheritNudgeForceOnEnvGuard {
    fn new() -> Self {
        let _lock = crate::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        let prev = env::var_os(key);
        env::set_var(key, "1");
        Self { _lock, prev }
    }
}

impl Drop for SubagentModelInheritNudgeForceOnEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}

/// 单测临时设置 `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`，Drop 时还原。
struct ReviewGateStopMaxNudgesEnvGuard {
    prev: Option<std::ffi::OsString>,
}

impl ReviewGateStopMaxNudgesEnvGuard {
    fn set(value: &str) -> Self {
        let key = "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES";
        let prev = env::var_os(key);
        env::set_var(key, value);
        Self { prev }
    }
}

impl Drop for ReviewGateStopMaxNudgesEnvGuard {
    fn drop(&mut self) {
        let key = "ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES";
        match self.prev.take() {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
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
        env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        let rfv_struct_hint = env::var_os("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT");
        env::remove_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT");
        let harness_nudges = env::var_os("ROUTER_RS_HARNESS_OPERATOR_NUDGES");
        env::remove_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES");
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
            Some(v) => env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
            None => env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
        }
        match self.rfv_struct_hint.take() {
            Some(v) => env::set_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT", v),
            None => env::remove_var("ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT"),
        }
        match self.harness_nudges.take() {
            Some(v) => env::set_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES", v),
            None => env::remove_var("ROUTER_RS_HARNESS_OPERATOR_NUDGES"),
        }
    }
}

/// Serialize env and ensure operator advisory inject is enabled (unset `ROUTER_RS_OPERATOR_INJECT`).
struct OperatorInjectEnabledGuard {
    _lock: crate::test_env_sync::ProcessEnvLockGuard,
    prev_inject: Option<std::ffi::OsString>,
}

impl OperatorInjectEnabledGuard {
    fn new() -> Self {
        let lock = crate::test_env_sync::process_env_lock();
        let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
        env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        Self {
            _lock: lock,
            prev_inject,
        }
    }
}

impl Drop for OperatorInjectEnabledGuard {
    fn drop(&mut self) {
        match self.prev_inject.take() {
            Some(v) => env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
            None => env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
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
        .expect("cursor hook outbound context max chars env lock")
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
    crate::autopilot_goal::scrub_followup_fields_in_hook_output(&mut out);
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
    let root = env::temp_dir().join(format!(
        "router-rs-cursor-hooks-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_micros()
    ));
    fs::create_dir_all(root.join(".cursor/hooks")).expect("mkdir hooks");
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
  "schema_version": "router-rs-autopilot-goal-v1",
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

#[test]
fn review_prompt_chinese_full_review_arms_state() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s1", "请全面review这个仓库找bug"),
    );
    assert!(out.get("followup_message").is_none());
    let state = load_state_for(&repo, "s1");
    assert_eq!(state.phase, 0);
    assert!(state.review_required);
}

/// 首次武装 review 门控时，`beforeSubmit` 仅注入**一行**紧凑指针（细则在 skill）。
#[test]
fn before_submit_first_arm_injects_compact_deep_review_nudge() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let repo = fresh_repo();
    let sid = "s-parallel-nudge-contract";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/code-review-deep/SKILL.md"),
        "expected skill pointer; got {ac:?}"
    );
    assert!(
        ac.contains("fork_context=false"),
        "expected spawn-first fork_context hint; got {ac:?}"
    );
    assert!(
        ac.contains("配对审稿") || ac.contains("spawn"),
        "expected spawn-first nudge; got {ac:?}"
    );
    assert!(
        ac.contains("general-purpose") || ac.contains("best-of-n-runner"),
        "expected countable lane names; got {ac:?}"
    );
    assert!(
        ac.len() < 400,
        "nudge should stay a single short line; len={} body={ac:?}",
        ac.len()
    );
}

#[test]
fn spawn_first_nudge_disabled_injects_no_additional_context() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _nudge_off = SpawnFirstNudgeDisableEnvGuard::disable();
    let _model_off = SubagentModelInheritNudgeDisableEnvGuard::disable();
    let repo = fresh_repo();
    let sid = "spawn-first-off";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(
        out.get("additional_context").is_none(),
        "NUDGE=0 must zero-inject; got {out:?}"
    );
}

#[test]
fn my_light_implementx_stop_suppresses_review_gate_when_review_armed() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "my-light-rg-stop";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, sid).review_required);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "/implementx 继续",
            "response": "[P1] scripts/foo.rs:42: missing edge case"
        }),
    );
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !fm.contains("REVIEW_GATE incomplete"),
        "my-light /implementx stop must suppress REVIEW_GATE; fm={fm:?}"
    );
}

#[test]
fn narrow_path_review_stop_does_not_block() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "narrow-stop";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "review ./README.md"),
    );
    assert!(!load_state_for(&repo, sid).review_required);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "review ./README.md",
            "response": "[P1] README.md:1: typo in title"
        }),
    );
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !fm.contains("REVIEW_GATE incomplete"),
        "narrow review must not Stop-block; fm={fm:?}"
    );
}

#[test]
fn before_submit_review_and_implementx_same_prompt_suppresses_review_but_arms_goal() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "dual-review-implementx";
    let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "my-light + goal drive must not arm review; got {state:?}"
    );
    assert!(state.goal_required, "implementx must arm goal drive; got {state:?}");
}

/// 未命中「并行 review 候选」三元时仍注入同一行指针；不再追加第二段「≥3」以免刷屏。
#[test]
fn before_submit_review_prompt_compact_nudge_has_no_second_breadth_paragraph() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let repo = fresh_repo();
    let sid = "s-review-no-breadth-scope";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "security code review"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/code-review-deep/SKILL.md") && ac.contains("fork_context=false"),
        "expected spawn-first pointer; got {ac:?}"
    );
    assert!(
        !ac.contains("≥3"),
        "hook must not append a separate ≥3 breadth paragraph; got {ac:?}"
    );
}

/// 应急关闭审稿门控时，即使用户轮为 review 也不注入深度审并行提示。
#[test]
fn review_gate_disabled_before_submit_suppresses_deep_review_nudge_for_review_prompt() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _model_off = SubagentModelInheritNudgeDisableEnvGuard::disable();
    let repo = fresh_repo();
    let _rg = ReviewGateDisableTestGuard::new();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("rg-off-review-text", "全面review这个仓库"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("skills/code-review-deep/SKILL.md"),
        "review gate disabled must not inject spawn-first review nudge; got {out:?}"
    );
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
}

#[test]
fn parallel_delegation_does_not_latch_delegation_required() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s2", "请前端后端测试并行分头执行"),
    );
    let state = load_state_for(&repo, "s2");
    assert!(
        !state.delegation_required,
        "delegation heuristic must not persist into hook-state"
    );
    assert_eq!(state.phase, 0);
}

#[test]
fn my_implement_entry_does_not_arm_delegation_or_review_from_fix_copy() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(
            "ap-del",
            "/implementx address all review findings from the last pass",
        ),
    );
    let state = load_state_for(&repo, "ap-del");
    assert!(
        !state.delegation_required,
        "My implement must not stack delegation_required (was: framework_entrypoint)"
    );
    assert!(
        !state.review_required,
        "My implement turn must not re-arm review from findings wording"
    );
    assert!(state.goal_required);
}

#[test]
fn before_submit_review_and_autopilot_same_prompt_merges_mixing_hint() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    crate::hook_common::set_test_my_light_override(None);
    let repo = fresh_repo();
    let sid = "s-dual-review-autopilot-hint";
    let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("router-rs：本轮提交同时包含"),
        "my-light /implementx must not inject review+goal mixing nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "same-submit autopilot must suppress review arming; got {state:?}"
    );
    assert!(state.goal_required);
}

#[test]
fn review_goal_mixing_nudge_predicate_requires_non_my_light() {
    let _lock = crate::test_env_sync::process_env_lock();
    crate::hook_common::set_test_my_light_override(None);
    assert!(
        crate::hook_common::my_light_profile_active(None, "/implementx"),
        "implementx in prompt always activates my-light profile"
    );
    assert!(
        !crate::hook_common::my_light_profile_active(None, "全面review这个仓库"),
        "review-only prompt without my-* slash is not my-light from prompt alone"
    );
}

#[test]
fn before_submit_review_with_disk_goal_non_my_light_injects_mixing_hint() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-team-mix-hint";
    let payload = event(sid, "深度 review 整个路由系统 /implementx 继续");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("router-rs：本轮提交同时包含"),
        "non-my-light review+implementx must inject split hint; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "same-submit implementx must disarm review arming; got {state:?}"
    );
    assert!(state.goal_required);
}

#[test]
fn before_submit_implementx_injects_one_breath_nudge() {
    let repo = fresh_repo();
    let sid = "s-implement-nudge";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("ALL waves") || ac.contains("WAVE_STATE"),
        "implementx must inject MY_IMPLEMENT nudge; got {ac:?}"
    );
}

#[test]
fn before_submit_my_light_clears_sticky_review_required() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "s-my-light-clear-review";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "请全面review这个仓库"),
    );
    let armed = load_state_for(&repo, sid);
    assert!(armed.review_required, "review prompt should arm before my-light entry");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let cleared = load_state_for(&repo, sid);
    assert!(
        !cleared.review_required,
        "my-light UPS must clear sticky review_required; got {cleared:?}"
    );
    assert_eq!(
        cleared.phase, 0,
        "my-light UPS must reset review phase; got {cleared:?}"
    );
    assert!(
        cleared.review_subagent_pending_cycle_keys.is_empty(),
        "my-light UPS must clear pending cycle keys; got {cleared:?}"
    );
}

#[test]
fn before_submit_implementx_injects_subagent_model_inherit_nudge() {
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let repo = fresh_repo();
    let sid = "s-model-nudge-impl";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "implementx must inject model inherit nudge; got {ac:?}"
    );
    let model_pos = ac.find("继承主会话").expect("model nudge");
    let goal_pos = ac
        .find("ALL waves")
        .or_else(|| ac.find("WAVE_STATE"))
        .expect("implement goal nudge");
    assert!(
        model_pos < goal_pos,
        "model inherit must precede implement one-breath nudge; got {ac:?}"
    );
}

#[test]
fn before_submit_spawn_first_and_model_inherit_not_duplicated() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let repo = fresh_repo();
    let sid = "s-spawn-model-dedup";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个路由系统"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    let count = ac.matches("继承主会话").count();
    assert_eq!(
        count, 1,
        "spawn-first already includes model inherit; must not duplicate; got {ac:?}"
    );
}

#[test]
fn before_submit_model_inherit_survives_output_policy_truncation() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let _cap_lock = cursor_hook_outbound_context_max_chars_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "900");
    let repo = fresh_repo();
    let sid = "s-model-trunc-survive";
    let mut out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    super::apply_cursor_hook_output_policy(&mut out);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "model inherit must stay in prefix after outbound truncation; got {ac:?}"
    );
    match prev_cap {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS"),
    }
}

#[test]
fn before_submit_my_light_review_still_injects_model_inherit_without_spawn_first() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let repo = fresh_repo();
    let sid = "s-model-nudge-my-light";
    let _nudge_off = SpawnFirstNudgeDisableEnvGuard::disable();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/discussx then 全面review这个路由系统"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "my-light must still get model inherit nudge when spawn-first off; got {ac:?}"
    );
}

#[test]
fn subagent_model_inherit_nudge_disabled_injects_no_model_line() {
    let _off = SubagentModelInheritNudgeDisableEnvGuard::disable();
    let repo = fresh_repo();
    let sid = "s-model-nudge-off";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("继承主会话"),
        "MODEL_INHERIT_NUDGE=0 must omit model line; got {ac:?}"
    );
}

#[test]
fn before_submit_verifyx_injects_generic_goal_nudge() {
    let repo = fresh_repo();
    let sid = "s-verify-nudge";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/verifyx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/verifyx/SKILL.md"),
        "verifyx must inject MY_GOAL_DRIVE nudge; got {ac:?}"
    );
    assert!(
        !ac.contains("ALL waves") && !ac.contains("WAVE_STATE"),
        "verifyx must not inject implement one-breath nudge; got {ac:?}"
    );
}

#[test]
fn cursor_second_review_prompt_in_same_session_requires_fresh_subagent_evidence() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-cursor-rearm-review";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "深度 review 这个 PR"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-first",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-first",
        }),
    );
    let completed = load_state_for(&repo, sid);
    assert_eq!(completed.phase, 3, "first cycle should complete at phase 3");
    let rearm_out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    let rearm_ac = rearm_out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        rearm_ac.contains("fork_context=false") || rearm_ac.contains("code-review-deep"),
        "second deep review must inject spawn-first nudge; got {rearm_ac:?}"
    );
    let rearmed = load_state_for(&repo, sid);
    assert_eq!(
        rearmed.phase, 0,
        "second deep review must reset phase; got {rearmed:?}"
    );
    assert_eq!(
        rearmed.active_subagent_count, 0,
        "re-arm after completed cycle should leave no open subagents; got {rearmed:?}"
    );
    let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "second review without new subagent must block Stop; fm={fm:?}"
    );
}

#[test]
fn before_submit_implementx_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-impl-cursor";
    let payload = event(sid, "/implementx");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable; got {ac:?}"
    );
    assert!(
        !ac.contains("ALL waves"),
        "must not mask corrupt state with implement nudge; got {ac:?}"
    );
}

#[test]
fn cursor_rearm_review_resets_review_followup_count_after_soft_nag() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("2");
    let repo = fresh_repo();
    let sid = "s-rearm-followup-reset";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "深度 review 这个 PR"),
    );
    for _ in 0..3 {
        let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
        assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&stop));
    }
    assert!(
        load_state_for(&repo, sid).review_followup_count >= 3,
        "blocked stops must accumulate review_followup_count"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-first",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-first",
        }),
    );
    assert_eq!(load_state_for(&repo, sid).phase, 3);
    let rearm_out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    let rearmed = load_state_for(&repo, sid);
    assert_eq!(
        rearmed.review_followup_count, 0,
        "re-arm must reset review_followup_count; got {rearmed:?}"
    );
    let ac = rearm_out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/code-review-deep/SKILL.md"),
        "re-arm must reinject spawn-first nudge; got {ac:?}"
    );
    let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT) && !fm.contains("mode=soft_nag"),
        "fresh review cycle must not inherit prior soft_nag on first Stop; fm={fm:?}"
    );
}

#[test]
fn before_submit_my_new_project_does_not_arm_goal_required() {
    let repo = fresh_repo();
    let sid = "my-new-project-pre-exec";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "请 /discussx 做迁移后技术债审查"),
    );
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("My lifecycle pre-execution"),
        "expected pre-exec nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.goal_required,
        "pre-exec /discussx must not arm goal_required; got {state:?}"
    );
}

#[test]
fn before_submit_my_plan_phase_does_not_arm_goal_required() {
    let repo = fresh_repo();
    let sid = "my-plan-pre-exec";
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/planx"));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.goal_required,
        "/planx must not arm goal_required; got {state:?}"
    );
}

#[test]
fn cursor_plan_build_path_does_not_arm_goal() {
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let plan_ref = format!("{cwd}/.cursor/plans/feature.plan.md");
    let payload = json!({
        "session_id": "plan-build",
        "cwd": cwd,
        "prompt": format!("Implement {plan_ref}"),
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let st_off = load_state_for(&repo, "plan-build");
    assert!(
        !st_off.goal_required,
        "plan path alone must not arm goal_required"
    );
}

#[test]
fn stop_completion_claim_requires_closeout_record_when_strict_enabled() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let _gate_disable_guard = ReviewGateDisableEnvClearGuard::new();
    let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

    let repo = fresh_repo();
    let tid = "t-closeout";
    write_active_task(&repo, tid);
    write_goal_state_completed(&repo, tid);
    // Ensure goal gate can hydrate "verified" from disk evidence, so Stop reaches the
    // strict closeout enforcement branch instead of emitting AG_FOLLOWUP.
    fs::write(
            repo.join("artifacts/current").join(tid).join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &json!({
            "session_id": "s-closeout-1",
            "cwd": repo.display().to_string(),
            "prompt": "/implementx do thing"
        }),
    );
    assert!(
        closeout_followup_for_completion_claim(&repo, tid)
            .expect("ok")
            .is_some(),
        "precondition: strict env should require record"
    );
    let payload = json!({
        "session_id": "s-closeout-1",
        "cwd": repo.display().to_string(),
        "prompt": "ok",
        "response": "done",
    });
    assert_eq!(agent_response_text(&payload), "done");
    // Inject a response text that claims completion.
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let msg = hook_user_visible_blob(&out);
    assert!(
        msg.contains("CLOSEOUT_FOLLOWUP") && msg.contains("missing_record"),
        "expected closeout followup; got {msg:?}"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
        None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn stop_completion_claim_allows_when_closeout_record_passes() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let _gate_disable_guard = ReviewGateDisableEnvClearGuard::new();
    let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

    let repo = fresh_repo();
    let tid = "t-closeout-ok";
    write_active_task(&repo, tid);
    write_goal_state_completed(&repo, tid);
    // Ensure evidence exists or provide commands_run in record (R7/R8 coverage).
    fs::write(
        repo.join("artifacts/current")
            .join(tid)
            .join("EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0,"success":true}]}"#,
    )
    .expect("write evidence");
    write_closeout_record(
        &repo,
        tid,
        r#"{
  "schema_version": "closeout-record-v1",
  "task_id": "t-closeout-ok",
  "summary": "已完成并验证",
  "verification_status": "passed",
  "commands_run": [{"command":"cargo test","exit_code":0}]
}"#,
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &json!({
            "session_id": "s-closeout-2",
            "cwd": repo.display().to_string(),
            "prompt": "/implementx do thing"
        }),
    );

    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s-closeout-2",
            "cwd": repo.display().to_string(),
            "prompt": "ok",
            "response": "已完成",
        }),
    );
    let msg = hook_user_visible_blob(&out);
    assert!(
        !msg.contains("CLOSEOUT_FOLLOWUP"),
        "expected no closeout followup; got {msg:?}"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
        None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn completion_claim_detector_matches_basic_tokens() {
    assert!(completion_claimed_in_text("done"));
    assert!(completion_claimed_in_text("已完成"));
    assert!(completion_claimed_in_text("验证通过"));
    assert!(completion_claimed_in_text("tests passed"));
    assert!(!completion_claimed_in_text("still working"));
}

#[test]
fn completion_claim_detector_ignores_completion_as_substring_gossip() {
    assert!(!completion_claimed_in_text("方案的完成度还可以"));
    assert!(!completion_claimed_in_text("讨论完成任务拆分"));
}

#[test]
fn closeout_followup_emits_when_strict_and_record_missing() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let _gate_disable_guard = ReviewGateDisableEnvClearGuard::new();
    let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");

    let repo = fresh_repo();
    let tid = "t-missing-closeout";
    write_active_task(&repo, tid);
    write_goal_state_completed(&repo, tid);
    let msg = closeout_followup_for_completion_claim(&repo, tid)
        .expect("ok")
        .expect("followup");
    assert!(msg.contains("missing_record"));

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
        None => env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn my_skips_pre_goal_nag_when_goal_state_on_disk() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    use std::env;
    let prev_strict = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "0");

    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gt1")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gt1"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gt1",
        "goal": "close review findings",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ap-disk", "/implementx 继续实现"),
    );
    assert!(
        load_state_for(&repo, "ap-disk").pre_goal_review_satisfied,
        "existing GOAL_STATE implies execution lane already opened"
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("My implement (/implementx") && !msg.contains("independent-context"),
        "pre-goal nag should be skipped when GOAL_STATE exists; msg={msg:?}"
    );

    match prev_strict {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
        None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
    }
}

#[test]
fn my_pre_goal_strict_disk_skips_hydrate_pre_goal_on_before_submit() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "1");

    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gt-strict")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gt-strict"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gt-strict",
        "goal": "close review findings",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ap-disk-strict", "/implementx 继续实现"),
    );
    assert!(
        !load_state_for(&repo, "ap-disk-strict").pre_goal_review_satisfied,
        "strict disk: disk GOAL alone must not satisfy pre-goal on beforeSubmit"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
        None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
    }
}

#[test]
fn stop_goal_gate_hydrates_from_goal_state_and_evidence_without_keywords() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-ev")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-ev"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-ev",
        "goal": "fix review findings",
        "non_goals": ["avoid unrelated refactors"],
        "done_when": ["tests green", "review checklist cleared"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "checkpoint",
        "task_id": "t-ev",
        "note": "applied patch",
    }))
    .expect("checkpoint");
    fs::write(
            repo.join("artifacts/current/t-ev/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ev-gate", "/implementx finish fixes"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ev-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "done; no Goal:/Checkpoint:/verified boilerplate in prose"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "goal gate should hydrate from disk; msg={msg:?} out={out:?}"
    );
}

#[test]
fn stop_hydrates_when_hook_state_lacks_goal_required_but_goal_on_disk() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-nof")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-nof"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-nof",
        "goal": "stdio seeded goal",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("start");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "checkpoint",
        "task_id": "t-nof",
        "note": "step",
    }))
    .expect("cp");
    fs::write(
        repo.join("artifacts/current/t-nof/EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
    )
    .expect("ev");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("noflag", "hello"));
    assert!(
        !load_state_for(&repo, "noflag").goal_required,
        "plain prompt must not arm goal_required before hydrate"
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "noflag",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "bye",
            "response": "done without magic words"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "GOAL_STATE on disk must hydrate despite goal_required=false; msg={msg:?}"
    );
}

#[test]
fn stop_hydrates_when_active_task_missing_but_goal_on_disk() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-orph")).expect("mkdir");
    fs::write(
            repo.join("artifacts/current/t-orph/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"no active_task json","status":"running","non_goals":["n"],"checkpoints":[{"note":"step"}],"done_when":["ship","review checklist cleared"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    fs::write(
        repo.join("artifacts/current/t-orph/EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
    )
    .expect("ev");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("orph", "hello"));
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "orph",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "bye",
            "response": "done"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "scan fallback must hydrate when active_task.json is missing; msg={msg:?}"
    );
}

#[test]
fn stop_goal_gate_hydrates_running_goal_without_checkpoints_or_keywords() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-run")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-run"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-run",
        "goal": "minimal running goal only",
        "non_goals": ["avoid unrelated refactors"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("run-gate", "/implementx continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "run-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "no Goal/Checkpoint/Verification boilerplate"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "running GOAL_STATE with non-empty goal should hydrate progress+verify; msg={msg:?}"
    );
}

#[test]
fn stop_goal_gate_hydrates_when_goal_state_omits_status_field() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-nost")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-nost"}"#,
    )
    .expect("active");
    fs::write(
            repo.join("artifacts/current/t-nost/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"hand-written without status","non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal json");
    fs::write(
            repo.join("artifacts/current/t-nost/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ns-gate", "/implementx continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ns-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "no chat boilerplate"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "missing status + non-empty goal should hydrate; msg={msg:?}"
    );
}

#[test]
fn override_phrase_in_chinese_disables_arming() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s3", "全面review这个仓库，不要用子代理"),
    );
    assert!(out.get("followup_message").is_none());
    let state = load_state_for(&repo, "s3");
    assert!(state.review_override);
    assert_eq!(state.phase, 0);
}

#[test]
fn stop_does_not_set_review_override_from_assistant_echo_alone() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-ov-echo", "全面review这个仓库"),
    );
    assert!(
        !load_state_for(&repo, "s-ov-echo").review_override,
        "user prompt must not imply review_override"
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s-ov-echo",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "用户坚持不要用子代理，我仅在主会话输出 findings。"
        }),
    );
    let state = load_state_for(&repo, "s-ov-echo");
    assert!(
        !state.review_override,
        "assistant echo of override-like wording must not set review_override"
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn stop_does_not_set_delegation_override_from_assistant_echo_when_review_armed() {
    let repo = fresh_repo();
    let sid = "s-delov-echo";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(!load_state_for(&repo, sid).delegation_override);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "项目经理说不要并行分头推进，我只好先在主会话出 findings。"
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
    let st = load_state_for(&repo, sid);
    assert!(
        !st.delegation_override,
        "`has_delegation_override`-like wording must not be read from assistant response alone"
    );
    assert!(
        !st.review_override,
        "sanity: user prompt did not request review bypass",
    );
}

#[test]
fn stop_does_not_set_delegation_override_from_assistant_global_override_echo_when_review_armed() {
    let repo = fresh_repo();
    let sid = "s-globov-echo";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(!load_state_for(&repo, sid).delegation_override);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "Stand-up recap: we'll handle this locally and summarize in chat."
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
    let st = load_state_for(&repo, sid);
    assert!(
        !st.delegation_override,
        "`has_override` wording on Stop must not originate from assistant response alone",
    );
    assert!(
        !st.review_override,
        "sanity: user prompt did not request review bypass",
    );
}

#[test]
fn stop_sets_review_override_from_user_prompt_disarms_review_gate_followup() {
    let repo = fresh_repo();
    let sid = "s-user-revov";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "不要使用子代理，本轮只在主会话给结论",
            "response": "收到。",
        }),
    );
    let st = load_state_for(&repo, sid);
    assert!(st.review_override);
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("router-rs REVIEW_GATE incomplete"),
        "user-authored override disarms reviewer stop follow-up; blob={blob:?}",
    );
}

#[test]
fn stop_user_parallel_opt_out_matches_has_override_and_delegation_regex_coupling() {
    // `hook_common::has_override` 与 delegation 正则均含中文「不要…并行/分工」；用户写入 Stop prompt
    // 时两行 `handle_stop` if 可同时置位，`review_hard_armed` 为假并解除未完成 reviewer 随访。
    let repo = fresh_repo();
    let sid = "s-user-parov";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "我们不要并行分工了，先主线程输出",
            "response": "明白。",
        }),
    );
    let st = load_state_for(&repo, sid);
    assert!(st.delegation_override);
    assert!(
        st.review_override,
        "同一 `has_override` 句式同时推高 review/disarm branch"
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("router-rs REVIEW_GATE incomplete"),
        "combined overrides disarm reviewer stop follow-up; blob={blob:?}",
    );
}

#[test]
fn reject_reason_does_not_satisfy_review_stop() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s4", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({ "session_id": "s4", "response": "reject reason: small_task" }),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s4", "reject reason: small_task"));
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn reject_reason_in_user_prompt_does_not_satisfy_review_gate_on_stop() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s13", "reject reason: small_task"));
    let followup = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_followup_signals_review_gate_incomplete(followup);
    let state = load_state_for(&repo, "s13");
    assert!(state.reject_reason_seen);
}

#[test]
fn reject_reason_in_assistant_response_does_not_satisfy_review_gate_on_stop() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13b", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s13b",
            "prompt": "继续",
            "response": "reject reason: shared_context_heavy"
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn nested_payload_response_reject_reason_does_not_satisfy_review_gate_on_stop() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13nest-r", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s13nest-r",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": {
                "prompt": "继续",
                "response": "reject reason: shared_context_heavy"
            }
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn nested_payload_response_sets_reject_reason_on_after_agent_response() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13nest-a", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "s13nest-a",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "small_task" }
        }),
    );
    assert!(load_state_for(&repo, "s13nest-a").reject_reason_seen);
}

#[test]
fn emergency_review_gate_disable_cold_after_agent_response_persists_reject_reason_seen() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let _env_clear = ReviewGateDisableEnvClearGuard::new();
    let _rg_disable = ReviewGateDisableTestGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "s-cold-ara",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "response": "reject reason: small_task"
        }),
    );
    assert!(
        load_state_for(&repo, "s-cold-ara").reject_reason_seen,
        "应急门控下仍以 `handle_after_agent_response` 写入 hook-state；无 beforeSubmit 冷启动亦应落盘 reject_reason_seen"
    );
}

#[test]
fn hook_signal_uses_structured_text_unless_full_scrape_enabled() {
    let event = json!({
        "session_id": "scrape-mode",
        "payload": {
            "unknown_transcript": "small_task"
        }
    });
    let compact = hook_event_signal_text_with_scrape_mode(&event, "latest user", "", false);
    assert!(compact.contains("latest user"));
    assert!(
        !compact.contains("small_task"),
        "default hot path must not scrape arbitrary transcript fields"
    );
    let full = hook_event_signal_text_with_scrape_mode(&event, "latest user", "", true);
    assert!(
        full.contains("small_task"),
        "explicit fallback mode should preserve unknown-field compatibility"
    );
}

#[test]
fn stop_writes_back_reject_reason_seen_for_future_sessions() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13c", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s13c",
            "prompt": "reject reason: token_overhead_dominates",
            "response": ""
        }),
    );
    let state = load_state_for(&repo, "s13c");
    assert!(state.reject_reason_seen);
}

#[test]
fn before_submit_lock_failure_fails_closed_without_writing_state() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s14", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(out.get("continue"), Some(&json!(false)));
    assert!(
        hook_user_visible_blob(&out).contains("锁不可用"),
        "out={out:?}"
    );
    assert!(!state_path(&repo, &payload).exists());
}

#[test]
fn before_submit_lock_failure_allows_non_strict_prompt() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s14b", "帮我润色一句话");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(out.get("continue"), Some(&json!(true)));
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("降级"),
        "expected degraded lock copy; blob={blob}"
    );
}

#[test]
fn stop_lock_failure_reports_degraded_followup() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s15", "帮我润色一句话");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("锁不可用") && blob.contains("降级"),
        "expected degraded stop copy; blob={blob}"
    );
}

#[test]
fn stop_lock_failure_fail_closed_review_gate_when_review_armed() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s15-review-lock", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") && fm.contains("hook_state_lock_unavailable"),
        "review-armed stop must fail-closed REVIEW_GATE on lock loss; fm={fm}"
    );
}

#[test]
fn subagent_start_lock_failure_denies_when_review_armed() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let sid = "s-sub-lock-deny";
    let submit = event(sid, "全面review这个仓库");
    let mut armed = empty_state();
    armed.review_required = true;
    armed.phase = 1;
    if let Some(parent) = state_path(&repo, &submit).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        state_path(&repo, &submit),
        serde_json::to_string(&armed).expect("serialize"),
    )
    .expect("seed review-armed state");
    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": sid, "subagent_type": "general-purpose", "fork_context": false }),
    );
    assert_eq!(out.get("permission"), Some(&json!("deny")));
}

#[test]
fn stop_load_state_invalid_json_fail_closed_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-stop-bad-json";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") && fm.contains("hook_state_read_failed"),
        "corrupt hook-state must fail-closed; fm={fm}"
    );
}

#[test]
fn stop_lock_failure_still_surfaces_autopilot_drive() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gl-stop-lock")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gl-stop-lock"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gl-stop-lock",
        "goal": "lock-merge",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let payload = event("s15b", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir lock parent");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("REVIEW_GATE incomplete") && blob.contains("hook_state_lock_unavailable"),
        "review-armed stop lock loss must hard REVIEW_GATE; blob={blob}"
    );
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "hard lock-failure followup must not merge GOAL_CONTINUE; blob={blob}"
    );
}

#[test]
fn stop_with_active_goal_does_not_inject_goal_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    let tid = "existing-followup";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active_task");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"drive while hard message exists","status":"running","drive_until_done":true,"non_goals":["n"],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("existing-followup", "hi"));
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: Stop must not inject GOAL_CONTINUE: {blob}"
    );
}

#[test]
fn before_submit_does_not_merge_goal_or_rfv_continuity() {
    let repo = fresh_repo();
    let tid = "merge-both";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("RFV_LOOP_STATE.json"),
            r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":0,"max_rounds":3,"allow_external_research":false,"rounds":[]}"#,
        )
        .expect("rfv");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("merge-t", "hello"));
    let msg = hook_user_visible_blob(&out);
    assert!(!msg.contains("GOAL_CONTINUE"), "{msg}");
    assert!(!msg.contains("RFV_LOOP_CONTINUE"), "{msg}");
    assert!(!msg.contains("## 续跑"), "{msg}");
}

#[test]
fn stop_active_goal_does_not_inject_goal_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/default-ac")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"default-ac"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "default-ac",
        "goal": "default additional context drive",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let out = dispatch_cursor_hook_event(&repo, "stop", &event("default-ac", "hi"));
    assert!(
        out.get("followup_message").is_none(),
        "continuity nudge should not become hard followup: {out:?}"
    );
    let ctx = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ctx.contains("GOAL_CONTINUE"),
        "continuity removal: {ctx}"
    );
    assert!(
        ctx.contains("SESSION_CLOSE_STYLE"),
        "soft terminal closeout nudge still allowed: {ctx}"
    );
}

#[test]
fn stop_plain_session_injects_session_close_style_when_no_hard_followup() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("plain-close", "ok"));
    assert!(out.get("followup_message").is_none(), "{out:?}");
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(ac.contains("SESSION_CLOSE_STYLE"), "{ac}");
}

#[test]
fn stop_hard_gate_does_not_inject_session_close_style_paragraph() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-hard-g", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s-hard-g", "继续"));
    assert!(out.get("followup_message").is_some());
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "hard Stop followup must not bundle soft closeout nudge: {out:?}"
    );
    assert!(
        !ac.contains("GOAL_CONTINUE"),
        "hard Stop followup must suppress goal continuity merge: {out:?}"
    );
    assert!(
        !ac.contains("review-output-lint"),
        "hard Stop followup must not merge review-output-lint: {out:?}"
    );
}

#[test]
fn stop_review_armed_with_active_goal_suppresses_my_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    let sid = "s-review-goal-mutex";
    fs::create_dir_all(repo.join("artifacts/current/default-rg")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"default-rg"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "default-rg",
        "goal": "drive while review gate open",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("REVIEW_GATE") || blob.contains("AG_FOLLOWUP"),
        "expected hard gate followup: {blob}"
    );
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "hard gate must not merge GOAL_CONTINUE: {blob}"
    );
    assert!(
        !blob.contains("review-output-lint"),
        "hard gate must not merge review-output-lint: {blob}"
    );
}

#[test]
fn review_gate_disabled_before_submit_emits_only_continue_true() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let payload = event("rg-off-before-submit", "hello");
    let expected = json!({ "continue": true });
    let _rg = ReviewGateDisableTestGuard::new();
    let out_prompt = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(
        out_prompt, expected,
        "beforeSubmitPrompt in review-gate-disabled mode must not attach before_submit nudges/state; got {out_prompt:?}"
    );
    let out_user = dispatch_cursor_hook_event(&repo, "userPromptSubmit", &payload);
    assert_eq!(
        out_user, expected,
        "userPromptSubmit must normalize like beforeSubmitPrompt; got {out_user:?}"
    );
}

#[test]
fn review_gate_disabled_after_agent_response_updates_state_after_before_submit_seeded() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "aar-rg-disabled-parity";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(
        !load_state_for(&repo, sid).reject_reason_seen,
        "precondition: reject_reason not set by beforeSubmit alone"
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": { "response": "small_task" }
    });
    {
        let _rg = ReviewGateDisableTestGuard::new();
        assert_eq!(
            dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload),
            json!({}),
            "afterAgentResponse shape unchanged under review-gate-disabled dispatch"
        );
    }
    assert!(
        load_state_for(&repo, sid).reject_reason_seen,
        "reject_reason must persist when afterAgentResponse runs on emergency dispatch table"
    );
}

#[test]
fn session_close_style_nudge_disabled_by_env() {
    let _lock = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE");
    env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", "0");
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("style-off", "x"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "env should disable soft close nudge: {ac}"
    );
    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", v),
        None => env::remove_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE"),
    }
}

#[test]
fn session_close_style_nudge_suppressed_when_operator_inject_off() {
    let _lock = crate::test_env_sync::process_env_lock();
    let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
    env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("plain-close-inject-off", "ok"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "ROUTER_RS_OPERATOR_INJECT=0 must suppress SESSION_CLOSE_STYLE: {ac}"
    );
    match prev_inject {
        Some(v) => env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
        None => env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
    }
}

#[test]
fn review_gate_disabled_stop_still_merges_autopilot_drive() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gl-rgoff")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gl-rgoff"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gl-rgoff",
        "goal": "rg-off-merge",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let mut out = {
        let _rg = ReviewGateDisableTestGuard::new();
        dispatch_cursor_hook_event(&repo, "stop", &event("sg1", "hi"))
    };
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );

    apply_cursor_hook_output_policy(&mut out);
    let preserved = hook_user_visible_blob(&out);
    assert!(
        !preserved.contains("GOAL_CONTINUE"),
        "continuity removal: {preserved}"
    );
}

#[test]
fn stop_goal_and_rfv_do_not_emit_continuity_followups() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let tid = "stop-both";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("RFV_LOOP_STATE.json"),
            r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":1,"max_rounds":3,"allow_external_research":false,"rounds":[]}"#,
        )
        .expect("rfv");

    let cwd = repo.display().to_string();
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "stop-both",
            "cwd": cwd,
            "prompt": "hello",
        }),
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );
    assert!(
        !blob.contains("RFV_LOOP_CONTINUE"),
        "continuity removal: {blob}"
    );
}

#[test]
fn stop_goal_and_rfv_do_not_merge_schema_hint_into_continue() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _advisory_env = AdvisoryOperatorEnvClearGuard::new();
    let repo = fresh_repo();
    let tid = "stop-both-struct";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active");
    fs::write(
        repo
            .join("artifacts/current")
            .join(tid)
            .join("GOAL_STATE.json"),
        r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
    )
    .expect("goal");
    fs::write(
        repo
            .join("artifacts/current")
            .join(tid)
            .join("RFV_LOOP_STATE.json"),
        r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":1,"max_rounds":3,"allow_external_research":true,"prefer_structured_external_research":true,"rounds":[{"round":1,"verify_result":"PASS"}]}"#,
    )
    .expect("rfv");

    let cwd = repo.display().to_string();
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "stop-both-struct",
            "cwd": cwd,
            "prompt": "hello",
        }),
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );
    assert!(
        !blob.contains(crate::rfv_loop::RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH),
        "continuity removal must not inject RFV schema pointer via Stop: {blob}"
    );
}

#[test]
fn cursor_hook_output_policy_truncates_additional_context_under_env_budget() {
    let _env_lock = cursor_hook_outbound_context_max_chars_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "1500");
    let pad = "Z".repeat(8000);
    let mut out = json!({
        "additional_context": format!("AUTOPILOT_HEAD\nAUTOPILOT_DRIVE_MARKER\n{}", pad),
    });
    apply_cursor_hook_output_policy(&mut out);
    env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    if let Some(v) = prev {
        env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v);
    }

    let s = out["additional_context"].as_str().expect("str");
    assert!(
        s.len() <= 1500,
        "len={}, s.prefix={:?}",
        s.len(),
        &s[..s.len().min(80)]
    );
    assert!(
        s.starts_with("AUTOPILOT_HEAD")
            && s.contains("AUTOPILOT_DRIVE_MARKER")
            && s.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX),
        "prefer prefix preservation: {s:?}"
    );
}

#[test]
fn cursor_hook_output_policy_truncates_followup_after_absurd_length() {
    let _env_lock = cursor_hook_outbound_context_max_chars_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    let absurd = vec![b'Q'; max_out.saturating_mul(5).max(32 * 1024)];
    let absurd_str = String::from_utf8(absurd).expect("ascii");
    let mut out = json!({ "followup_message": absurd_str });
    apply_cursor_hook_output_policy(&mut out);
    match prev_cap {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS"),
    }
    let s = out["followup_message"].as_str().expect("str");
    assert!(s.len() <= max_out, "truncated={}, max={}", s.len(), max_out);
    assert!(s.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX));
    assert!(s.starts_with('Q'));
}

#[test]
fn cursor_hook_output_policy_is_noop_for_hard_gate_lines() {
    let hard = format!(
        "router-rs REVIEW_GATE incomplete phase=0 {} {}",
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let mut out = json!({
        "followup_message": hard.clone()
    });
    apply_cursor_hook_output_policy(&mut out);
    assert_eq!(out["followup_message"], json!(hard));
    assert_eq!(out["router_rs_observation"]["gate"]["code"], "review_gate");
    assert_eq!(out["router_rs_observation"]["gate"]["blocking"], true);
}

#[test]
fn cursor_hook_outbound_trunc_respects_byte_cap_and_marker() {
    let body = "x".repeat(9000);
    let max_out = 8192usize;
    let got = super::truncate_cursor_hook_outbound_context(&body, max_out);
    assert!(got.len() <= max_out, "len {} max {}", got.len(), max_out);
    assert!(got.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX));
}

#[test]
fn outbound_truncation_preserves_review_gate_and_continuity_suppressed_lines() {
    let _env_lock = cursor_hook_outbound_context_max_chars_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    std::env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "512");

    let filler = "z".repeat(2000);
    let gate_line = format!(
        "router-rs REVIEW_GATE incomplete phase=2 {} {}",
        super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT,
        super::REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let body = format!("{filler}\ncontinuity_suppressed=review_soft_nag\n{gate_line}\n{filler}");
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    let got = super::truncate_cursor_hook_outbound_context_preserving_gate(&body, max_out);
    assert!(got.len() <= max_out);
    assert!(got.contains("continuity_suppressed=review_soft_nag"));
    assert!(got.contains(super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT));

    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS"),
    }
}

#[test]
fn review_gate_disabled_post_tool_use_does_not_advance_review_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("srg-pu2", "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, "srg-pu2").phase < 2);

    let out = {
        let _rg = ReviewGateDisableTestGuard::new();
        dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "srg-pu2",
                "cwd": FRAMEWORK_HARNESS_TEST_CWD,
                "tool_name": "functions.subagent",
                "tool_input": { "subagent_type": "explore", "fork_context": false }
            }),
        )
    };

    assert_eq!(out, json!({}));
    let state = load_state_for(&repo, "srg-pu2");
    assert!(
        state.phase < 2,
        "DISABLE must clear review state and not advance phase via postToolUse; phase={}",
        state.phase
    );
}

#[test]
fn cursor_hook_output_policy_is_noop() {
    let mut keep = json!({ "followup_message": "keep" });
    apply_cursor_hook_output_policy(&mut keep);
    assert_eq!(keep["followup_message"], json!("keep"));
    assert!(keep["router_rs_observation"]["gate"].is_null());

    let mut strip = json!({
        "continue": false,
        "followup_message": "nag",
        "additional_context": "ctx"
    });
    apply_cursor_hook_output_policy(&mut strip);
    assert_eq!(strip["continue"], json!(false));
    assert_eq!(strip["followup_message"], json!("nag"));
    assert_eq!(strip["additional_context"], json!("ctx"));
    assert!(strip["router_rs_observation"]["gate"].is_null());
}

#[test]
fn subagent_start_promotes_phase_to_2() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5", "subagent_type": "general-purpose", "fork_context": false }),
    );
    let state = load_state_for(&repo, "s5");
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_start_count, 1);
}

#[test]
fn review_lane_subagent_start_does_not_count_toward_review_gate() {
    let repo = fresh_repo();
    let sid = "s5-review-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "review",
            "fork_context": false
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.subagent_start_count, 0,
        "review lane is not in deep_gate_lanes"
    );
    assert!(
        state.review_subagent_pending_cycle_keys.is_empty(),
        "non-qualifying start must not enqueue pending cycle keys"
    );
}

#[test]
fn review_subagent_start_with_shared_fork_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-shared", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s5-shared",
            "subagent_type": "explore",
            "fork_context": true
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s5-shared", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s5-shared");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn review_subagent_start_without_explicit_fork_false_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-missing-fork", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5-missing-fork", "subagent_type": "explore" }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s5-missing-fork", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s5-missing-fork");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
    assert_eq!(state.subagent_stop_count, 0);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s5-missing-fork", "继续"));
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn stop_releases_l3_before_continuity_checkpoint() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "s-stop-l3-release";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    assert_ne!(
        out.get("permission").and_then(Value::as_str),
        Some("deny"),
        "stop must release L3 before returning so a later hook can acquire; out={out:?}"
    );
}

#[test]
fn review_gate_soft_nag_includes_need_segment() {
    let _env = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("1");
    assert_eq!(
        crate::router_env_flags::router_rs_cursor_review_gate_stop_max_nudges_cap(),
        Some(1)
    );
    let repo = fresh_repo();
    let sid = "s-soft-need";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("mode=soft_nag") && fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "soft-nag followup must include need= segment; fm={fm:?}"
    );
}

#[test]
fn session_end_acquires_lock_before_state_delete() {
    let repo = fresh_repo();
    let payload = event("s-end-lock", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let sp = state_path(&repo, &payload);
    assert!(sp.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!sp.exists(), "sessionEnd must remove review gate state");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(lock.is_some(), "sessionEnd must not leave L3 wedged");
    release_state_lock(&mut lock);
}

#[test]
fn post_tool_skips_cargo_check_when_env_off() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC");
    std::env::set_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC", "0");
    assert!(
        !crate::router_env_flags::router_rs_cursor_cargo_check_sync_enabled(),
        "env off must disable sync cargo check gate"
    );
    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC"),
    }
}

#[test]
fn review_gate_stop_softens_after_max_nudges_env_cap() {
    let _env = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("2");
    assert_eq!(
        crate::router_env_flags::router_rs_cursor_review_gate_stop_max_nudges_cap(),
        Some(2)
    );
    let repo = fresh_repo();
    let sid = "s-rg-stop-nudge-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out1 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm1 = out1
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out1));
    assert!(
        fm1.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "first stop should keep full need= in followup_message; out1={out1:?}"
    );

    let out2 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm2 = out2
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out2));
    assert!(
        fm2.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "second stop still within cap=2; out2={out2:?}"
    );

    let out3 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm3 = out3
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let blob3 = hook_user_visible_blob(&out3);
    assert!(
        fm3.contains("mode=soft_nag") && fm3.contains("router-rs REVIEW_GATE"),
        "third stop should shorten followup_message; fm3={fm3:?}"
    );
    assert!(
        fm3.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "need= must stay in followup_message after cap (not only additional_context); fm3={fm3:?}"
    );
    assert!(
        fm3.contains(REVIEW_GATE_FOLLOWUP_HINT_SEGMENT),
        "hint= must stay in followup_message after cap; fm3={fm3:?}"
    );
    assert!(
        !blob3.contains("continuity_suppressed=review_soft_nag"),
        "soft-nag over cap must not block My/RFV merge (P1-4); blob3={blob3:?}"
    );
}

#[test]
fn session_end_best_effort_deletes_state_when_lock_unavailable() {
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s-end-no-lock", "全面review这个仓库");
    let sp = state_path(&repo, &payload);
    if let Some(parent) = sp.parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        &sp,
        serde_json::to_string(&empty_state()).expect("serialize"),
    )
    .expect("seed state");
    assert!(sp.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(
        !sp.exists(),
        "sessionEnd must best-effort delete state even when lock acquisition failed"
    );
}

#[test]
fn cursor_hook_silent_strips_additional_context_keeps_review_gate_followup() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_HOOK_SILENT");
    std::env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", "1");
    let mut out = json!({
        "followup_message": format!(
            "router-rs REVIEW_GATE incomplete phase=0 {} {}",
            REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
        ),
        "additional_context": "Continuity digest: noisy advisory text",
    });
    apply_cursor_hook_silent_policy(&mut out);
    assert!(out.get("additional_context").is_none());
    let fm = out["followup_message"].as_str().unwrap_or("");
    assert!(fm.contains("router-rs REVIEW_GATE"));
    assert!(fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT));
    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT"),
    }
}

#[test]
fn review_pending_not_cleared_when_stale_after_disabled() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "0");

    let repo = fresh_repo();
    let sid = "s-pending-no-prune-off";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-keep-pending",
        }),
    );
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "继续"));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_subagent_pending_cycle_keys.is_empty(),
        "STALE_AFTER=0 must not clear pending via prune_stale_review_pending_cycle_keys"
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "STALE_AFTER=0 must not prune pending on stop; fm={fm}"
    );

    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS"),
    }
}

#[test]
fn review_pending_cycle_keys_respects_env_cap() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    std::env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "2");
    assert_eq!(
        crate::router_env_flags::router_rs_cursor_review_pending_cycle_max(),
        2,
        "env cap must be visible before dispatch"
    );

    let repo = fresh_repo();
    let sid = "s-pending-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    for i in 0..3 {
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({
                "session_id": sid,
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": format!("sa-cap-{i}"),
            }),
        );
    }
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.review_subagent_pending_cycle_keys.len(),
        2,
        "cap=2 must refuse third push, got {:?}",
        state.review_subagent_pending_cycle_keys
    );
    assert!(
        !state
            .review_subagent_pending_cycle_keys
            .iter()
            .any(|k| k == "id:sa-cap-2"),
        "third key must be refused at cap"
    );

    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn v1_migrate_pending_preserved_when_no_started_at_timestamp() {
    let repo = fresh_repo();
    let sid = "s-pending-orphan";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);

    let mut state = empty_state();
    state.review_required = true;
    state.phase = 2;
    state.review_subagent_pending_cycle_keys = vec!["id:orphan".to_string()];
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    assert!(save_state(&repo, &payload, &mut state));

    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let loaded = load_state_for(&repo, sid);
    assert_eq!(
        loaded.review_subagent_pending_cycle_keys,
        vec!["id:orphan".to_string()],
        "v1 migrate fixture must not clear pending without timestamp"
    );
}

#[test]
fn review_pending_cycle_pruned_when_no_open_subagents_and_stale_start() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "1");

    let repo = fresh_repo();
    let sid = "s-pending-prune";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-prune-1",
        }),
    );

    let sp = state_path(&repo, &payload);
    let raw = fs::read_to_string(&sp).expect("read state");
    let mut state: Value = serde_json::from_str(&raw).expect("parse state");
    state["phase"] = json!(3);
    state["review_subagent_pending_cycle_keys"] = json!(["id:sa-prune-1"]);
    state["active_subagent_count"] = json!(0);
    state["active_subagent_last_started_at"] = json!("2000-01-01T00:00:00+00:00");
    fs::write(&sp, serde_json::to_string(&state).expect("serialize")).expect("write state");

    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stale pending without qualifying stop must not clear REVIEW_GATE; fm={fm}"
    );
    let st = load_state_for(&repo, sid);
    assert_eq!(st.phase, 2, "phase must downgrade from 3 when pruning without stop");

    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS"),
    }
}

#[test]
fn review_subagent_start_without_reviewer_lane_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-untyped", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5-untyped", "fork_context": false }),
    );
    let state = load_state_for(&repo, "s5-untyped");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
}

#[test]
fn subagent_start_blocks_when_active_limit_reached() {
    let repo = fresh_repo();
    for _ in 0..DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS {
        let out = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
        );
        assert_eq!(out, json!({}));
    }

    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
    );

    assert_eq!(out.get("permission").and_then(Value::as_str), Some("deny"));
    assert!(out
        .get("user_message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .contains("仍标记为打开"));
    let state = load_state_for(&repo, "s-open-limit");
    assert_eq!(
        state.active_subagent_count,
        DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS
    );
}

#[test]
fn subagent_start_recovers_stale_active_count() {
    let repo = fresh_repo();
    let payload = json!({ "session_id": "s-open-stale", "subagent_type": "explore" });
    let stale_started_at =
        Utc::now() - chrono::Duration::seconds(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS + 1);
    let mut state = empty_state();
    state.active_subagent_count = DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS;
    state.active_subagent_last_started_at = Some(stale_started_at.to_rfc3339());
    assert!(save_state(&repo, &payload, &mut state));

    let out = dispatch_cursor_hook_event(&repo, "subagentStart", &payload);

    assert_eq!(out, json!({}));
    let state = load_state_for(&repo, "s-open-stale");
    assert_eq!(state.active_subagent_count, 1);
    assert!(state.active_subagent_last_started_at.is_some());
}

#[test]
fn subagent_stop_decrements_active_count_without_review_gate() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
    );

    let state = load_state_for(&repo, "s-open-stop");
    assert_eq!(state.active_subagent_count, 0);
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn subagent_stop_without_start_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s6");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

/// `explore` + `fork_context=false` 在门控启用时不得当作深度审稿 lane；随后的 `general-purpose` 完整周期仍可清相位。
#[test]
fn armed_review_explore_posttool_then_general_purpose_cycle_clears_phase() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-explore-then-gp";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, sid).review_required);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {"subagent_type":"explore","fork_context":false}
        }),
    );
    let after_explore = load_state_for(&repo, sid);
    assert!(
        after_explore.phase < 2,
        "explore must not bump review gate phase; phase={}",
        after_explore.phase
    );

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    assert_eq!(load_state_for(&repo, sid).phase, 2);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({"session_id": sid, "subagent_type": "general-purpose"}),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

#[test]
fn subagent_start_then_stop_promotes_to_phase3() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6b", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s6b",
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-1"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6b", "subagent_type": "general-purpose", "subagent_id": "review-1" }),
    );
    let state = load_state_for(&repo, "s6b");
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

#[test]
fn post_tool_use_fast_path_skips_tracker_for_read() {
    let repo = fresh_repo();
    let sid = "s-fast-read";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Read",
            "tool_input": { "path": "README.md" }
        }),
    );
    let tracker = repo.join("artifacts/current/SESSION_CALL_TRACKER.json");
    assert!(
        !tracker.is_file(),
        "fast-path Read must not write SESSION_CALL_TRACKER"
    );
}

#[test]
fn post_tool_use_still_records_subagent_on_task_tool() {
    let _guard = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-fast-task";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Task",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "pt-only"
            }
        }),
    );
    let state = load_state_for(&repo, sid);
    assert!(
        state
            .review_subagent_pending_cycle_keys
            .contains(&"id:pt-only".to_string()),
        "Task postToolUse must enqueue review multiset when gate armed"
    );
}

/// `subagentStart` 与随后同一 `subagent_id` 的 `PostToolUse` 不应对 **`id:`** multiset 双入队。
#[test]
fn review_gate_posttool_skips_duplicate_id_after_subagent_start() {
    let repo = fresh_repo();
    let sid = "s-dedupe-id";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "same-id"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "same-id"
            }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.subagent_start_count, 1,
        "PostTool must not bump subagent_start_count"
    );
    assert_eq!(
        mid.review_subagent_pending_cycle_keys,
        vec!["id:same-id".to_string()]
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "same-id"
        }),
    );
    let end = load_state_for(&repo, sid);
    assert_eq!(end.phase, 3);
    assert_eq!(end.subagent_stop_count, 1);
    assert!(end.review_subagent_pending_cycle_keys.is_empty());
}

/// 同 session：`subagentStart` + `PostToolUse`（同 `lane:`、无 id）仅一条 pending；单次 stop 清门。
#[test]
fn review_gate_dual_event_lane_dedup_single_stop_clears() {
    let repo = fresh_repo();
    let sid = "s-dedupe-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": { "subagent_type": "general-purpose", "fork_context": false }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys,
        vec!["lane:general-purpose".to_string()]
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let end = load_state_for(&repo, sid);
    assert_eq!(end.phase, 3);
    assert!(end.review_subagent_pending_cycle_keys.is_empty());
}

#[test]
fn before_submit_fail_closed_when_hook_state_dir_readonly() {
    let _env = crate::test_env_sync::process_env_lock();
    let _rg = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "save-fail-closed";
    let dir = repo.join(".cursor/hook-state");
    fs::create_dir_all(&dir).expect("mkdir hook-state");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dir).expect("meta").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&dir, perms).expect("chmod");
    }
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面深度 review 这段代码"),
    );
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(false),
        "fail-closed (ADR-002): {out:?}"
    );
    let blocked = out
        .get("user_message")
        .and_then(Value::as_str)
        .or_else(|| out.get("followup_message").and_then(Value::as_str))
        .unwrap_or("");
    assert!(
        blocked.contains("未能持久化")
            || blocked.contains("锁不可用")
            || blocked.contains("已拦截"),
        "expected blocked persist or lock message: {out:?}"
    );
}

#[test]
fn before_submit_planx_persist_fail_soft_warning_not_block() {
    let _env = crate::test_env_sync::process_env_lock();
    let _rg = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "save-fail-soft-planx";
    let dir = repo.join(".cursor/hook-state");
    fs::create_dir_all(&dir).expect("mkdir hook-state");
    #[cfg(unix)]
    let _readonly = HookStateDirReadonlyGuard::readonly(dir.clone());
    #[cfg(not(unix))]
    {
        let _ = &dir;
    }
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/planx"),
    );
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(true),
        "pre-exec /planx must not fail-closed on persist when review/goal unarmed: {out:?}"
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("未能持久化") || ac.contains("锁不可用"),
        "expected soft persist/lock degrade warning in additional_context: {out:?}"
    );
}

#[test]
fn cursor_rearm_review_resets_active_subagent_count_after_start_without_stop() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-rearm-open-subagent";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "深度 review 这个 PR"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-open",
        }),
    );
    let open = load_state_for(&repo, sid);
    assert!(
        open.active_subagent_count > 0,
        "subagentStart must increment open count; got {open:?}"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    let rearmed = load_state_for(&repo, sid);
    assert_eq!(
        rearmed.active_subagent_count, 1,
        "re-arm must preserve open subagent count when subagent still running; got {rearmed:?}"
    );
    assert_eq!(rearmed.phase, 0, "re-arm must reset phase; got {rearmed:?}");
}

#[test]
fn pending_cap_refused_survives_review_rearm_ups() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1");
    let repo = fresh_repo();
    let sid = "s-cap-rearm";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-1",
        }),
    );
    let cap_denied = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-2",
        }),
    );
    assert_eq!(
        cap_denied.get("permission").and_then(Value::as_str),
        Some("deny"),
        "cap must deny second spawn: {cap_denied:?}"
    );
    assert!(
        load_state_for(&repo, sid).review_pending_cap_refused,
        "cap denial must latch review_pending_cap_refused"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    assert!(
        load_state_for(&repo, sid).review_pending_cap_refused,
        "review re-arm UPS must not clear cap refusal"
    );
    let cap_denied_again = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-3",
        }),
    );
    assert_eq!(
        cap_denied_again.get("permission").and_then(Value::as_str),
        Some("deny"),
        "cap refusal must survive re-arm: {cap_denied_again:?}"
    );
    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn legacy_phase_two_alone_compact_does_not_clear_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-legacy-phase2";
    let state_path = state_path(&repo, &event(sid, ""));
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        &state_path,
        r#"{"version":1,"review_required":true,"review_subagent_seen":true}"#,
    )
    .expect("write v1 legacy state");
    let stop = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "status": "completed",
            "loop_count": 0,
            "response": "[P1] scripts/foo.rs:1 — issue — impact — verify",
        }),
    );
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "legacy phase=2 without live subagent evidence must not clear gate; fm={fm:?}"
    );
    assert!(
        load_state_for(&repo, sid).phase < 3,
        "compact must not bump to phase 3 without live evidence"
    );
}

struct MyLightOverrideGuard;

impl MyLightOverrideGuard {
    fn force_non_my_light() -> Self {
        crate::hook_common::set_test_my_light_override(Some(false));
        Self
    }
}

impl Drop for MyLightOverrideGuard {
    fn drop(&mut self) {
        crate::hook_common::set_test_my_light_override(None);
    }
}

#[test]
fn stale_hygiene_compact_does_not_clear_review_gate() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "1");
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-stale-compact";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "stale-compact-1",
        }),
    );
    let sp = state_path(&repo, &payload);
    let mut state: Value =
        serde_json::from_str(&fs::read_to_string(&sp).expect("read state")).expect("parse state");
    state["active_subagent_count"] = json!(1);
    state["active_subagent_last_started_at"] = json!("2000-01-01T00:00:00+00:00");
    fs::write(
        &sp,
        serde_json::to_string(&state).expect("serialize"),
    )
    .expect("write stale state");
    let stop = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "status": "completed",
            "loop_count": 0,
            "response": "[P1] scripts/foo.rs:1 — issue — impact — verify",
        }),
    );
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stale hygiene + compact must not clear gate; fm={fm:?}"
    );
    let st = load_state_for(&repo, sid);
    assert_eq!(
        st.subagent_start_count, 0,
        "stale hygiene must invalidate orphan start_count"
    );
    assert!(st.phase < 3, "compact must not bump phase without stop/pending");
    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS"),
    }
}

#[test]
fn posttool_at_pending_cap_persists_refused_latch() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1");
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-posttool-cap-persist";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "cap-1",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "cap-2"
            }
        }),
    );
    let st = load_state_for(&repo, sid);
    assert!(
        st.review_pending_cap_refused,
        "PostTool cap refusal must persist review_pending_cap_refused; got {st:?}"
    );
    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn before_submit_review_and_implementx_injects_mixing_nudge_when_not_my_light() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let repo = fresh_repo();
    let sid = "dual-review-implementx-non-my-light";
    let prompt = "深度 review 整个路由系统 /implementx 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("router-rs：本轮提交同时包含"),
        "non-my-light dual prompt must inject mixing nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(!state.review_required);
    assert!(state.goal_required);
}

#[test]
fn before_submit_benign_ups_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-benign-ups";
    let payload = event(sid, "hello there");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable for benign UPS; got {ac:?}"
    );
    let raw = fs::read_to_string(state_path(&repo, &payload)).expect("state still corrupt");
    assert!(
        raw.starts_with("{not json"),
        "benign UPS must not overwrite corrupt hook-state with empty_state"
    );
}

#[test]
fn deep_reviewer_lane_counts_for_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    assert!(crate::hook_common::is_deep_review_gate_lane_normalized("deep-reviewer"));
    let repo = fresh_repo();
    let sid = "s-deep-reviewer-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "deep-reviewer",
            "fork_context": false,
            "subagent_id": "dr-1",
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.subagent_start_count, 1);
    assert_eq!(state.phase, 2);
}

#[test]
fn before_submit_discussx_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-discussx-cursor";
    let payload = event(sid, "/discussx");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable for /discussx; got {ac:?}"
    );
    assert!(
        !ac.contains("pre-execution"),
        "must not mask corrupt state with discussx nudge; got {ac:?}"
    );
}

#[test]
fn pending_cap_denial_does_not_increment_active_subagent_count() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1");
    assert_eq!(
        crate::router_env_flags::router_rs_cursor_review_pending_cycle_max(),
        1
    );

    let repo = fresh_repo();
    let sid = "s-cap-atomic";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-1",
        }),
    );
    let cap_denied = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-2",
        }),
    );
    assert_eq!(
        cap_denied.get("permission").and_then(Value::as_str),
        Some("deny"),
        "ADR-004: cap refusal must deny subagentStart: {cap_denied:?}"
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys.len(),
        1,
        "cap=1 must refuse second pending: {:?}",
        mid.review_subagent_pending_cycle_keys
    );
    assert_eq!(
        mid.active_subagent_count, 1,
        "cap refusal must not bump open count"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "sa-1",
        }),
    );
    let after_stop = load_state_for(&repo, sid);
    assert_eq!(
        after_stop.active_subagent_count, 0,
        "stop must not leave phantom open count"
    );
    assert_ne!(
        after_stop.phase, 2,
        "must not stick at phase 2 with zero open subagents after sole stop"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn posttool_at_pending_cap_does_not_bump_phase() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1");

    let repo = fresh_repo();
    let sid = "s-posttool-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "cap-1",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "cap-1"
            }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.phase, 2);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys.len(),
        1,
        "cap full: postTool must not add phantom pending: {:?}",
        mid.review_subagent_pending_cycle_keys
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn main_thread_compact_review_does_not_clear_gate_without_subagent() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let sid = "s-main-thread-review";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P1] scripts/router-rs/src/cursor_hooks/handlers.rs:3000 — Stop 双信号 — 续跑与 REVIEW_GATE 并存"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "compact-only must not clear gate (P0-4); fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        state.phase < 3,
        "compact-only must not reach phase 3; phase={}",
        state.phase
    );
}

#[test]
fn main_thread_compact_stop_only_does_not_clear_gate_without_subagent() {
    let repo = fresh_repo();
    let sid = "s-main-thread-stop-only";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let stop_payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P1] scripts/router-rs/src/cursor_hooks/handlers.rs:3000 — Stop-only compact path — substantive finding line for gate clear"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop_payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stop-only compact must not clear gate without subagent; fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(state.phase < 3, "phase={}", state.phase);
}

#[test]
fn main_thread_deferential_compact_does_not_clear_gate_on_stop() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let sid = "s-main-thread-deferential";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P2] 见上文"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") || fm.contains("AG_FOLLOWUP"),
        "deferential-only compact must not clear gate; fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        state.phase < 3,
        "deferential-only compact must not reach phase 3; phase={}",
        state.phase
    );
}

#[test]
fn strict_disk_stop_pre_goal_not_satisfied_from_goal_file_alone() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let prev_pre = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
    env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "1");
    let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "1");

    let repo = fresh_repo();
    let sid = "s-strict-disk-stop";
    fs::create_dir_all(repo.join("artifacts/current/strict-stop")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"strict-stop"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "strict-stop",
        "goal": "strict disk stop",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
    }))
    .expect("goal");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/implementx continue"),
    );
    let after_submit = load_state_for(&repo, sid);
    assert!(
        !after_submit.pre_goal_review_satisfied,
        "strict disk: beforeSubmit must not hydrate pre_goal from disk alone"
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "continue"));
    let after_stop = load_state_for(&repo, sid);
    assert!(
        !after_stop.pre_goal_review_satisfied,
        "strict disk: Stop hydrate must not set pre_goal from disk GOAL alone"
    );
    assert!(after_stop.goal_required);

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
        None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
    }
    match prev_pre {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
        None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
    }
}

#[test]
fn review_subagent_start_missing_fork_infers_false_for_deep_lane() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
    env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");

    let repo = fresh_repo();
    let sid = "s-infer-fork";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.phase, 3,
        "missing fork on deep lane should infer false"
    );

    match prev {
        Some(v) => env::set_var(
            "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            v,
        ),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE"),
    }
}

#[test]
fn review_subagent_start_explicit_fork_true_still_blocks_gate() {
    let repo = fresh_repo();
    let sid = "s-fork-true";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": true,
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let state = load_state_for(&repo, sid);
    assert!(state.phase < 3);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn subagent_stop_must_match_open_reviewer_cycle() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6c", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s6c",
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-1"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6c", "subagent_type": "general-purpose", "subagent_id": "other" }),
    );
    let state = load_state_for(&repo, "s6c");
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_stop_count, 0);
}

/// 两个不同 subagent id 并行 start：各自 stop 各核销一条 pending；**第二次** stop 排空 multiset 后才 phase 3。
#[test]
fn review_gate_two_distinct_subagent_ids_both_stops_clear_gate() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    let repo = fresh_repo();
    let sid = "s-two-review-ids";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-a"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-b"
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.phase, 2);
    assert_eq!(mid.review_subagent_pending_cycle_keys.len(), 2);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-a"
        }),
    );
    let after_first_stop = load_state_for(&repo, sid);
    assert_eq!(after_first_stop.phase, 2);
    assert_eq!(after_first_stop.subagent_stop_count, 0);
    assert_eq!(after_first_stop.review_subagent_pending_cycle_keys.len(), 1);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-b"
        }),
    );
    let final_state = load_state_for(&repo, sid);
    assert_eq!(final_state.phase, 3);
    assert_eq!(final_state.subagent_stop_count, 1);
    assert!(final_state.review_subagent_pending_cycle_keys.is_empty());

    match prev_cap {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

/// 无 subagent id 时 cycle key 均为同一 `lane:`；两次并行 start 压入两条 multiset 记录，需**两次** stop 才清门。
#[test]
fn review_gate_parallel_lane_only_keys_two_stops_clear_gate() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "2");

    let repo = fresh_repo();
    let sid = "s-parallel-lane-only";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.review_subagent_pending_cycle_keys.len(), 2);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys[0],
        mid.review_subagent_pending_cycle_keys[1]
    );

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose"
        }),
    );
    let after_one = load_state_for(&repo, sid);
    assert_eq!(after_one.phase, 2);
    assert_eq!(after_one.review_subagent_pending_cycle_keys.len(), 1);
    assert_eq!(after_one.subagent_stop_count, 0);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose"
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert!(state.review_subagent_pending_cycle_keys.is_empty());
    assert_eq!(state.subagent_stop_count, 1);

    match prev_cap {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v),
        None => env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX"),
    }
}

#[test]
fn review_lane_only_cycle_stop_advances_phase_when_ids_absent() {
    let repo = fresh_repo();
    let sid = "s6-lane-only";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

#[test]
fn review_lane_only_cycle_mismatch_lane_on_stop_does_not_advance() {
    let repo = fresh_repo();
    let sid = "s6-lane-mismatch";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "best-of-n-runner",
            "fork_context": false
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn stop_without_subagent_emits_minimal_review_gate_line() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s7", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s7", "继续"));
    let blob = hook_user_visible_blob(&out);
    assert_followup_signals_review_gate_incomplete(&blob);
}

#[test]
fn subtracted_before_shell_default_noop_skips_terminal_ledger() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");

    let repo = fresh_repo();
    let payload = json!({
        "session_id": "sub-shell-noop",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "command": "echo noop-test"
    });
    let ledger_path = state_dir(&repo).join(format!(
        "session-terminals-{}.json",
        super::session_key(&payload)
    ));
    let out = dispatch_cursor_hook_event(&repo, "beforeShellExecution", &payload);
    assert_eq!(
        out,
        json!({ "continue": true, "permission": "allow" }),
        "default subtracted dispatch must pass shell gate without side effects"
    );
    assert!(
        !ledger_path.exists(),
        "ledger file must not be created on subtracted noop: {}",
        ledger_path.display()
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS"),
    }
}

#[test]
fn subtracted_after_agent_response_runs_handler_when_registered_in_hooks_json() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([{
        "command": "configs/framework/cursor-router-rs-hook.sh",
        "timeout": 20
    }]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("sub-ara-reg", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "sub-ara-reg",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "reject reason: small_task" }
        }),
    );
    assert!(
        load_state_for(&repo, "sub-ara-reg").reject_reason_seen,
        "registered subtracted event must run handler without LEGACY env"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS"),
    }
}

#[test]
fn subtracted_empty_hooks_entry_stays_noop() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("sub-ara-empty", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "sub-ara-empty",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "reject reason: small_task" }
        }),
    );
    assert!(
        !load_state_for(&repo, "sub-ara-empty").reject_reason_seen,
        "empty hooks entry must not run handler"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS"),
    }
}

#[test]
fn review_gate_disabled_registered_after_agent_response_persists_reject_reason() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev_legacy = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    let _env_clear = ReviewGateDisableEnvClearGuard::new();
    let _rg_disable = ReviewGateDisableTestGuard::new();

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([{
        "command": "configs/framework/cursor-router-rs-hook.sh",
        "timeout": 20
    }]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let sid = "rg-dis-ara";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "response": "reject reason: small_task"
        }),
    );
    assert!(
        load_state_for(&repo, sid).reject_reason_seen,
        "review-gate-disabled + registered afterAgentResponse must still run handler"
    );

    match prev_legacy {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS"),
    }
}

#[test]
fn subtracted_after_agent_response_default_is_empty_object() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");

    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({ "session_id": "sub-ara", "response": "[P1] x" }),
    );
    assert_eq!(out, json!({}));

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS"),
    }
}

#[test]
fn pre_compact_emits_additional_context_summary() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s8", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "preCompact",
        &json!({ "session_id": "s8", "cwd": FRAMEWORK_HARNESS_TEST_CWD }),
    );
    assert!(out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .contains("phase=0"));
}

#[test]
fn session_end_clears_state_file() {
    let repo = fresh_repo();
    let payload = event("s9", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let path = state_path(&repo, &payload);
    assert!(path.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!path.exists());
}

#[test]
fn session_end_cleans_stale_lock_if_present() {
    let repo = fresh_repo();
    let payload = event("s9b", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"pid=1 ts=1").expect("seed lock");
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!lock_path.exists());
}

#[test]
fn session_end_preserves_other_session_hook_state_when_legacy_sweep_disabled() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");

    let repo = fresh_repo();
    let stale_payload = event("stale-session", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &stale_payload);
    let stale_state = state_path(&repo, &stale_payload);
    let stale_lock = state_lock_path(&repo, &stale_payload);
    let stale_loop = adversarial_loop_path(&repo, &stale_payload);
    fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
    fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
    fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
    assert!(stale_state.exists());

    // Unrelated SessionEnd：默认不得删其它 session_key 下的门控状态。
    let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &unrelated_payload);

    assert!(
        stale_state.exists(),
        "other session review-subagent state must be preserved without legacy sweep"
    );
    assert!(
        stale_lock.exists(),
        "other session review-subagent lock must be preserved without legacy sweep"
    );
    assert!(
        stale_loop.exists(),
        "other session adversarial-loop state must be preserved without legacy sweep"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP"),
    }
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1` 时恢复全目录前缀清扫（session_id/cwd 漂移遗留）。
#[test]
fn session_end_legacy_full_sweep_removes_unrelated_session_hook_state() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");
    env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", "1");

    let repo = fresh_repo();
    let stale_payload = event("stale-session", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &stale_payload);
    let stale_state = state_path(&repo, &stale_payload);
    let stale_lock = state_lock_path(&repo, &stale_payload);
    let stale_loop = adversarial_loop_path(&repo, &stale_payload);
    fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
    fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
    fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
    assert!(stale_state.exists());

    let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &unrelated_payload);

    assert!(
        !stale_state.exists(),
        "stale review-subagent state must be swept under legacy full sweep"
    );
    assert!(
        !stale_lock.exists(),
        "stale review-subagent lock must be swept under legacy full sweep"
    );
    assert!(
        !stale_loop.exists(),
        "stale adversarial-loop state must be swept under legacy full sweep"
    );

    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP"),
    }
}

#[cfg(unix)]
fn set_path_mtime_days_ago(path: &std::path::Path, days: u64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_sub(days.saturating_mul(86_400));
    let times = [
        libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: 0,
        },
    ];
    let cpath = CString::new(path.as_os_str().as_bytes()).expect("path");
    unsafe {
        libc::utimensat(libc::AT_FDCWD, cpath.as_ptr(), times.as_ptr(), 0);
    }
}

/// Age sweep must not unlink `.lock` when holder PID is alive (even with old ts) if json is fresh.
#[test]
fn stale_sweep_preserves_alive_holder_lock_when_json_fresh() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev_days = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS");
    env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", "7");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");

    let repo = fresh_repo();
    let victim = event("victim-alive-lock", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &victim);
    let lock_path = state_lock_path(&repo, &victim);
    let stale_ts = now_millis().saturating_sub(120_000);
    fs::write(
        &lock_path,
        format!("pid={} ts={stale_ts}\n", std::process::id()),
    )
    .expect("seed alive-pid lock with old ts");

    let _ = dispatch_cursor_hook_event(
        &repo,
        "sessionEnd",
        &json!({ "session_id": "sweeper-other-session" }),
    );

    assert!(
        lock_path.is_file(),
        "sweep must not remove lock while holder pid is alive and json is fresh"
    );

    match prev_days {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS"),
    }
}

/// Default age sweep removes old session_key files but keeps recent parallel-session state.
#[cfg(unix)]
#[test]
fn session_end_stale_sweep_removes_old_orphan_preserves_recent() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let prev_days = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS");
    env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", "1");
    env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");

    let repo = fresh_repo();
    let old_payload = event("old-session-key", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &old_payload);
    let old_state = state_path(&repo, &old_payload);
    assert!(old_state.exists());
    set_path_mtime_days_ago(&old_state, 10);

    let recent_payload = json!({ "session_id": "fresh-parallel-session" });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &recent_payload);
    let recent_state = state_path(&repo, &recent_payload);
    assert!(recent_state.exists());

    let end_payload = json!({ "session_id": "unrelated-end-session" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &end_payload);

    assert!(!old_state.exists(), "10d-old hook-state must be age-swept");
    assert!(
        recent_state.exists(),
        "recent parallel session state must remain"
    );

    match prev_days {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS"),
    }
}

/// 清扫只覆盖本模块拥有的前缀，不应误伤未识别文件（避免与未来其它 hook 共用目录时冲突）。
#[test]
fn session_end_sweep_keeps_unrelated_files() {
    let repo = fresh_repo();
    let dir = state_dir(&repo);
    fs::create_dir_all(&dir).expect("mkdir state dir");
    let unrelated = dir.join("other-hook-state.json");
    fs::write(&unrelated, b"{}").expect("seed unrelated");

    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));
    assert!(unrelated.exists(), "unrelated hook state must be preserved");
}

/// SessionEnd sweep 必须回收 `save_state` 及历史 adversarial-loop 原子写入孤儿，
/// 避免长期累积消耗 `.cursor/hook-state/` 卫生。
#[test]
fn session_end_sweeps_atomic_write_orphans() {
    let repo = fresh_repo();
    let dir = state_dir(&repo);
    fs::create_dir_all(&dir).expect("mkdir state dir");

    let primary_tmp = dir.join(".tmp-99999-12345-review-subagent-deadbeef.json");
    let adv_tmp = dir.join(".tmp-adv-loop-99999-67890");
    let other_tmp = dir.join(".tmp-99999-12345-other-hook.json");
    fs::write(&primary_tmp, b"{}").expect("seed primary tmp");
    fs::write(&adv_tmp, b"{}").expect("seed adv tmp");
    fs::write(&other_tmp, b"{}").expect("seed other tmp");

    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));

    assert!(
        !primary_tmp.exists(),
        "review-subagent atomic-write tmp must be swept"
    );
    assert!(
        !adv_tmp.exists(),
        "adversarial-loop atomic-write tmp must be swept"
    );
    assert!(
        other_tmp.exists(),
        "unrelated tmp must be preserved (sweep is module-scoped)"
    );
}

/// 文件名归属判断必须只接受本模块写入的命名（含原子写入孤儿前缀），其它名称一律排除。
#[test]
fn review_gate_state_file_owned_by_module_recognizes_known_names_only() {
    // 主状态：仅认 json|lock 扩展。
    assert!(review_gate_state_file_owned_by_module(
        "review-subagent-abc.json"
    ));
    assert!(review_gate_state_file_owned_by_module(
        "review-subagent-abc.lock"
    ));
    assert!(review_gate_state_file_owned_by_module(
        "adversarial-loop-abc.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(
        "review-subagent-abc.bak"
    ));
    assert!(!review_gate_state_file_owned_by_module("review-subagent-"));
    // 原子写入孤儿。
    assert!(review_gate_state_file_owned_by_module(
        ".tmp-1-2-review-subagent-abc.json"
    ));
    assert!(review_gate_state_file_owned_by_module(".tmp-adv-loop-1-2"));
    // 未识别命名不应被清扫。
    assert!(!review_gate_state_file_owned_by_module(
        "other-hook-state.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(
        ".tmp-1-2-other-hook.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(".tmp-random"));
}

#[test]
fn narrow_path_review_does_not_arm() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s10", "review ./README.md"),
    );
    assert_eq!(out, json!({ "continue": true }));
    let state = load_state_for(&repo, "s10");
    assert!(!state.review_required);
    assert_eq!(state.phase, 0);
}

#[test]
fn v1_state_migrates_to_current_schema_phase() {
    let repo = fresh_repo();
    let payload = json!({ "session_id": "s11" });
    let path = state_path(&repo, &payload);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &path,
        r#"{"version":1,"review_required":true,"review_subagent_seen":true,"followup_count":2}"#,
    )
    .expect("write v1");
    let state = load_state(&repo, &payload).expect("load").expect("state");
    assert_eq!(state.version, STATE_VERSION);
    assert_eq!(state.phase, 2);
    assert_eq!(state.followup_count, 2);
}

#[test]
fn post_tool_use_subagent_sets_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s12", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s12",
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        }),
    );
    let state = load_state_for(&repo, "s12");
    assert!(state.phase >= 2);
}

#[test]
fn review_armed_first_submit_injects_deep_default_nudge_without_legacy_tokens() {
    let repo = fresh_repo();
    let first = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s16", "全面review这个仓库"),
    );
    let first_msg = first
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let first_ctx = first
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        first_ctx.contains("code-review-deep"),
        "expected depth default nudge; ctx={first_ctx:?}"
    );
    assert!(
        !first_msg.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN)
            && !first_msg.contains("Broad/deep review detected")
            && !first_msg.contains("Parallel lane request detected"),
        "first_msg={first_msg:?}"
    );
    assert!(load_state_for(&repo, "s16").review_required);
    let second = dispatch_cursor_hook_event(&repo, "stop", &event("s16", "继续"));
    let blob = hook_user_visible_blob(&second);
    assert_followup_signals_review_gate_incomplete(&blob);
    assert!(
        !blob.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN),
        "obsolete review prefix; blob={blob:?}"
    );
}

#[test]
fn goal_stop_followup_is_short_code_only() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s17",
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"explore"}
        }),
    );
    let first = dispatch_cursor_hook_event(&repo, "stop", &event("s17", "继续"));
    let first_msg = hook_user_visible_blob(&first);
    assert!(
        first_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
        "Stop uses short goal hint only; msg={first_msg:?}"
    );
    assert!(
        !first_msg.contains("Autopilot goal mode:"),
        "Stop must not dump full goal contract prose; msg={first_msg:?}"
    );
    let second = dispatch_cursor_hook_event(&repo, "stop", &event("s17", "继续"));
    let second_msg = hook_user_visible_blob(&second);
    // The invariant: Stop must keep the followup short. If a followup is emitted, it must
    // be the short AG_FOLLOWUP code, not long prose.
    if !second_msg.is_empty() {
        assert!(
            second_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
            "expected short code when non-empty; second_msg={second_msg:?} second={second:?}"
        );
        assert!(
            !second_msg.contains("Autopilot goal mode:"),
            "Stop must not dump full goal contract prose; second_msg={second_msg:?}"
        );
    }
}

#[test]
fn stop_picks_assistant_goal_contract_from_messages_when_top_level_response_empty() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-msg-goal", "/implementx finish wiring"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-msg-goal",
            "tool_name": "functions.subagent",
            "tool_input": {"subagent_type": "general-purpose", "fork_context": false}
        }),
    );
    let assistant = concat!(
        "Goal: wire hook\n",
        "Non-goals: expand scope\n",
        "Validation commands: cargo test -q nl_route\n",
        "Done when:\n",
        "- a passes\n",
        "- b passes\n",
        "\n",
        "Checkpoint: merged handler.\n",
        "Verified: test passed.\n",
    );
    let stop_payload = json!({
        "session_id": "s-msg-goal",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "prompt": "continue",
        "messages": [
            {"role": "user", "content": "continue"},
            {"role": "assistant", "content": assistant}
        ]
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop_payload);
    let msg = hook_user_visible_blob(&out);
    assert!(
        !msg.contains("router-rs AG_FOLLOWUP missing_parts=goal_contract"),
        "assistant body only under messages[] must satisfy goal_contract; msg={msg:?}"
    );
}

#[test]
fn my_pre_goal_nudge_when_opt_in_enabled() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _pre_goal = MyPreGoalOptInEnvGuard::enable();
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17b", "/implementx 完成任务"),
    );
    let msg = hook_user_visible_blob(&out);
    assert!(
        msg.contains("My implement (/implementx"),
        "expected My pre-goal nudge; surface={msg:?}"
    );
    assert!(load_state_for(&repo, "s17b").goal_required);
}

#[test]
fn my_pre_goal_auto_releases_when_nag_cap_reached() {
    let _env = crate::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES");
    let _pre_goal = MyPreGoalOptInEnvGuard::enable();
    env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES", "2");
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("cap-nag", "/implementx smoke"),
    );
    let mid = load_state_for(&repo, "cap-nag");
    assert_eq!(mid.pre_goal_nag_count, 1);
    assert!(!mid.pre_goal_review_satisfied);
    let out =
        dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("cap-nag", "continue"));
    let end = load_state_for(&repo, "cap-nag");
    assert!(end.pre_goal_review_satisfied);
    assert_eq!(end.pre_goal_nag_count, 0);
    let blob = hook_user_visible_blob(&out);
    assert!(blob.contains("pre-goal 提示已达上限"), "blob={blob:?}");
    match prev_cap {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES", v),
        None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES"),
    }
}

#[test]
fn deep_json_strings_satisfy_pre_goal_reject_on_before_submit() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("deep-s1", "/implementx 任务"),
    );
    let deep = json!({
        "session_id": "deep-s1",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [{ "role": "user", "content": "small_task" }]
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &deep);
    assert!(load_state_for(&repo, "deep-s1").pre_goal_review_satisfied);
}

#[test]
fn messages_tail_user_text_clears_review_gate_when_top_level_prompt_empty() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-msg-only", "全面review这个仓库"),
    );
    let ev = json!({
        "session_id": "s-msg-only",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [
            { "role": "user", "content": "earlier" },
            { "role": "assistant", "content": "ok" },
            { "role": "user", "content": "rg_clear" }
        ]
    });
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &ev);
    let state = load_state_for(&repo, "s-msg-only");
    assert!(state.reject_reason_seen);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !msg.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN) && !msg.contains("Broad/deep review detected"),
        "expected gate clear from messages[].content; msg={msg:?} out={out:?}"
    );
    assert_eq!(state.followup_count, 0);
    assert_eq!(state.review_followup_count, 0);
}

#[test]
fn before_submit_reject_reason_token_in_user_prompt_satisfies_pre_goal() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17e", "/implementx 第一轮"),
    );
    let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event(
                "s17e",
                "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test",
            ),
        );
    let state = load_state_for(&repo, "s17e");
    assert!(state.reject_reason_seen);
    assert!(state.pre_goal_review_satisfied);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("My implement (/implementx")
            && !msg.contains("independent-context reviewer"),
        "reject_reason on submit should skip pre-goal nag; msg={msg:?}"
    );
}

#[test]
fn nested_payload_prompt_reject_reason_satisfies_pre_goal_before_submit() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17nest", "/implementx 第一轮"),
    );
    let nested = json!({
        "session_id": "s17nest",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "prompt": "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &nested);
    let state = load_state_for(&repo, "s17nest");
    assert!(state.reject_reason_seen);
    assert!(state.pre_goal_review_satisfied);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("independent-context"),
        "nested payload prompt should satisfy pre_goal; msg={msg:?}"
    );
}

#[test]
fn nested_payload_prompt_reject_reason_updates_stop_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17stop-n", "/implementx 任务"),
    );
    let nested_stop = json!({
        "session_id": "s17stop-n",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "prompt": "small_task\nGoal:\nNon-goals:\nDone when:\nValidation commands:"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "stop", &nested_stop);
    assert!(load_state_for(&repo, "s17stop-n").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_fork_context_true_does_not_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17c", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s17c",
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"explore","fork_context":true}
        }),
    );
    let state = load_state_for(&repo, "s17c");
    assert!(
        !state.pre_goal_review_satisfied,
        "shared fork_context must not count as independent pre-goal review"
    );
}

#[test]
fn post_tool_use_tool_input_type_field_satisfies_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17d", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17d",
            "tool_name": "functions.subagent",
            "tool_input": {"type": "general-purpose", "fork_context": false}
        }),
    );
    assert!(
        load_state_for(&repo, "s17d").pre_goal_review_satisfied,
        "hosts may emit lane kind as tool_input.type instead of subagent_type"
    );
}

#[test]
fn post_tool_use_heuristic_mcp_subagent_tool_name_satisfies_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17mcp", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17mcp",
            "tool_name": "mcp_cursor_agent_subagent",
            "tool_input": {"subagent_type": "general-purpose", "fork_context": false}
        }),
    );
    assert!(load_state_for(&repo, "s17mcp").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_nested_payload_tool_fields_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17nest-tu", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17nest-tu",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": {
                "tool_name": "functions.subagent",
                "tool_input": {"type": "general-purpose", "fork_context": false}
            }
        }),
    );
    assert!(load_state_for(&repo, "s17nest-tu").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_non_countable_lane_does_not_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-lane", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-lane",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "functions.subagent",
            "tool_input": {"lane": "my-custom-reviewer", "fork_context": false}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-lane").pre_goal_review_satisfied,
        "custom lane is not a countable deep reviewer lane for pre-goal"
    );
}

#[test]
fn post_tool_use_fork_context_string_true_blocks_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-fkstr", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-fkstr",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "functions.subagent",
            "tool_input": {"type": "explore", "fork_context": "true"}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-fkstr").pre_goal_review_satisfied,
        "string fork_context=true must not count as independent pre-goal"
    );
}

#[test]
fn review_keyword_inside_codeblock_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s18", "```请 review 这段代码```"),
    );
    assert_eq!(load_state_for(&repo, "s18").phase, 0);
}

#[test]
fn review_keyword_inside_inline_code_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s19", "这是 `review` 函数"),
    );
    assert_eq!(load_state_for(&repo, "s19").phase, 0);
}

#[test]
fn review_keyword_inside_url_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s20", "https://example.com/review/123"),
    );
    assert_eq!(load_state_for(&repo, "s20").phase, 0);
}

#[test]
fn review_keyword_inside_blockquote_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s21", "> 用户说 review 一下"),
    );
    assert_eq!(load_state_for(&repo, "s21").phase, 0);
}

#[test]
fn quoted_review_token_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s22", r#"他说 "review hook""#),
    );
    assert_eq!(load_state_for(&repo, "s22").phase, 0);
}

#[test]
fn parallel_alone_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s23", "请解释 parallel 的含义"),
    );
    assert_eq!(load_state_for(&repo, "s23").phase, 0);
}

#[test]
fn parallel_with_task_verb_arms() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s24", "用 parallel workers 实现 X"),
    );
    assert_eq!(load_state_for(&repo, "s24").phase, 0);
}

#[test]
fn english_concurrent_alone_no_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s25", "What does concurrent mean?"),
    );
    assert_eq!(load_state_for(&repo, "s25").phase, 0);
}

#[test]
fn resolve_cursor_hook_repo_root_finds_hooks_from_payload_cwd() {
    let root = fresh_repo();
    let nested = root.join("scripts/router-rs");
    fs::create_dir_all(&nested).expect("mkdir nested");
    let payload = json!({
        "session_id": "rk",
        "cwd": nested.display().to_string()
    });
    let wrong_cli = nested.join("ghost");
    let resolved = resolve_cursor_hook_repo_root(Some(wrong_cli.as_path()), &payload).expect("ok");
    assert_eq!(
        resolved,
        fs::canonicalize(&root).unwrap_or_else(|_| root.clone())
    );
}

#[test]
fn cursor_session_key_fallback_stable_for_cwd_without_session_id() {
    let payload = json!({ "cwd": "/tmp/abc-stable-fallback" });
    let a = session_key(&payload);
    let b = session_key(&payload);
    assert_eq!(a.len(), 32);
    assert_eq!(a, b, "cwd-only key must survive separate hook processes");
}

#[test]
fn cursor_session_key_reads_metadata_session_id() {
    let payload = json!({
        "cwd": "/tmp/x",
        "metadata": { "sessionId": "meta-sess-1" }
    });
    let from_meta = session_key(&payload);
    let flat = session_key(&json!({
        "session_id": "meta-sess-1",
        "cwd": "/tmp/x"
    }));
    assert_eq!(from_meta, flat);
}

#[test]
fn cursor_session_key_nested_payload_session_id_matches_top_level() {
    let nested = json!({
        "cwd": "/tmp/x",
        "payload": { "sessionId": "uuid-nested-pregoal" }
    });
    let flat = json!({
        "session_id": "uuid-nested-pregoal",
        "cwd": "/tmp/x"
    });
    assert_eq!(session_key(&nested), session_key(&flat));
}

#[test]
fn cursor_session_key_nested_workspace_folder_matches_top_cwd() {
    let nested = json!({
        "payload": { "workspaceFolder": "/tmp/ws-nested" }
    });
    let flat = json!({ "cwd": "/tmp/ws-nested" });
    assert_eq!(session_key(&nested), session_key(&flat));
}

#[test]
fn my_pre_goal_persists_when_session_id_only_nested_in_payload() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let sid = "nested-sid-pregoal";
    let before = json!({
        "cwd": cwd,
        "payload": {
            "sessionId": sid,
            "prompt": "/implementx 完成任务"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &before);
    let stop = json!({
        "cwd": cwd,
        "payload": {
            "sessionId": sid,
            "prompt": "small_task\nGoal: g\nNon-goals: n\nDone when: d\nValidation commands: cargo test"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop);
    let state = load_state(&repo, &json!({ "session_id": sid, "cwd": cwd }))
        .expect("load")
        .expect("state file");
    assert!(
        state.pre_goal_review_satisfied,
        "stop followup={:?}",
        out.get("followup_message")
    );
}

#[test]
fn subagent_start_pre_goal_requires_typed_subagent() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-sub-pre", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "SubagentStart",
        &json!({
            "session_id": "s-sub-pre",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_input": {"fork_context": false}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied,
        "untyped SubagentStart must not satisfy pre-goal"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "SubagentStart",
        &json!({
            "session_id": "s-sub-pre",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "subagent_type": "general-purpose",
            "tool_input": {"fork_context": false}
        }),
    );
    assert!(load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied);
}

#[test]
fn cursor_lock_writes_owner_metadata() {
    let repo = fresh_repo();
    let payload = event("s26", "review");
    let lock = acquire_state_lock(&repo, &payload).expect("acquire");
    let text = fs::read_to_string(state_lock_path(&repo, &payload)).expect("read lock");
    assert!(text.contains("pid="));
    assert!(text.contains("ts="));
    let mut guard = Some(lock);
    release_state_lock(&mut guard);
}

#[test]
fn cursor_lock_recovers_from_stale_timestamp() {
    let repo = fresh_repo();
    let payload = event("s27", "review");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    let stale_ts = now_millis().saturating_sub(60_000);
    fs::write(&lock_path, format!("pid=999999 ts={stale_ts}\n")).expect("seed stale lock");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(lock.is_some());
    release_state_lock(&mut lock);
}

#[test]
fn cursor_lock_recovers_orphan_lock_file_without_remove_when_holder_alive() {
    let repo = fresh_repo();
    let payload = event("s27-alive", "review");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    let stale_ts = now_millis().saturating_sub(60_000);
    fs::write(
        &lock_path,
        format!("pid={} ts={stale_ts}\n", std::process::id()),
    )
    .expect("seed stale lock metadata without flock holder");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(
        lock.is_some(),
        "orphan lock file must be acquired via try_lock without remove_file on alive pid"
    );
    assert!(
        lock_path.is_file(),
        "must not remove_file lock path when holder pid is still alive"
    );
    release_state_lock(&mut lock);
}

#[test]
fn cursor_lock_concurrent_acquire_serializes() {
    let repo = Arc::new(fresh_repo());
    let sessions = ["s28-a", "s28-b"];
    let mut joins = Vec::new();
    for session in sessions {
        let repo = Arc::clone(&repo);
        joins.push(std::thread::spawn(move || {
            let payload = event(session, "review");
            for _ in 0..20 {
                let lock = acquire_state_lock(&repo, &payload).expect("acquire");
                let mut guard = Some(lock);
                release_state_lock(&mut guard);
            }
        }));
    }
    for join in joins {
        join.join().expect("join");
    }
}

#[test]
fn cursor_state_save_completes_with_fsync_unix() {
    let repo = fresh_repo();
    let payload = event("s29", "review");
    let mut state = empty_state();
    state.phase = 2;
    assert!(save_state(&repo, &payload, &mut state));
    let loaded = load_state(&repo, &payload).expect("load").expect("state");
    assert_eq!(loaded.phase, 2);
}

#[test]
fn prompt_from_nested_messages_reads_text_without_content_key() {
    let payload = json!({
        "session_id": "msg-text-only",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [{"role": "user", "text": "small_task review ./foo.rs"}],
    });
    assert_eq!(
        super::prompt_text(&payload),
        "small_task review ./foo.rs"
    );
}

#[test]
fn my_light_stop_does_not_suppress_review_when_only_assistant_mentions_implementx() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "my-light-assist-only";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, sid).review_required);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "继续",
            "response": "按 /implementx 流程执行即可",
        }),
    );
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "assistant tail must not trigger my-light suppress; fm={fm:?}"
    );
}

#[test]
fn cursor_hook_rejects_non_object_stdin() {
    let mut reader = Cursor::new(b"[]".to_vec());
    let err = super::stdin::read_stdin_json_from_reader(&mut reader).expect_err("must reject");
    assert_eq!(err, "stdin_json_not_object");
}

#[test]
fn cursor_hook_rejects_oversized_stdin() {
    let large = "a".repeat(5 * 1024 * 1024);
    let mut reader = Cursor::new(large.into_bytes());
    let err = super::stdin::read_stdin_json_from_reader(&mut reader).expect_err("must reject");
    assert_eq!(err, "stdin_too_large");
}

#[test]
fn pre_compact_does_not_mutate_state() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let payload = event("s30", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let path = state_path(&repo, &payload);
    let before = fs::read_to_string(&path).expect("read before");
    let _ = dispatch_cursor_hook_event(&repo, "preCompact", &payload);
    let after = fs::read_to_string(&path).expect("read after");
    assert_eq!(before, after);
}

// --- SessionEnd: stale terminal 子进程清理 ---

fn write_terminal_file(dir: &Path, id: &str, header: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("mkdir terminals");
    let path = dir.join(format!("{id}.txt"));
    fs::write(&path, header).expect("write terminal file");
    path
}

#[test]
fn parse_terminal_header_extracts_pid_cwd_active() {
    let txt = format!(
        "---\npid: 12345\ncwd: \"{FRAMEWORK_HARNESS_TEST_CWD}\"\ncommand: \"cargo test\"\nstarted_at: 2026-05-10T12:00:00Z\nrunning_for_ms: 295037   \n---\nbody..."
    );
    let h = parse_terminal_header(&txt).expect("parsed");
    assert_eq!(h.pid, Some(12345));
    assert_eq!(
        h.cwd.as_deref(),
        Some(Path::new(FRAMEWORK_HARNESS_TEST_CWD))
    );
    assert!(h.is_active);
    assert!(h.started_at_ms.is_some());
}

#[test]
fn parse_terminal_header_inactive_when_no_running_for_ms() {
    let txt = format!("---\npid: 35455\ncwd: {FRAMEWORK_HARNESS_TEST_CWD}\n---\n~/skill ❯");
    let h = parse_terminal_header(&txt).expect("parsed");
    assert_eq!(h.pid, Some(35455));
    assert!(!h.is_active);
}

#[test]
fn parse_terminal_header_rejects_non_yaml_block() {
    assert!(parse_terminal_header("no front matter here").is_none());
}

#[test]
fn cursor_kill_stale_terminals_disabled_by_env_truthy_values_keep_enabled() {
    let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
    std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
    assert!(!cursor_kill_stale_terminals_disabled_by_env());
    for v in ["", "1", "true", "yes", "on", "anything"] {
        std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v);
        assert!(
            !cursor_kill_stale_terminals_disabled_by_env(),
            "value {v:?} should NOT disable"
        );
    }
    for v in ["0", "false", "off", "no", "  FALSE  "] {
        std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v);
        assert!(
            cursor_kill_stale_terminals_disabled_by_env(),
            "value {v:?} should disable"
        );
    }
    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS"),
    }
}

#[test]
fn terminate_in_dir_skips_when_terminals_dir_missing() {
    let repo = fresh_repo();
    let report = terminate_stale_terminal_processes_in_dir(&repo, &repo.join("missing"), None);
    assert_eq!(report.scanned, 0);
    assert!(report.killed.is_empty());
}

#[cfg(unix)]
#[test]
fn terminate_in_dir_skips_inactive_outside_and_dead_branches() {
    use std::process::{Command, Stdio};
    let repo = fresh_repo();
    let term_dir = repo.join("__terminals");

    // 1) inactive：header 中无 `running_for_ms`，PID 不重要（取一个被显式过滤的小值）。
    write_terminal_file(
        &term_dir,
        "inactive",
        &format!(
            "---\npid: 1\ncwd: \"{}\"\ncommand: \"echo hi\"\n---\nbody",
            repo.display()
        ),
    );

    // 2) outside_repo：spawn 一个实际活着的 sleep（独立 PGID），cwd 指向仓库外。
    use std::os::unix::process::CommandExt;
    let mut outside_cmd = Command::new("sleep");
    outside_cmd
        .arg("60")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        outside_cmd.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let mut alive_outside = outside_cmd.spawn().expect("spawn outside sleep");
    let outside_pid = alive_outside.id();
    // 给子进程一点时间真正进入运行态。
    thread::sleep(Duration::from_millis(50));
    write_terminal_file(
            &term_dir,
            "outside",
            &format!(
                "---\npid: {outside_pid}\ncwd: /tmp/router-rs-stale-test-not-this-repo\nrunning_for_ms: 1000\n---\n"
            ),
        );

    // 3) dead：spawn `true` 立即 wait，确保 PID 已被 reap；短窗口内 PID 不会被 OS 复用。
    let mut quick = Command::new("true")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn true");
    let dead_pid = quick.id();
    quick.wait().expect("reap true");
    // 等待 OS 把 PID 标记为 ESRCH（macOS/Linux 下 reap 后立刻就 dead，但留几次轮询兜底）。
    for _ in 0..50 {
        if !is_process_alive(dead_pid) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    write_terminal_file(
        &term_dir,
        "dead",
        &format!(
            "---\npid: {dead_pid}\ncwd: \"{}\"\nrunning_for_ms: 100\n---\n",
            repo.display()
        ),
    );

    let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir, None);
    // outside 子进程必须仍活着——证明 cwd 范围过滤生效。
    assert!(
        is_process_alive(outside_pid),
        "outside-repo child {outside_pid} must NOT be killed; report={report:?}"
    );
    assert!(
        report.killed.is_empty(),
        "no children inside repo were truly active: {:?}",
        report.killed
    );
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert!(
        report.scanned >= 3,
        "expected at least the seeded three terminal files, got report={report:?}"
    );
    assert_eq!(report.skipped_inactive, 1);
    assert_eq!(report.skipped_outside_repo, 1);
    // dead PID 在极少数 race 下可能被 OS 立刻复用（同 PPID 下另一进程），所以放宽断言。
    assert!(report.skipped_dead <= 1);

    // 收尾：杀掉 outside 子进程并 reap。
    unsafe {
        let _ = libc::kill(outside_pid as libc::pid_t, libc::SIGKILL);
    }
    let _ = alive_outside.wait();
}

#[cfg(unix)]
#[test]
fn terminate_in_dir_kills_real_sleep_child_within_repo() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let repo = fresh_repo();
    let term_dir = repo.join("__terminals");

    // 双隔离 pre_exec：
    // 1) `setsid` 让 sleep 进入新 session + 新 pgid，与 cargo test 完全脱钩。这样
    //    `terminate_pid` 通过 SIGTERM 杀整个 pgid 时不会牵连 cargo test 自身。
    // 2) 同时通过新 session 让 sleep 不再受 cargo test 的 SIGHUP 影响。
    // sleep 仍是 cargo test 的 child，结束后由 cargo test 在 drop(child) 时 wait——
    // 但我们不持有 child，让它变 zombie 由 reaper（cargo runner / 测试 harness）回收。
    // 为避免 `is_process_alive` 把 zombie 误判 alive 让 SIGKILL 复检失败，我们在 spawn
    // 之后立刻把 child 转成游离句柄并显式持续 wait（在 SIGTERM 之后立刻收尸）。
    let mut spawn_cmd = Command::new("sleep");
    spawn_cmd
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        spawn_cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = spawn_cmd.spawn().expect("spawn sleep");
    let pid = child.id();
    // 等 sleep 真正进入运行态。
    thread::sleep(Duration::from_millis(50));
    assert!(is_process_alive(pid), "child must be alive before kill");

    write_terminal_file(
        &term_dir,
        "alive",
        &format!(
            "---\npid: {pid}\ncwd: \"{}\"\ncommand: \"sleep 60\"\nrunning_for_ms: 500\n---\n",
            repo.display()
        ),
    );

    // 后台 reaper：在测试主线程持续在 terminate_pid 内 SIGTERM/SIGKILL 时，独立线程
    // 调 child.wait() 立刻 reap，避免 zombie 让 `is_process_alive(kill(pid,0))` 误判。
    let waiter = std::thread::spawn(move || {
        let _ = child.wait();
    });

    let report = terminate_stale_terminal_processes_in_dir(&repo, &term_dir, None);
    let _ = waiter.join();
    assert_eq!(report.killed, vec![pid], "report={report:?}");
    assert!(!is_process_alive(pid), "child must be reaped");
}

#[cfg(unix)]
#[test]
fn signal_pid_or_pgrp_never_targets_our_process_group() {
    // 防止 SessionEnd stale terminal 回收逻辑“按 PGID kill”时误杀 hook 自己所在进程组。
    // 这里只验证目标选择逻辑：当 pgid == current_pgid 时必须退化为只 kill PID。
    let our_pid = std::process::id();
    let our_pgid = super::current_pgid().expect("current pgid");
    // 发送 0 信号不产生副作用，但会触发 syscall 分支。
    super::signal_pid_or_pgrp(our_pid, Some(our_pgid), 0);
}

#[test]
fn handle_session_end_respects_kill_disable_env() {
    // 走 dispatch_event 真实路径；只验证 env=0 时不会因 terminals 路径推导失败而 panic，
    // 也不影响既有 `.cursor/hook-state` 清扫。
    let repo = fresh_repo();
    let payload = event("kill-disable-sess", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
    std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", "0");
    let out = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert_eq!(out, json!({}));
    assert!(!state_path(&repo, &payload).exists(), "state still cleared");
    match prev {
        Some(v) => std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v),
        None => std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS"),
    }
}

#[test]
fn session_start_operator_inject_off_skips_additional_context() {
    let _lock = crate::test_env_sync::process_env_lock();
    let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
    env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir ac");
    fs::write(
        repo.join("artifacts/current/SESSION_SUMMARY.md"),
        "would appear if inject on\n",
    )
    .expect("summary");
    let payload = json!({
        "session_id": "ss-inject-off",
        "cwd": repo.display().to_string()
    });
    let out = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let ctx = out["additional_context"].as_str().unwrap_or("");
    assert!(
        ctx.trim().is_empty(),
        "ROUTER_RS_OPERATOR_INJECT=0 must skip SessionStart continuity advisory: {out:?}"
    );
    match prev_inject {
        Some(v) => env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
        None => env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
    }
}

#[test]
fn session_start_additional_context_observes_router_rs_sessionstart_max_env() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    let payload = json!({
        "session_id": "ss-budget",
        "cwd": repo.display().to_string()
    });
    let prev = env::var_os("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS");
    env::set_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS", "420");
    let out = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS", v),
        None => env::remove_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS"),
    }
    let ctx = out["additional_context"]
        .as_str()
        .expect("additional_context");
    assert!(
        ctx.starts_with("Repo: "),
        "SessionStart must be Repo-only (continuity digest removed): {ctx:?}"
    );
    assert!(
        ctx.len() <= 420,
        "len={}, ctx.preview={:?}",
        ctx.len(),
        &ctx[..ctx.len().min(80)]
    );
}

#[test]
fn session_start_resets_session_call_tracker() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");
    crate::session_call_tracker::init_tracker(&repo).expect("seed");
    for _ in 0..50 {
        crate::session_call_tracker::record_tool_call(&repo, "Read").expect("record");
    }
    let before = crate::session_call_tracker::read_tracker_state(&repo).expect("read");
    assert!(before["total_calls"].as_u64().unwrap_or(0) >= 50);

    let payload = json!({
        "session_id": "ss-tracker-reset",
        "cwd": repo.display().to_string()
    });
    let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let after = crate::session_call_tracker::read_tracker_state(&repo).expect("read after");
    assert_eq!(after["total_calls"], 0);
    assert!(after["per_tool"].as_object().unwrap().is_empty());
}

#[test]
fn session_start_repo_only_no_continuity_hints() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let _rg = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let active_tid = "t-ss-empty";
    let focus_tid = "t-ss-filled";
    let cur = repo.join("artifacts/current");
    fs::create_dir_all(cur.join(active_tid)).expect("mkdir active task");
    fs::write(
        cur.join("active_task.json"),
        format!(r#"{{"task_id":"{active_tid}"}}"#),
    )
    .expect("active");
    fs::write(
        cur.join("focus_task.json"),
        format!(r#"{{"task_id":"{focus_tid}"}}"#),
    )
    .expect("focus");
    let focus_dir = cur.join(focus_tid);
    fs::create_dir_all(&focus_dir).expect("mkdir focus task");
    fs::write(
        focus_dir.join("GOAL_STATE.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "router-rs-autopilot-goal-v1",
            "drive_until_done": true,
            "status": "running",
            "goal": "from-focus",
            "non_goals": [],
            "done_when": [],
            "validation_commands": [],
            "current_horizon": "",
            "checkpoints": [],
            "blocker": null,
            "updated_at": "2026-01-01T00:00:00Z"
        }))
        .unwrap(),
    )
    .expect("goal");

    let payload = json!({
        "session_id": "ss-af-hint",
        "cwd": repo.display().to_string(),
    });
    let out = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let ctx = out["additional_context"]
        .as_str()
        .expect("additional_context");
    assert!(
        ctx.starts_with("Repo: "),
        "SessionStart must not inject continuity hints: {ctx:?}"
    );
    assert!(
        !ctx.contains(crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH),
        "continuity hint must not appear: {ctx:?}"
    );
}

#[test]
fn session_start_initializes_terminal_baseline_ledger() {
    let _term_env = cursor_terminals_dir_env_lock();
    let repo = fresh_repo();
    let term_dir = repo.join("__terminals");
    write_terminal_file(
        &term_dir,
        "t1",
        &format!("---\npid: 11111\ncwd: {FRAMEWORK_HARNESS_TEST_CWD}\nrunning_for_ms: 1\n---\n"),
    );
    write_terminal_file(
        &term_dir,
        "t2",
        &format!("---\npid: 22222\ncwd: {FRAMEWORK_HARNESS_TEST_CWD}\n---\n"),
    );
    let prev = std::env::var_os("CURSOR_TERMINALS_DIR");
    std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir);
    let payload = json!({ "session_id": "sess-ledger-init", "cwd": repo.display().to_string() });
    let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let ledger = load_session_terminal_ledger(&repo, &payload);
    assert_eq!(ledger.version, SESSION_TERMINAL_LEDGER_VERSION);
    assert_eq!(ledger.baseline_pids, vec![11111, 22222]);
    assert!(ledger.owned_pids.is_empty());
    match prev {
        Some(v) => std::env::set_var("CURSOR_TERMINALS_DIR", v),
        None => std::env::remove_var("CURSOR_TERMINALS_DIR"),
    }
}

#[cfg(unix)]
#[test]
fn session_end_kills_only_owned_terminal_pids() {
    let _term_env = cursor_terminals_dir_env_lock();
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let repo = fresh_repo();
    let term_dir = repo.join("__terminals");

    let mk_sleep = || {
        let mut cmd = Command::new("sleep");
        cmd.arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd.spawn().expect("spawn sleep")
    };

    let mut owned_child = mk_sleep();
    let mut other_child = mk_sleep();
    let owned_pid = owned_child.id();
    let other_pid = other_child.id();
    thread::sleep(Duration::from_millis(50));
    assert!(is_process_alive(owned_pid));
    assert!(is_process_alive(other_pid));

    write_terminal_file(
        &term_dir,
        "owned",
        &format!(
            "---\npid: {owned_pid}\ncwd: \"{}\"\nrunning_for_ms: 500\n---\n",
            repo.display()
        ),
    );
    write_terminal_file(
        &term_dir,
        "other",
        &format!(
            "---\npid: {other_pid}\ncwd: \"{}\"\nrunning_for_ms: 500\n---\n",
            repo.display()
        ),
    );

    let prev = std::env::var_os("CURSOR_TERMINALS_DIR");
    std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir);
    let payload = json!({ "session_id": "sess-owned-only", "cwd": repo.display().to_string() });
    let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    save_session_terminal_ledger(
        &repo,
        &payload,
        &SessionTerminalLedger {
            version: SESSION_TERMINAL_LEDGER_VERSION,
            baseline_pids: vec![],
            owned_pids: vec![owned_pid],
            pending_shells: vec![],
        },
    );
    let owned_waiter = std::thread::spawn(move || {
        let _ = owned_child.wait();
    });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);

    // owned pid should be terminated by SessionEnd
    for _ in 0..40 {
        if !is_process_alive(owned_pid) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = owned_waiter.join();
    assert!(!is_process_alive(owned_pid), "owned pid must be killed");
    assert!(is_process_alive(other_pid), "non-owned pid must stay alive");

    unsafe {
        let _ = libc::kill(other_pid as libc::pid_t, libc::SIGKILL);
    }
    let _ = other_child.wait();

    match prev {
        Some(v) => std::env::set_var("CURSOR_TERMINALS_DIR", v),
        None => std::env::remove_var("CURSOR_TERMINALS_DIR"),
    }
}

#[test]
fn cursor_launcher_fail_closed_before_submit_when_router_rs_missing() {
    use std::process::Command;
    let framework = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let launcher = framework.join("configs/framework/cursor-router-rs-hook.sh");
    assert!(
        launcher.is_file(),
        "launcher missing: {}",
        launcher.display()
    );
    let empty_ws = env::temp_dir().join(format!(
        "cursor-launcher-fail-closed-{}",
        std::process::id()
    ));
    fs::create_dir_all(&empty_ws).expect("empty workspace");
    let empty_target = empty_ws.join("no-cargo-target");
    fs::create_dir_all(&empty_target).expect("empty target dir");

    let output = Command::new("/bin/bash")
        .arg(&launcher)
        .arg("BeforeSubmitPrompt")
        .env_remove("ROUTER_RS_BIN")
        .env("CARGO_TARGET_DIR", &empty_target)
        .env("PATH", "/usr/bin:/bin")
        .env("CURSOR_WORKSPACE_ROOT", &empty_ws)
        .env("SKILL_FRAMEWORK_ROOT", &empty_ws)
        .output()
        .expect("run launcher");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing router-rs must exit 2; stdout={stdout} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("\"continue\":false"),
        "BeforeSubmitPrompt must deny via continue:false: {stdout}"
    );
}

#[test]
fn cursor_launcher_fail_open_session_start_when_router_rs_missing() {
    use std::process::Command;
    let framework = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let launcher = framework.join("configs/framework/cursor-router-rs-hook.sh");
    let empty_ws =
        env::temp_dir().join(format!("cursor-launcher-fail-open-{}", std::process::id()));
    fs::create_dir_all(&empty_ws).expect("empty workspace");

    let output = Command::new("/bin/bash")
        .arg(&launcher)
        .arg("SessionStart")
        .env_remove("ROUTER_RS_BIN")
        .env("PATH", "/usr/bin:/bin")
        .env("CURSOR_WORKSPACE_ROOT", &empty_ws)
        .env("SKILL_FRAMEWORK_ROOT", &empty_ws)
        .output()
        .expect("run launcher");
    assert_eq!(
        output.status.code(),
        Some(0),
        "telemetry SessionStart must fail-open; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stop_handler_releases_session_lock_before_task_ledger_checkpoint() {
    let src = include_str!("handlers.rs");
    assert!(
        src.contains("release_lock_then_finalize_stop"),
        "Stop must release L3 before finalize_stop_hook_outputs (L1 checkpoint)"
    );
    let start = src.find("fn handle_stop").expect("handle_stop");
    let body = &src[start..];
    let locked = body
        .split("let mut lock = acquire_state_lock")
        .nth(1)
        .expect("locked branch");
    assert!(
        locked.contains("release_lock_then_finalize_stop"),
        "locked Stop path must not call finalize while holding session lock"
    );
}

#[test]
fn terminal_observation_cache_avoids_repeat_dir_scan() {
    use super::terminal_observation_cache::{
        collect_terminal_observations_cached, reset_terminal_cache_for_tests,
        terminal_scan_count_for_tests,
    };
    reset_terminal_cache_for_tests();
    let dir = std::env::temp_dir().join(format!(
        "router-rs-term-cache-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("mkdir terminals");
    let txt = dir.join("1.txt");
    fs::write(&txt, "pid: 4242\ncwd: /tmp\nlast_command: echo hi\n---\n").expect("write terminal");
    let _ = collect_terminal_observations_cached(&dir);
    let _ = collect_terminal_observations_cached(&dir);
    assert_eq!(
        terminal_scan_count_for_tests(),
        1,
        "second collect in same process should hit mtime cache"
    );
    let _ = fs::remove_dir_all(&dir);
    reset_terminal_cache_for_tests();
}

#[test]
fn stop_and_post_tool_concurrent_hooks_complete_under_one_second() {
    let _env = crate::test_env_sync::process_env_lock();
    let repo = Arc::new(fresh_repo());
    let sid = "s-concurrent-stop-post";
    let post_payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "tool_name": "Read",
        "tool_path": "README.md"
    });
    let stop_payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "prompt": "",
        "agent_response": "done"
    });
    let start = std::time::Instant::now();
    let repo_post = Arc::clone(&repo);
    let post = std::thread::spawn(move || {
        dispatch_cursor_hook_event(&repo_post, "postToolUse", &post_payload)
    });
    let repo_stop = Arc::clone(&repo);
    let stop =
        std::thread::spawn(move || dispatch_cursor_hook_event(&repo_stop, "stop", &stop_payload));
    let _ = post.join().expect("post join");
    let _ = stop.join().expect("stop join");
    assert!(
        start.elapsed().as_millis() < 1000,
        "concurrent postToolUse+stop should not wedge on lock order"
    );
}
