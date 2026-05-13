use serde_json::json;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

/// 模板/模型偶发的陈旧 Review 续跑前缀（分段拼接，避免在对外文案里复述整词）。
const LEGACY_REVIEW_FOLLOWUP_TOKEN: &str = concat!("RG", "_FOLLOWUP");

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

/// 对齐 `frag_04_review_gate_runtime.rs` 中 `review_stop_followup_line` / `REVIEW_GATE_FOLLOWUP_NEED_SEGMENT` 形态。
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

/// 对齐 `review_gate::run_review_gate` 写出 stdout 前的 `scrub_followup_fields_in_hook_output`。
/// `merge_additional_context`（`frag_03_paths_terminal_merge_lock_persist.rs`）在合并追加时对
/// `extra` 与合并后的 `additional_context` 调用同一 `scrub_spoof_host_followup_lines`。
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

/// 与 `dispatch.rs` 内 `dispatch_cursor_hook_event` **应急**分支 `match` 字面键顺序一致（`|` 拆成两行各记一次）。
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
        "cwd": "/Users/joe/Documents/skill",
        "prompt": prompt
    })
}

fn load_state_for(repo: &Path, session: &str) -> ReviewGateState {
    let payload = json!({ "session_id": session, "cwd": "/Users/joe/Documents/skill" });
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
        ac.contains("≥2") && ac.contains("fork_context=false"),
        "expected compact ≥2 + fork_context hint; got {ac:?}"
    );
    assert!(
        ac.contains("general-purpose") && ac.contains("best-of-n-runner"),
        "expected countable lane names; got {ac:?}"
    );
    assert!(
        ac.len() < 280,
        "nudge should stay a single short line; len={} body={ac:?}",
        ac.len()
    );
}

/// 未命中「并行 review 候选」三元时仍注入同一行指针；不再追加第二段「≥3」以免刷屏。
#[test]
fn before_submit_review_prompt_compact_nudge_has_no_second_breadth_paragraph() {
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
        ac.contains("skills/code-review-deep/SKILL.md") && ac.contains("≥2"),
        "expected same compact pointer; got {ac:?}"
    );
    assert!(
        !ac.contains("≥3"),
        "hook must not append a separate ≥3 breadth paragraph; got {ac:?}"
    );
}

/// 应急关闭审稿门控时，即使用户轮为 review 也不注入深度审并行提示。
#[test]
fn review_gate_disabled_before_submit_suppresses_deep_review_nudge_for_review_prompt() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let _rg = ReviewGateDisableTestGuard::new();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("rg-off-review-text", "全面review这个仓库"),
    );
    assert_eq!(out, json!({ "continue": true }));
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
fn autopilot_entry_does_not_arm_delegation_or_review_from_fix_copy() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(
            "ap-del",
            "/autopilot address all review findings from the last pass",
        ),
    );
    let state = load_state_for(&repo, "ap-del");
    assert!(
        !state.delegation_required,
        "autopilot must not stack delegation_required (was: framework_entrypoint)"
    );
    assert!(
        !state.review_required,
        "autopilot execution turn must not re-arm review from findings wording"
    );
    assert!(state.goal_required);
}

#[test]
fn before_submit_review_and_autopilot_same_prompt_merges_mixing_hint() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "s-dual-review-autopilot-hint";
    let prompt = "请全面review这个仓库 /autopilot 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("router-rs：本轮提交同时包含") && ac.contains("REVIEW_GATE"),
        "expected dual-signal mixing nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "same-submit autopilot must suppress review arming; got {state:?}"
    );
    assert!(state.goal_required);
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
            "prompt": "/autopilot do thing"
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
            "prompt": "/autopilot do thing"
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
fn autopilot_skips_pre_goal_nag_when_goal_state_on_disk() {
    let _env = crate::test_env_sync::process_env_lock();
    use std::env;
    let prev_strict = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");

    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gt1")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gt1"}"#,
    )
    .expect("active");
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
        &event("ap-disk", "/autopilot 继续实现"),
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
        !msg.contains("Autopilot (/autopilot)") && !msg.contains("independent-context"),
        "pre-goal nag should be skipped when GOAL_STATE exists; msg={msg:?}"
    );

    match prev_strict {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v),
        None => env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK"),
    }
}

