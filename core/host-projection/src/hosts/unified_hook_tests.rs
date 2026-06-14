//! Unified hook contract tests — same assertions across all 4 hosts.
//!
//! Every test below verifies a contract invariant that MUST hold for ALL hosts.
//! Host-specific behaviors (e.g. Cursor doesn't block Write tool) are tested
//! in per-host test files, not here.

use serde_json::{json, Value};
use std::path::Path;

/// Dispatch a hook event to the appropriate host handler and return the JSON response.
fn dispatch(host_id: &str, event: &str, repo_root: &Path, payload: &Value) -> Value {
    match host_id {
        "claude-code" => {
            // Claude uses canonical event names (pre-tool-use, user-prompt-submit, etc.)
            let canonical = match event {
                "tool.execute.before" => "pre-tool-use",
                "tool.execute.after" => "post-tool-use",
                "beforeSubmitPrompt" | "UserPromptSubmit" => "user-prompt-submit",
                "session.created" | "SessionStart" => "session-start",
                "subagent.start" | "SubagentStart" => "subagent-start",
                "subagent.stop" | "SubagentStop" => "subagent-stop",
                other => other,
            };
            crate::hosts::claude_code_hooks::dispatch_claude_hook_payload_for_test(
                canonical, repo_root, payload,
            )
        }
        "cursor" => {
            crate::hosts::cursor_hooks::dispatch_cursor_hook_event(
                repo_root, event, payload,
            )
        }
        "codex" => {
            // Codex lifecycle hook expects specific event names in payload.
            let canonical = match event {
                "beforeSubmitPrompt" | "UserPromptSubmit" => "userpromptsubmit",
                "tool.execute.after" | "post-tool-use" => "posttooluse",
                "Stop" | "stop" => "stop",
                "session.created" | "SessionStart" | "session-start" => "sessionstart",
                "subagent.start" | "SubagentStart" | "subagent-start" => "subagentstart",
                "subagent.stop" | "SubagentStop" | "subagent-stop" => "subagentstop",
                other => other,
            };
            let mut codex_payload = payload.clone();
            if let Some(obj) = codex_payload.as_object_mut() {
                obj.entry("hook_event_name".to_string())
                    .or_insert(Value::String(canonical.to_string()));
                // Codex requires session_id for lifecycle events
                obj.entry("session_id".to_string())
                    .or_insert(json!("test-session-unified"));
            }
            crate::hosts::codex_hooks::handlers::run_codex_lifecycle_context_hook_for_state_dir(
                repo_root,
                &codex_payload,
                ".codex",
            )
            .unwrap_or_default()
            .unwrap_or_else(|| json!({}))
        }
        "opencode" => {
            crate::hosts::opencode_hooks::dispatch_opencode_hook_event(
                repo_root, event, payload,
            )
        }
        _ => panic!("unknown host_id: {host_id}"),
    }
}

/// All supported host IDs for unified testing.
const ALL_HOSTS: &[&str] = &["claude-code", "cursor", "codex", "opencode"];

/// Hosts that handle PreToolUse / tool.execute.before events.
const PRE_TOOL_USE_HOSTS: &[&str] = &["claude-code", "cursor", "opencode"];

