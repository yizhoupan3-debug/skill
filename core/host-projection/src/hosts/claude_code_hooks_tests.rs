//! Tests for claude_code_hooks module.
//!
//! Extracted from claude_code_hooks.rs to keep file size ≤2000 lines.

use super::*;
use serde_json::json;
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn silent_for_safe_read_only_bash() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status --short" }
        });
        assert!(run_pre_tool_use(Path::new("/repo"), &payload).is_none());
    }

    #[test]
    fn claude_stdin_limited_rejects_over_size() {
        let large = vec![b'a'; 5 * 1024 * 1024];
        let mut cursor = std::io::Cursor::new(large);
        let err = read_stdio_agent_stdin_limited(&mut cursor).unwrap_err();
        assert!(err.contains("4 MiB"), "unexpected err: [{err}]");
    }

    #[test]
    fn claude_stdin_limited_rejects_invalid_utf8() {
        let mut cursor = std::io::Cursor::new(vec![0xff, 0xfe, 0xfd]);
        let err = read_stdio_agent_stdin_limited(&mut cursor).unwrap_err();
        assert_eq!(err, "stdin_invalid_utf8");
    }

    #[test]
    fn subagent_tool_accepts_dotted_subagent_segment() {
        let p = json!({"tool_name": "lane.subagent.run"});
        assert!(subagent_tool(&p));
    }

    #[test]
    fn subagent_tool_rejects_subagent_as_plain_substring() {
        let p = json!({"tool_name": "not_really_subagent_helpers"});
        assert!(!subagent_tool(&p));
    }

    #[test]
    fn stop_blocks_when_no_exit_code_present() {
        let repo = unique_test_repo("stop-text-framework");
        let payload = json!({ "session_id": "s-text", "transcript": "cargo test passed" });
        persist_touch_state(&repo, &payload, false, true, false, false);

        let output = run_stop(&repo, &payload).unwrap();

        // Advisory — untested framework emits add_context, not block_stop
        let ctx = output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("advisory should have additionalContext");
        assert!(
            ctx.contains("Framework source files were modified"),
            "unexpected advisory: {ctx}"
        );
        clear_touch_state(&repo, &payload);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn successful_framework_test_allows_stop() {
        let repo = unique_test_repo("framework-tested");
        let session = json!({ "session_id": "s-framework-ok" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-framework-ok",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --manifest-path core/router-rs/Cargo.toml claude_code_hooks"
            },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        assert!(!touch_state_path(&repo, &session).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn failed_framework_test_keeps_stop_blocked() {
        let repo = unique_test_repo("framework-test-failed");
        let session = json!({ "session_id": "s-framework-fail" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-framework-fail",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --manifest-path core/router-rs/Cargo.toml claude_code_hooks"
            },
            "exit_code": 101
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        let output = run_stop(&repo, &session).unwrap();

        // Advisory — failed test still leaves framework_tested=false;
        // run_stop returns add_context, not block_stop.
        let ctx = output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("advisory should have additionalContext");
        assert!(
            ctx.contains("Framework source files were modified"),
            "unexpected advisory: {ctx}"
        );
        clear_touch_state(&repo, &session);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn non_automation_prompt_is_silent() {
        let repo = unique_test_repo("non-automation-prompt");
        let payload = json!({ "prompt": "fix the failing test in main.rs" });
        assert!(run_user_prompt_submit(&repo, &payload).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_review_state_lock_file_created_on_write() {
        let repo = unique_test_repo("claude-flock");
        let payload = json!({ "session_id": "flock-s1", "prompt": "深度 review" });
        let _ = run_user_prompt_submit(&repo, &payload);
        let path = review_state_path(&repo, &payload);
        assert!(path.is_file());
        assert!(
            PathBuf::from(format!("{}.lock", path.display())).is_file(),
            "flock sidecar should exist after locked write"
        );
        let _ = fs::remove_dir_all(repo);
    }

    fn assert_stop_review_gate_advisory(stop: &Value) {
        assert_ne!(
            stop.get("decision").and_then(Value::as_str),
            Some("block"),
            "review gate must not hard-block Stop: {stop:?}"
        );
        let ctx = stop["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");
        assert!(
            ctx.contains("CLAUDE_REVIEW_GATE"),
            "expected advisory Stop context: {stop:?}"
        );
    }

    #[test]
    fn review_prompt_advises_stop_until_independent_reviewer_seen() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-block");
        let payload = json!({ "session_id": "s-review", "prompt": "深度 review 这个 PR" });
        let context = run_user_prompt_submit(&repo, &payload).expect("review context");
        assert!(context["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("")
            .contains("fork_context=false"));
        let stop = run_stop(&repo, &json!({ "session_id": "s-review" })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_requires_explicit_false_fork() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-shared-fork");
        let prompt = json!({ "session_id": "s-shared", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let shared = json!({
            "session_id": "s-shared",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": true}
        });
        assert!(run_post_tool_use(&repo, &shared).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": "s-shared" })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_allows_matching_independent_reviewer() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-pass");
        let prompt = json!({ "session_id": "s-pass", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let reviewer = json!({
            "session_id": "s-pass",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        assert!(run_stop(&repo, &json!({ "session_id": "s-pass" })).is_none());
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_accepts_review_lane_with_fork_false() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-review-lane");
        let prompt = json!({ "session_id": "s-review-lane", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let reviewer = json!({
            "session_id": "s-review-lane",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"subagent_type": "review", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        assert!(
            run_stop(&repo, &json!({ "session_id": "s-review-lane" })).is_none(),
            "Claude reviewer_lanes includes review; independent evidence should clear gate"
        );
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_rejects_explore_even_with_fork_false() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("review-gate-explore-reject");
        let prompt = json!({ "session_id": "s-explore", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &prompt);
        let explorer = json!({
            "session_id": "s-explore",
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &explorer).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": "s-explore" })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        let _ = fs::remove_dir_all(repo);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
    }

    #[test]
    fn review_gate_skipped_when_disable_env_set() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", "1");
        let repo = unique_test_repo("review-gate-disabled-env");
        let payload = json!({ "session_id": "s-off", "prompt": "深度 review 这个 PR" });
        assert!(
            run_user_prompt_submit(&repo, &payload).is_none(),
            "disable env must suppress UserPromptSubmit review nag"
        );
        let stop = run_stop(&repo, &json!({ "session_id": "s-off" }));
        assert!(
            stop.is_none(),
            "disable env must allow Stop without independent reviewer evidence; got {stop:?}"
        );
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_gate_still_advises_when_disable_env_is_noncanonical_token() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", "maybe");
        let repo = unique_test_repo("review-gate-disable-garbage");
        let payload = json!({ "session_id": "s-garbage", "prompt": "深度 review 这个 PR" });
        let _ = run_user_prompt_submit(&repo, &payload).expect("review nag");
        let stop = run_stop(&repo, &json!({ "session_id": "s-garbage" })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        match prev {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parse_stdio_agent_hook_stdin_trimmed_accepts_empty_and_valid_json() {
        assert_eq!(
            super::parse_stdio_agent_hook_stdin_trimmed("").unwrap(),
            json!({})
        );
        assert_eq!(
            super::parse_stdio_agent_hook_stdin_trimmed(r#"{"session_id":"x"}"#).unwrap(),
            json!({"session_id":"x"})
        );
    }

    #[test]
    fn parse_stdio_agent_hook_stdin_trimmed_rejects_invalid_json() {
        let err = super::parse_stdio_agent_hook_stdin_trimmed("not json").unwrap_err();
        assert!(
            err.starts_with("stdin_json_invalid:"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn session_key_metadata_session_id_matches_flat() {
        let repo = unique_test_repo("claude-meta-session");
        let flat = json!({"session_id": "sid-meta", "prompt": "x"});
        let nested = json!({"metadata": {"sessionId": "sid-meta"}, "prompt": "x"});
        assert_eq!(session_key(&repo, &flat), session_key(&repo, &nested));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn session_key_namespace_splits_same_repo_empty_payload() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-ns");
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-a");
        let a = session_key(&repo, &json!({}));
        std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", "lane-b");
        let b = session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_ne!(a, b, "namespace must split state for empty payload");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn session_key_repo_fallback_stable_without_id() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_ns = std::env::var_os("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE");
        let repo = unique_test_repo("claude-repo-fb");
        let k1 = session_key(&repo, &json!({}));
        let k2 = session_key(&repo, &json!({}));
        match prev_ns {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_SESSION_NAMESPACE"),
        }
        assert_eq!(k1, k2);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_repo_review_subagent_hook_state_path() {
        let repo = unique_test_repo("deny-review-gate-pretool");
        let session = json!({ "session_id": "s-deny-rg" });
        let sk = session_key(&repo, &session);
        let payload = json!({
            "tool_name": "Write",
            "file_path": format!(".claude/hook-state/review-subagent-{sk}.json")
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let reason = out["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("");
        assert!(
            reason.contains("host-private"),
            "unexpected reason: {reason}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_legacy_review_gate_hook_state_path() {
        let repo = unique_test_repo("deny-legacy-hook-state-rg");
        let session = json!({ "session_id": "s-legacy-hook-rg" });
        let sk = session_key(&repo, &session);
        let payload = json!({
            "tool_name": "Write",
            "file_path": format!(".claude/hook-state/review_gate_{sk}.json")
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_denies_legacy_flat_review_gate_path() {
        let repo = unique_test_repo("deny-legacy-review-gate");
        let sk = session_key(&repo, &json!({ "session_id": "s-legacy-rg" }));
        let payload = json!({
            "tool_name": "Edit",
            "file_path": format!(".claude/review_gate_{sk}.json")
        });
        let out = run_pre_tool_use(&repo, &payload).expect("deny");
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn load_review_gate_migrates_legacy_flat_file_to_hook_state() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let prev_canon = std::env::var_os("ROUTER_RS_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("legacy-review-gate-migrate");
        let sid = "s-legacy-load";
        let session = json!({ "session_id": sid });
        let sk = session_key(&repo, &session);
        let legacy = repo.join(format!(".claude/review_gate_{sk}.json"));
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"review_required":true,"review_override":false,"independent_reviewer_seen":false}"#,
        )
        .unwrap();
        let loaded = match load_review_gate_disk(&repo, &session) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected legacy load, got {other:?}"),
        };
        assert!(loaded.review_required);
        assert!(!loaded.independent_reviewer_seen);
        let new_path = review_state_path(&repo, &session);
        assert!(
            new_path.is_file(),
            "legacy load must migrate to hook-state: {}",
            new_path.display()
        );
        let out = run_stop(&repo, &json!({ "session_id": sid, "prompt": "继续" }))
            .expect("armed legacy state must still advise Stop until reviewer contract met");
        assert_stop_review_gate_advisory(&out);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        match prev_canon {
            Some(v) => std::env::set_var("ROUTER_RS_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_state_path_lives_under_hook_state_dir() {
        let repo = unique_test_repo("hook-state-dir");
        let session = json!({ "session_id": "s-path" });
        let path = review_state_path(&repo, &session);
        assert!(
            path.to_string_lossy().contains("/.claude/hook-state/review-subagent-"),
            "unexpected path: {}",
            path.display()
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn load_review_gate_migrates_legacy_hook_state_review_gate_file() {
        let repo = unique_test_repo("legacy-hook-state-review-gate");
        let sid = "s-legacy-hook-state";
        let session = json!({ "session_id": sid });
        let sk = session_key(&repo, &session);
        let legacy = repo.join(format!(".claude/hook-state/review_gate_{sk}.json"));
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"review_required":true,"review_override":false,"independent_reviewer_seen":false}"#,
        )
        .unwrap();
        let loaded = match load_review_gate_disk(&repo, &session) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected legacy hook-state load, got {other:?}"),
        };
        assert!(loaded.review_required);
        let new_path = review_state_path(&repo, &session);
        assert!(
            new_path.is_file(),
            "legacy hook-state review_gate must migrate to review-subagent: {}",
            new_path.display()
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_warns_lexical_traversal_to_framework_data_source() {
        let repo = unique_test_repo("lexical-fw-path");
        fs::create_dir_all(repo.join("nest")).unwrap();
        assert_eq!(
            super::repo_relative_slash_path(&repo, "nest/../../skills/SKILL_ROUTING_RUNTIME.json")
                .as_deref(),
            Some("skills/SKILL_ROUTING_RUNTIME.json")
        );
        let payload = json!({
            "tool_name": "Write",
            "file_path": "nest/../../skills/SKILL_ROUTING_RUNTIME.json"
        });
        // SKILL_ROUTING_RUNTIME.json is now warn-only (not deny)
        let out = run_pre_tool_use(&repo, &payload).expect("warn");
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        let ctx = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("SKILL_ROUTING_RUNTIME.json"), "warn should mention the file");
        assert_eq!(out["suppressOutput"], true, "warn should suppress output");
        assert!(
            out.get("decision").is_none(),
            "warn should have no permissionDecision"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn pre_tool_use_allows_lexical_traversal_to_agents_md() {
        let repo = unique_test_repo("lexical-entrypoint");
        fs::write(repo.join("AGENTS.md"), b"x").unwrap();
        let payload = json!({
            "tool_name": "Edit",
            "file_path": "a/../../AGENTS.md"
        });
        // AGENTS.md is not in GENERATED_ENTRYPOINT_PATHS_CLAUDE and intentionally
        // has no PreToolUse warn — it is a cross-host shared document (not a Claude-specific
        // generated entrypoint), so direct editing is normal and a warn would be noise.
        let out = run_pre_tool_use(&repo, &payload);
        assert!(out.is_none(), "AGENTS.md should not be denied");
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stop_blocks_when_review_gate_state_corrupt() {
        let repo = unique_test_repo("corrupt-review-gate");
        let session = json!({ "session_id": "s-corrupt-rg" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_stop(&repo, &session).expect("advisory");
        // Advisory — corrupted state is not a reason to block indefinitely
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("advisory should have additionalContext");
        assert!(
            ctx.contains("hook-state unreadable"),
            "unexpected advisory: {ctx}"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stop_blocks_when_touch_state_corrupt() {
        let repo = unique_test_repo("corrupt-touch");
        let session = json!({ "session_id": "s-corrupt-touch" });
        let path = touch_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_stop(&repo, &session).expect("advisory");
        // Advisory — corrupted state is not a reason to block indefinitely
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("advisory should have additionalContext");
        assert!(
            ctx.contains("hook-state unreadable"),
            "unexpected advisory: {ctx}"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_returns_context_when_review_gate_corrupt() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-ups");
        let session = json!({ "session_id": "s-corrupt-ups", "prompt": "深度 review 这个 PR" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        assert!(out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(CLAUDE_HOOK_STATE_UNREADABLE));
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_implementx_returns_unreadable_when_review_gate_corrupt() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-implementx");
        let session = json!({ "session_id": "s-corrupt-impl", "prompt": "/implementx" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            ctx.contains(CLAUDE_HOOK_STATE_UNREADABLE),
            "corrupt hook-state must surface unreadable; got {ctx:?}"
        );
        assert!(
            !ctx.contains("ALL waves"),
            "must not mask corrupt state with implement nudge; got {ctx:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_discussx_returns_unreadable_when_review_gate_corrupt() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("corrupt-review-discussx");
        let session = json!({ "session_id": "s-corrupt-discuss", "prompt": "/discussx" });
        let path = review_state_path(&repo, &session);
        fs::write(&path, "{not json").unwrap();
        let out = run_user_prompt_submit(&repo, &session).expect("context");
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            ctx.contains(CLAUDE_HOOK_STATE_UNREADABLE),
            "corrupt hook-state must surface unreadable before pre-exec nudge; got {ctx:?}"
        );
        assert!(
            !ctx.contains("READ-ONLY"),
            "must not mask corrupt state with pre-exec nudge; got {ctx:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn user_prompt_submit_review_and_implementx_suppresses_review_arming() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("claude-dual-review-implementx");
        let sid = "s-claude-dual";
        let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": prompt }),
        );
        let state = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected state, got {other:?}"),
        };
        assert!(
            !state.review_required,
            "goal drive must suppress review arming on Claude UPS; got {state:?}"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn canonical_command_rejects_unknown_event() {
        let err = canonical_stdio_agent_hook_command("unknown-event").unwrap_err();
        assert!(
            err.contains("Unsupported stdio agent hook command"),
            "{err}"
        );
    }

    #[test]
    fn successful_settings_validation_allows_stop() {
        let repo = unique_test_repo("settings-validated");
        let session = json!({ "session_id": "s-settings-ok" });
        persist_touch_state(&repo, &session, true, false, false, false);
        let payload = json!({
            "session_id": "s-settings-ok",
            "tool_name": "Bash",
            "tool_input": { "command": "jq empty .claude/settings.json" },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        assert!(!touch_state_path(&repo, &session).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn root_contract_tests_count_as_framework_validation() {
        let repo = unique_test_repo("framework-root-contracts");
        let session = json!({ "session_id": "s-root-contracts" });
        persist_touch_state(&repo, &session, false, true, false, false);
        let payload = json!({
            "session_id": "s-root-contracts",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test --test policy_contracts --test documentation_contracts"
            },
            "exit_code": 0
        });

        assert!(run_post_tool_use(&repo, &payload).is_none());
        assert!(run_stop(&repo, &session).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn legacy_repo_scoped_touch_state_does_not_block_new_session() {
        let repo = unique_test_repo("legacy-touch-state");
        let legacy = legacy_touch_state_path(&repo);
        fs::write(
            &legacy,
            "{\"framework\":true,\"framework_tested\":false,\"settings\":false,\"settings_validated\":false}\n",
        )
        .unwrap();

        assert!(run_stop(&repo, &json!({ "session_id": "fresh-session" })).is_none());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_payload_sent_to_claude_hook_is_ignored() {
        let repo = unique_test_repo("cursor-payload-isolation");
        let payload = json!({
            "session_id": "cursor-session",
            "hook_event_name": "postToolUse",
            "cursor_version": "3.3.30",
            "workspace_roots": [repo.to_string_lossy()],
            "transcript_path": "/Users/joe/.cursor/projects/example/session.json",
            "tool_name": "Bash",
            "tool_input": {
                "command": "apply_patch core/router-rs/src/claude_code_hooks.rs"
            },
            "file_path": "core/router-rs/src/claude_code_hooks.rs",
            "exit_code": 0
        });

        let output = dispatch_stdio_agent_hook_payload("post-tool-use", &repo, &payload);

        assert_eq!(output, silent_success());
        assert!(!legacy_touch_state_path(&repo).exists());
        assert!(!touch_state_path(&repo, &payload).exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_stop_payload_sent_to_claude_hook_does_not_block() {
        let repo = unique_test_repo("cursor-stop-isolation");
        persist_touch_state(
            &repo,
            &json!({ "session_id": "cursor-session" }),
            false,
            true,
            false,
            false,
        );
        let payload = json!({
            "session_id": "cursor-session",
            "hook_event_name": "stop",
            "cursor_version": "3.3.30",
            "workspace_roots": [repo.to_string_lossy()]
        });

        let output = dispatch_stdio_agent_hook_payload("stop", &repo, &payload);

        assert_eq!(output, silent_success());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn partial_cursor_envelope_without_hook_event_runs_claude_pre_tool() {
        let repo = unique_test_repo("forge-cursor-envelope-partial");
        let payload = json!({
            "session_id": "forge",
            "cursor_version": "9.9.9",
            "workspace_roots": [repo.to_string_lossy()],
            "tool_name": "Edit",
            "file_path": "configs/framework/RUNTIME_REGISTRY.json"
        });
        // Partial envelope without hook_event_name is not a valid Cursor envelope,
        // so it routes through Claude pre-tool-use path guard.
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert!(
            output.get("hookSpecificOutput").is_some(),
            "must not silent_success on partial envelope; got {output:?}"
        );
        assert!(
            output["hookSpecificOutput"]["permissionDecision"]
                .as_str()
                .unwrap_or("")
                .contains("deny"),
            "expected deny decision; got {output:?}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn claude_payload_with_nested_cursor_path_is_not_silenced_as_cursor_stdin() {
        let repo = unique_test_repo("claude-cursor-path-not-envelope");
        let cursor_plan = repo.join(".cursor").join("plans").join("feature.plan.md");
        fs::create_dir_all(cursor_plan.parent().unwrap()).unwrap();
        let payload = json!({
            "session_id": "claude-session",
            "tool_name": "Edit",
            "file_path": "configs/framework/RUNTIME_REGISTRY.json",
            "context": cursor_plan.to_string_lossy(),
        });
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert!(
            output.get("hookSpecificOutput").is_some(),
            "expected PreToolUse decision payload, not bare silent_success; got {output:?}"
        );
        assert_eq!(
            output["hookSpecificOutput"]["permissionDecision"],
            json!("deny")
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn cursor_version_without_workspace_roots_is_not_envelope() {
        let repo = unique_test_repo("cursor-version-only-not-envelope");
        let payload = json!({
            "session_id": "mixed",
            "cursor_version": "3.3.30",
            "tool_name": "Edit",
            "file_path": "configs/framework/RUNTIME_REGISTRY.json",
        });
        let output = dispatch_stdio_agent_hook_payload("pre-tool-use", &repo, &payload);
        assert!(
            output.get("hookSpecificOutput").is_some(),
            "expected PreToolUse decision for framework guarded path; got {output:?}"
        );
        assert!(
            output["hookSpecificOutput"]["permissionDecision"]
                .as_str()
                .unwrap_or("")
                .contains("deny"),
            "expected deny decision; got {output:?}"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn my_light_implementx_stop_suppresses_review_gate_when_review_armed() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("my-light-stop");
        let sid = "s-my-light";
        let armed = json!({ "session_id": sid, "prompt": "全面review" });
        let _ = run_user_prompt_submit(&repo, &armed);
        let stop = json!({ "session_id": sid, "prompt": "/implementx finish" });
        assert!(
            run_stop(&repo, &stop).is_none(),
            "my-light must suppress CLAUDE_REVIEW_GATE on Stop"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn my_light_user_prompt_clears_sticky_review_required() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("my-light-clear");
        let sid = "s-clear";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let armed = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected armed state, got {other:?}"),
        };
        assert!(armed.review_required);
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "/implementx run waves" }),
        );
        let cleared = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected cleared state, got {other:?}"),
        };
        assert!(
            !cleared.review_required,
            "my-light UPS must clear sticky review_required"
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn second_review_prompt_in_same_session_requires_fresh_reviewer_evidence() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("rearm-review");
        let sid = "s-rearm";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let reviewer = json!({
            "session_id": sid,
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &reviewer).is_none());
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "Please do another code review of this change." }),
        );
        let stop = run_stop(&repo, &json!({ "session_id": sid })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn narrow_path_review_disarms_sticky_deep_arm() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("narrow-disarm");
        let sid = "s-narrow";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 整个路由系统" }),
        );
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "review ./README.md" }),
        );
        let cleared = match load_review_gate_disk(&repo, &json!({ "session_id": sid })) {
            AgentDiskState::Ok(s) => s,
            other => panic!("expected state, got {other:?}"),
        };
        assert!(!cleared.review_required);
        assert!(
            run_stop(&repo, &json!({ "session_id": sid, "prompt": "review ./README.md" })).is_none()
        );
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn failed_subagent_post_tool_does_not_record_reviewer_evidence() {
        let _env = crate::hosts::test_shim::process_env_lock();
        let prev_disable = std::env::var_os("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
        let repo = unique_test_repo("failed-subagent");
        let sid = "s-fail";
        let _ = run_user_prompt_submit(
            &repo,
            &json!({ "session_id": sid, "prompt": "深度 review 这个 PR" }),
        );
        let failed = json!({
            "session_id": sid,
            "tool_name": "functions.spawn_agent",
            "exit_code": 1,
            "tool_input": {"agent_type": "general-purpose", "fork_context": false}
        });
        assert!(run_post_tool_use(&repo, &failed).is_none());
        let stop = run_stop(&repo, &json!({ "session_id": sid })).expect("stop advisory");
        assert_stop_review_gate_advisory(&stop);
        match prev_disable {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE"),
        }
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    #[serial]
    fn user_prompt_submit_injects_paper_prose_by_default() {
        let _g = core_policy::test_env_sync::process_env_lock();
        let prior = std::env::var_os("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK");
        std::env::remove_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK");
        let repo = unique_test_repo("prose-ups");
        let payload = json!({
            "prompt": "polish this abstract for clarity",
            "session_id": "claude-prose-1"
        });
        let out = run_user_prompt_submit(&repo, &payload);
        let ctx = out
            .as_ref()
            .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
            .unwrap_or_default();
        assert!(
            ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
            "expected prose hook: {ctx}"
        );
        let _ = fs::remove_dir_all(&repo);
        match prior {
            Some(v) => std::env::set_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK", v),
            None => std::env::remove_var("ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK"),
        }
    }

    #[test]
    fn is_host_private_path_exempts_claude_plans_directory() {
        // .claude/plans/ is a session scratch area (plan mode), not host-private state
        assert!(!is_host_private_path("~/.claude/plans/dynamic-baking-curry.md"));
        assert!(!is_host_private_path(".claude/plans/foo.md"));
        // Absolute path under HOME also exempt
        if let Ok(home) = std::env::var("HOME") {
            let plans_path = format!("{home}/.claude/plans/some-plan.md");
            assert!(!is_host_private_path(&plans_path));
            // But actual host-private paths under HOME must still be blocked
            let settings_path = format!("{home}/.claude/settings.json");
            assert!(is_host_private_path(&settings_path));
        }
        // Relative host-private paths must still be blocked
        assert!(is_host_private_path("~/.claude/settings.json"));
        assert!(is_host_private_path(".claude/hook-state/review_gate_123.json"));
    }

    fn unique_test_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "router-rs-claude-hooks-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".claude").join("hook-state")).unwrap();
        path
    }
}
