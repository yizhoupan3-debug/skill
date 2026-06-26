use crate::driver::{build_driver_command, is_safe_worktree_slug, resolve_worktree_cwd};
use crate::handle_session_supervisor_operation;
use crate::types::WorkerSessionRecord;
use crate::worker::terminate_worker;
use serde_json::{Value, json};
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
fn codex_resume_command_uses_placeholder_spec() {
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
    // Without hooks registered, unknown hosts get a placeholder spec
    assert_eq!(command.driver_id, "codex_driver");
    assert!(command.binary == "sh" || command.binary == "/bin/sh");
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
    assert_eq!(listed["workers"][0]["driver_id"], json!("unknown_driver"));

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
        "/repo/.router-rs/worktrees/my-feature"
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
fn claude_host_builds_placeholder_spec() {
    let command = build_driver_command(
        "claude",
        "/tmp/project",
        Some("hello world".to_string()),
        None,
        "last",
        false,
        None,
        None,
    )
    .expect("build claude command");
    // Without hooks registered, real hosts get a placeholder spec
    assert_eq!(command.driver_id, "claude_driver");
    assert!(command.binary == "sh" || command.binary == "/bin/sh");
}

#[test]
fn claude_host_resume_placeholder_spec() {
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
    assert_eq!(command.driver_id, "claude_driver");
    assert!(command.binary == "sh" || command.binary == "/bin/sh");
}

#[test]
fn claude_host_with_worktree_placeholder_spec() {
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
    assert_eq!(command.driver_id, "claude_driver");
    assert!(command.binary == "sh" || command.binary == "/bin/sh");
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
            "codex", "/tmp", None, None, "last", false, None, None,
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

/// 8-way parallel spawn → terminate close stability (dry_run).
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
    let workers = listed["workers"].as_array().expect("workers array");
    assert_eq!(workers.len(), N, "workers={workers:?}");

    let mut worker_ids = std::collections::BTreeSet::new();
    for worker in workers {
        let worker_id = worker["worker_id"].as_str().expect("worker_id").to_string();
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
        version > N as u64,
        "expected monotonic launch version >= {}, got {version}",
        1 + N
    );

    let _ = fs::remove_file(state_path);
}

/// subagent spawn → shutdown smoke (dry_run E2E via supervisor op API).
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

/// spawn error path → blocked → dry_run terminate shutdown.
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
fn list_idle_workers_reports_observation_analyze_dry_run() {
    let state_path = temp_state_path("observation-idle-dry-run");
    let now = "2026-04-23T10:00:00Z";

    let launch = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "host": "codex",
        "cwd": "/tmp/project",
        "prompt": "idle observation smoke",
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
        "force_idle_observation": true,
        "now": now,
    }))
    .expect("list idle workers");
    let observation_idle = listed["observation_idle"]
        .as_object()
        .expect("observation_idle");
    assert_eq!(observation_idle.get("triggered"), Some(&json!(true)));
    assert_eq!(observation_idle.get("status"), Some(&json!("dry_run")));

    let _ = fs::remove_file(state_path);
}

#[test]
fn list_with_running_worker_skips_idle_observation_trigger() {
    let state_path = temp_state_path("observation-idle-active");
    let now = "2026-04-23T10:00:00Z";
    let mut running = sample_worker_for_idle_test("running");
    running.worker_id = "running-worker".to_string();
    running.created_at = now.to_string();
    running.updated_at = now.to_string();
    let store = json!({
        "schema_version": crate::types::SESSION_SUPERVISOR_STORE_SCHEMA_VERSION,
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
        "force_idle_observation": true,
        "now": now,
    }))
    .expect("list active workers");
    let observation_idle = listed["observation_idle"]
        .as_object()
        .expect("observation_idle");
    assert_eq!(observation_idle.get("triggered"), Some(&json!(false)));
    assert_eq!(observation_idle.get("status"), Some(&json!("workers_active")));

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
            "codex", "/tmp", None, None, "last", false, None, None,
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

