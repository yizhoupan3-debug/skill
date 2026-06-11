use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) static SEQ: AtomicU64 = AtomicU64::new(0);

pub(super) fn env_lock() -> core_policy::test_env_sync::ProcessEnvLockGuard {
    core_policy::test_env_sync::process_env_lock()
}

pub(super) fn fresh_repo() -> std::path::PathBuf {
    super::ensure_test_deps();
    let dir = std::env::temp_dir().join(format!(
        "codex-lifecycle-context-test-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(dir.join(".codex/hook-state")).unwrap();
    dir
}

pub(super) fn run_gate(repo: &std::path::Path, payload: &Value) -> Result<Option<Value>, String> {
    let _g = env_lock();
    run_codex_review_subagent_gate(repo, payload)
}

pub(super) const TEST_COMPACT_FINDING: &str = "[P1] core/router-rs/src/hosts/codex_hooks/mod.rs:1 — wave-2 compact gate clear evidence line";

#[test]
fn operator_inject_off_skips_session_start_additional_context() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
    std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
    let repo = fresh_repo();
    let out =
        handlers::handle_codex_session_start(&repo, &json!({"source": "startup"}));
    assert!(
        out.is_none(),
        "advisory SessionStart must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
    );
    match prior {
        Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
        None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
    }
}

#[test]
fn operator_inject_off_skips_user_prompt_submit_additional_context() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
    std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
    let repo = fresh_repo();
    let evt = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-inject-off-ups",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let out = handlers::handle_codex_userpromptsubmit(&repo, &evt);
    assert!(
        out.is_none(),
        "advisory UserPromptSubmit must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
    );
    match prior {
        Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
        None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
    }
}

#[test]
#[serial]
fn user_prompt_submit_injects_paper_prose_hook_by_default() {
    let _g = env_lock();
    let prior_hook = std::env::var_os("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
    std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
    let repo = fresh_repo();
    let evt = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"prose-ups-default",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"SCI润色 abstract"
    });
    let out = handlers::handle_codex_userpromptsubmit(&repo, &evt);
    let ctx = out
        .as_ref()
        .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
        .unwrap_or_default();
    assert!(
        ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
        "expected prose hook in UPS context: {ctx}"
    );
    match prior_hook {
        Some(v) => std::env::set_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK", v),
        None => std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK"),
    }
}

#[test]
fn user_prompt_submit_review_emits_subagent_gate_context() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-1",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review全仓找bug"
    });
    let out = run_gate(&repo, &payload).unwrap();
    let ctx = out
        .as_ref()
        .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
        .unwrap_or_default();
    assert!(
        ctx.contains("配对审稿") || ctx.contains("fork_context"),
        "spawn-first nudge: {ctx}"
    );
    assert!(ctx.contains("fork_context=false"));
    assert!(ctx.contains("general-purpose") || ctx.contains("best-of-n-runner"));
    if !ctx.is_empty() {
        assert!(ctx.len() <= codex_additional_context_max_bytes());
    }
    let state = codex_load_state(&repo, &payload).unwrap().unwrap();
    assert_eq!(state.seq, 1);
    assert!(state.review_gate.review_required);
}

#[test]
fn user_prompt_submit_narrow_path_skips_review_arm() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-narrow",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"review ./README.md"
    });
    let out = run_gate(&repo, &payload).unwrap();
    assert!(
        out.is_none(),
        "narrow single-path review must not arm gate: {out:?}"
    );
    let armed = codex_load_state(&repo, &payload)
        .ok()
        .flatten()
        .map(|s| s.review_gate.review_required)
        .unwrap_or(false);
    assert!(!armed, "narrow prompt should not set review_required");
}

#[test]
fn user_prompt_submit_with_override_does_not_emit() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-ovr",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review全仓找bug，不要用子代理"
    });
    let out = run_gate(&repo, &payload).unwrap();
    assert!(out.is_none());
}

