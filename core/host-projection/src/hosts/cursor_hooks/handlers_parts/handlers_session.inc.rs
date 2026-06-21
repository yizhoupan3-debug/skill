// Session / shell / terminal lifecycle: `terminal_observation_cache` dedupes read_dir within one hook
// subprocess; `terminate_stale_terminal_processes_in_dir` may still walk terminals for kill (see env

fn handle_session_start(repo_root: &Path, event: &Value) -> Value {
    maybe_init_session_terminal_ledger(repo_root, event);
    if !hooks::router_rs_operator_inject_globally_enabled() {
        return json!({ "additional_context": "" });
    }
    let ctx = format!("Repo: {}", repo_root.display());
    let ctx = crate::hosts::hook_dispatch::compact_contexts(vec![ctx], crate::hooks::router_rs_sessionstart_context_max_bytes()).unwrap_or_default();
    if let Err(e) = hooks::init_tracker(repo_root) {
        eprintln!("[router-rs warning] init_tracker failed: {e}");
    }
    sweep_session_call_tracker_tmp_orphans(repo_root);
    sweep_stale_hook_state_by_age(repo_root, event);
    json!({ "additional_context": ctx })
}

fn shell_event_command(event: &Value) -> Option<String> {
    first_nonempty_event_str(event, &["command"])
        .split('\n')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn shell_event_cwd(event: &Value) -> Option<PathBuf> {
    let cwd = first_nonempty_event_str(event, &["cwd"]);
    if cwd.trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(cwd))
}

fn maybe_init_session_terminal_ledger(repo_root: &Path, event: &Value) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    let mut baseline: Vec<u32> = observations.iter().map(|o| o.pid).collect();
    baseline.sort_unstable();
    baseline.dedup();
    let ledger = SessionTerminalLedger {
        version: SESSION_TERMINAL_LEDGER_VERSION,
        baseline_pids: baseline,
        owned_pids: Vec::new(),
        pending_shells: Vec::new(),
    };
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn maybe_track_shell_owned_terminals(
    repo_root: &Path,
    event: &Value,
    matched_after_ms: Option<u64>,
) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    if observations.is_empty() {
        return;
    }
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    if ledger.version != SESSION_TERMINAL_LEDGER_VERSION {
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    }
    let baseline: HashSet<u32> = ledger.baseline_pids.iter().copied().collect();
    let mut owned: HashSet<u32> = ledger.owned_pids.iter().copied().collect();
    let cwd_filter = shell_event_cwd(event);
    let cmd_filter = shell_event_command(event).map(|s| normalize_shell_command(&s));
    for obs in observations {
        if baseline.contains(&obs.pid) {
            continue;
        }
        if let Some(t0) = matched_after_ms
            && let Some(sa) = obs.started_at_ms {
                let floor = t0.saturating_sub(SHELL_TERMINAL_TIME_MATCH_SLACK_MS);
                if sa < floor {
                    continue;
                }
            }
        if !obs.cwd.is_absolute() {
            continue;
        }
        if let Some(ref cwd) = cwd_filter {
            let obs_canon = obs.cwd.canonicalize().unwrap_or_else(|_| obs.cwd.clone());
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !obs_canon.starts_with(&cwd_canon) && !cwd_canon.starts_with(&obs_canon) {
                continue;
            }
        }
        if let Some(ref cmd) = cmd_filter {
            let active = obs
                .active_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            let last = obs
                .last_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            if !active.is_empty()
                && !last.is_empty()
                && !active.contains(cmd)
                && !cmd.contains(&active)
                && !last.contains(cmd)
                && !cmd.contains(&last)
            {
                continue;
            }
        }
        owned.insert(obs.pid);
    }
    let mut owned_vec: Vec<u32> = owned.into_iter().collect();
    owned_vec.sort_unstable();
    ledger.owned_pids = owned_vec;
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn handle_before_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    if !cmd_norm.is_empty() {
        let mut ledger = load_session_terminal_ledger(repo_root, event);
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
        let cwd_raw = shell_event_cwd(event)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        ledger.pending_shells.push(PendingShellRecord {
            command_norm: cmd_norm,
            cwd_raw,
            queued_ms: crate::hosts::file_state_lock::now_millis(),
        });
        trim_pending_shell_records(&mut ledger);
        save_session_terminal_ledger(repo_root, event, &ledger);
        // Shell 仍未真正启动 PID 前：仅用 baseline-diff + 指令/cwd 启发式扩展 owned（不关时间窗）。
        maybe_track_shell_owned_terminals(repo_root, event, None);
    }
    json!({
        "continue": true,
        "permission": "allow"
    })
}