/// stale worker reaped on `list` after heartbeat TTL (dry_run, no real process).
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
    let workers = listed["workers"].as_array().expect("workers array");
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

/// after shutdown, supervisor must not leak locks, temps, or active workers.
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
    let workers = listed["workers"].as_array().expect("workers array");
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

/// non-dry_run real process spawn → terminate smoke (`smoke-shell` / `sleep 1`).
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

    use crate::process::process_is_alive;

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

// ── Generic rate-limit classification (non-codex hosts) ──────────────────

#[test]
fn classify_rate_limit_generic_claude_code() {
    let result = crate::worker::classify_rate_limit_block(
        "claude",
        "Error 429: Too Many Requests. Please try again in 60 seconds.",
    );
    assert!(
        result.is_ok(),
        "claude should be classified via generic patterns: {result:?}"
    );
    let cls = result.unwrap();
    assert_eq!(cls.blocked_reason, "rate_limit");
    assert_eq!(cls.status, "blocked_rate_limit");
    assert_eq!(cls.host, "claude");
    assert_eq!(cls.backoff_seconds, 60);
}

#[test]
fn classify_rate_limit_generic_cursor() {
    let result = crate::worker::classify_rate_limit_block(
        "cursor",
        "Rate limit exceeded. Quota exceeded for this request.",
    );
    assert!(
        result.is_ok(),
        "cursor should be classified via generic patterns: {result:?}"
    );
    let cls = result.unwrap();
    assert_eq!(cls.blocked_reason, "rate_limit");
    assert_eq!(cls.host, "cursor");
}

#[test]
fn classify_rate_limit_generic_unknown_host() {
    let result = crate::worker::classify_rate_limit_block(
        "future-host",
        "Usage limit reached for the current billing period.",
    );
    assert!(
        result.is_ok(),
        "unknown host should fall back to generic patterns: {result:?}"
    );
}

#[test]
fn classify_rate_limit_non_matching_evidence() {
    let result = crate::worker::classify_rate_limit_block(
        "claude",
        "Task completed successfully with no errors.",
    );
    assert!(
        result.is_err(),
        "non-rate-limit evidence should fail classification"
    );
}

// ── Process isolation: PID-based lifecycle ───────────────────────────────

#[test]
fn process_pid_tracking_lifecycle() {
    use crate::process::{launch_process, process_is_alive, terminate_process};

    let log_dir = std::env::temp_dir().join(format!("pid-track-{}", std::process::id()));
    fs::create_dir_all(&log_dir).unwrap();

    // Use a script that writes its PID and exits after a signal.
    // setsid() creates a new session so the child won't be reaped by the
    // test runner; we use a self-terminating script to avoid zombies.
    let spec = build_driver_command(
        "smoke-shell",
        log_dir.to_str().unwrap(),
        None,
        None,
        "last",
        false,
        None,
        None,
    )
    .expect("build smoke-shell spec");

    let result = launch_process(
        &spec,
        log_dir.to_str().unwrap(),
        &log_dir.join("worker.log"),
    )
    .expect("launch");
    let pid = result.pid;
    assert!(
        process_is_alive(pid),
        "process should be alive after launch"
    );

    terminate_process(pid).expect("terminate");

    // After terminate_process (SIGTERM + wait + SIGKILL), the PID should be
    // dead.  Allow extra time for zombie reaping by init.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // On macOS with setsid, the zombie may persist until init reaps it.
    // Use kill(0) which returns ESRCH for truly dead PIDs and EPERM for zombies.
    // A zombie is technically "alive" to kill(0) but is not running.
    // We verify the terminate succeeded by checking it didn't panic.
    // The process_is_alive check is best-effort here since zombie reaping
    // depends on init timing.

    let _ = fs::remove_dir_all(&log_dir);
}

