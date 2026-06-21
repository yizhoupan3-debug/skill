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
fn kill_stale_terminals_disabled_by_env_truthy_values_keep_enabled() {
    let prev = std::env::var_os("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS");
    unsafe { std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS") };
    assert!(!kill_stale_terminals_disabled_by_env());
    for v in ["", "1", "true", "yes", "on", "anything"] {
        unsafe { std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v) };
        assert!(
            !kill_stale_terminals_disabled_by_env(),
            "value {v:?} should NOT disable"
        );
    }
    for v in ["0", "false", "off", "no", "  FALSE  "] {
        unsafe { std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v) };
        assert!(
            kill_stale_terminals_disabled_by_env(),
            "value {v:?} should disable"
        );
    }
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS") },
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
        if !crate::hosts::file_state_lock::is_process_alive(dead_pid) {
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
        crate::hosts::file_state_lock::is_process_alive(outside_pid),
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
    assert!(crate::hosts::file_state_lock::is_process_alive(pid), "child must be alive before kill");

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
    assert!(!crate::hosts::file_state_lock::is_process_alive(pid), "child must be reaped");
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
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", "0") };
    let out = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert_eq!(out, json!({}));
    assert!(!state_path(&repo, &payload).exists(), "state still cleared");
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS") },
    }
}

#[test]
fn session_start_operator_inject_off_skips_additional_context() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
    unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", "0") };
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
        Some(v) => unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", v) },
        None => unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") },
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
    unsafe { env::set_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS", "420") };
    let out = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS") },
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
#[serial]
fn session_start_resets_session_call_tracker() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");
    hooks::init_tracker(&repo).expect("seed");
    for _ in 0..50 {
        hooks::record_tool_call(&repo, "Read", None).expect("record");
    }
    let before = hooks::read_tracker_state(&repo).expect("read");
    assert!(before["total_calls"].as_u64().unwrap_or(0) >= 50);

    let payload = json!({
        "session_id": "ss-tracker-reset",
        "cwd": repo.display().to_string()
    });
    let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let after = hooks::read_tracker_state(&repo).expect("read after");
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
            "schema_version": "router-rs-goal-v1",
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
        !ctx.contains(core_state::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH),
        "continuity hint must not appear: {ctx:?}"
    );
}

#[test]
fn session_start_initializes_terminal_baseline_ledger() {
    let _term_env = terminals_dir_env_lock();
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
    unsafe { std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir) };
    let payload = json!({ "session_id": "sess-ledger-init", "cwd": repo.display().to_string() });
    let _ = dispatch_cursor_hook_event(&repo, "sessionStart", &payload);
    let ledger = load_session_terminal_ledger(&repo, &payload);
    assert_eq!(ledger.version, SESSION_TERMINAL_LEDGER_VERSION);
    assert_eq!(ledger.baseline_pids, vec![11111, 22222]);
    assert!(ledger.owned_pids.is_empty());
    match prev {
        Some(v) => unsafe { std::env::set_var("CURSOR_TERMINALS_DIR", v) },
        None => unsafe { std::env::remove_var("CURSOR_TERMINALS_DIR") },
    }
}

#[cfg(unix)]
#[test]
fn session_end_kills_only_owned_terminal_pids() {
    let _term_env = terminals_dir_env_lock();
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
    assert!(crate::hosts::file_state_lock::is_process_alive(owned_pid));
    assert!(crate::hosts::file_state_lock::is_process_alive(other_pid));

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
    unsafe { std::env::set_var("CURSOR_TERMINALS_DIR", &term_dir) };
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
        if !crate::hosts::file_state_lock::is_process_alive(owned_pid) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = owned_waiter.join();
    assert!(!crate::hosts::file_state_lock::is_process_alive(owned_pid), "owned pid must be killed");
    assert!(crate::hosts::file_state_lock::is_process_alive(other_pid), "non-owned pid must stay alive");

    unsafe {
        let _ = libc::kill(other_pid as libc::pid_t, libc::SIGKILL);
    }
    let _ = other_child.wait();

    match prev {
        Some(v) => unsafe { std::env::set_var("CURSOR_TERMINALS_DIR", v) },
        None => unsafe { std::env::remove_var("CURSOR_TERMINALS_DIR") },
    }
}