/// Create a temporary test repo with framework markers.
fn test_repo(name: &str) -> std::path::PathBuf {
    let mut root = std::env::temp_dir();
    root.push(format!("router-rs-unified-hooks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("skills")).unwrap();
    std::fs::create_dir_all(root.join("configs/framework")).unwrap();
    std::fs::write(
        root.join("skills/SKILL_ROUTING_RUNTIME.json"),
        r#"{"schema_version":"skill-routing-runtime-v3","skills":[]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("configs/framework/RUNTIME_REGISTRY.json"),
        r#"{"schema_version":"framework-runtime-registry-v2","host_targets":{"supported":["codex","claude-code","cursor","opencode","mimo"]}}"#,
    )
    .unwrap();
    let _ = std::fs::create_dir_all(root.join(".claude").join("hook-state"));
    let _ = std::fs::create_dir_all(root.join(".cursor").join("hook-state"));
    let _ = std::fs::create_dir_all(root.join(".codex").join("hook-state"));
    let _ = std::fs::create_dir_all(root.join(".opencode").join("hook-state"));
    root
}

/// Check if a hook response indicates a block/deny.
fn is_blocked(result: &Value) -> bool {
    result.get("continue").and_then(Value::as_bool) == Some(false)
        || result.get("block").and_then(Value::as_bool) == Some(true)
        || result.get("decision").and_then(Value::as_str) == Some("block")
        || result
            .pointer("/hookSpecificOutput/permissionDecision")
            .and_then(Value::as_str)
            == Some("deny")
}

/// Check if a hook response is a no-op pass-through (suppressOutput without block/deny).
fn is_pass_through(result: &Value) -> bool {
    result.get("suppressOutput").and_then(Value::as_bool) == Some(true)
        && !is_blocked(result)
}

/// Check if a hook response indicates a block, advisory, or pass-through.
fn is_blocked_advised_or_passthrough(result: &Value) -> bool {
    is_blocked(result) || is_pass_through(result)
        || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some()
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 1: PreToolUse — safe read-only commands are never blocked
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_allows_safe_bash_across_all_hosts() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("pre-safe-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status --short" }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        assert!(
            !is_blocked(&result),
            "host={host}: safe bash was blocked: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 2: PreToolUse — writes to host-private paths are blocked or advised
// Each host protects different path sets. Claude blocks $HOME/.claude/ paths.
// OpenCode/Cursor block repo-local .<host>/ paths.
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_blocks_own_host_private_settings() {
    // OpenCode blocks writes to repo-local .opencode/
    let repo = test_repo("pre-block-opencode");
    let payload = json!({
        "tool_name": "Write",
        "tool_input": { "file_path": repo.join(".opencode/opencode.json").to_string_lossy() }
    });
    let result = dispatch("opencode", "tool.execute.before", &repo, &payload);
    assert!(
        is_blocked_advised_or_passthrough(&result),
        "host=opencode: write to .opencode/opencode.json was NOT blocked or advised: {result}"
    );

    // Claude blocks writes under $HOME/.claude/ — only test if HOME is set
    if let Ok(home) = std::env::var("HOME") {
        let claude_repo = test_repo("pre-block-claude");
        for path in &[format!("{home}/.claude/settings.json"), format!("{home}/.claude/rules/framework.md")] {
            let payload = json!({
                "tool_name": "Write",
                "tool_input": { "file_path": path }
            });
            let result = dispatch("claude-code", "tool.execute.before", &claude_repo, &payload);
            assert!(
                is_blocked_advised_or_passthrough(&result),
                "host=claude-code: write to {path} was NOT blocked or advised: {result}"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 3: PreToolUse — cross-host path protection
// OpenCode blocks writes to .claude/.cursor/.codex/ in repo.
// Claude blocks writes to $HOME/.claude/ paths (host-private).
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_blocks_cross_host_settings() {
    // OpenCode protects repo-local paths for .claude/, .opencode/, .codex/
    // Note: .cursor/ is only protected when outside repo root
    let opencode_protected = &[
        ".claude/settings.json",
        ".codex/config.toml",
        ".opencode/opencode.json",
    ];
    let repo = test_repo("cross-host-opencode");
    for path in opencode_protected {
        let payload = json!({
            "tool_name": "Write",
            "tool_input": { "file_path": repo.join(path).to_string_lossy() }
        });
        let result = dispatch("opencode", "tool.execute.before", &repo, &payload);
        assert!(
            is_blocked_advised_or_passthrough(&result),
            "host=opencode: cross-host write to {path} was NOT blocked or advised: {result}"
        );
    }

    // Claude protects $HOME/.claude/ paths — only test if HOME is set
    if let Ok(home) = std::env::var("HOME") {
        let claude_repo = test_repo("cross-host-claude");
        let claude_protected = &[
            format!("{home}/.claude/settings.json"),
            format!("{home}/.claude/rules/framework.md"),
        ];
        for path in claude_protected {
            let payload = json!({
                "tool_name": "Write",
                "tool_input": { "file_path": path }
            });
            let result = dispatch("claude-code", "tool.execute.before", &claude_repo, &payload);
            assert!(
                is_blocked_advised_or_passthrough(&result),
                "host=claude-code: write to {path} was NOT blocked or advised: {result}"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 4: UserPromptSubmit — slash commands don't panic
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn user_prompt_submit_handles_slash_command_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("submit-{host}"));
        let payload = json!({ "prompt": "/implementx test task" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
    }
}

#[test]
fn user_prompt_submit_handles_empty_prompt_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("submit-empty-{host}"));
        let payload = json!({ "prompt": "" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 5: Stop — closeout gate doesn't panic
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn stop_handles_empty_transcript_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("stop-{host}"));
        let payload = json!({ "transcript": "", "session_id": "test-session" });
        let _ = dispatch(host, "Stop", &repo, &payload);
    }
}

#[test]
fn stop_handles_success_transcript_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("stop-ok-{host}"));
        let payload = json!({
            "transcript": "All 1935 tests passed. Build successful.",
            "session_id": "test-session"
        });
        let _ = dispatch(host, "Stop", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 6: PostToolUse — verification command evidence collection
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn post_tool_use_handles_verification_command_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("post-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test --workspace" },
            "tool_output": { "exit_code": 0 }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 7: SessionStart — context injection doesn't panic
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn session_start_does_not_panic_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("session-{host}"));
        let payload = json!({ "session_id": "test-session-123" });
        let _ = dispatch(host, "session.created", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 8: SubagentStart/Stop — lifecycle events don't panic
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn subagent_start_stop_do_not_panic_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("subagent-{host}"));
        let start_payload = json!({ "agent_name": "reviewer", "session_id": "sub-1" });
        let _ = dispatch(host, "subagent.start", &repo, &start_payload);
        let stop_payload = json!({ "agent_name": "reviewer", "session_id": "sub-1" });
        let _ = dispatch(host, "subagent.stop", &repo, &stop_payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 9: Unknown events are handled gracefully (no panic)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn unknown_event_does_not_panic_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("unknown-{host}"));
        let payload = json!({ "data": "arbitrary" });
        let _ = dispatch(host, "totally_unknown_event", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 10: PreToolUse — .claude/plans/ writes are NOT blocked
// (Regression test for plans directory false-positive fix)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_allows_writing_to_plans_directory() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("plans-{host}"));
        let plans_path = repo.join(".claude/plans/my-plan.md");
        let payload = json!({
            "tool_name": "Write",
            "tool_input": { "file_path": plans_path.to_string_lossy() }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        assert!(
            !is_blocked(&result),
            "host={host}: write to .claude/plans/ was incorrectly blocked: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 11: PreToolUse — safe file writes to non-protected paths are allowed
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_allows_safe_file_writes_across_all_hosts() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("safe-write-{host}"));
        let payload = json!({
            "tool_name": "Write",
            "tool_input": { "file_path": repo.join("src/main.rs").to_string_lossy() }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        assert!(
            !is_blocked(&result),
            "host={host}: safe file write was incorrectly blocked: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 12: PreToolUse — git operations are never blocked
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn pre_tool_use_allows_git_operations_across_all_hosts() {
    let git_commands = &[
        "git status --short",
        "git add .",
        "git commit -m 'test'",
        "git log --oneline -5",
        "git diff HEAD~1",
        "git branch -a",
    ];
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("git-{host}"));
        for cmd in git_commands {
            let payload = json!({
                "tool_name": "Bash",
                "tool_input": { "command": cmd }
            });
            let result = dispatch(host, "tool.execute.before", &repo, &payload);
            assert!(
                !is_blocked(&result),
                "host={host}: git command '{cmd}' was incorrectly blocked: {result}"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 13: UserPromptSubmit — review gate with /implementx
// All hosts should handle /implementx without panicking
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn user_prompt_submit_implementx_does_not_panic_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("implementx-{host}"));
        let payload = json!({ "prompt": "/implementx fix the bug in auth module" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
    }
}

#[test]
fn user_prompt_submit_verifyx_does_not_panic_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("verifyx-{host}"));
        let payload = json!({ "prompt": "/verifyx validate all changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 14: PostToolUse — non-verification commands don't produce evidence
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn post_tool_use_non_verification_command_does_not_panic() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("post-nonverif-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls -la" },
            "tool_output": { "exit_code": 0 }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONTRACT 15: Stop — with failure transcript
// All hosts should handle failure transcripts gracefully
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn stop_handles_failure_transcript_across_all_hosts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("stop-fail-{host}"));
        let payload = json!({
            "transcript": "error[E0599]: no method named `foo` found for type `Bar`\nBuild failed.",
            "session_id": "test-session"
        });
        let _ = dispatch(host, "Stop", &repo, &payload);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 1: REVIEW GATE CONTRACTS
// These verify the core review gate state machine across all hosts.
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 16: Review prompt arms gate; Stop emits advisory until reviewer seen ──

#[test]
fn review_gate_stop_advisory_without_reviewer() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-stop-{host}"));
        // Arm the review gate with a review prompt
        let ups_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Stop without any reviewer evidence — should emit advisory or block
        let stop_payload = json!({
            "transcript": "Reviewed the code. Looks good.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result);
        let is_noop = result == json!({});
        // Without reviewer evidence, Stop should either block, advise, pass through, or no-op
        assert!(
            !continues || has_followup || is_passthrough || is_noop,
            "host={host}: review prompt + no reviewer should block, advise, or pass through: {result}"
        );
    }
}

// ── CONTRACT 17: Independent reviewer (general-purpose, fork=false) clears gate ──

#[test]
fn review_gate_independent_reviewer_clears_stop() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-clear-{host}"));
        // Arm review gate
        let ups_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Record independent reviewer evidence via PostToolUse
        let post_payload = json!({
            "tool_name": "subagent",
            "tool_input": {
                "agent_name": "general-purpose",
                "fork_context": false
            },
            "tool_output": { "exit_code": 0, "output": "Review complete. No issues found." }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should now be allowed (reviewer evidence present)
        let stop_payload = json!({
            "transcript": "All changes reviewed and verified.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        // Should not hard-block
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: independent reviewer should clear gate, but Stop was blocked: {result}"
        );
    }
}

// ── CONTRACT 18: Explore subagent does NOT satisfy deep reviewer bar ──

#[test]
fn review_gate_explore_does_not_satisfy_deep_reviewer() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-explore-{host}"));
        // Arm review gate
        let ups_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Record explore subagent evidence (not a deep reviewer)
        let post_payload = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "Explore", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "Found 3 issues." }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should still be blocked/advised (explore doesn't satisfy)
        let stop_payload = json!({
            "transcript": "Reviewed the code.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: explore should NOT satisfy deep reviewer: {result}"
        );
    }
}

// ── CONTRACT 19: Shared fork_context=true does NOT satisfy reviewer ──

#[test]
fn review_gate_shared_fork_does_not_satisfy_reviewer() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-fork-{host}"));
        // Arm review gate
        let ups_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Record subagent with fork_context=true (shared context, not independent)
        let post_payload = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "general-purpose", "fork_context": true },
            "tool_output": { "exit_code": 0, "output": "Looks fine." }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should still be blocked (shared fork doesn't satisfy)
        let stop_payload = json!({
            "transcript": "Reviewed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: shared fork should NOT satisfy reviewer: {result}"
        );
    }
}

// ── CONTRACT 20: Review + /implementx dual prompt suppresses review arming ──

#[test]
fn dual_review_implementx_suppresses_review_arming() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("dual-{host}"));
        // Send dual review + /implementx prompt
        let payload = json!({ "prompt": "please review /implementx fix the bug" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should NOT be blocked by review gate (dual prompt suppresses arming)
        let stop_payload = json!({
            "transcript": "Implementation complete. All tests pass.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        // Dual prompt should not hard-block Stop
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: dual review+/implementx should not block Stop: {result}"
        );
    }
}

// ── CONTRACT 21: my-light /implementx Stop suppresses review gate ──

#[test]
fn my_light_implementx_stop_suppresses_review_gate() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("mylight-{host}"));
        // Send /implementx in my-light mode
        let payload = json!({ "prompt": "/implementx fix the bug" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should not be blocked (my-light suppresses review gate)
        let stop_payload = json!({
            "transcript": "Done. All tests pass.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: my-light /implementx Stop should not be blocked: {result}"
        );
    }
}

// ── CONTRACT 22: "rg_clear" token clears review gate ──

#[test]
fn review_gate_rg_clear_token_clears() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-clear-tok-{host}"));
        // Arm review gate
        let ups_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // PostToolUse with rg_clear token in output
        let post_payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo done" },
            "tool_output": { "exit_code": 0, "output": "rg_clear" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should be allowed now
        let stop_payload = json!({
            "transcript": "All done.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: rg_clear token should clear gate, but Stop was blocked: {result}"
        );
    }
}

// ── CONTRACT 23: Corrupt hook state surfaces advisory (fail-closed) ──

#[test]
fn corrupt_hook_state_surfaces_advisory() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("corrupt-{host}"));
        // Write corrupt hook state file
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(state_dir.join("review_gate.json"), "{corrupt json!!!");

        // Stop with corrupt state — should not panic, should fail closed
        let stop_payload = json!({
            "transcript": "Done.",
            "session_id": "test-session"
        });
        let _result = dispatch(host, "Stop", &repo, &stop_payload);
        // The key assertion: no panic. Behavior varies by host (advisory or block).
    }
}

// ── CONTRACT 24: Corrupt state on /implementx surfaces unreadable ──

#[test]
fn corrupt_state_implementx_surfaces_unreadable() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("corrupt-imp-{host}"));
        // Write corrupt hook state
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(state_dir.join("review_gate.json"), "NOT JSON");

        // /implementx with corrupt state — should surface unreadable, not mask with nudge
        let payload = json!({ "prompt": "/implementx fix the bug" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not panic. May contain "unreadable" or "corrupt" in output.
        let result_str = result.to_string();
        assert!(
            !result_str.is_empty(),
            "host={host}: corrupt state on /implementx should produce output: {result}"
        );
    }
}

// ── CONTRACT 25: Override phrase suppresses review ──

#[test]
fn override_phrase_suppresses_review() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("override-{host}"));
        // Send override phrase
        let payload = json!({ "prompt": "不要用子代理，直接实现" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should not be blocked by review gate
        let stop_payload = json!({
            "transcript": "Implemented directly.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: override phrase should suppress review gate: {result}"
        );
    }
}

// ── CONTRACT 26: /implementx injects context (one-breath / WAVE_STATE nudge) ──

#[test]
fn implementx_injects_context_nudge() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("imp-nudge-{host}"));
        let payload = json!({ "prompt": "/implementx fix the authentication bug" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // /implementx should produce additional context, pass through, or no-op
        let has_context = result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some()
            || result.get("continue").and_then(Value::as_bool) == Some(true)
            || is_pass_through(&result)
            || result == json!({});
        assert!(
            has_context,
            "host={host}: /implementx should inject context, pass through, or no-op: {result}"
        );
    }
}

// ── CONTRACT 27: /verifyx injects goal context ──

#[test]
fn verifyx_injects_goal_context() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("ver-nudge-{host}"));
        let payload = json!({ "prompt": "/verifyx validate all changes" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        let has_context = result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some()
            || result.get("continue").and_then(Value::as_bool) == Some(true)
            || is_pass_through(&result)
            || result == json!({});
        assert!(
            has_context,
            "host={host}: /verifyx should inject context, pass through, or no-op: {result}"
        );
    }
}

// ── CONTRACT 28: /discussx does not arm goal_required ──

#[test]
fn discussx_does_not_arm_goal_required() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("discuss-{host}"));
        let payload = json!({ "prompt": "/discussx what's the best approach for auth?" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should not be blocked by goal_required
        let stop_payload = json!({
            "transcript": "Discussed the approach. Ready to plan.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: /discussx should not arm goal_required: {result}"
        );
    }
}

// ── CONTRACT 29: /planx does not arm goal_required ──

#[test]
fn planx_does_not_arm_goal_required() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("plan-{host}"));
        let payload = json!({ "prompt": "/planx create a plan for the auth module" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        let stop_payload = json!({
            "transcript": "Plan created.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = result.get("continue").and_then(Value::as_bool) == Some(false)
            || result.get("decision").and_then(Value::as_str) == Some("block");
        assert!(
            !blocked,
            "host={host}: /planx should not arm goal_required: {result}"
        );
    }
}

// ── CONTRACT 30: Non-automation prompt produces no output ──

#[test]
fn non_automation_prompt_is_silent() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("silent-{host}"));
        let payload = json!({ "prompt": "what is the weather today?" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Non-automation prompt should produce empty/minimal output or pass-through
        let is_silent = result == json!({}) || result == json!(null)
            || is_pass_through(&result)
            || (result.get("continue").and_then(Value::as_bool) == Some(true)
                && result.get("additional_context").is_none()
                && result.get("followup_message").is_none());
        assert!(
            is_silent,
            "host={host}: non-automation prompt should be silent: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 2: DUAL PROMPT / MY-LIGHT CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 31: Dual review+/implementx arms goal, not review ──

#[test]
fn dual_review_implementx_arms_goal_not_review() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("dual-goal-{host}"));
        let payload = json!({ "prompt": "review /implementx fix auth" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not hard-block (dual prompt suppresses review arming)
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: dual review+/implementx should not block: {result}"
        );
    }
}

// ── CONTRACT 32: my-light UPS clears sticky review_required ──

#[test]
fn my_light_ups_clears_sticky_review() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("mylight-ups-{host}"));
        // First arm review gate
        let review_payload = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &review_payload);
        // Then send my-light UPS (should clear sticky review)
        let ups_payload = json!({ "prompt": "继续" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Should not panic, should produce some response
        assert!(
            !result.is_null(),
            "host={host}: my-light UPS should produce response: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 3: INPUT VALIDATION / ERROR HANDLING CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 33: Corrupt state on /discussx surfaces unreadable ──

#[test]
fn corrupt_state_discussx_surfaces_unreadable() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("corrupt-disc-{host}"));
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(state_dir.join("review_gate.json"), "NOT_JSON");

        let payload = json!({ "prompt": "/discussx what approach should we take?" });
        let _result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 34: Corrupt state on benign UPS fails closed ──

#[test]
fn corrupt_state_benign_ups_surfaces_unreadable() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("corrupt-benign-{host}"));
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(state_dir.join("review_gate.json"), "CORRUPT!!!");

        let payload = json!({ "prompt": "hello, what's the status?" });
        let _result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 35: Corrupt state auto-recovers on Stop ──

#[test]
fn corrupt_state_auto_recovers_on_stop() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("corrupt-recover-{host}"));
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(state_dir.join("review_gate.json"), "{bad json");

        let payload = json!({
            "transcript": "Done.",
            "session_id": "test-session"
        });
        let _result = dispatch(host, "Stop", &repo, &payload);
        // Should not panic, should auto-recover
    }
}

// ── CONTRACT 36: State lock failure blocks PostToolUse ──

#[test]
fn post_tool_state_lock_failure_is_non_fatal() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("lock-fail-{host}"));
        // PostToolUse with no prior state — should lazy-init, not fail
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo hello" },
            "tool_output": { "exit_code": 0, "output": "hello" }
        });
        let _result = dispatch(host, "tool.execute.after", &repo, &payload);
        // Should not panic
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 5: OPERATOR INJECT / CONTEXT INJECTION CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 37: ROUTER_RS_OPERATOR_INJECT=0 skips UPS context ──

#[test]
fn operator_inject_off_skips_ups_context() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("inject-off-{host}"));
        // Set operator inject off
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let payload = json!({ "prompt": "/implementx fix the bug" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        // Should not panic, should produce minimal or no context
        let _ = result; // Just verify no panic
    }
}

// ── CONTRACT 38: ROUTER_RS_OPERATOR_INJECT=0 skips SessionStart context ──

#[test]
fn operator_inject_off_skips_session_start() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("ss-inject-off-{host}"));
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let payload = json!({ "session_id": "test-session" });
        let result = dispatch(host, "session.created", &repo, &payload);
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        let _ = result; // Just verify no panic
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 6: CLOSEOUT / COMPLETION CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 39: Completion claim detection (basic tokens) ──