fn handle_after_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    let cwd_buf = shell_event_cwd(event);
    let cwd_hint = cwd_buf.as_deref();
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    let matched_after_ms = pop_matching_pending_shell(&mut ledger, &cmd_norm, cwd_hint);
    save_session_terminal_ledger(repo_root, event, &ledger);
    // 配对成功则用 pending 队列时间压低「它仓并发 terminal」误判；配对失败退回纯启发式（None）。
    maybe_track_shell_owned_terminals(repo_root, event, matched_after_ms);
    json!({})
}
fn handle_after_file_edit(repo_root: &Path, event: &Value) -> Value {
    let path = event.get("file_path").and_then(Value::as_str).unwrap_or("");
    let p = PathBuf::from(path);
    if p.extension().and_then(|e| e.to_str()) != Some("rs") {
        return json!({});
    }
    if !p.is_file() {
        return json!({});
    }
    if !core_state::utils::path_guard::path_is_within_repo_root(repo_root, &p) {
        return json!({});
    }
    if which::which("rustfmt").is_err() {
        return json!({});
    }
    let _ = rustfmt_with_timeout(&p, std::time::Duration::from_secs(10));
    json!({})
}

fn rustfmt_with_timeout(path: &Path, timeout: std::time::Duration) -> Option<i32> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    eprintln!(
                        "[router-rs] rustfmt exceeded timeout ({}s) for {}",
                        timeout.as_secs(),
                        path.display()
                    );
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
}

fn handle_session_end(repo_root: &Path, event: &Value) -> Value {
    // **必须先读出 terminal 账本**，再删除本会话 `session-terminals-*.json`：否则账本先被删会导致 `owned_pids` 为空。
    let ledger = load_session_terminal_ledger(repo_root, event);
    let owned_vec = ledger.owned_pids.clone();
    let owned: HashSet<u32> = owned_vec.into_iter().collect();
    // 按本会话 `session_key` 精准删除主状态：须先持锁再删，避免与并发 hook 双 inode 双 flock。
    let mut lock = acquire_state_lock(repo_root, event);
    let sp = state_path(repo_root, event);
    if lock.is_some() {
        let _ = fs::remove_file(&sp);
    } else {
        eprintln!("[router-rs] session_end_state_delete_skipped=lock_unavailable");
    }
    release_state_lock(&mut lock);
    let lock_path = state_lock_path(repo_root, event);
    let _ = fs::remove_file(&lock_path);
    remove_adversarial_loop(repo_root, event);
    let _ = fs::remove_file(session_terminal_ledger_path(repo_root, event));
    // 原子写入孤儿：始终全局清扫（与 session_key 无关）。
    sweep_hook_state_tmp_orphans(repo_root);
    sweep_session_call_tracker_tmp_orphans(repo_root);
    sweep_stale_hook_state_by_age(repo_root, event);
    // 默认不扫其它会话的 review/adversarial/session 文件（同仓库多 Cursor 会话避免互删）。
    // 需清 session_id/cwd 漂移遗留的全目录 stale 时，设 `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1`。
    if hooks::router_rs_hook_state_legacy_full_sweep_enabled() {
        sweep_review_gate_state_dir(repo_root);
    }
    let owned_filter = if terminal_kill_use_scoped_ownership() {
        Some(&owned)
    } else {
        None
    };
    // 默认仅回收本会话 shell 账本登记的 terminal；`ROUTER_RS_CURSOR_TERMINAL_KILL_MODE=legacy` 等恢复全仓 stale 扫描。
    let report = terminate_stale_terminal_processes(repo_root, owned_filter);
    if !report.killed.is_empty() {
        eprintln!(
            "router-rs SessionEnd: terminated {} stale terminal pid(s) {:?} (scanned={}, outside_repo={}, dead={}, not_owned={})",
            report.killed.len(),
            report.killed,
            report.scanned,
            report.skipped_outside_repo,
            report.skipped_dead,
            report.skipped_not_owned,
        );
    }
    if !report.failed.is_empty() {
        eprintln!(
            "router-rs SessionEnd: failed to terminate pid(s): {:?}",
            report.failed
        );
    }
    json!({})
}