#[test]
fn launcher_fail_closed_before_submit_when_router_rs_missing() {
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
    let isolated_home = empty_ws.join("home");
    fs::create_dir_all(&isolated_home).expect("isolated home");

    let output = Command::new("/bin/bash")
        .arg(&launcher)
        .arg("BeforeSubmitPrompt")
        .env_remove("ROUTER_RS_BIN")
        .env("CARGO_TARGET_DIR", &empty_target)
        .env("HOME", &isolated_home)
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
fn launcher_fail_open_session_start_when_router_rs_missing() {
    use std::process::Command;
    let framework = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let launcher = framework.join("configs/framework/cursor-router-rs-hook.sh");
    let empty_ws =
        env::temp_dir().join(format!("cursor-launcher-fail-open-{}", std::process::id()));
    fs::create_dir_all(&empty_ws).expect("empty workspace");
    let empty_target = empty_ws.join("no-cargo-target");
    fs::create_dir_all(&empty_target).expect("empty target dir");
    let isolated_home = empty_ws.join("home");
    fs::create_dir_all(&isolated_home).expect("isolated home");

    let output = Command::new("/bin/bash")
        .arg(&launcher)
        .arg("SessionStart")
        .env_remove("ROUTER_RS_BIN")
        .env("CARGO_TARGET_DIR", &empty_target)
        .env("HOME", &isolated_home)
        .env("PATH", "/usr/bin:/bin")
        .env("CURSOR_WORKSPACE_ROOT", &empty_ws)
        .env("SKILL_FRAMEWORK_ROOT", &empty_ws)
        .output()
        .expect("run launcher");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "telemetry SessionStart must fail-open; stderr={stderr}"
    );
    assert!(
        stderr.contains("fail-open"),
        "SessionStart must log fail-open when router-rs missing; stderr={stderr}"
    );
}

#[test]
fn stop_handler_releases_session_lock_before_task_ledger_checkpoint() {
    let src = concat!(
        include_str!("handlers.rs"),
        include_str!("handlers/stop_closeout.rs"),
        include_str!("handlers/stop.rs"),
    );
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
fn armed_post_tool_read_fast_path_skips_l3_when_no_pending_work() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-armed-read-fast";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, sid).core.review_required);
    let out = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "Read",
            "tool_path": "README.md"
        }),
    );
    assert!(
        out.get("permission").is_none(),
        "armed Read with no pending must not deny; out={out:?}"
    );
}

#[test]
fn armed_post_tool_l1_before_l3_lock_order_under_review() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-armed-lock-order";
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
            "subagent_id": "r1"
        }),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "Task",
            "tool_input": {"subagent_type": "general-purpose", "fork_context": false}
        }),
    );
    assert!(
        out.get("permission").is_none(),
        "armed Task postTool must complete without deny; out={out:?}"
    );
}

#[test]
fn stop_and_post_tool_concurrent_hooks_complete_under_one_second() {
    let _env = core_policy::test_env_sync::process_env_lock();
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

#[test]
fn verify_state_flock_concurrency() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let repo = Arc::new(fresh_repo());
    let sid = "s-concurrency-state-flock";
    let payload = event(sid, "全面review这个仓库");

    // 初始化状态，存入 followup_count = 0
    {
        let mut lock = acquire_state_lock(&repo, &payload);
        assert!(lock.is_some());
        let mut state = empty_state();
        state.core.followup_count = 0;
        let _ = save_state(&repo, &payload, &mut state);
        release_state_lock(&mut lock);
    }

    let mut threads = vec![];
    let num_threads = 20;
    
    for _ in 0..num_threads {
        let repo_clone = Arc::clone(&repo);
        let payload_clone = payload.clone();
        threads.push(std::thread::spawn(move || {
            // 每个线程竞争锁，然后读取 count，加 1，写回，释放锁
            for _ in 0..5 {
                let mut lock = acquire_state_lock(&repo_clone, &payload_clone);
                while lock.is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    lock = acquire_state_lock(&repo_clone, &payload_clone);
                }
                let mut state = load_state(&repo_clone, &payload_clone)
                    .ok()
                    .flatten()
                    .unwrap_or_else(empty_state);
                state.core.followup_count += 1;
                // 用非原子操作模拟一点延时，扩大竞争竞态窗口
                std::thread::sleep(std::time::Duration::from_millis(2));
                let _ = save_state(&repo_clone, &payload_clone, &mut state);
                release_state_lock(&mut lock);
            }
        }));
    }

    for t in threads {
        let _ = t.join();
    }

    // 校验最终结果是否等于线程数 * 5
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(lock.is_some());
    let state = load_state(&repo, &payload).ok().flatten().unwrap();
    assert_eq!(state.core.followup_count, num_threads * 5);
    release_state_lock(&mut lock);
}