#[test]
#[serial]
fn additional_context_is_deduped_and_capped() {
    let duplicate = "Codex live state: one".to_string();
    let long_line = "x".repeat(codex_additional_context_max_bytes());
    let ctx = codex_compact_contexts(vec![
        duplicate.clone(),
        duplicate,
        long_line.clone(),
        long_line,
    ])
    .unwrap();
    assert!(ctx.len() <= codex_additional_context_max_bytes());
    assert_eq!(ctx.matches("Codex live state: one").count(), 1);
}

#[test]
fn session_start_compact_context_under_small_budget_without_digest() {
    let repo = fresh_repo();
    let task_id = "session-priority";
    fs::create_dir_all(repo.join("artifacts/current").join(task_id)).expect("mkdir task");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{task_id}"}}"#),
    )
    .expect("write active");
    fs::write(
        repo.join("artifacts/current").join(task_id).join("GOAL_STATE.json"),
        r#"{"goal":"keep the active goal visible before any static context","status":"running","drive_until_done":true,"done_when":["done"],"validation_commands":["cargo test -q"]}"#,
    )
    .expect("write goal");
    fs::write(
        repo.join("artifacts/current/SESSION_SUMMARY.md"),
        "very long continuity line ".repeat(80),
    )
    .expect("write summary");

    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
    let out = handle_codex_session_start(&repo, &json!({"source":"startup"}))
        .expect("session start output");
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    let ctx = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext");
    assert!(!ctx.contains("Continuity digest:"), "{ctx}");
    assert!(ctx.contains("Repo:"), "{ctx}");
    assert!(!ctx.contains("Goal: running"), "{ctx}");
    assert!(ctx.len() <= 256, "len={} ctx={ctx:?}", ctx.len());
}

#[test]
fn post_tool_use_with_subagent_marks_seen_without_explore_counting_deep_independent() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(state.review_subagent_seen);
    assert!(
        !state.review_gate.independent_reviewer_seen,
        "explore must not satisfy Codex independent deep-review bar"
    );
    assert!(state.generic_subagent_seen);
    assert!(state.review_lane_seen);
    assert!(!state.parallel_lane_seen);
    assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
}

#[test]
fn post_tool_general_purpose_fork_false_counts_deep_independent() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2gp",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2gp",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(state.review_gate.independent_reviewer_seen);
    assert!(state.review_lane_seen);
}

#[test]
fn post_tool_review_lane_fork_false_does_not_count_deep_independent() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2rev",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2rev",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"review","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(
        !state.review_gate.independent_reviewer_seen,
        "review subagent_type is Claude-only; must not satisfy Codex reviewer_lanes"
    );
}

#[test]
fn post_tool_use_without_subagent_type_marks_generic_and_untyped_label() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2b",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2b",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"prompt":"no type field"}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(state.generic_subagent_seen);
    assert!(state.review_subagent_seen);
    assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#untyped"));
    assert!(!state.review_lane_seen);
    assert!(!state.parallel_lane_seen);
}

#[test]
fn saw_subagent_codex_accepts_whitelisted_tool_without_recognized_type() {
    assert!(saw_subagent_codex(
        "Task",
        &json!({"prompt":"missing type"})
    ));
}

#[test]
fn delegation_stop_unblocks_after_worker_subagent() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-6c",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"前端后端测试并行推进"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-6c",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"worker"}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-6c",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none());
}

#[test]
fn stop_blocks_when_hook_state_corrupt() {
    let _guard = env_lock();
    std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"Stop",
        "session_id":"stop-corrupt-1",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"x"
    });
    let path = state::codex_state_path(&repo, &payload);
    fs::write(&path, b"{not json").unwrap();
    // B-3: corrupted state auto-recovers (backup .bak + reset to fresh)
    let out = handlers::handle_codex_stop(&repo, &payload);
    // Stop with no review_required proceeds normally (None = allow)
    assert!(out.is_none(), "corrupted state should auto-recover, not block: {out:?}");
    // Verify backup was created
    let bak_path = path.with_extension("json.bak");
    assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
}

