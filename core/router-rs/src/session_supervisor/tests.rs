use super::driver::{build_driver_command, is_safe_worktree_slug, resolve_worktree_cwd};
use super::handle_session_supervisor_operation;
use super::worker::terminate_worker;
use super::types::WorkerSessionRecord;
use serde_json::{json, Value};
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
fn smoke_shell_driver_uses_short_sleep_command() {
    let command = build_driver_command(
        "smoke-shell",
        "/tmp/project",
        Some("ignored".to_string()),
        None,
        "last",
        false,
        None,
        None,
    )
    .expect("build smoke-shell command");
    assert_eq!(command.driver_id, "smoke_shell_driver");
    assert_eq!(
        command.args,
        vec!["-c".to_string(), "while true; do sleep 1; done".to_string()]
    );
    assert!(!command.supports_resume);
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
    assert_eq!(resolve_worktree_cwd("/repo", Some("../etc"), None), "/repo");
    assert_eq!(resolve_worktree_cwd("/repo", Some(".."), None), "/repo");
    assert_eq!(resolve_worktree_cwd("/repo", Some("a/b"), None), "/repo");
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
        Some("my-branch".to_string()),
        None,
    )
    .expect("build claude with worktree");
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
    assert!(!is_safe_worktree_slug(""));
    assert!(!is_safe_worktree_slug("../etc"));
    assert!(!is_safe_worktree_slug("a/b"));
    assert!(!is_safe_worktree_slug("a b"));
    assert!(!is_safe_worktree_slug(&"x".repeat(129)));
}

#[test]
fn terminate_non_dry_run_without_pid_marks_interrupted() {
    let mut worker = WorkerSessionRecord {
        worker_id: "worker-no-pid".to_string(),
        host: "codex".to_string(),
        driver_id: "codex_driver".to_string(),
        cwd: "/tmp".to_string(),
        worktree_path: None,
        status: "running".to_string(),
        pid: None,
        log_path: None,
        attached_session_id: None,
        resume_target: None,
        resume_mode: None,
        blocked_reason: None,
        next_resume_at: None,
        retry_policy: json!({}),
        prompt: None,
        launch_command: build_driver_command(
            "codex",
            "/tmp",
            None,
            None,
            "last",
            false,
            None,
            None,
        )
        .expect("command"),
        resume_command: None,
        last_error: None,
        created_at: "2026-04-23T10:00:00Z".to_string(),
        updated_at: "2026-04-23T10:00:00Z".to_string(),
        metadata: json!({}),
        events: Vec::new(),
    };
    let terminated = terminate_worker(&mut worker, false, "2026-04-23T10:01:00Z")
        .expect("terminate without pid");
    assert!(terminated);
    assert_eq!(worker.status, "interrupted");
}

/// Roadmap v5 §6.4 cat.1: 8-way parallel spawn → terminate close stability (dry_run).
#[test]
fn subagent_parallel_spawn_close_stability() {
    const N: usize = 8;
    let state_path = temp_state_path("parallel-spawn-close");
    let launch_now = "2026-04-23T10:00:00Z";
    let terminate_now = "2026-04-23T10:01:00Z";

    let mut launch_handles = Vec::new();
    for idx in 0..N {
        let state_path = state_path.clone();
        launch_handles.push(std::thread::spawn(move || {
            handle_session_supervisor_operation(json!({
                "operation": "launch",
                "state_path": state_path,
                "worker_id": format!("parallel-{idx}"),
                "host": "codex",
                "cwd": "/tmp/project",
                "prompt": format!("parallel lane {idx}"),
                "dry_run": true,
                "now": launch_now,
            }))
            .expect("launch worker");
        }));
    }
    for handle in launch_handles {
        handle.join().expect("launch thread join");
    }

    let mut terminate_handles = Vec::new();
    for idx in 0..N {
        let state_path = state_path.clone();
        terminate_handles.push(std::thread::spawn(move || {
            handle_session_supervisor_operation(json!({
                "operation": "terminate",
                "state_path": state_path,
                "worker_id": format!("parallel-{idx}"),
                "dry_run": true,
                "now": terminate_now,
            }))
            .expect("terminate worker");
        }));
    }
    for handle in terminate_handles {
        handle.join().expect("terminate thread join");
    }

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "now": terminate_now,
    }))
    .expect("list workers");
    let workers = listed["workers"]
        .as_array()
        .expect("workers array");
    assert_eq!(workers.len(), N, "workers={workers:?}");

    let mut worker_ids = std::collections::BTreeSet::new();
    for worker in workers {
        let worker_id = worker["worker_id"]
            .as_str()
            .expect("worker_id")
            .to_string();
        worker_ids.insert(worker_id.clone());
        assert_eq!(worker["status"], json!("interrupted"), "worker {worker_id}");
        let event_names = worker["events"]
            .as_array()
            .expect("events")
            .iter()
            .filter_map(|event| event.get("event").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert!(
            event_names.contains(&"launch_planned"),
            "worker {worker_id} missing launch_planned: {event_names:?}"
        );
        assert!(
            event_names.contains(&"terminate_planned"),
            "worker {worker_id} missing terminate_planned: {event_names:?}"
        );
    }
    for idx in 0..N {
        assert!(worker_ids.contains(&format!("parallel-{idx}")));
    }

    let store_text = fs::read_to_string(&state_path).expect("read supervisor store");
    let store: Value = serde_json::from_str(&store_text).expect("parse supervisor store");
    let version = store["version"].as_u64().expect("store version");
    // upsert_worker bumps version on launch only; terminate updates worker in place.
    assert!(
        version >= 1 + N as u64,
        "expected monotonic launch version >= {}, got {version}",
        1 + N
    );

    let _ = fs::remove_file(state_path);
}