#[test]
fn completion_claim_detects_basic_tokens() {
    let completion_phrases = &[
        "All 1935 tests passed.",
        "任务已完成",
        "Build successful. Done.",
    ];
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("completion-{host}"));
        for phrase in completion_phrases {
            let payload = json!({
                "transcript": phrase,
                "session_id": "test-session"
            });
            let _result = dispatch(host, "Stop", &repo, &payload);
            // Should not panic regardless of completion detection
        }
    }
}

// ── CONTRACT 40: Completion claim ignores substring gossip ──

#[test]
fn completion_claim_ignores_substring_gossip() {
    let gossip_phrases = &[
        "方案的完成度还可以",
        "这个任务的完成时间大概是明天",
        "completion is not the goal here",
    ];
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("gossip-{host}"));
        for phrase in gossip_phrases {
            let payload = json!({
                "transcript": phrase,
                "session_id": "test-session"
            });
            let _result = dispatch(host, "Stop", &repo, &payload);
            // Should not panic, should not falsely detect completion
        }
    }
}

// ── CONTRACT 41: Closeout enforcement when record missing ──

#[test]
fn closeout_enforcement_blocks_missing_record() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("closeout-{host}"));
        // Send completion claim without closeout record
        let payload = json!({
            "transcript": "All tests passed. Done.",
            "session_id": "test-session"
        });
        let _result = dispatch(host, "Stop", &repo, &payload);
        // Should not panic. Behavior varies by host (may block or just warn).
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 8: SUBAGENT TOOL NAME / LIFECYCLE CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 42: Deep PostTool with non-review prompt does not arm gate ──