#[test]
fn session_key_without_stable_identifier_is_deterministic() {
    let _g = env_lock();
    std::env::remove_var("CODEX_SESSION_ID");
    std::env::remove_var("CODEX_CONVERSATION_ID");
    std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT");
    let repo = fresh_repo();
    let event = json!({"cwd": repo.to_string_lossy()});
    let k1 = state::codex_session_key(&repo, &event);
    let k2 = state::codex_session_key(&repo, &event);
    assert_eq!(k1, k2, "fallback keys must alias the same hook-state file");
    assert_eq!(k1.len(), 32);
}

#[test]
fn codex_session_key_differs_by_payload_session_when_strict_off() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
    std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0");
    std::env::remove_var("CODEX_SESSION_ID");
    std::env::remove_var("CODEX_CONVERSATION_ID");
    let repo = fresh_repo();
    let cwd = repo.to_string_lossy().to_string();
    let k1 = state::codex_session_key(
        &repo,
        &json!({"session_id":"sess-a","cwd":cwd}),
    );
    let k2 = state::codex_session_key(
        &repo,
        &json!({"session_id":"sess-b","cwd":cwd}),
    );
    assert_ne!(k1, k2, "payload session_id must isolate hook-state when strict off");
    match prior {
        Some(v) => std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", v),
        None => std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY"),
    }
}

#[test]
fn delegation_stop_does_not_block_when_only_explore_subagent_observed() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-6b",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"前端后端测试并行推进"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-6b",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-6b",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none());
}

#[test]
#[serial]
fn additional_context_truncates_on_newline_preference_under_small_budget() {
    // codex_additional_context_max_bytes clamps to [256, 8192]; use the
    // floor so the assertions exercise the real budget rather than a
    // value that the clamp silently rewrites.
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
    let line1 = format!("{}{}", "A".repeat(24), ": L1");
    let line2 = format!("{}{}", "C".repeat(24), ": L2");
    let line3 = "B".repeat(240);
    let ctx = codex_compact_contexts(vec![format!("{line1}\n{line2}\n{line3}")]).unwrap();
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    assert!(ctx.ends_with("..."));
    assert!(
        ctx.matches('\n').count() >= 1,
        "expected multiple lines before ellipsis when budget allows: {ctx:?}"
    );
    assert!(ctx.len() <= 256);
}

#[test]
fn codex_compact_contexts_dedup_requires_exact_trim_match() {
    let a = "Repo: /path/A";
    let b = "repo: /path/B";
    let ctx = codex_compact_contexts(vec![a.to_string(), b.to_string()]).expect("ctx");
    assert!(
        ctx.contains(a),
        "distinct lines must not merge on ASCII case: {ctx:?}"
    );
    assert!(
        ctx.contains(b),
        "distinct lines must not merge on ASCII case: {ctx:?}"
    );
}

/// Multi-segment `codex_compact_contexts` join order is preserved when the
/// combined string is truncated (SessionStart budget). Complements
/// `additional_context_truncates_on_newline_preference_under_small_budget`
/// (single blob + newline preference inside one segment).
#[test]
#[serial]
fn codex_compact_contexts_preserves_join_order_under_small_budget() {
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
    let part1 = "CODEX_JOIN_ORDER_MARK_FIRST:alpha";
    let part2 = "CODEX_JOIN_ORDER_MARK_SECOND:beta";
    let part3 = format!("CODEX_JOIN_ORDER_MARK_TAIL:{}", "Z".repeat(280));
    let ctx = codex_compact_contexts(vec![part1.to_string(), part2.to_string(), part3])
        .expect("expected combined contexts");
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
    std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
    assert!(ctx.len() <= 256, "len={}", ctx.len());
    assert!(ctx.ends_with("..."));
    assert!(
        ctx.contains("CODEX_JOIN_ORDER_MARK_FIRST"),
        "first joined segment should survive truncation: {ctx:?}"
    );
    assert!(
        ctx.contains("CODEX_JOIN_ORDER_MARK_SECOND"),
        "second joined segment should appear before tail is cut: {ctx:?}"
    );
    let pos_first = ctx.find("CODEX_JOIN_ORDER_MARK_FIRST").expect("first mark");
    let pos_second = ctx
        .find("CODEX_JOIN_ORDER_MARK_SECOND")
        .expect("second mark");
    assert!(
        pos_first < pos_second,
        "join order should be preserved in truncated output: {ctx:?}"
    );
}