fn hook_state_paths_for_session(repo_root: &Path, event: &Value) -> Vec<PathBuf> {
    let key = session_key(event);
    vec![
        state_dir(repo_root).join(format!("review-subagent-{key}.json")),
        state_dir(repo_root).join(format!("review-subagent-{key}.lock")),
        state_dir(repo_root).join(format!("adversarial-loop-{key}.json")),
        state_dir(repo_root).join(format!("session-terminals-{key}.json")),
    ]
}

/// Age sweep may remove `.lock` only when absent, unreadable, or holder PID is dead (aligned with L3 acquire).
fn hook_state_lock_removable_for_sweep(lock_path: &Path) -> bool {
    if !lock_path.is_file() {
        return true;
    }
    let days = hooks::router_rs_hook_state_stale_sweep_days();
    if days > 0 {
        let cutoff_system = SystemTime::now() - std::time::Duration::from_secs(days.saturating_mul(86_400));
        if hook_state_file_mtime_stale(lock_path, cutoff_system) {
            return true;
        }
    }
    let Ok(existing) = fs::read_to_string(lock_path) else {
        return true;
    };
    let Some((pid, ts_ms)) = crate::hosts::file_state_lock::parse_lock_metadata(&existing) else {
        return true;
    };
    if days > 0 {
        let cutoff_ms = crate::hosts::file_state_lock::now_millis().saturating_sub(days.saturating_mul(86_400 * 1000));
        if ts_ms < cutoff_ms {
            return true;
        }
    }
    !crate::hosts::file_state_lock::is_process_alive(pid)
}

fn companion_lock_path_for_hook_state_file(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    if !name.ends_with(".json") {
        return None;
    }
    let stem = name.strip_suffix(".json")?;
    Some(path.with_file_name(format!("{stem}.lock")))
}

fn hook_state_json_updated_at_stale(path: &Path, cutoff: chrono::DateTime<Utc>) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    let Some(updated_at) = v.get("updated_at").and_then(Value::as_str) else {
        return false;
    };
    chrono::DateTime::parse_from_rfc3339(updated_at)
        .ok()
        .map(|dt| dt.with_timezone(&Utc) < cutoff)
        .unwrap_or(false)
}

fn hook_state_file_mtime_stale(path: &Path, cutoff: SystemTime) -> bool {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .is_some_and(|mtime| mtime < cutoff)
}

fn hook_state_file_is_stale(
    path: &Path,
    cutoff_system: SystemTime,
    cutoff_chrono: chrono::DateTime<Utc>,
) -> bool {
    if hook_state_file_mtime_stale(path, cutoff_system) {
        return true;
    }
    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        return hook_state_json_updated_at_stale(path, cutoff_chrono);
    }
    false
}

/// Age-based sweep of owned hook-state files (default 7d). Skips current `session_key` paths.
fn sweep_stale_hook_state_by_age(repo_root: &Path, event: &Value) {
    let days = hooks::router_rs_hook_state_stale_sweep_days();
    if days == 0 {
        return;
    }
    let dir = state_dir(repo_root);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let cutoff_chrono = Utc::now() - chrono::Duration::days(days as i64);
    let cutoff_system =
        SystemTime::now() - std::time::Duration::from_secs(days.saturating_mul(86_400));
    let skip: std::collections::HashSet<PathBuf> = hook_state_paths_for_session(repo_root, event)
        .into_iter()
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !review_gate_state_file_owned_by_module(name) {
            continue;
        }
        if skip.contains(&path) {
            continue;
        }
        if name.ends_with(".lock") {
            if !hook_state_lock_removable_for_sweep(&path) {
                continue;
            }
            let json_path = path.with_file_name(format!(
                "{}.json",
                name.strip_suffix(".lock").unwrap_or(name)
            ));
            if json_path.is_file()
                && !hook_state_file_is_stale(&json_path, cutoff_system, cutoff_chrono)
            {
                continue;
            }
            let _ = fs::remove_file(&path);
            continue;
        }
        if !hook_state_file_is_stale(&path, cutoff_system, cutoff_chrono) {
            continue;
        }
        let lock_ok = companion_lock_path_for_hook_state_file(&path)
            .map(|lp| hook_state_lock_removable_for_sweep(&lp))
            .unwrap_or(true);
        if lock_ok {
            let _ = fs::remove_file(&path);
            if let Some(lp) = companion_lock_path_for_hook_state_file(&path) {
                let _ = fs::remove_file(lp);
            }
        }
    }
}