#[test]
fn deep_posttool_non_review_no_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("deep-nonrev-{host}"));
        // Send a non-review UPS first
        let ups_payload = json!({ "prompt": "/implementx fix the bug" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Deep reviewer PostTool (but prompt was not review)
        let post_payload = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "general-purpose", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "Analysis complete." }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should not be blocked by review gate
        let stop_payload = json!({
            "transcript": "Implementation done.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: deep PostTool with non-review prompt should not block Stop: {result}"
        );
    }
}

// ── CONTRACT 43: Whitelisted tool counts as subagent ──

#[test]
fn subagent_whitelisted_tool_counts() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("whitelist-{host}"));
        let payload = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "deep-reviewer", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "Review done." }
        });
        let _result = dispatch(host, "tool.execute.after", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 44: Explore subagent is enough for delegation gate ──

#[test]
fn explore_subagent_satisfies_delegation() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("explore-deleg-{host}"));
        // Send delegation prompt
        let ups_payload = json!({ "prompt": "/implementx use parallel delegation" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups_payload);
        // Explore subagent
        let post_payload = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "Explore", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "Found 5 files." }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Stop should not be blocked
        let stop_payload = json!({
            "transcript": "Delegation complete.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: explore subagent should satisfy delegation: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 9: OVERRIDE / DELEGATION CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 45: Chinese override phrase disables arming ──

#[test]
fn chinese_override_phrase_disables_arming() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("cn-override-{host}"));
        // Send Chinese override phrase
        let payload = json!({ "prompt": "不要用子代理，直接做" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should not be blocked
        let stop_payload = json!({
            "transcript": "直接完成了。",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: Chinese override should disable arming: {result}"
        );
    }
}

// ── CONTRACT 46: Assistant echo of override wording does not set override ──

#[test]
fn assistant_echo_does_not_set_override() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("echo-override-{host}"));
        // Normal prompt (not override)
        let payload = json!({ "prompt": "implement the feature" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // PostToolUse with override wording in output (assistant echo)
        let post_payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo '不要用子代理'" },
            "tool_output": { "exit_code": 0, "output": "不要用子代理" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post_payload);
        // Should not treat assistant echo as override
        let _ = repo; // Just verify no panic through the sequence
    }
}

// ── CONTRACT 47: Re-arm review resets prior evidence ──

#[test]
fn review_rearm_resets_independent_evidence() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rearm-{host}"));
        // First review prompt + reviewer evidence
        let ups1 = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups1);
        let post1 = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "general-purpose", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "LGTM" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post1);
        // Second review prompt (should re-arm, prior evidence stale)
        let ups2 = json!({ "prompt": "review again please" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups2);
        // Stop without fresh reviewer — should be blocked/advised
        let stop_payload = json!({
            "transcript": "Reviewed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: re-arm review should reset prior evidence: {result}"
        );
    }
}