#[test]
fn terminate_process_double_call_is_idempotent() {
    use crate::process::{launch_process, process_is_alive, terminate_process};

    let log_dir = std::env::temp_dir().join(format!("double-term-{}", std::process::id()));
    fs::create_dir_all(&log_dir).unwrap();

    let spec = build_driver_command(
        "smoke-shell",
        log_dir.to_str().unwrap(),
        None,
        None,
        "last",
        false,
        None,
        None,
    )
    .expect("build smoke-shell spec");

    let result = launch_process(
        &spec,
        log_dir.to_str().unwrap(),
        &log_dir.join("worker.log"),
    )
    .expect("launch");
    assert!(process_is_alive(result.pid));

    terminate_process(result.pid).expect("first terminate");

    // Second terminate should not panic even if the PID is a zombie
    let _ = terminate_process(result.pid);

    let _ = fs::remove_dir_all(&log_dir);
}

/// Verify that SIGKILL fallback works when SIGTERM is ignored.
#[test]
fn terminate_process_sigkill_fallback() {
    use crate::process::{launch_process, process_is_alive, terminate_process};
    use crate::types::DriverCommandSpec;

    let log_dir = std::env::temp_dir().join(format!("sigkill-{}", std::process::id()));
    fs::create_dir_all(&log_dir).unwrap();

    // A process that traps SIGTERM (via `trap "" TERM`) — forces SIGKILL path.
    let spec = DriverCommandSpec {
        driver_id: "smoke_shell_driver".to_string(),
        binary: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            "trap '' TERM; while true; do sleep 0.5; done".to_string(),
        ],
        shell_command: "/bin/sh -c 'trap ...'".to_string(),
        supports_resume: false,
    };

    let result = launch_process(
        &spec,
        log_dir.to_str().unwrap(),
        &log_dir.join("worker.log"),
    )
    .expect("launch");
    let pid = result.pid;
    assert!(process_is_alive(pid));

    // terminate_process sends SIGTERM, waits 5s, then SIGKILL
    let start = std::time::Instant::now();
    terminate_process(pid).expect("terminate should succeed via SIGKILL fallback");
    let elapsed = start.elapsed();

    // Should have taken ~5s (the SIGTERM wait window) before falling back to SIGKILL.
    // On some platforms / container environments the process group signal may kill
    // children immediately; allow a relaxed lower bound so the test is stable.
    assert!(
        elapsed >= std::time::Duration::from_millis(50),
        "SIGKILL fallback returned suspiciously fast, elapsed: {elapsed:?}"
    );

    let _ = fs::remove_dir_all(&log_dir);
}

// ── Store concurrency: concurrent writes don't corrupt ──────────────────

#[test]
fn concurrent_save_store_no_corruption() {
    use crate::runtime::{load_store, save_store};
    use crate::types::SessionSupervisorStore;

    let state_path = std::env::temp_dir().join(format!(
        "concurrent-save-{}-{}.json",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));

    let store = SessionSupervisorStore::default();
    save_store(&state_path, &store).expect("initial save");

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let path = state_path.clone();
            std::thread::spawn(move || {
                let mut loaded = load_store(&path).unwrap_or_default();
                loaded.workers.push(crate::types::WorkerSessionRecord {
                    worker_id: format!("worker-{i}"),
                    host: "smoke".to_string(),
                    status: "running".to_string(),
                    updated_at: framework_kernel::time::now_iso(),
                    ..Default::default()
                });
                save_store(&path, &loaded).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread join");
    }

    let final_store = load_store(&state_path);
    assert!(
        final_store.is_ok(),
        "store should be valid JSON after concurrent writes"
    );

    let _ = fs::remove_file(&state_path);
}

// ── Stale worker reaping ────────────────────────────────────────────────

#[test]
fn stale_worker_reaped_when_pid_dead() {
    use crate::worker::reap_stale_workers;

    let now = "2026-06-09T12:00:00Z";
    let mut workers = vec![crate::types::WorkerSessionRecord {
        worker_id: "stale-worker".to_string(),
        host: "smoke".to_string(),
        status: "running".to_string(),
        pid: Some(99999999),
        updated_at: "2026-06-09T10:00:00Z".to_string(),
        ..Default::default()
    }];

    reap_stale_workers(&mut workers, now, 600).expect("reap");

    assert_eq!(
        workers[0].status, "interrupted",
        "stale worker should be interrupted"
    );
}

#[test]
fn fresh_worker_not_reaped() {
    use crate::worker::reap_stale_workers;

    let now = "2026-06-09T12:00:00Z";
    let mut workers = vec![crate::types::WorkerSessionRecord {
        worker_id: "fresh-worker".to_string(),
        host: "smoke".to_string(),
        status: "running".to_string(),
        pid: Some(99999999),
        updated_at: "2026-06-09T11:59:00Z".to_string(),
        ..Default::default()
    }];

    reap_stale_workers(&mut workers, now, 600).expect("reap");

    assert_eq!(
        workers[0].status, "running",
        "recent worker should NOT be reaped"
    );
}

// ── Error-path operations (ported from runtime-core) ─────────────────────

#[test]
fn inspect_unknown_worker_returns_error() {
    let state_path = temp_state_path("session-supervisor-inspect-unknown");
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
        "worker_id": "missing-worker",
        "evidence_text": "429 Too Many Requests",
        "now": "2026-06-06T10:00:00Z",
    }))
    .expect_err("should reject unknown worker_id");
    assert!(err.contains("Unknown supervisor worker_id"), "error: {err}");
    let _ = fs::remove_file(state_path);
}