#[test]
fn saw_subagent_codex_accepts_subagent_type_field() {
    assert!(saw_subagent_codex(
        "Task",
        &json!({"subagent_type":"explore"})
    ));
}

#[test]
fn saw_subagent_codex_accepts_agent_type_field() {
    assert!(saw_subagent_codex(
        "Task",
        &json!({"agent_type":"ci-investigator"})
    ));
}

#[test]
fn saw_subagent_codex_accepts_native_codex_agent_types() {
    for agent_type in ["default", "explorer", "worker"] {
        assert!(
            saw_subagent_codex("functions.spawn_agent", &json!({"agent_type":agent_type})),
            "expected native Codex agent_type={agent_type} to count as a subagent"
        );
    }
}

#[test]
fn saw_subagent_codex_accepts_whitelisted_tool_even_when_type_unrecognized() {
    assert!(saw_subagent_codex(
        "Task",
        &json!({"subagent_type":"random-thing"})
    ));
}

#[test]
fn post_tool_use_without_state_is_non_fatal() {
    let repo = fresh_repo();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2c",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post)
        .unwrap()
        .expect("lazy hook-state");
    assert!(state.generic_subagent_seen);
    assert!(
        !state.review_gate.independent_reviewer_seen,
        "explore must not satisfy deep independent reviewer ledger"
    );
}

#[test]
fn post_tool_use_without_prior_state_persists_independent_deep_reviewer() {
    let _g = env_lock();
    let repo = fresh_repo();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-no-ups-deep",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review",
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().expect("state");
    assert!(state.review_gate.independent_reviewer_seen);
    assert!(
        state.review_gate.review_required,
        "deep PostTool with review prompt must arm review_required (B5 lazy bypass)"
    );
}

#[test]
fn post_tool_deep_reviewer_without_review_prompt_does_not_arm_gate() {
    let repo = fresh_repo();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-no-review-arm",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"前端后端测试并行推进",
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let state = codex_load_state(&repo, &post).unwrap().expect("state");
    assert!(state.review_gate.independent_reviewer_seen);
    assert!(!state.review_gate.review_required, "non-review PostTool must not arm review_required");
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-no-review-arm",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "Stop must not block when review_required was never armed: {out:?}"
    );
}

#[test]
fn lazy_post_tool_deep_reviewer_arms_gate_and_stop_blocks_without_compact() {
    let _g = env_lock();
    let repo = fresh_repo();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-lazy-stop-contract",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review",
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    assert!(run_gate(&repo, &post)
        .unwrap()
        .is_none());
    let loaded = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(loaded.review_gate.independent_reviewer_seen);
    assert!(loaded.review_gate.review_required, "deep PostTool must arm review_required");
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-lazy-stop-contract",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":""
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "independent reviewer evidence must clear Stop advisory: {out:?}"
    );
}

#[test]
fn post_tool_use_observes_fork_context_on_event_root() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-event-fork",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-event-fork",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "fork_context": false,
        "tool_input":{"subagent_type":"general-purpose"}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-event-fork",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "response": TEST_COMPACT_FINDING
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "event-root fork_context should satisfy independent reviewer; out={out:?}"
    );
}