fn sweep_session_call_tracker_tmp_orphans(repo_root: &Path) {
    let path = repo_root
        .join("artifacts")
        .join("current")
        .join("SESSION_CALL_TRACKER.tmp");
    if path.is_file() {
        let _ = fs::remove_file(path);
    }
}

/// 仅清理由崩溃残留的原子写入 tmp（与 `session_key` 无关）。
fn sweep_hook_state_tmp_orphans(repo_root: &Path) {
    let dir = state_dir(repo_root);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if hook_state_tmp_orphan_filename(name) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// **Legacy / opt-in**：清扫 `.cursor/hook-state/` 下所有由本模块写入的状态文件：
/// 1. review gate 主状态：`review-subagent-<key>.json` / `.lock`；
/// 2. adversarial-loop 主状态：`adversarial-loop-<key>.json`；
/// 3. `session-terminals-<key>.json`；
/// 4. 原子写入孤儿（与 [`sweep_hook_state_tmp_orphans`] 重叠；幂等）。
///
/// 不递归子目录、不删除其它前缀的文件，避免误伤共用目录的其它 hook 状态。
fn sweep_review_gate_state_dir(repo_root: &Path) {
    let dir = state_dir(repo_root);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if review_gate_state_file_owned_by_module(name) {
            let _ = fs::remove_file(&path);
        }
    }
}

fn hook_state_tmp_orphan_filename(name: &str) -> bool {
    if name.starts_with(".tmp-") && name.contains("review-subagent-") {
        return true;
    }
    if name.starts_with(".tmp-adv-loop-") {
        return true;
    }
    false
}

/// 判断 `.cursor/hook-state/` 下的文件名是否由本模块写入。仅识别已知前缀以避免误伤
/// 与本模块共用目录的其它 hook 状态；命名约定与 `state_path` / `state_lock_path` /
/// `adversarial_loop_path` / `save_state` 文件名规则保持一致。
fn review_gate_state_file_owned_by_module(name: &str) -> bool {
    // 主状态：扩展名约束 json|lock，避免误删用户放进来的同前缀其它扩展文件。
    if name.starts_with("review-subagent-") || name.starts_with("adversarial-loop-") {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
        {
            return matches!(ext, "json" | "lock");
        }
        return false;
    }
    if name.starts_with("session-terminals-") {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
        {
            return ext == "json";
        }
        return false;
    }
    hook_state_tmp_orphan_filename(name)
}

// --- SessionEnd: 清理本仓库 Cursor terminal 留下的 stale 子进程 ---
//
// 痛点：`run_terminal_cmd` 等 shell 工具发起的 `cargo test` / python 实验脚本，
// 因工具超时被断开但子进程仍在跑（`block_until_ms: 0` 后台命令同理）。多个会话叠加
// 内存与 CPU 越占越多。SessionEnd 时按 Cursor `terminals/<id>.txt` header 找出
// 仍 active 且 cwd 在本仓库内的 PID，发 SIGTERM → 2s 兜底 SIGKILL（含进程组）。
// 默认开启；`ROUTER_RS_CURSOR_KILL_STALE_TERMINALS=0|false|off|no` 关闭整个步骤。

#[derive(Debug, Default, Clone)]
struct StaleTerminalKillReport {
    scanned: usize,
    killed: Vec<u32>,
    skipped_outside_repo: usize,
    skipped_inactive: usize,
    skipped_dead: usize,
    skipped_not_owned: usize,
    failed: Vec<(u32, String)>,
}

#[derive(Debug, Default, Clone)]
struct TerminalHeader {
    pid: Option<u32>,
    cwd: Option<PathBuf>,
    is_active: bool,
    active_command: Option<String>,
    last_command: Option<String>,
    started_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct TerminalKillTarget {
    pid: u32,
    pgid: Option<u32>,
}

#[derive(Debug, Clone)]
struct TerminalObservation {
    pid: u32,
    cwd: PathBuf,
    active_command: Option<String>,
    last_command: Option<String>,
    started_at_ms: Option<u64>,
}

fn kill_stale_terminals_disabled_by_env() -> bool {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_KILL_STALE_TERMINALS") else {
        return false;
    };
    let t = raw.trim().to_ascii_lowercase();
    matches!(t.as_str(), "0" | "false" | "off" | "no")
}