/// Roadmap v5 §6.4 cat.1: subagent spawn → shutdown smoke (dry_run E2E via supervisor op API).
#[test]
fn subagent_lifecycle_spawn_terminate_shutdown_smoke() {
    let state_path = temp_state_path("subagent-lifecycle-smoke");
    let now = "2026-04-23T10:00:00Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "worker_id": "smoke-worker",
        "host": "codex",
        "cwd": "/tmp/project",
        "prompt": "smoke bounded lane",
        "dry_run": true,
        "now": now,
    }))
    .expect("launch worker");
    assert_eq!(launch["worker"]["status"], json!("queued"));

    let terminate = handle_session_supervisor_operation(json!({
        "operation": "terminate",
        "state_path": state_path,
        "worker_id": "smoke-worker",
        "dry_run": true,
        "now": "2026-04-23T10:01:00Z",
    }))
    .expect("terminate worker");
    assert_eq!(terminate["worker"]["status"], json!("interrupted"));
    assert_eq!(terminate["terminated"], json!(true));

    let event_names = terminate["worker"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event.get("event").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(
        event_names.contains(&"launch_planned"),
        "expected launch_planned in events, got {event_names:?}"
    );
    assert!(
        event_names.contains(&"terminate_planned"),
        "expected terminate_planned in events, got {event_names:?}"
    );

    let _ = fs::remove_file(state_path);
}

/// Roadmap v5 §6.4 cat.1: spawn error path → blocked → dry_run terminate shutdown.
#[test]
fn subagent_spawn_error_shutdown_smoke() {
    let state_path = temp_state_path("error-shutdown-smoke");
    let launch_now = "2026-04-23T10:00:00Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "worker_id": "error-worker",
        "host": "codex",
        "cwd": "/tmp/project",
        "prompt": "smoke error lane",
        "dry_run": true,
        "now": launch_now,
    }))
    .expect("launch worker");
    assert_eq!(launch["worker"]["status"], json!("queued"));

    let blocked = handle_session_supervisor_operation(json!({
        "operation": "mark_blocked",
        "state_path": state_path,
        "worker_id": "error-worker",
        "evidence_text": "429 Too Many Requests. Please try again in 5 minutes.",
        "now": "2026-04-23T10:00:30Z",
    }))
    .expect("mark blocked after simulated spawn/execute error");
    assert_eq!(blocked["worker"]["status"], json!("blocked_rate_limit"));
    assert!(blocked["worker"]["last_error"].is_string());

    let terminate = handle_session_supervisor_operation(json!({
        "operation": "terminate",
        "state_path": state_path,
        "worker_id": "error-worker",
        "dry_run": true,
        "now": "2026-04-23T10:01:00Z",
    }))
    .expect("terminate after error");
    assert_eq!(terminate["worker"]["status"], json!("interrupted"));
    assert_eq!(terminate["terminated"], json!(true));

    let event_names = terminate["worker"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event.get("event").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(
        event_names.contains(&"blocked"),
        "expected blocked event after error, got {event_names:?}"
    );
    assert!(
        event_names.contains(&"terminate_planned"),
        "expected terminate_planned after error shutdown, got {event_names:?}"
    );

    let _ = fs::remove_file(state_path);
}