#[test]
fn autopilot_pre_goal_strict_disk_skips_hydrate_pre_goal_on_before_submit() {
    let _env = crate::test_env_sync::process_env_lock();
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
        &event("ap-disk-strict", "/autopilot 继续实现"),
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
        &event("ev-gate", "/autopilot finish fixes"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ev-gate",
            "cwd": "/Users/joe/Documents/skill",
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
        &event("run-gate", "/autopilot continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "run-gate",
            "cwd": "/Users/joe/Documents/skill",
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
        &event("ns-gate", "/autopilot continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ns-gate",
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
            "payload": { "response": "small_task" }
        }),
    );
    assert!(load_state_for(&repo, "s13nest-a").reject_reason_seen);
}

#[test]
fn emergency_review_gate_disable_cold_after_agent_response_persists_reject_reason_seen() {
    let _env_clear = ReviewGateDisableEnvClearGuard::new();
    let _rg_disable = ReviewGateDisableTestGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "s-cold-ara",
            "cwd": "/Users/joe/Documents/skill",
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
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
    assert!(blob.contains("锁不可用"), "{blob}");
    assert!(
        blob.contains("AUTOPILOT_DRIVE"),
        "lock failure should keep active drive visible; blob={blob}"
    );
}

#[test]
fn continuity_followup_with_existing_hard_message_uses_additional_context_and_dedupes() {
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
    let frame = crate::task_state::resolve_cursor_continuity_frame(&repo);
    let hard_gate_followup = format!(
        "router-rs REVIEW_GATE incomplete phase=0 {} {}",
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let mut out = json!({
        "followup_message": hard_gate_followup.clone(),
        "additional_context": "AUTOPILOT_DRIVE: stale\nGoal: old"
    });

    merge_continuity_followups(&repo, &mut out, &frame);
    merge_continuity_followups(&repo, &mut out, &frame);

    assert_eq!(
        out["followup_message"].as_str(),
        Some(hard_gate_followup.as_str())
    );
    let ctx = out["additional_context"].as_str().unwrap_or("");
    assert!(ctx.contains("AUTOPILOT_DRIVE"), "{ctx}");
    assert!(ctx.contains("drive while hard message exists"), "{ctx}");
    assert!(!ctx.contains("Goal: old"), "{ctx}");
    assert_eq!(ctx.matches("AUTOPILOT_DRIVE").count(), 1, "{ctx}");
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
    assert!(!msg.contains("AUTOPILOT_DRIVE"), "{msg}");
    assert!(!msg.contains("RFV_LOOP_CONTINUE"), "{msg}");
    assert!(!msg.contains("## 续跑"), "{msg}");
}

#[test]
fn stop_active_goal_continuity_uses_additional_context_by_default() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/default-ac")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"default-ac"}"#,
    )
    .expect("active_task");
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
    assert!(ctx.contains("AUTOPILOT_DRIVE"), "{ctx}");
    assert!(ctx.contains("default additional context drive"), "{ctx}");
    assert!(
        ctx.contains("SESSION_CLOSE_STYLE"),
        "soft terminal nudge merges with autopilot continuity: {ctx}"
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
        "cwd": "/Users/joe/Documents/skill",
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
    crate::autopilot_goal::framework_autopilot_goal(json!({
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
    assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");

    apply_cursor_hook_output_policy(&mut out);
    let preserved = hook_user_visible_blob(&out);
    assert!(preserved.contains("AUTOPILOT_DRIVE"), "{preserved}");
}

#[test]
fn stop_goal_and_rfv_emit_dual_continuity_followups() {
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
    assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");
    // Goal+RFV 同时活跃时合并为单段 `AUTOPILOT_DRIVE`，RFV 信息压缩为尾注（见 `merge_continuity_followups`）。
    assert!(blob.contains("RFV") || blob.contains("rfv-line"), "{blob}");
    assert!(
        blob.matches("AUTOPILOT_DRIVE").count() >= 1,
        "expected AUTOPILOT_DRIVE marker in merged continuity blob: {blob}"
    );
}

/// Goal+RFV 合并为单段 `AUTOPILOT_DRIVE` 时须保留结构化外研 schema 指针行（出站前缀裁剪下更易存活）。
#[test]
fn stop_goal_and_rfv_merge_preserves_external_struct_schema_hint_line() {
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
    assert!(blob.contains("AUTOPILOT_DRIVE"), "{blob}");
    assert!(
        blob.contains(crate::rfv_loop::RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH),
        "merged AUTOPILOT_DRIVE should retain external struct schema pointer: {blob}"
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
fn review_gate_disabled_post_tool_use_still_advances_phase_after_arm() {
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
                "cwd": "/Users/joe/Documents/skill",
                "tool_name": "functions.subagent",
                "tool_input": { "subagent_type": "explore", "fork_context": false }
            }),
        )
    };

    assert_eq!(out, json!({}));
    let state = load_state_for(&repo, "srg-pu2");
    assert!(state.phase >= 2, "phase={}", state.phase);
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
fn review_gate_stop_softens_after_max_nudges_env_cap() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("2");
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
        !fm3.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "full need= should leave followup_message after cap; fm3={fm3:?}"
    );
    assert!(
        blob3.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "full line should remain visible via additional_context; blob3={blob3:?}"
    );
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
}