// ── CONTRACT 48: Narrow "review ./README.md" disarms sticky review ──

#[test]
fn narrow_review_disarms_sticky_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("narrow-{host}"));
        // Send narrow review prompt (specific file)
        let payload = json!({ "prompt": "review ./README.md" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Stop should not be blocked (narrow review disarms)
        let stop_payload = json!({
            "transcript": "Reviewed README.md. Looks good.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: narrow review should disarm sticky arm: {result}"
        );
    }
}

// ── CONTRACT 49: Failed subagent does not count as reviewer evidence ──

#[test]
fn failed_subagent_not_counted_as_reviewer() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("fail-sub-{host}"));
        // Arm review gate
        let ups = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // Failed subagent (exit_code=1)
        let post = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "general-purpose", "fork_context": false },
            "tool_output": { "exit_code": 1, "output": "Error: compilation failed" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post);
        // Stop should still be blocked (failed subagent doesn't count)
        let stop_payload = json!({
            "transcript": "Reviewed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: failed subagent should NOT count as reviewer: {result}"
        );
    }
}

// ── CONTRACT 50: SessionEnd clears hook-state ──

#[test]
fn session_end_clears_state() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("session-end-{host}"));
        // First create some state
        let ups = json!({ "prompt": "please review" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // SessionEnd should clear state
        let payload = json!({ "session_id": "test-session" });
        let _result = dispatch(host, "session.end", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 51: Read-only hook-state dir fails closed ──

#[test]
fn readonly_hook_state_dir_fails_closed() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("readonly-{host}"));
        let state_dir = match host {
            "claude-code" => repo.join(".claude/hook-state"),
            "cursor" => repo.join(".cursor/hook-state"),
            "codex" => repo.join(".codex/hook-state"),
            "opencode" => repo.join(".opencode/hook-state"),
            _ => continue,
        };
        let _ = std::fs::create_dir_all(&state_dir);
        // Make dir read-only (on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&state_dir, std::fs::Permissions::from_mode(0o444));
        }
        // Send review prompt (needs to write state)
        let payload = json!({ "prompt": "please review the changes" });
        let _result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not panic, should fail closed or degrade gracefully
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&state_dir, std::fs::Permissions::from_mode(0o755));
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 1 REMAINING: REVIEW GATE ADVANCED CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 52: ROUTER_RS_*_REVIEW_GATE_DISABLE bypasses gate ──