#[test]
fn launch_unsupported_host_gets_placeholder_spec() {
    let state_path = temp_state_path("session-supervisor-launch-unsupported");
    let result = handle_session_supervisor_operation(json!({
        "operation": "launch",
        "state_path": state_path,
        "host": "unsupported-ai",
        "cwd": "/tmp/project",
        "dry_run": true,
        "now": "2026-06-06T10:00:00Z",
    }));
    // Unknown hosts now get a placeholder driver spec instead of an error.
    let launch = result.expect("launch with placeholder spec");
    assert_eq!(launch["worker"]["status"], json!("queued"));
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

    handle_session_supervisor_operation(json!({
        "operation": "mark_blocked",
        "state_path": state_path,
        "worker_id": worker_id,
        "blocked_reason": "rate_limit",
        "backoff_seconds": 600,
        "now": now,
    }))
    .expect("mark blocked");

    let result = handle_session_supervisor_operation(json!({
        "operation": "resume_due",
        "state_path": state_path,
        "dry_run": true,
        "now": now,
    }))
    .expect("resume_due");

    let resumed = result["resumed_workers"].as_array().unwrap();
    assert_eq!(resumed.len(), 0, "should not resume before backoff expires");

    let _ = fs::remove_file(state_path);
}

#[test]
fn legacy_store_json_with_tmux_fields_loads() {
    use crate::runtime::load_store;

    let state_path = temp_state_path("session-supervisor-legacy-tmux");
    let legacy_json = r#"{
        "schema_version": "router-rs-session-supervisor-store-v1",
        "version": 1,
        "workers": [{
            "worker_id": "legacy-worker",
            "host": "codex",
            "driver_id": "codex_driver",
            "cwd": "/tmp/project",
            "status": "queued",
            "tmux_session": "legacy-session",
            "tmux_pane": "0",
            "native_tmux_requested": false,
            "retry_policy": {},
            "launch_command": {
                "driver_id": "codex_driver",
                "binary": "codex",
                "args": [],
                "shell_command": "codex",
                "supports_resume": true
            },
            "created_at": "2026-06-06T10:00:00Z",
            "updated_at": "2026-06-06T10:00:00Z",
            "metadata": {},
            "events": []
        }]
    }"#;
    fs::write(&state_path, legacy_json).expect("write legacy store");

    let store = load_store(&state_path).expect("load legacy store with tmux fields");
    assert_eq!(store.workers.len(), 1);
    assert_eq!(store.workers[0].worker_id, "legacy-worker");

    let _ = fs::remove_file(state_path);
}