/// 无 subagent id 时 cycle key 均为同一 `lane:`；两次并行 start 压入两条 multiset 记录，需**两次** stop 才清门。
#[test]
fn review_gate_parallel_lane_only_keys_two_stops_clear_gate() {
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
fn pre_compact_emits_additional_context_summary() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s8", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "preCompact",
        &json!({ "session_id": "s8", "cwd": "/Users/joe/Documents/skill" }),
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
        &event("s17", "/autopilot 完成任务"),
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
        &event("s-msg-goal", "/autopilot finish wiring"),
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
        "cwd": "/Users/joe/Documents/skill",
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
fn autopilot_before_submit_prompts_pre_goal_review_when_opt_in() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
    env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "1");
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17b", "/autopilot 完成任务"),
    );
    let msg = hook_user_visible_blob(&out);
    assert!(msg.contains("Autopilot (/autopilot)"), "surface={msg:?}");
    match prev {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
        None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
    }
}

#[test]
fn autopilot_pre_goal_auto_releases_when_nag_cap_reached() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES");
    let prev_pg = env::var_os("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED");
    env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", "1");
    env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES", "2");
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("cap-nag", "/autopilot smoke"),
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
    match prev_pg {
        Some(v) => env::set_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED", v),
        None => env::remove_var("ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED"),
    }
}

#[test]
fn deep_json_strings_satisfy_pre_goal_reject_on_before_submit() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("deep-s1", "/autopilot 任务"),
    );
    let deep = json!({
        "session_id": "deep-s1",
        "cwd": "/Users/joe/Documents/skill",
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
        "cwd": "/Users/joe/Documents/skill",
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
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17e", "/autopilot 第一轮"),
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
        !msg.contains("Autopilot (/autopilot)") && !msg.contains("independent-context reviewer"),
        "reject_reason on submit should skip pre-goal nag; msg={msg:?}"
    );
}