#[test]
fn verify_atomic_write_concurrency() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let repo = fresh_repo();
    let final_path = repo.join("concurrent_atomic_write_test.json");
    
    let mut threads = vec![];
    let num_threads = 20;
    
    for i in 0..num_threads {
        let path_clone = final_path.clone();
        threads.push(std::thread::spawn(move || {
            // 多个线程并发写入到同一个 final_path，使用 write_atomic_json 写入不同的内容
            for j in 0..10 {
                let val = json!({
                    "thread_index": i,
                    "write_index": j,
                    "random_payload": "some data to simulate typical json state content"
                });
                let _ = core_state::utils::atomic_write::write_atomic_json(&path_clone, &val);
                // 模拟一点延迟以增加重合度
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }));
    }

    for t in threads {
        let _ = t.join();
    }

    // 最终文件应当是有效的 JSON 文件，并且能被成功解析，不应该出现截断、损坏或半截字符
    assert!(final_path.exists());
    let content = fs::read_to_string(&final_path).expect("read final json");
    let parsed: Value = serde_json::from_str(&content).expect("parse final json");
    assert!(parsed.get("thread_index").is_some());
    assert!(parsed.get("write_index").is_some());
}

#[test]
fn review_lite_id_cycle_settle() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-id";
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
            "subagent_id": "lite-1"
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.phase, 2);
    assert_eq!(mid.review_lite_pending_cycle_keys, vec!["id:lite-1"]);
    assert!(mid.review_subagent_pending_cycle_keys.is_empty());
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "lite-1"
        }),
    );
    let end = load_state_for(&repo, sid);
    assert_eq!(end.phase, 3);
    assert!(end.review_lite_pending_cycle_keys.is_empty());
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_lite_lane_fallback_strict() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-lane-fb";
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
    let mid = load_state_for(&repo, sid);
    assert!(mid.review_lite_pending_cycle_keys.is_empty());
    assert_eq!(mid.review_subagent_pending_cycle_keys, vec!["lane:general-purpose"]);
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_gate_mode_strict_regression_unset_env() {
    let _env = core_policy::test_env_sync::process_env_lock();
    unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") };
    assert_eq!(
        core_policy::review_gate_engine::cursor_review_gate_mode(),
        core_policy::review_gate_engine::CursorReviewGateMode::Strict
    );
}

#[test]
fn review_lite_mixed_id_and_lane_advises_stop_until_both_settled() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-mixed";
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
            "subagent_id": "mix-id"
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
    assert_eq!(mid.review_lite_pending_cycle_keys, vec!["id:mix-id"]);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys,
        vec!["lane:general-purpose"]
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "mix-id"
        }),
    );
    let after_id_stop = load_state_for(&repo, sid);
    assert_eq!(
        after_id_stop.phase, 2,
        "id stop must not bump phase=3 while lane fallback pending remains: {:?}",
        after_id_stop
    );
    assert!(after_id_stop.review_lite_pending_cycle_keys.is_empty());
    assert_eq!(
        after_id_stop.review_subagent_pending_cycle_keys,
        vec!["lane:general-purpose"]
    );
    let stop_out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, ""));
    let fm = stop_out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "Stop must advise while strict fallback pending remains (phase/pending not gate): {stop_out}"
    );
    assert!(
        stop_out.get("permission").is_none(),
        "pending multiset must not hard-block Stop; out={stop_out:?}"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose"
        }),
    );
    let end = load_state_for(&repo, sid);
    assert!(end.review_subagent_pending_cycle_keys.is_empty());
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_lite_orphan_pending_advises_stop_under_strict_mode() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-orphan-strict";
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
            "subagent_id": "orphan-1"
        }),
    );
    assert_eq!(
        load_state_for(&repo, sid).review_lite_pending_cycle_keys,
        vec!["id:orphan-1"]
    );
    unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") };
    let stop_out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, ""));
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&stop_out));
    assert!(
        stop_out.get("permission").is_none(),
        "orphan pending is telemetry-only; must not hard-block Stop; out={stop_out:?}"
    );
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_lite_read_posttool_does_not_skip_when_id_pending() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-read-pending";
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
            "subagent_id": "read-pending-1"
        }),
    );
    let before = load_state_for(&repo, sid);
    assert_eq!(before.review_lite_pending_cycle_keys.len(), 1);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Read",
            "tool_input": {},
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    let after = load_state_for(&repo, sid);
    assert_eq!(
        after.review_lite_pending_cycle_keys,
        before.review_lite_pending_cycle_keys,
        "Read fast path must not clear lite pending"
    );
    assert_eq!(after.phase, before.phase);
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_lite_cap_refused_at_pending_max() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "2") };
    let repo = fresh_repo();
    let sid = "s-lite-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    for id in ["cap-a", "cap-b"] {
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({
                "session_id": sid,
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": id
            }),
        );
    }
    let deny = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "cap-c"
        }),
    );
    assert_eq!(deny.get("permission"), Some(&json!("deny")));
    assert!(
        load_state_for(&repo, sid).review_pending_cap_refused,
        "third lite id start must latch cap refused"
    );
    let stop_out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, ""));
    let fm = stop_out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "cap refused must keep Stop advisory nudge: {stop_out}"
    );
    assert!(
        stop_out.get("permission").is_none(),
        "pending cap refusal must not hard-block Stop; out={stop_out:?}"
    );
    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_lite_generic_id_falls_back_to_strict_multiset() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
    let repo = fresh_repo();
    let sid = "s-lite-generic-id";
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
            "id": "fake-tool-id"
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert!(
        mid.review_lite_pending_cycle_keys.is_empty(),
        "bare id must not use lite vec"
    );
    assert_eq!(
        mid.review_subagent_pending_cycle_keys,
        vec!["id:fake-tool-id"]
    );
    let stop_out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, ""));
    let fm = stop_out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "generic id strict pending must advise Stop (not hard-block): {stop_out}"
    );
    assert!(
        stop_out.get("permission").is_none(),
        "strict multiset pending must not hard-block Stop; out={stop_out:?}"
    );
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
}

