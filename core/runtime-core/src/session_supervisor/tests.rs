use super::driver::*;
use super::handle_session_supervisor_operation;
use super::runtime::*;
use super::types::*;
use super::worker::*;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_state_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("router-rs-{name}-{nonce}.json"))
    }

    #[test]
    fn codex_resume_command_uses_resume_subcommand() {
        let command = build_driver_command(
            "codex",
            "/tmp/project",
            None,
            None,
            "last",
            true,
            false,
            None,
            None,
        )
        .expect("build codex resume command");
        assert_eq!(command.driver_id, "codex_driver");
        assert_eq!(command.binary, "codex");
        assert!(command.args.starts_with(&[
            "-C".to_string(),
            "/tmp/project".to_string(),
            "resume".to_string()
        ]));
        assert!(command.args.contains(&"--last".to_string()));
    }

    #[test]
    fn dry_run_launch_and_resume_round_trip_persists_state() {
        let state_path = temp_state_path("session-supervisor");
        let now = "2026-04-23T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "prompt": "继续处理 backlog",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch worker");
        let worker_id = launch["worker"]["worker_id"]
            .as_str()
            .expect("worker_id")
            .to_string();
        assert_eq!(launch["worker"]["status"], json!("queued"));
        assert_eq!(
            launch["worker"]["metadata"]["lane_contract"]["goal"],
            json!("继续处理 backlog")
        );
        assert_eq!(
            launch["worker"]["metadata"]["lane_contract"]["lane_goal"],
            json!("继续处理 backlog")
        );
        assert_eq!(
            launch["worker"]["metadata"]["lane_contract"]["verification_required"],
            json!(true)
        );
        assert_eq!(
            launch["worker"]["metadata"]["lane_contract"]["final_digest"],
            Value::Null
        );
        assert_eq!(
            launch["worker"]["metadata"]["lane_contract"]["expected_output"]["changed_files"],
            json!([])
        );

        let marked = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "evidence_text": "429 Too Many Requests. Please try again in 5 minutes.",
            "now": now,
        }))
        .expect("mark blocked");
        assert_eq!(marked["worker"]["status"], json!("blocked_rate_limit"));

        let resumed = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": "2026-04-23T10:06:00Z",
        }))
        .expect("resume due");
        let resumed_workers = resumed["resumed_workers"]
            .as_array()
            .expect("resumed workers");
        assert_eq!(resumed_workers.len(), 1);
        assert_eq!(resumed_workers[0]["action"], json!("dry_run"));

        let listed = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-04-23T10:06:00Z",
        }))
        .expect("list workers");
        assert_eq!(listed["workers"][0]["driver_id"], json!("codex_driver"));

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn launch_merges_empty_lane_contract_with_required_defaults() {
        let state_path = temp_state_path("session-supervisor-lane-contract");
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "worker_id": "lane-empty-contract",
            "host": "codex",
            "cwd": "/tmp/project",
            "prompt": "bounded review",
            "lane_contract": {},
            "dry_run": true,
            "now": "2026-04-23T10:00:00Z",
        }))
        .expect("launch worker");
        let contract = &launch["worker"]["metadata"]["lane_contract"];
        assert_eq!(contract["lane_goal"], json!("bounded review"));
        assert_eq!(contract["bounded_scope"], json!("/tmp/project"));
        assert_eq!(
            contract["forbidden_scope"],
            json!("outside assigned lane-local scope")
        );
        assert_eq!(contract["verification_required"], json!(true));
        assert_eq!(contract["final_digest"], Value::Null);
        assert_eq!(contract["integration_status"], json!("planned"));
        assert!(contract["expected_output"]["verification"].is_array());
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn concurrent_dry_run_launches_do_not_clobber_store() {
        let state_path = temp_state_path("session-supervisor-concurrent");
        let mut handles = Vec::new();
        for idx in 0..6 {
            let state_path = state_path.clone();
            handles.push(std::thread::spawn(move || {
                handle_session_supervisor_operation(json!({
                    "operation": "launch",
                    "state_path": state_path,
                    "worker_id": format!("worker-{idx}"),
                    "host": "codex",
                    "cwd": "/tmp/project",
                    "prompt": format!("lane {idx}"),
                    "dry_run": true,
                    "now": "2026-04-23T10:00:00Z",
                }))
                .expect("launch worker");
            }));
        }
        for handle in handles {
            handle.join().expect("thread join");
        }

        let listed = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-04-23T10:00:00Z",
        }))
        .expect("list workers");
        let worker_ids = listed["workers"]
            .as_array()
            .expect("workers")
            .iter()
            .map(|worker| worker["worker_id"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(worker_ids.len(), 6, "workers={worker_ids:?}");
        for idx in 0..6 {
            assert!(worker_ids.contains(&format!("worker-{idx}")));
        }
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn resolve_worktree_cwd_defaults_to_cwd() {
        assert_eq!(resolve_worktree_cwd("/repo", None, None), "/repo");
    }

    #[test]
    fn resolve_worktree_cwd_with_safe_name() {
        assert_eq!(
            resolve_worktree_cwd("/repo", Some("my-feature"), None),
            "/repo/.claude/worktrees/my-feature"
        );
    }

    #[test]
    fn resolve_worktree_cwd_rejects_traversal_in_name() {
        // name with ../ should be rejected, falling back to cwd
        assert_eq!(
            resolve_worktree_cwd("/repo", Some("../etc"), None),
            "/repo"
        );
        assert_eq!(
            resolve_worktree_cwd("/repo", Some(".."), None),
            "/repo"
        );
        assert_eq!(
            resolve_worktree_cwd("/repo", Some("a/b"), None),
            "/repo"
        );
    }

    #[test]
    fn resolve_worktree_cwd_with_explicit_path() {
        assert_eq!(
            resolve_worktree_cwd("/repo", None, Some("/wt/my-branch")),
            "/wt/my-branch"
        );
    }

    #[test]
    fn resolve_worktree_cwd_rejects_traversal_in_path() {
        assert_eq!(
            resolve_worktree_cwd("/repo", None, Some("/repo/../etc/passwd")),
            "/repo"
        );
    }

    #[test]
    fn resolve_worktree_cwd_relative_path_resolves_against_cwd() {
        assert_eq!(
            resolve_worktree_cwd("/repo", None, Some("worktrees/foo")),
            "/repo/worktrees/foo"
        );
    }

    #[test]
    fn resolve_worktree_cwd_path_overrides_name() {
        // worktree_path takes priority over worktree_name
        assert_eq!(
            resolve_worktree_cwd("/repo", Some("branch-name"), Some("/explicit/path")),
            "/explicit/path"
        );
    }

    #[test]
    fn claude_host_builds_print_command() {
        let command = build_driver_command(
            "claude-code",
            "/tmp/project",
            Some("hello world".to_string()),
            None,
            "last",
            false,
            false,
            None,
            None,
        )
        .expect("build claude command");
        assert_eq!(command.driver_id, "claude_code_driver");
        assert_eq!(command.binary, "claude");
        assert!(command.args.contains(&"--print".to_string()));
        assert!(command.args.contains(&"-p".to_string()));
        assert!(command.args.contains(&"hello world".to_string()));
        assert!(command.supports_resume);
    }

    #[test]
    fn claude_host_resume_command() {
        let command = build_driver_command(
            "claude",
            "/tmp/project",
            None,
            Some("session-123".to_string()),
            "specific",
            true,
            false,
            None,
            None,
        )
        .expect("build claude resume command");
        assert_eq!(command.driver_id, "claude_code_driver");
        assert!(command.args.contains(&"--resume".to_string()));
        assert!(command.args.contains(&"session-123".to_string()));
    }

    #[test]
    fn claude_host_with_worktree_uses_effective_cwd() {
        let command = build_driver_command(
            "claude",
            "/repo",
            Some("test".to_string()),
            None,
            "last",
            false,
            false,
            Some("my-branch".to_string()),
            None,
        )
        .expect("build claude with worktree");
        // claude host does not embed cwd in args (unlike codex -C),
        // but the command should build successfully with worktree
        assert_eq!(command.driver_id, "claude_code_driver");
        assert!(command.args.contains(&"--print".to_string()));
        assert!(command.args.contains(&"test".to_string()));
    }

    #[test]
    fn is_safe_worktree_slug_accepts_valid_names() {
        assert!(is_safe_worktree_slug("my-feature"));
        assert!(is_safe_worktree_slug("fix_bug_123"));
        assert!(is_safe_worktree_slug("v2"));
    }

    #[test]
    fn is_safe_worktree_slug_rejects_unsafe_names() {
        assert!(!is_safe_worktree_slug(""));           // empty
        assert!(!is_safe_worktree_slug("../etc"));     // traversal
        assert!(!is_safe_worktree_slug("a/b"));        // slash
        assert!(!is_safe_worktree_slug("a b"));        // space
        assert!(!is_safe_worktree_slug(&"x".repeat(129))); // too long
    }

    // ── classify_rate_limit_block ──

    #[test]
    fn classify_codex_rate_limit_pattern() {
        let result = classify_rate_limit_block("codex", "Error: rate limit exceeded, try again later")
            .expect("should classify");
        assert_eq!(result.host, "codex");
        assert_eq!(result.status, "blocked_rate_limit");
        assert_eq!(result.blocked_reason, "rate_limit");
        assert!(result.matched_text.is_some());
    }

    #[test]
    fn classify_codex_429_pattern() {
        let result = classify_rate_limit_block("codex", "HTTP 429 Too Many Requests")
            .expect("should classify 429");
        assert_eq!(result.host, "codex");
        assert_eq!(result.status, "blocked_rate_limit");
    }

    #[test]
    fn classify_codex_too_many_requests_pattern() {
        let result = classify_rate_limit_block("codex", "too many requests, please wait")
            .expect("should classify too many requests");
        assert_eq!(result.status, "blocked_rate_limit");
    }

    #[test]
    fn classify_codex_overloaded_pattern() {
        let result = classify_rate_limit_block("codex", "The server is overloaded right now")
            .expect("should classify overloaded");
        assert_eq!(result.status, "blocked_rate_limit");
    }

    #[test]
    fn classify_codex_try_again_pattern() {
        let result = classify_rate_limit_block("codex", "please try again in a few moments")
            .expect("should classify try again");
        assert_eq!(result.status, "blocked_rate_limit");
    }

    #[test]
    fn classify_codex_parses_duration_seconds() {
        let result = classify_rate_limit_block("codex", "rate limit exceeded. Please try again in 30 seconds.")
            .expect("should classify");
        assert_eq!(result.backoff_seconds, 30);
    }

    #[test]
    fn classify_codex_parses_duration_minutes() {
        let result = classify_rate_limit_block("codex", "429 error. Try again in 5 minutes.")
            .expect("should classify");
        assert_eq!(result.backoff_seconds, 300);
    }

    #[test]
    fn classify_codex_parses_duration_hours() {
        let result = classify_rate_limit_block("codex", "rate limit exceeded. Retry in 2 hours.")
            .expect("should classify");
        assert_eq!(result.backoff_seconds, 7200);
    }

    #[test]
    fn classify_codex_defaults_backoff_when_no_duration() {
        let result = classify_rate_limit_block("codex", "rate limit exceeded")
            .expect("should classify");
        assert_eq!(result.backoff_seconds, DEFAULT_BACKOFF_SECONDS);
    }

    #[test]
    fn classify_unsupported_host_returns_error() {
        let err = classify_rate_limit_block("unsupported-host", "rate limit exceeded")
            .expect_err("should reject unsupported host");
        assert!(err.contains("Unsupported"), "error: {err}");
    }

    #[test]
    fn classify_no_matching_pattern_returns_error() {
        let err = classify_rate_limit_block("codex", "everything is fine, no issues here")
            .expect_err("should not classify benign text");
        assert!(err.contains("Could not classify"), "error: {err}");
    }

    // ── handle_session_supervisor_operation error paths ──

    #[test]
    fn unsupported_operation_returns_error() {
        let state_path = temp_state_path("session-supervisor-unsup-op");
        let err = handle_session_supervisor_operation(json!({
            "operation": "nonexistent_op",
            "state_path": state_path,
        }))
        .expect_err("should reject unsupported operation");
        assert!(err.contains("Unsupported"), "error: {err}");
    }

    #[test]
    fn missing_operation_returns_error() {
        let state_path = temp_state_path("session-supervisor-missing-op");
        let err = handle_session_supervisor_operation(json!({
            "state_path": state_path,
        }))
        .expect_err("should reject missing operation");
        assert!(err.contains("operation"), "error: {err}");
    }

    #[test]
    fn empty_operation_returns_error() {
        let state_path = temp_state_path("session-supervisor-empty-op");
        let err = handle_session_supervisor_operation(json!({
            "operation": "",
            "state_path": state_path,
        }))
        .expect_err("should reject empty operation");
        assert!(err.contains("operation"), "error: {err}");
    }

    #[test]
    fn inspect_unknown_worker_returns_error() {
        let state_path = temp_state_path("session-supervisor-inspect-unknown");
        // Write an empty store so the file exists
        let _ = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-06-06T10:00:00Z",
        }));
        let err = handle_session_supervisor_operation(json!({
            "operation": "inspect",
            "state_path": state_path,
            "worker_id": "does-not-exist",
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject unknown worker_id");
        assert!(err.contains("Unknown supervisor worker_id"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn terminate_unknown_worker_returns_error() {
        let state_path = temp_state_path("session-supervisor-term-unknown");
        let _ = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-06-06T10:00:00Z",
        }));
        let err = handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": "ghost-worker",
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject unknown worker_id");
        assert!(err.contains("Unknown supervisor worker_id"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn mark_blocked_unknown_worker_returns_error() {
        let state_path = temp_state_path("session-supervisor-mark-unknown");
        let _ = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-06-06T10:00:00Z",
        }));
        let err = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": "no-such-worker",
            "blocked_reason": "rate_limit",
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject unknown worker_id");
        assert!(err.contains("Unknown supervisor worker_id"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    // ── terminate_worker (dry_run through public API) ──

    #[test]
    fn terminate_dry_run_marks_interrupted() {
        let state_path = temp_state_path("session-supervisor-term-dryrun");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "term-test-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        let result = handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": worker_id,
            "dry_run": true,
            "now": now,
        }))
        .expect("terminate dry_run");

        assert_eq!(result["worker"]["status"], json!("interrupted"));
        assert_eq!(result["terminated"], json!(true));
        assert_eq!(result["dry_run"], json!(true));

        // Verify event was recorded
        let events = result["worker"]["events"].as_array().unwrap();
        assert!(events.iter().any(|e| e["event"] == "terminate_planned"));

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn terminate_non_dry_run_without_tmux_session_marks_interrupted() {
        let state_path = temp_state_path("session-supervisor-term-nodryrun");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "term-real-worker",
            "dry_run": true,  // create a queued worker without real tmux
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        // Now terminate without dry_run. The worker has no real tmux session,
        // so it should still mark as interrupted (tmux_session_exists returns false).
        let result = handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": worker_id,
            "dry_run": false,
            "now": now,
        }))
        .expect("terminate");

        assert_eq!(result["worker"]["status"], json!("interrupted"));
        let events = result["worker"]["events"].as_array().unwrap();
        assert!(events.iter().any(|e| e["event"] == "terminated"));

        let _ = fs::remove_file(state_path);
    }

    // ── mark_worker_blocked (through public API, more paths) ──

    #[test]
    fn mark_blocked_with_explicit_reason_and_backoff() {
        let state_path = temp_state_path("session-supervisor-mark-explicit");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "mark-explicit-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        let result = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "blocked_reason": "api_quota",
            "backoff_seconds": 120,
            "now": now,
        }))
        .expect("mark blocked");

        assert_eq!(result["worker"]["status"], json!("blocked_rate_limit"));
        assert_eq!(result["worker"]["blocked_reason"], json!("api_quota"));
        assert!(result["worker"]["next_resume_at"].as_str().is_some());

        let classification = &result["classification"];
        assert_eq!(classification["blocked_reason"], json!("api_quota"));
        assert_eq!(classification["backoff_seconds"], json!(120));

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn mark_blocked_with_evidence_classifies_automatically() {
        let state_path = temp_state_path("session-supervisor-mark-evidence");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "mark-evidence-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        let result = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "evidence_text": "Error: Too many requests. Please try again in 10 minutes.",
            "now": now,
        }))
        .expect("mark blocked with evidence");

        assert_eq!(result["worker"]["status"], json!("blocked_rate_limit"));
        let classification = &result["classification"];
        assert_eq!(classification["backoff_seconds"], json!(600)); // 10 minutes
        assert!(classification["matched_text"].as_str().is_some());

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn mark_blocked_with_unclassifiable_evidence_returns_error() {
        let state_path = temp_state_path("session-supervisor-mark-unclassifiable");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "mark-unclassifiable",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        let err = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "evidence_text": "everything is working perfectly fine",
            "now": now,
        }))
        .expect_err("should fail to classify benign text");

        assert!(err.contains("Could not classify"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    // ── resume_due edge cases ──

    #[test]
    fn resume_due_empty_store_returns_no_workers() {
        let state_path = temp_state_path("session-supervisor-resume-empty");
        // Ensure the store exists by listing first
        let _ = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": state_path,
            "now": "2026-06-06T10:00:00Z",
        }));

        let result = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect("resume_due");

        let resumed = result["resumed_workers"].as_array().unwrap();
        let failed = result["failed_workers"].as_array().unwrap();
        assert_eq!(resumed.len(), 0);
        assert_eq!(failed.len(), 0);

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn resume_due_skips_worker_not_yet_due() {
        let state_path = temp_state_path("session-supervisor-resume-not-due");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "not-due-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        // Mark blocked with 10-minute backoff
        handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "blocked_reason": "rate_limit",
            "backoff_seconds": 600,
            "now": now,
        }))
        .expect("mark blocked");

        // Resume immediately (not yet due)
        let result = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": now,  // same time, 10 min hasn't passed
        }))
        .expect("resume_due");

        let resumed = result["resumed_workers"].as_array().unwrap();
        assert_eq!(resumed.len(), 0, "should not resume before backoff expires");

        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn resume_due_resumes_after_backoff_expires() {
        let state_path = temp_state_path("session-supervisor-resume-after");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "will-resume-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let worker_id = launch["worker"]["worker_id"].as_str().unwrap();

        handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": worker_id,
            "blocked_reason": "rate_limit",
            "backoff_seconds": 60,
            "now": now,
        }))
        .expect("mark blocked");

        // Resume after 2 minutes (backoff was 60 seconds)
        let result = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": "2026-06-06T10:02:00Z",
        }))
        .expect("resume_due");

        let resumed = result["resumed_workers"].as_array().unwrap();
        assert_eq!(resumed.len(), 1, "should resume after backoff");
        assert_eq!(resumed[0]["worker_id"], json!(worker_id));
        assert_eq!(resumed[0]["action"], json!("dry_run"));

        let _ = fs::remove_file(state_path);
    }

    // ── resume_worker without resume_command (through state manipulation) ──

    #[test]
    fn resume_due_worker_without_resume_command_fails() {
        let state_path = temp_state_path("session-supervisor-resume-no-cmd");
        let now = "2026-06-06T10:00:00Z";

        // Directly construct a store with a worker that has no resume_command
        let store = SessionSupervisorStore {
            schema_version: SESSION_SUPERVISOR_STORE_SCHEMA_VERSION.to_string(),
            version: 1,
            workers: vec![WorkerSessionRecord {
                worker_id: "no-resume-cmd-worker".to_string(),
                host: "codex".to_string(),
                driver_id: "codex_driver".to_string(),
                cwd: "/tmp/project".to_string(),
                worktree_path: None,
                status: "blocked_rate_limit".to_string(),
                tmux_session: Some("test-session".to_string()),
                tmux_pane: None,
                attached_session_id: None,
                resume_target: None,
                resume_mode: Some("last".to_string()),
                blocked_reason: Some("rate_limit".to_string()),
                next_resume_at: Some("2026-06-06T10:00:00Z".to_string()),
                retry_policy: json!({"kind": "rate_limit_auto_resume"}),
                prompt: None,
                launch_command: DriverCommandSpec {
                    driver_id: "codex_driver".to_string(),
                    binary: "codex".to_string(),
                    args: vec![],
                    shell_command: "codex".to_string(),
                    supports_resume: true,
                    supports_native_tmux: false,
                    supports_external_tmux: true,
                },
                resume_command: None,  // no resume command
                native_tmux_requested: false,
                last_error: None,
                created_at: now.to_string(),
                updated_at: now.to_string(),
                metadata: json!({}),
                events: vec![],
            }],
        };

        save_store(&state_path, &store).expect("save store");

        let result = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": false,
            "now": "2026-06-06T10:05:00Z",
        }))
        .expect("resume_due should succeed but with failed workers");

        let failed = result["failed_workers"].as_array().unwrap();
        assert_eq!(failed.len(), 1, "worker without resume_command should fail");
        assert!(failed[0]["error"].as_str().unwrap().contains("no resume command"));

        let _ = fs::remove_file(state_path);
    }

    // ── full lifecycle: launch -> mark_blocked -> resume_due -> terminate ──

    #[test]
    fn full_lifecycle_dry_run_round_trip() {
        let state_path = temp_state_path("session-supervisor-full-lifecycle");
        let now = "2026-06-06T10:00:00Z";

        // 1. Launch
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "lifecycle-worker",
            "prompt": "fix tests",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        assert_eq!(launch["worker"]["status"], json!("queued"));
        let wid = launch["worker"]["worker_id"].as_str().unwrap().to_string();

        // 2. Mark blocked
        let blocked = handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": wid,
            "evidence_text": "rate limit hit, retry in 2 minutes",
            "now": now,
        }))
        .expect("mark_blocked");
        assert_eq!(blocked["worker"]["status"], json!("blocked_rate_limit"));

        // 3. Resume (not yet due)
        let resume_early = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": "2026-06-06T10:01:00Z",
        }))
        .expect("resume early");
        assert_eq!(resume_early["resumed_workers"].as_array().unwrap().len(), 0);

        // 4. Resume (after backoff)
        let resume_later = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": "2026-06-06T10:03:00Z",
        }))
        .expect("resume later");
        let resumed = resume_later["resumed_workers"].as_array().unwrap();
        assert_eq!(resumed.len(), 1);
        assert_eq!(resumed[0]["action"], json!("dry_run"));

        // 5. Terminate
        let terminated = handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": wid,
            "dry_run": true,
            "now": "2026-06-06T10:04:00Z",
        }))
        .expect("terminate");
        assert_eq!(terminated["worker"]["status"], json!("interrupted"));
        assert_eq!(terminated["terminated"], json!(true));

        let _ = fs::remove_file(state_path);
    }

    // ── launch host validation ──

    #[test]
    fn launch_unsupported_host_returns_error() {
        let state_path = temp_state_path("session-supervisor-launch-unsupported");
        let err = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "unsupported-ai",
            "cwd": "/tmp/project",
            "dry_run": true,
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject unsupported host");
        assert!(err.contains("Unsupported"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn launch_missing_host_returns_error() {
        let state_path = temp_state_path("session-supervisor-launch-no-host");
        let err = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "cwd": "/tmp/project",
            "dry_run": true,
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject missing host");
        assert!(err.contains("host"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    #[test]
    fn launch_missing_cwd_returns_error() {
        let state_path = temp_state_path("session-supervisor-launch-no-cwd");
        let err = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "dry_run": true,
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect_err("should reject missing cwd");
        assert!(err.contains("cwd"), "error: {err}");
        let _ = fs::remove_file(state_path);
    }

    // ── shell_escape and sanitize_segment ──

    #[test]
    fn shell_escape_simple_strings_pass_through() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("/usr/bin/cmd"), "/usr/bin/cmd");
        assert_eq!(shell_escape("arg=value"), "arg=value");
    }

    #[test]
    fn shell_escape_special_chars_are_quoted() {
        let escaped = shell_escape("hello world");
        assert_eq!(escaped, "'hello world'");
        let escaped = shell_escape("it's");
        assert_eq!(escaped, "'it'\"'\"'s'");
    }

    #[test]
    fn sanitize_segment_converts_to_slug() {
        assert_eq!(sanitize_segment("My Worker ID"), "my-worker-id");
        assert_eq!(sanitize_segment("--hello--"), "hello");
        assert_eq!(sanitize_segment("a__b"), "a-b");
    }

    #[test]
    fn sanitize_segment_empty_becomes_worker() {
        assert_eq!(sanitize_segment("???"), "worker");
        assert_eq!(sanitize_segment(""), "worker");
    }

    // ── resolve_state_path validation ──

    #[test]
    fn resolve_state_path_rejects_path_outside_cwd_and_temp() {
        let err = handle_session_supervisor_operation(json!({
            "operation": "list",
            "state_path": "/etc/passwd",
        }))
        .expect_err("should reject path outside cwd and temp");
        assert!(err.contains("must be under cwd"), "error: {err}");
    }

    // ── lane_contract merge with nested expected_output ──

    #[test]
    fn lane_contract_merges_nested_expected_output() {
        let state_path = temp_state_path("session-supervisor-lane-nested");
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "worker_id": "nested-lane",
            "host": "codex",
            "cwd": "/tmp/project",
            "prompt": "test",
            "lane_contract": {
                "expected_output": {
                    "changed_files": ["foo.rs"],
                    "risk": "low"
                },
                "custom_key": "custom_value"
            },
            "dry_run": true,
            "now": "2026-06-06T10:00:00Z",
        }))
        .expect("launch");

        let contract = &launch["worker"]["metadata"]["lane_contract"];
        // Provided values are merged in
        assert_eq!(contract["expected_output"]["changed_files"], json!(["foo.rs"]));
        assert_eq!(contract["expected_output"]["risk"], json!("low"));
        assert_eq!(contract["custom_key"], json!("custom_value"));
        // Defaults still present for other expected_output fields
        assert!(contract["expected_output"]["evidence"].is_array());
        assert!(contract["expected_output"]["verification"].is_array());
        assert_eq!(contract["expected_output"]["next_action"], Value::Null);

        let _ = fs::remove_file(state_path);
    }

    // ── event history accumulates ──

    #[test]
    fn events_accumulate_through_lifecycle() {
        let state_path = temp_state_path("session-supervisor-events");
        let now = "2026-06-06T10:00:00Z";
        let launch = handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "host": "codex",
            "cwd": "/tmp/project",
            "worker_id": "event-worker",
            "dry_run": true,
            "now": now,
        }))
        .expect("launch");
        let wid = launch["worker"]["worker_id"].as_str().unwrap().to_string();

        handle_session_supervisor_operation(json!({
            "operation": "mark_blocked",
            "state_path": state_path,
            "worker_id": wid,
            "blocked_reason": "rate_limit",
            "backoff_seconds": 60,
            "now": now,
        }))
        .expect("mark blocked");

        let result = handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": wid,
            "dry_run": true,
            "now": now,
        }))
        .expect("terminate");

        let events = result["worker"]["events"].as_array().unwrap();
        let event_names: Vec<&str> = events.iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert!(event_names.contains(&"launch_planned"));
        assert!(event_names.contains(&"blocked"));
        assert!(event_names.contains(&"terminate_planned"));
        assert_eq!(events.len(), 3);

        let _ = fs::remove_file(state_path);
    }

    // ── resume_scheduled status also triggers resume_due ──

    #[test]
    fn resume_scheduled_status_is_ready_for_resume() {
        let state_path = temp_state_path("session-supervisor-resume-scheduled");
        let now = "2026-06-06T10:00:00Z";
        let store = SessionSupervisorStore {
            schema_version: SESSION_SUPERVISOR_STORE_SCHEMA_VERSION.to_string(),
            version: 1,
            workers: vec![WorkerSessionRecord {
                worker_id: "scheduled-worker".to_string(),
                host: "codex".to_string(),
                driver_id: "codex_driver".to_string(),
                cwd: "/tmp/project".to_string(),
                worktree_path: None,
                status: "resume_scheduled".to_string(),
                tmux_session: Some("test-session".to_string()),
                tmux_pane: None,
                attached_session_id: None,
                resume_target: Some("last".to_string()),
                resume_mode: Some("last".to_string()),
                blocked_reason: None,
                next_resume_at: Some(now.to_string()),
                retry_policy: json!({"kind": "rate_limit_auto_resume"}),
                prompt: None,
                launch_command: DriverCommandSpec {
                    driver_id: "codex_driver".to_string(),
                    binary: "codex".to_string(),
                    args: vec!["-C".to_string(), "/tmp/project".to_string()],
                    shell_command: "codex -C /tmp/project".to_string(),
                    supports_resume: true,
                    supports_native_tmux: false,
                    supports_external_tmux: true,
                },
                resume_command: Some(DriverCommandSpec {
                    driver_id: "codex_driver".to_string(),
                    binary: "codex".to_string(),
                    args: vec!["-C".to_string(), "/tmp/project".to_string(), "resume".to_string(), "--last".to_string()],
                    shell_command: "codex -C /tmp/project resume --last".to_string(),
                    supports_resume: true,
                    supports_native_tmux: false,
                    supports_external_tmux: true,
                }),
                native_tmux_requested: false,
                last_error: None,
                created_at: now.to_string(),
                updated_at: now.to_string(),
                metadata: json!({}),
                events: vec![],
            }],
        };
        save_store(&state_path, &store).expect("save store");

        let result = handle_session_supervisor_operation(json!({
            "operation": "resume_due",
            "state_path": state_path,
            "dry_run": true,
            "now": now,
        }))
        .expect("resume_due");

        let resumed = result["resumed_workers"].as_array().unwrap();
        assert_eq!(resumed.len(), 1, "resume_scheduled worker should be picked up");
        assert_eq!(resumed[0]["worker_id"], json!("scheduled-worker"));

        let _ = fs::remove_file(state_path);
    }