#[test]
fn list_idle_workers_reports_evolution_analyze_dry_run() {
    let state_path = temp_state_path("evolution-idle-dry-run");
    let now = "2026-04-23T10:00:00Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "host": "codex",
        "cwd": "/tmp/project",
        "prompt": "idle evolution smoke",
        "dry_run": true,
        "now": now,
    }))
    .expect("launch worker");
    let worker_id = launch["worker"]["worker_id"]
        .as_str()
        .expect("worker_id")
        .to_string();

    let _ = handle_session_supervisor_operation(json!({
        "operation": "terminate",
        "state_path": state_path,
        "worker_id": worker_id,
        "dry_run": true,
        "now": now,
    }))
    .expect("terminate worker");

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "dry_run": true,
        "force_evolution_idle": true,
        "now": now,
    }))
    .expect("list idle workers");
    let evolution_idle = listed["evolution_idle"].as_object().expect("evolution_idle");
    assert_eq!(evolution_idle.get("triggered"), Some(&json!(true)));
    assert_eq!(evolution_idle.get("status"), Some(&json!("dry_run")));

    let _ = fs::remove_file(state_path);
}

#[test]
fn list_with_running_worker_skips_evolution_idle_trigger() {
    let state_path = temp_state_path("evolution-idle-active");
    let now = "2026-04-23T10:00:00Z";
    let mut running = sample_worker_for_idle_test("running");
    running.worker_id = "running-worker".to_string();
    running.created_at = now.to_string();
    running.updated_at = now.to_string();
    let store = json!({
        "schema_version": super::types::SESSION_SUPERVISOR_STORE_SCHEMA_VERSION,
        "version": 1,
        "workers": [running],
    });
    fs::write(
        &state_path,
        serde_json::to_string_pretty(&store).expect("serialize store"),
    )
    .expect("write store");

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "dry_run": true,
        "force_evolution_idle": true,
        "now": now,
    }))
    .expect("list active workers");
    let evolution_idle = listed["evolution_idle"].as_object().expect("evolution_idle");
    assert_eq!(evolution_idle.get("triggered"), Some(&json!(false)));
    assert_eq!(evolution_idle.get("status"), Some(&json!("workers_active")));

    let _ = fs::remove_file(state_path);
}

fn sample_worker_for_idle_test(status: &str) -> WorkerSessionRecord {
    WorkerSessionRecord {
        worker_id: "w1".to_string(),
        host: "codex".to_string(),
        driver_id: "codex_driver".to_string(),
        cwd: "/tmp".to_string(),
        worktree_path: None,
        status: status.to_string(),
        pid: None,
        log_path: None,
        attached_session_id: None,
        resume_target: None,
        resume_mode: None,
        blocked_reason: None,
        next_resume_at: None,
        retry_policy: json!({}),
        prompt: None,
        launch_command: build_driver_command(
            "codex",
            "/tmp",
            None,
            None,
            "last",
            false,
            None,
            None,
        )
        .expect("command"),
        resume_command: None,
        last_error: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        metadata: json!({}),
        events: Vec::new(),
    }
}

/// Roadmap v5 §6.4 cat.1: stale worker reaped on `list` after heartbeat TTL (dry_run, no real process).
#[test]
fn subagent_spawn_timeout_shutdown_smoke() {
    let state_path = temp_state_path("timeout-shutdown-smoke");
    let launch_now = "2026-04-23T10:00:00Z";
    let stale_now = "2026-04-23T11:00:01Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "worker_id": "timeout-worker",
        "host": "codex",
        "cwd": "/tmp/project",
        "prompt": "smoke timeout lane",
        "dry_run": true,
        "now": launch_now,
    }))
    .expect("launch worker");
    assert_eq!(launch["worker"]["status"], json!("queued"));

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "now": stale_now,
        "stale_after_secs": 3600,
    }))
    .expect("list workers after stale threshold");
    let workers = listed["workers"]
        .as_array()
        .expect("workers array");
    assert_eq!(workers.len(), 1);
    let worker = &workers[0];
    assert_eq!(worker["worker_id"], json!("timeout-worker"));
    assert_eq!(worker["status"], json!("interrupted"));
    let event_names = worker["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event.get("event").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(
        event_names.contains(&"launch_planned"),
        "expected launch_planned, got {event_names:?}"
    );
    assert!(
        event_names.contains(&"stale_timeout"),
        "expected stale_timeout reap on list, got {event_names:?}"
    );

    let _ = fs::remove_file(state_path);
}