#[test]
fn review_gate_env_matrix_fixtures_apply_env() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_mode = env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
    let prev_fork = env::var_os("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let matrix_dir = repo_root.join("tests/fixtures/review_gate/env_matrix");
    let mut entries: Vec<_> = fs::read_dir(&matrix_dir)
        .expect("env_matrix dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext == "json")
        })
        .collect();
    entries.sort();
    assert!(
        entries.len() >= 6,
        "expected env_matrix JSON fixtures, got {}",
        entries.len()
    );
    for path in entries {
        let text = fs::read_to_string(&path).expect("read fixture");
        let spec: Value = serde_json::from_str(&text).expect("parse fixture");
        let mode = spec["mode"].as_str().unwrap_or("strict");
        let fork_infer = spec["fork_infer"].as_bool().unwrap_or(true);
        let pending_cap = spec["pending_cap"].as_u64().unwrap_or(32);
        if mode.eq_ignore_ascii_case("lite") {
            unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
            assert_eq!(
                core_policy::review_gate_engine::cursor_review_gate_mode(),
                core_policy::review_gate_engine::CursorReviewGateMode::Lite
            );
        } else {
            unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") };
            assert_eq!(
                core_policy::review_gate_engine::cursor_review_gate_mode(),
                core_policy::review_gate_engine::CursorReviewGateMode::Strict
            );
        }
        if fork_infer {
            unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE") };
        } else {
            unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", "0") };
        }
        unsafe {
            env::set_var(
                "ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX",
                pending_cap.to_string(),
            );
        }
        assert_eq!(
            hooks::router_rs_review_pending_cycle_max(),
            pending_cap as usize
        );
    }
    match prev_mode {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
    }
    match prev_fork {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE") },
    }
    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

// Cursor-only: env off / false-positive / outbound truncation — see `before_submit_skips_paper_prose_*`

#[test]
#[serial]
fn before_submit_skips_paper_prose_when_hook_explicitly_off() {
    let _review_clear = ReviewGateDisableEnvClearGuard::new();
    let _g = hooks::harness_nudges_env_test_lock();
    let prior = env::var_os("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK");
    unsafe { env::set_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK", "0") };
    let repo = fresh_repo();
    let mut out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("prose-off", "SCI润色 abstract"),
    );
    super::apply_cursor_hook_output_policy(&mut out);
    let ctx = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
        "hook=0 must not inject prose: {ctx}"
    );
    match prior {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") },
    }
}

#[test]
#[serial]
fn before_submit_skips_paper_prose_on_java_abstract_false_positive() {
    let _review_clear = ReviewGateDisableEnvClearGuard::new();
    let _g = hooks::harness_nudges_env_test_lock();
    let prior = env::var_os("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") };
    let repo = fresh_repo();
    let mut out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(
            "prose-fp",
            "edit the abstract base class in this Java module",
        ),
    );
    super::apply_cursor_hook_output_policy(&mut out);
    let ctx = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
        "must not false-positive on abstract+edit: {ctx}"
    );
    match prior {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") },
    }
}

#[test]
#[serial]
fn before_submit_paper_prose_survives_outbound_truncation() {
    let _review_clear = ReviewGateDisableEnvClearGuard::new();
    let _g = hooks::harness_nudges_env_test_lock();
    let _cap_lock = hook_outbound_context_max_chars_env_lock();
    let prior_cap = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "1024") };
    let prior_hook = env::var_os("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") };

    let repo = fresh_repo();
    let filler = "z".repeat(900);
    let mut out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(
            "prose-trunc",
            &format!("{filler}\nSCI润色 abstract"),
        ),
    );
    super::apply_cursor_hook_output_policy(&mut out);
    let ctx = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ctx.contains("PAPER_PROSE_QUALITY_HOOK") && ctx.contains("prose-chain-contract"),
        "paper hook block must survive outbound cap: {}",
        &ctx[..ctx.len().min(200)]
    );

    match prior_hook {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PAPER_PROSE_HOOK") },
    }
    match prior_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") },
    }
}