/// terminals 目录定位优先级：
/// 1. `CURSOR_TERMINALS_DIR`（显式覆盖，便于测试与定制）
/// 2. `$HOME/.cursor/projects/<repo_root 绝对路径替换 / 为 - 去前导 ->/terminals/`
fn resolve_cursor_terminals_dir(repo_root: &Path) -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("CURSOR_TERMINALS_DIR") {
        let p = PathBuf::from(explicit);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    let abs = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let abs_str = abs.to_str()?;
    let trimmed = abs_str.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let mangled = trimmed.replace('/', "-");
    let dir = PathBuf::from(home)
        .join(".cursor")
        .join("projects")
        .join(mangled)
        .join("terminals");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// 解析 Cursor terminals/*.txt 头部 YAML-front-matter（首个 `---` ... `---` 区段）。
/// 仅取关心的字段；缺失字段返回 `None`/默认值，调用方再做过滤。
fn parse_terminal_header(text: &str) -> Option<TerminalHeader> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut header = TerminalHeader::default();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, val)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim().trim_matches('"').trim();
        match key {
            "pid" => header.pid = val.parse().ok(),
            "cwd"
                if !val.is_empty() => {
                    header.cwd = Some(PathBuf::from(val));
                }
            "running_for_ms" => header.is_active = val.parse::<u64>().map_or(!val.is_empty(), |ms| ms > 0),
            "active_command"
                if !val.is_empty() => {
                    header.active_command = Some(val.to_string());
                }
            "last_command"
                if !val.is_empty() => {
                    header.last_command = Some(val.to_string());
                }
            "started_at" => {
                header.started_at_ms = parse_terminal_started_at_unix_ms(val);
            }
            _ => {}
        }
    }
    Some(header)
}