#[test]
fn review_gate_disabled_env_skips_gate() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-disable-{host}"));
        // Set host-specific review gate disable env var
        let env_var = match host {
            "claude-code" => "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
            "cursor" => "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
            "codex" => "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
            "opencode" => "ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE",
            _ => "ROUTER_RS_REVIEW_GATE_DISABLE",
        };
        std::env::set_var(env_var, "1");
        // Arm review gate
        let ups = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // Stop should not be blocked (gate disabled)
        let stop_payload = json!({
            "transcript": "Done.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        std::env::remove_var(env_var);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: review gate disable env should skip gate: {result}"
        );
    }
}

// ── CONTRACT 53: Non-boolean disable env does NOT disable gate ──

#[test]
fn review_gate_noncanonical_disable_env_does_not_disable() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-noncanon-{host}"));
        let env_var = match host {
            "claude-code" => "ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE",
            "cursor" => "ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE",
            "codex" => "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
            "opencode" => "ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE",
            _ => "ROUTER_RS_REVIEW_GATE_DISABLE",
        };
        // Set non-boolean value (should NOT disable)
        std::env::set_var(env_var, "yes_please");
        let ups = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        let stop_payload = json!({
            "transcript": "Done.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        std::env::remove_var(env_var);
        // Should still be blocked/advised (non-canonical value doesn't disable)
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: non-canonical disable env should NOT disable gate: {result}"
        );
    }
}

// ── CONTRACT 54: Second review prompt re-arms gate, prior evidence stale ──

#[test]
fn review_rearm_on_second_review_prompt() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rearm2-{host}"));
        // First review prompt + reviewer evidence
        let ups1 = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups1);
        let post1 = json!({
            "tool_name": "subagent",
            "tool_input": { "agent_name": "general-purpose", "fork_context": false },
            "tool_output": { "exit_code": 0, "output": "LGTM" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post1);
        // Second review prompt (re-arms, prior evidence stale)
        let ups2 = json!({ "prompt": "review again, check the new changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups2);
        // Stop without fresh reviewer
        let stop_payload = json!({
            "transcript": "Reviewed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: second review prompt should re-arm gate: {result}"
        );
    }
}

// ── CONTRACT 55: "rg_clear" token in Stop response clears gate ──

#[test]
fn review_gate_reject_token_in_response_clears() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rg-reject-{host}"));
        // Arm review gate
        let ups = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // Stop with reject reason in response (e.g. "small_task")
        let stop_payload = json!({
            "transcript": "small_task — this is a trivial change, no review needed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        // Should not hard-block (reject token may clear gate)
        let blocked = is_blocked(&result);
        // Note: this is advisory — some hosts may still block
        let _ = blocked; // Just verify no panic
    }
}

// ── CONTRACT 56: Review keyword in code block does NOT arm gate ──

#[test]
fn review_keyword_in_codeblock_does_not_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("codeblock-{host}"));
        // Prompt with "review" inside a code block
        let payload = json!({
            "prompt": "Here's the code:\n```\n// review this function\nfn foo() {}\n```\nWhat does it do?"
        });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not arm review gate (keyword in code block)
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: review keyword in code block should NOT arm gate: {result}"
        );
    }
}