/// Roadmap v5 §6.4 cat.1: after shutdown, supervisor must not leak locks, temps, or active workers.
#[test]
fn subagent_resource_leak_detection() {
    const N: usize = 4;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let state_dir = std::env::temp_dir().join(format!("router-rs-leak-detect-{nonce}"));
    let _ = fs::remove_dir_all(&state_dir);
    fs::create_dir_all(&state_dir).expect("mkdir state dir");
    let state_path = state_dir.join("state.json");
    let launch_now = "2026-04-23T10:00:00Z";
    let terminate_now = "2026-04-23T10:01:00Z";

    for idx in 0..N {
        let worker_id = format!("leak-{idx}");
        handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path,
            "worker_id": worker_id,
            "host": "codex",
            "cwd": "/tmp/project",
            "prompt": format!("leak detect lane {idx}"),
            "dry_run": true,
            "now": launch_now,
        }))
        .expect("launch worker");
        handle_session_supervisor_operation(json!({
            "operation": "terminate",
            "state_path": state_path,
            "worker_id": worker_id,
            "dry_run": true,
            "now": terminate_now,
        }))
        .expect("terminate worker");
    }

    for entry in fs::read_dir(&state_dir).expect("read state dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.ends_with(".tmp"),
            "supervisor temp payload leaked after shutdown: {name}"
        );
    }

    // Sentinel `.lock` files are retained by design; a follow-up op must succeed (flock released).
    handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "now": terminate_now,
    }))
    .expect("list after shutdown must not be blocked by stale flock");

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path,
        "now": terminate_now,
    }))
    .expect("list workers after shutdown");
    let workers = listed["workers"]
        .as_array()
        .expect("workers array");
    assert_eq!(workers.len(), N, "workers={workers:?}");

    for worker in workers {
        assert!(
            worker.get("pid").is_none_or(|v| v.is_null()),
            "pid must be cleared for dry_run workers: {worker}"
        );
        assert!(
            worker.get("log_path").is_none_or(|v| v.is_null()),
            "log_path must not be set for dry_run workers: {worker}"
        );
        let status = worker["status"].as_str().expect("status");
        assert!(
            !matches!(
                status,
                "queued" | "launching" | "running" | "resume_scheduled"
            ),
            "worker still active after terminate: {worker}"
        );
    }

    let logs_dir = state_dir.join("logs");
    if logs_dir.is_dir() {
        assert!(
            fs::read_dir(&logs_dir)
                .expect("read logs dir")
                .next()
                .is_none(),
            "dry_run shutdown must not leave worker log files"
        );
    }

    let _ = fs::remove_dir_all(&state_dir);
}

/// Roadmap v5 §6.4 cat.1: non-dry_run real process spawn → terminate smoke (`smoke-shell` / `sleep 1`).
///
/// Opt-in: `ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE=1` (skipped by default for CI).
#[test]
fn subagent_spawn_real_process_smoke() {
    if !crate::router_env_flags::router_rs_session_supervisor_real_process_smoke_enabled() {
        eprintln!(
            "skip subagent_spawn_real_process_smoke: set ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE=1"
        );
        return;
    }
    #[cfg(not(unix))]
    {
        eprintln!("skip: real-process smoke requires unix");
        return;
    }

    use super::process::process_is_alive;

    let state_path = temp_state_path("real-process-smoke");
    let cwd = std::env::temp_dir();
    let cwd_str = cwd.to_string_lossy().into_owned();
    let launch_now = "2026-04-23T10:00:00Z";
    let terminate_now = "2026-04-23T10:00:15Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "worker_id": "real-smoke-worker",
        "host": "smoke-shell",
        "cwd": cwd_str,
        "prompt": "ignored for smoke-shell",
        "dry_run": false,
        "now": launch_now,
    }))
    .expect("launch real worker");
    let worker = &launch["worker"];
    assert_eq!(worker["status"], json!("running"));
    let pid = worker["pid"].as_u64().expect("pid") as u32;
    assert!(
        process_is_alive(pid),
        "smoke worker should be alive after launch"
    );
    assert!(worker["log_path"].is_string());

    let terminate = handle_session_supervisor_operation(json!({
        "operation": "terminate",
        "state_path": state_path,
        "worker_id": "real-smoke-worker",
        "dry_run": false,
        "now": terminate_now,
    }))
    .expect("terminate real worker");
    assert_eq!(terminate["worker"]["status"], json!("interrupted"));
    assert_eq!(terminate["terminated"], json!(true));
    assert!(
        !process_is_alive(pid),
        "smoke worker pid {pid} should be dead after terminate"
    );

    let event_names = terminate["worker"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event.get("event").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(
        event_names.contains(&"launched"),
        "expected launched event, got {event_names:?}"
    );
    assert!(
        event_names.contains(&"terminated"),
        "expected terminated event, got {event_names:?}"
    );

    if let Some(log_path) = worker["log_path"].as_str() {
        let _ = fs::remove_file(log_path);
        if let Some(parent) = std::path::Path::new(log_path).parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
    let _ = fs::remove_file(state_path);
}