fn normalize_shell_command(raw: &str) -> String {
    raw.trim_matches('"')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn collect_terminal_observations(terminals_dir: &Path) -> Vec<TerminalObservation> {
    terminal_observation_cache::collect_terminal_observations_cached(terminals_dir)
}

#[cfg(unix)]
fn process_pgid(pid: u32) -> Option<u32> {
    // SAFETY: getpgid() reads the kernel's process group table for the given pid.
    // The pid originates from Cursor terminal metadata (a text file header).
    // The worst case is that the pid no longer exists, in which case -1 is returned
    // and the None path is taken. No memory safety concern.
    let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_pgid() -> Option<u32> {
    // SAFETY: getpgrp() returns the process group ID of the calling process.
    // It is a simple read-only syscall with no arguments; it cannot fail or cause UB.
    let pgid = unsafe { libc::getpgrp() };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_ppid() -> Option<u32> {
    // SAFETY: getppid() returns the parent process ID of the calling process.
    // It is a simple read-only syscall with no arguments; it cannot fail or cause UB.
    let ppid = unsafe { libc::getppid() };
    if ppid <= 0 {
        None
    } else {
        Some(ppid as u32)
    }
}

#[cfg(not(unix))]
fn process_pgid(_pid: u32) -> Option<u32> {
    None
}

#[cfg(unix)]
fn signal_pid_or_pgrp(pid: u32, pgid: Option<u32>, signal: libc::c_int) {
    let safe_pgid = match (pgid, current_pgid()) {
        (Some(target), Some(ours)) if target == ours => None,
        (other, _) => other,
    };
    let target = match safe_pgid {
        Some(g) => -(g as libc::pid_t),
        None => pid as libc::pid_t,
    };
    // SAFETY: kill(target, signal) sends a signal to a process or process group.
    // The pid comes from Cursor terminal metadata; the pgid from getpgid() above.
    // The safe_pgid guard prevents accidentally signaling our own process group.
    // kill is a simple syscall; if the target no longer exists it returns -1 (ESRCH),
    // which is harmless. No memory safety concern.
    unsafe {
        let _ = libc::kill(target, signal);
    }
}

/// SIGTERM → 最多等 2s → SIGKILL；优先按进程组信号，覆盖 `cargo test`/`python -m` 这类 fork 子进程的命令。
#[cfg(unix)]
fn terminate_pids_batch(targets: &[TerminalKillTarget]) -> (Vec<u32>, Vec<(u32, String)>) {
    if targets.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Phase 1: SIGTERM fan-out.
    for t in targets {
        signal_pid_or_pgrp(t.pid, t.pgid, libc::SIGTERM);
    }

    // Phase 2: shared wait budget (<= 2s total) instead of per-pid waits.
    let mut remaining: Vec<TerminalKillTarget> = targets.to_vec();
    let mut deadline_slices = 20;
    while deadline_slices > 0 && !remaining.is_empty() {
        thread::sleep(Duration::from_millis(100));
        remaining.retain(|t| crate::hosts::file_state_lock::is_process_alive(t.pid));
        deadline_slices -= 1;
    }

    // Phase 3: SIGKILL for any stragglers.
    if !remaining.is_empty() {
        for t in &remaining {
            signal_pid_or_pgrp(t.pid, t.pgid, libc::SIGKILL);
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Build outputs in a stable, deterministic order (input order).
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for t in targets {
        if !crate::hosts::file_state_lock::is_process_alive(t.pid) {
            killed.push(t.pid);
        } else {
            failed.push((t.pid, format!("SIGKILL did not reap pid={}", t.pid)));
        }
    }
    (killed, failed)
}

#[cfg(not(unix))]
fn terminate_pids_batch(_targets: &[TerminalKillTarget]) -> (Vec<u32>, Vec<(u32, String)>) {
    (Vec::new(), Vec::new())
}

fn terminate_stale_terminal_processes(
    repo_root: &Path,
    owned_pids: Option<&HashSet<u32>>,
) -> StaleTerminalKillReport {
    if kill_stale_terminals_disabled_by_env() {
        return StaleTerminalKillReport::default();
    }
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return StaleTerminalKillReport::default();
    };
    terminate_stale_terminal_processes_in_dir(repo_root, &terminals_dir, owned_pids)
}

/// 纯逻辑形式：调用方提供 terminals 目录（便于测试与显式覆盖路径）。不再读 env 开关。
fn terminate_stale_terminal_processes_in_dir(
    repo_root: &Path,
    terminals_dir: &Path,
    owned_pids: Option<&HashSet<u32>>,
) -> StaleTerminalKillReport {
    let mut report = StaleTerminalKillReport::default();
    let entries = match fs::read_dir(terminals_dir) {
        Ok(e) => e,
        Err(_) => return report,
    };
    let our_pid = std::process::id();
    #[cfg(unix)]
    let our_ppid = current_ppid().unwrap_or(0);
    let abs_repo = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut kill_targets: Vec<TerminalKillTarget> = Vec::new();
    let mut buf = String::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.ends_with(".txt") {
            continue;
        }
        if let Ok(ft) = entry.file_type()
            && !ft.is_file() {
                continue;
            }
        let path = entry.path();
        report.scanned += 1;
        // header 在前 ~4KB 内，避免读整个 terminal 输出文件。
        buf.clear();
        if let Ok(file) = fs::File::open(&path) {
            let _ = file.take(4096).read_to_string(&mut buf);
        }
        let Some(header) = parse_terminal_header(&buf) else {
            continue;
        };
        if !header.is_active {
            report.skipped_inactive += 1;
            continue;
        }
        let Some(pid) = header.pid else {
            continue;
        };
        if pid <= 1 || pid == our_pid {
            continue;
        }
        #[cfg(unix)]
        if pid == our_ppid {
            continue;
        }
        // 范围过滤：cwd 必须落在本仓库内，避免误杀同机器其他项目的 terminal。
        // 先于 is_process_alive：pid 已消失但仍带“外仓 cwd”的文件应记为 skipped_outside_repo，而非 skipped_dead。
        let Some(cwd) = header.cwd.as_ref() else {
            report.skipped_outside_repo += 1;
            continue;
        };
        // 绝不接受相对路径 cwd：相对路径 canonicalize 依赖当前进程 cwd，存在误判扩大范围的风险。
        if !cwd.is_absolute() {
            report.skipped_outside_repo += 1;
            continue;
        }
        // Fast path: avoid canonicalize() for obvious outside-repo paths.
        if !cwd.starts_with(repo_root) && !cwd.starts_with(&abs_repo) {
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !cwd_canon.starts_with(&abs_repo) {
                report.skipped_outside_repo += 1;
                continue;
            }
        } else {
            // Even when the raw path looks inside, normalize once to avoid symlink surprises.
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !cwd_canon.starts_with(&abs_repo) {
                report.skipped_outside_repo += 1;
                continue;
            }
        }
        if !crate::hosts::file_state_lock::is_process_alive(pid) {
            report.skipped_dead += 1;
            continue;
        }
        if let Some(owned) = owned_pids
            && !owned.contains(&pid) {
                report.skipped_not_owned += 1;
                continue;
            }
        kill_targets.push(TerminalKillTarget {
            pid,
            pgid: process_pgid(pid),
        });
    }
    let (killed, failed) = terminate_pids_batch(&kill_targets);
    report.killed.extend(killed);
    report.failed.extend(failed);
    report
}
/// 在 `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` 应急关闭时，仍调用各事件的真实 handler，
/// 但需要把可能附加给用户的督促 (`user-visible nags`) 从这几类事件输出里剥离。
/// 与正常模式相比，纯粹是「输出清洁」差异，handler 行为本身不变。
fn dispatch_disabled_should_strip_nags(lowered: &str) -> bool {
    matches!(
        lowered,
        "posttooluse" | "subagentstart" | "subagentstop" | "precompact"
    )
}

/// ADR-006: strip review-arming fields while keeping unrelated hook-state (shell ledger, etc.).
fn clear_review_gate_hook_state(repo_root: &Path, event: &Value) {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        eprintln!("[router-rs] review_gate_disabled_state_clear_skipped: hook-state lock unavailable");
        return;
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    if !state.core.review_required
        && state.review_subagent_pending_cycle_keys.is_empty()
        && state.review_lite_pending_cycle_keys.is_empty()
    {
        release_state_lock(&mut lock);
        return;
    }
    state.core.review_required = false;
    state.review_subagent_pending_cycle_keys.clear();
    state.review_lite_pending_cycle_keys.clear();
    sync_review_cycle_legacy_fields(&mut state);
    clear_review_gate_escalation_counters(&mut state);
    let _ = save_state(repo_root, event, &mut state);
    release_state_lock(&mut lock);
    eprintln!("[router-rs] review_gate_disabled_state_cleared");
}

pub fn dispatch_cursor_hook_event(
    repo_root: &Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let lowered = event_name.trim().to_lowercase();
    let lowered = lowered.as_str();
    let dispatch_text = crate::hosts::hook_dispatch::extract_prompt_text(payload);
    let disabled = crate::hosts::hook_dispatch::is_review_gate_suppressed("cursor", Some(repo_root), &dispatch_text);

    if disabled && matches!(
        lowered,
        "sessionstart" | "posttooluse" | "subagentstart" | "subagentstop"
    ) {
        clear_review_gate_hook_state(repo_root, payload);
    }

    // my-light suppresses REVIEW_GATE Stop nudge inside handlers; do not skip beforeSubmit
    // entirely — goal drive / pre-goal / MY_* nudges still run for /implementx|verifyx.

    if subtraction::should_noop_subtracted_event(repo_root, lowered) {
        return subtraction::subtracted_event_noop_output(lowered);
    }

    let mut out = match lowered {
        "sessionstart" => handle_session_start(repo_root, payload),
        "beforesubmitprompt" | "userpromptsubmit" => handle_before_submit(repo_root, payload),
        "subagentstart" => handle_subagent_start(repo_root, payload),
        "subagentstop" => handle_subagent_stop(repo_root, payload),
        "posttooluse" => handle_post_tool_use(repo_root, payload),
        "beforeshellexecution" => handle_before_shell_execution(repo_root, payload),
        "aftershellexecution" => handle_after_shell_execution(repo_root, payload),
        "afteragentresponse" => handle_after_agent_response(repo_root, payload),
        "stop" => handle_stop(repo_root, payload),
        "afterfileedit" => handle_after_file_edit(repo_root, payload),
        "precompact" => handle_pre_compact(repo_root, payload),
        "sessionend" => handle_session_end(repo_root, payload),
        _ => json!({}),
    };

    if disabled && dispatch_disabled_should_strip_nags(lowered) {
        strip_cursor_hook_user_visible_nags(&mut out);
    }

    out
}