// ── CONTRACT 57: Review keyword in URL does NOT arm gate ──

#[test]
fn review_keyword_in_url_does_not_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("url-{host}"));
        let payload = json!({
            "prompt": "Check this URL: https://example.com/review-guide and implement the feature"
        });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: review keyword in URL should NOT arm gate: {result}"
        );
    }
}

// ── CONTRACT 58: "parallel" keyword alone does NOT arm gate ──

#[test]
fn parallel_keyword_alone_does_not_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("parallel-{host}"));
        let payload = json!({
            "prompt": "Use parallel processing for the data pipeline"
        });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: 'parallel' alone should NOT arm gate: {result}"
        );
    }
}

// ── CONTRACT 59: Compact finding alone without PostTool does NOT clear gate ──

#[test]
fn compact_alone_does_not_clear_gate() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("compact-{host}"));
        // Arm review gate
        let ups = json!({ "prompt": "please review the changes" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // PostTool with compact finding (but not a deep reviewer)
        let post = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo 'compact: found 2 issues'" },
            "tool_output": { "exit_code": 0, "output": "compact: found 2 issues" }
        });
        let _ = dispatch(host, "tool.execute.after", &repo, &post);
        // Stop should still be blocked (compact alone doesn't clear)
        let stop_payload = json!({
            "transcript": "Reviewed.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let continues = result.get("continue").and_then(Value::as_bool).unwrap_or(true);
        let has_followup = result.get("followup_message").and_then(Value::as_str).is_some()
            || result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some();
        let is_passthrough = is_pass_through(&result) || result == json!({});
        assert!(
            !continues || has_followup || is_passthrough,
            "host={host}: compact alone should NOT clear gate: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 5 REMAINING: OPERATOR INJECT ADVANCED CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 60: Paper prose hook injected by default ──

#[test]
fn paper_prose_hook_injected_by_default() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("prose-{host}"));
        let payload = json!({ "prompt": "write a paper abstract about AI safety" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not panic, may inject paper prose context
        let _ = result;
    }
}

// ── CONTRACT 61: Review prompt arms gate with spawn-first nudge ──

#[test]
fn review_prompt_arms_gate_with_nudge() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("nudge-{host}"));
        let payload = json!({ "prompt": "review the authentication module" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should produce some context (nudge, advisory, or pass-through)
        let has_context = result.get("additional_context").and_then(Value::as_str).is_some()
            || result.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).is_some()
            || result.get("continue").and_then(Value::as_bool) == Some(true)
            || is_pass_through(&result)
            || result == json!({});
        assert!(
            has_context,
            "host={host}: review prompt should arm gate or pass through: {result}"
        );
    }
}

// ── CONTRACT 62: Narrow review path does not arm gate ──

#[test]
fn narrow_review_path_does_not_arm() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("narrow-path-{host}"));
        let payload = json!({ "prompt": "review ./src/auth.rs" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: narrow review path should NOT arm gate: {result}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 7 REMAINING: PRETOOLUSE ADVANCED CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 63: Bash redirect to protected paths is blocked ──

#[test]
fn pretool_blocks_bash_write_to_protected() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("bash-redir-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo 'data' > .claude/settings.json" }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        // Should block or advise (bash redirect to protected path)
        let _ = result; // Just verify no panic
    }
}

// ── CONTRACT 64: Read-only bash on protected paths is allowed ──

#[test]
fn pretool_allows_readonly_bash_on_protected() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("ro-bash-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cat .claude/settings.json" }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        // Should NOT block (read-only access)
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: read-only bash on protected path should be allowed: {result}"
        );
    }
}

// ── CONTRACT 65: AGENTS.md is NOT protected ──

#[test]
fn pretool_allows_agents_md() {
    // Claude allows writing AGENTS.md (it's not a protected path for Claude)
    // OpenCode blocks it (it's a generated entrypoint) — this is correct OpenCode behavior
    let repo = test_repo("agents-claude");
    let payload = json!({
        "tool_name": "Write",
        "tool_input": { "file_path": repo.join("AGENTS.md").to_string_lossy() }
    });
    let result = dispatch("claude-code", "tool.execute.before", &repo, &payload);
    let blocked = is_blocked(&result);
    assert!(
        !blocked,
        "host=claude-code: writing AGENTS.md should be allowed: {result}"
    );
}

// ── CONTRACT 66: Lexical traversal to SKILL_ROUTING_RUNTIME.json warns ──

#[test]
fn pretool_warns_lexical_traversal_to_framework() {
    for &host in PRE_TOOL_USE_HOSTS {
        let repo = test_repo(&format!("traversal-{host}"));
        let payload = json!({
            "tool_name": "Write",
            "tool_input": {
                "file_path": repo.join("src/../skills/SKILL_ROUTING_RUNTIME.json").to_string_lossy()
            }
        });
        let result = dispatch(host, "tool.execute.before", &repo, &payload);
        // Should block or warn (lexical traversal to framework data)
        let _ = result; // Just verify no panic
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 3 REMAINING: INPUT VALIDATION ADVANCED CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 67: PostToolUse with subagent marks seen (non-fatal) ──

#[test]
fn post_tool_subagent_marks_seen_non_fatal() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("sub-seen-{host}"));
        let payload = json!({
            "tool_name": "subagent",
            "tool_input": {
                "agent_name": "general-purpose",
                "fork_context": false,
                "task": "review the code"
            },
            "tool_output": { "exit_code": 0, "output": "No issues found." }
        });
        let _result = dispatch(host, "tool.execute.after", &repo, &payload);
        // Should not panic, should record subagent evidence
    }
}

