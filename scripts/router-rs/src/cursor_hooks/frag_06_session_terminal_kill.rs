fn handle_after_file_edit(repo_root: &Path, event: &Value) -> Value {
    let path = event.get("file_path").and_then(Value::as_str).unwrap_or("");
    let p = PathBuf::from(path);
    if p.extension().and_then(|e| e.to_str()) != Some("rs") {
        return json!({});
    }
    if !p.is_file() {
        return json!({});
    }
    if !crate::path_guard::path_is_within_repo_root(repo_root, &p) {
        return json!({});
    }
    if which::which("rustfmt").is_err() {
        return json!({});
    }
    let _ = std::process::Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(&p)
        .status();
    json!({})
}

fn handle_session_end(repo_root: &Path, event: &Value) -> Value {
    // **必须先读出 terminal 账本**，再删除本会话 `session-terminals-*.json`：否则账本先被删会导致 `owned_pids` 为空。
    let ledger = load_session_terminal_ledger(repo_root, event);
    let owned_vec = ledger.owned_pids.clone();
    let owned: HashSet<u32> = owned_vec.into_iter().collect();
    // 按本会话 `session_key` 精准删除主状态 / lock / adversarial-loop / terminal 账本。
    let _ = fs::remove_file(state_path(repo_root, event));
    let _ = fs::remove_file(state_lock_path(repo_root, event));
    remove_adversarial_loop(repo_root, event);
    let _ = fs::remove_file(session_terminal_ledger_path(repo_root, event));
    // 原子写入孤儿：始终全局清扫（与 session_key 无关）。
    sweep_hook_state_tmp_orphans(repo_root);
    // 默认不扫其它会话的 review/adversarial/session 文件（同仓库多 Cursor 会话避免互删）。
    // 需清 session_id/cwd 漂移遗留的全目录 stale 时，设 `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1`。
    if crate::router_env_flags::router_rs_cursor_hook_state_legacy_full_sweep_enabled() {
        sweep_review_gate_state_dir(repo_root);
    }
    let owned_filter = if cursor_terminal_kill_use_scoped_ownership() {
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

fn cursor_kill_stale_terminals_disabled_by_env() -> bool {
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
            "cwd" => {
                if !val.is_empty() {
                    header.cwd = Some(PathBuf::from(val));
                }
            }
            "running_for_ms" => header.is_active = !val.is_empty(),
            "active_command" => {
                if !val.is_empty() {
                    header.active_command = Some(val.to_string());
                }
            }
            "last_command" => {
                if !val.is_empty() {
                    header.last_command = Some(val.to_string());
                }
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
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(terminals_dir) else {
        return out;
    };
    let mut buf = String::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        buf.clear();
        if let Ok(file) = fs::File::open(&path) {
            let _ = file.take(4096).read_to_string(&mut buf);
        }
        let Some(header) = parse_terminal_header(&buf) else {
            continue;
        };
        let (Some(pid), Some(cwd)) = (header.pid, header.cwd) else {
            continue;
        };
        out.push(TerminalObservation {
            pid,
            cwd,
            active_command: header.active_command,
            last_command: header.last_command,
            started_at_ms: header.started_at_ms,
        });
    }
    out
}

#[cfg(unix)]
fn process_pgid(pid: u32) -> Option<u32> {
    let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_pgid() -> Option<u32> {
    let pgid = unsafe { libc::getpgrp() };
    if pgid <= 0 {
        None
    } else {
        Some(pgid as u32)
    }
}

#[cfg(unix)]
fn current_ppid() -> Option<u32> {
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
        remaining.retain(|t| is_process_alive(t.pid));
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
        if !is_process_alive(t.pid) {
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
    if cursor_kill_stale_terminals_disabled_by_env() {
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
        if let Ok(ft) = entry.file_type() {
            if !ft.is_file() {
                continue;
            }
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
        if !is_process_alive(pid) {
            report.skipped_dead += 1;
            continue;
        }
        if let Some(owned) = owned_pids {
            if !owned.contains(&pid) {
                report.skipped_not_owned += 1;
                continue;
            }
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