#[test]
fn post_tool_use_with_invalid_state_blocks_fail_closed() {
    let _guard = env_lock();
    std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2d",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let state_path = codex_state_path(&repo, &start);
    fs::write(&state_path, "{invalid").unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2d",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore"}
    });
    // B-3: corrupted state auto-recovers; PostToolUse proceeds with fresh state
    let out = run_gate(&repo, &post).unwrap();
    // Fresh state with subagent_type=explore should trigger review gate
    // but not due to corruption block
    assert!(
        out.is_none() || out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str) != Some("block"),
        "invalid hook-state should auto-recover on PostToolUse, not block: {out:?}"
    );
    // Verify backup was created
    let bak_path = state_path.with_extension("json.bak");
    assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
}

#[test]
fn stop_without_state_blocks_when_review_prompt_without_ups_evidence() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-3",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let out = run_gate(&repo, &payload).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
}

#[test]
fn stop_without_state_does_not_block_when_no_text() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-4",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":""
    });
    let out = run_gate(&repo, &payload).unwrap();
    assert!(out.is_none());
}

#[test]
fn stop_with_review_prompt_no_subagent_blocks() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-5",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-5",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
}

#[test]
fn stop_with_review_prompt_shared_fork_subagent_blocks() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-5b",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-5b",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore","fork_context":true}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-5b",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
}

#[test]
fn stop_with_review_prompt_missing_fork_context_subagent_blocks() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-5c",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-5c",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore"}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-5c",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
}

#[test]
fn stop_with_delegation_prompt_does_not_block() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-6",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"前端后端测试并行推进"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-6",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none());
}

#[test]
fn stop_with_subagent_seen_resets_state_after_general_purpose_deep_reviewer() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-7",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-7",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-7",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "response": TEST_COMPACT_FINDING
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &stop).unwrap().unwrap();
    assert_eq!(state.seq, 0);
    assert!(!state.review_subagent_seen);
    assert!(!state.review_gate.independent_reviewer_seen);
}

#[test]
fn stop_blocks_after_posttool_without_compact_findings() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-wave2-post-only",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-wave2-post-only",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-wave2-post-only",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "independent reviewer PostTool must clear Stop advisory: {out:?}"
    );
}

#[test]
fn stop_compact_alone_without_posttool_blocks() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-wave2-compact-only",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-wave2-compact-only",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "response": TEST_COMPACT_FINDING
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("CODEX_REVIEW_GATE"),
        "compact alone must not clear without countable posttool: {out:?}"
    );
}

#[test]
fn stop_rg_clear_clears_review_gate() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-rg-clear",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-rg-clear",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"rg_clear"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none(), "rg_clear must clear codex review gate: {out:?}");
}

#[test]
fn my_light_implementx_stop_suppresses_review_gate() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-my-light",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"/implementx run waves"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let armed = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-my-light",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &armed).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-my-light",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"/implementx finish"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "my-light must suppress CODEX_REVIEW_GATE on Stop: {out:?}"
    );
}

#[test]
fn my_light_post_tool_suppress_clears_hook_state() {
    let repo = fresh_repo();
    let sid = "sm-my-light-post";
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    assert!(
        codex_load_state(&repo, &arm)
            .unwrap()
            .map(|s| s.review_gate.review_required)
            .unwrap_or(false)
    );
    let my = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"/implementx run waves"
    });
    let _ = run_gate(&repo, &my).unwrap();
    assert!(
        !codex_load_state(&repo, &my)
            .unwrap()
            .map(|s| s.review_gate.review_required)
            .unwrap_or(true),
        "my-light UPS must clear review_required"
    );
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"/implementx",
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    assert!(
        codex_load_state(&repo, &post)
            .unwrap()
            .map(|s| s.seq)
            .unwrap_or(0)
            == 0,
        "my-light PostTool (suppress) must clear hook-state"
    );
}