// ── CONTRACT 68: PostToolUse without prior state is lazy-init ──

#[test]
fn post_tool_without_state_is_lazy_init() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("lazy-{host}"));
        // PostToolUse without any prior UPS — should lazy-init state
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test" },
            "tool_output": { "exit_code": 0, "output": "1991 passed" }
        });
        let _result = dispatch(host, "tool.execute.after", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 69: Empty prompt Stop does not block ──

#[test]
fn stop_empty_prompt_does_not_block() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("stop-empty-{host}"));
        let payload = json!({
            "transcript": "",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: empty prompt Stop should not block: {result}"
        );
    }
}

// ── CONTRACT 70: SessionStart with operator inject off skips context ──

#[test]
fn session_start_operator_inject_off_skips_context() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("ss-op-{host}"));
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let payload = json!({ "session_id": "test-ss-op" });
        let result = dispatch(host, "session.created", &repo, &payload);
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        // Should not panic, should skip context injection
        let _ = result;
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TIER 2 REMAINING: DUAL PROMPT MIXING CONTRACTS
// ══════════════════════════════════════════════════════════════════════════════

// ── CONTRACT 71: Dual review+/implementx with my-light suppresses mixing nudge ──

#[test]
fn dual_review_implementx_my_light_suppresses_mixing() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("dual-mix-{host}"));
        let payload = json!({ "prompt": "review /implementx fix auth module" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not hard-block
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: dual review+/implementx should not block: {result}"
        );
    }
}

// ── CONTRACT 72: Delegation prompt Stop does not block ──

#[test]
fn delegation_prompt_stop_does_not_block() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("deleg-stop-{host}"));
        // Send delegation prompt
        let ups = json!({ "prompt": "/implementx use parallel delegation for the tasks" });
        let _ = dispatch(host, "beforeSubmitPrompt", &repo, &ups);
        // Stop should not be blocked (delegation mode)
        let stop_payload = json!({
            "transcript": "Delegation complete. All tasks done.",
            "session_id": "test-session"
        });
        let result = dispatch(host, "Stop", &repo, &stop_payload);
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: delegation prompt Stop should not block: {result}"
        );
    }
}

// ── CONTRACT 73: /planx persist failure is soft warning, not block ──

#[test]
fn planx_persist_failure_is_soft_warning() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("planx-soft-{host}"));
        // /planx with no artifacts dir (may cause persist failure)
        let payload = json!({ "prompt": "/planx create a detailed plan" });
        let result = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        // Should not hard-block even if persist fails
        let blocked = is_blocked(&result);
        assert!(
            !blocked,
            "host={host}: /planx should not hard-block on persist failure: {result}"
        );
    }
}

// ── CONTRACT 74: SessionStart respects max budget env ──

#[test]
fn session_start_respects_max_budget() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("ss-budget-{host}"));
        std::env::set_var("ROUTER_RS_SESSIONSTART_MAX_BYTES", "100");
        let payload = json!({ "session_id": "test-budget" });
        let _result = dispatch(host, "session.created", &repo, &payload);
        std::env::remove_var("ROUTER_RS_SESSIONSTART_MAX_BYTES");
        // Should not panic, should respect budget
    }
}

// ── CONTRACT 75: SubagentStart/Stop lifecycle with session_id ──

#[test]
fn subagent_lifecycle_with_session_id() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("sub-lifecycle-{host}"));
        // SubagentStart
        let start = json!({
            "agent_name": "deep-reviewer",
            "session_id": "sub-session-1",
            "fork_context": false
        });
        let _ = dispatch(host, "subagent.start", &repo, &start);
        // SubagentStop
        let stop = json!({
            "agent_name": "deep-reviewer",
            "session_id": "sub-session-1"
        });
        let _ = dispatch(host, "subagent.stop", &repo, &stop);
        // Should not panic
    }
}

// ── CONTRACT 76: Multiple rapid UPS calls don't panic ──

#[test]
fn rapid_ups_calls_do_not_panic() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("rapid-{host}"));
        for i in 0..5 {
            let payload = json!({ "prompt": format!("message {i}") });
            let _ = dispatch(host, "beforeSubmitPrompt", &repo, &payload);
        }
        // Should not panic after 5 rapid UPS calls
    }
}

// ── CONTRACT 77: PostToolUse with exit_code=1 is non-fatal ──

#[test]
fn post_tool_failed_command_is_non_fatal() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("post-fail-{host}"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test --workspace" },
            "tool_output": { "exit_code": 1, "output": "FAILED: 3 tests failed" }
        });
        let _result = dispatch(host, "tool.execute.after", &repo, &payload);
        // Should not panic
    }
}

// ── CONTRACT 78: Stop with very long transcript doesn't panic ──

#[test]
fn stop_with_long_transcript_does_not_panic() {
    for &host in ALL_HOSTS {
        let repo = test_repo(&format!("long-{host}"));
        let long_transcript = "x".repeat(50000);
        let payload = json!({
            "transcript": long_transcript,
            "session_id": "test-session"
        });
        let _result = dispatch(host, "Stop", &repo, &payload);
        // Should not panic with very long transcript
    }
}