#[test]
fn nested_payload_prompt_reject_reason_satisfies_pre_goal_before_submit() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17nest", "/autopilot 第一轮"),
    );
    let nested = json!({
        "session_id": "s17nest",
        "cwd": "/Users/joe/Documents/skill",
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
        &event("s17stop-n", "/autopilot 任务"),
    );
    let nested_stop = json!({
        "session_id": "s17stop-n",
        "cwd": "/Users/joe/Documents/skill",
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
        &event("s17c", "/autopilot 完成任务"),
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
        &event("s17d", "/autopilot 完成任务"),
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
        &event("s17mcp", "/autopilot 完成任务"),
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
        &event("s17nest-tu", "/autopilot 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17nest-tu",
            "cwd": "/Users/joe/Documents/skill",
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
        &event("s-lane", "/autopilot 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-lane",
            "cwd": "/Users/joe/Documents/skill",
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
        &event("s-fkstr", "/autopilot 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-fkstr",
            "cwd": "/Users/joe/Documents/skill",
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
fn autopilot_pre_goal_persists_when_session_id_only_nested_in_payload() {
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let sid = "nested-sid-pregoal";
    let before = json!({
        "cwd": cwd,
        "payload": {
            "sessionId": sid,
            "prompt": "/autopilot 完成任务"
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
        &event("s-sub-pre", "/autopilot 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "SubagentStart",
        &json!({
            "session_id": "s-sub-pre",
            "cwd": "/Users/joe/Documents/skill",
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
            "cwd": "/Users/joe/Documents/skill",
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
fn cursor_lock_concurrent_acquire_serializes() {
    let repo = Arc::new(fresh_repo());
    let payload = event("s28-shared", "review");
    let mut joins = Vec::new();
    for _ in 0..2 {
        let repo = Arc::clone(&repo);
        let payload = payload.clone();
        joins.push(std::thread::spawn(move || {
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
fn cursor_hook_rejects_oversized_stdin() {
    let large = "a".repeat(5 * 1024 * 1024);
    let mut reader = Cursor::new(large.into_bytes());
    let err = super::stdin::read_stdin_json_from_reader(&mut reader).expect_err("must reject");
    assert_eq!(err, "stdin_too_large");
}

#[test]
fn pre_compact_does_not_mutate_state() {
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
    let txt = "---\npid: 12345\ncwd: \"/Users/joe/Documents/skill\"\ncommand: \"cargo test\"\nstarted_at: 2026-05-10T12:00:00Z\nrunning_for_ms: 295037   \n---\nbody...";
    let h = parse_terminal_header(txt).expect("parsed");
    assert_eq!(h.pid, Some(12345));
    assert_eq!(
        h.cwd.as_deref(),
        Some(Path::new("/Users/joe/Documents/skill"))
    );
    assert!(h.is_active);
    assert!(h.started_at_ms.is_some());
}

#[test]
fn parse_terminal_header_inactive_when_no_running_for_ms() {
    let txt = "---\npid: 35455\ncwd: /Users/joe/Documents/skill\n---\n~/skill ❯";
    let h = parse_terminal_header(txt).expect("parsed");
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
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir ac");
    fs::write(
        repo.join("artifacts/current/SESSION_SUMMARY.md"),
        "SESSIONSTART_BUDGET_LINE\n".repeat(400),
    )
    .expect("summary");
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
        ctx.len() <= 420,
        "len={}, ctx.preview={:?}",
        ctx.len(),
        &ctx[..ctx.len().min(80)]
    );
    assert!(
        ctx.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX),
        "expected UTF-8 byte cap truncation with fixed suffix: {ctx:?}"
    );
}

#[test]
fn session_start_prepends_active_focus_goal_mismatch_hint() {
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
        ctx.starts_with("连续性提示:"),
        "hint must lead sections for SessionStart prefix truncation: {ctx:?}"
    );
    assert!(
        ctx.contains(crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH),
        "full zh hint constant must appear: {ctx:?}"
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
        "---\npid: 11111\ncwd: /Users/joe/Documents/skill\nrunning_for_ms: 1\n---\n",
    );
    write_terminal_file(
        &term_dir,
        "t2",
        "---\npid: 22222\ncwd: /Users/joe/Documents/skill\n---\n",
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
