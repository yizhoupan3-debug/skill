use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use core_errors::FrameworkError;
use core_state_utils::atomic_write::write_atomic_text;
use framework_runtime::process_utils;

use crate::types::{AgentHealthEntry, AgentHealthStore, AgentSpawnOrigin, DriverCommandSpec, WorkerSessionRecord};

pub struct ProcessLaunchResult {
    pub pid: u32,
    pub log_path: String,
}

pub fn launch_process(
    command: &DriverCommandSpec,
    cwd: &str,
    log_path: &Path,
) -> Result<ProcessLaunchResult, FrameworkError> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let stderr_log = log_file.try_clone()?;

    let mut cmd = Command::new(&command.binary);
    cmd.args(&command.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: pre_exec runs in the forked child just before exec.
        // setsid() creates a new session and detaches the child from the parent's
        // controlling terminal, which is the intended behavior for a daemon-like
        // worker process. apply_subprocess_rlimits() calls setrlimit which is
        // async-signal-safe. The closure does not touch parent state;
        // Rust's pre_exec contract guarantees it runs in a single-threaded child.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                process_utils::apply_subprocess_rlimits()?;
                Ok(())
            });
        }
    }

    let child = cmd.spawn()?;
    let pid = child.id();

    Ok(ProcessLaunchResult {
        pid,
        log_path: log_path.display().to_string(),
    })
}

#[cfg(unix)]
pub fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if reap_child_if_exited(pid) {
        return false;
    }
    // SAFETY: signal 0 is a POSIX existence check that delivers no signal.
    unsafe {
        let rc = libc::kill(pid as libc::pid_t, 0);
        if rc == 0 {
            return true;
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ESRCH) => false,
            Some(libc::EPERM) => true,
            _ => true,
        }
    }
}

#[cfg(not(unix))]
pub fn process_is_alive(_pid: u32) -> bool {
    // No platform support for process-liveness checks on non-Unix;
    // conservatively report dead so the supervisor doesn't hang.
    false
}

/// Direct `kill(pid, 0)` probe — bypasses `reap_child_if_exited` to avoid
/// ECHILD false-positives when the child's children die but the shell is still alive.
#[cfg(unix)]
fn kill_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: kill(pid, 0) is a POSIX existence check that delivers no signal.
    // The pid is validated to be non-zero above; the return value is checked for
    // ESRCH (no such process) and EPERM (exists but no permission). No UB path.
    unsafe {
        let rc = libc::kill(pid as libc::pid_t, 0);
        if rc == 0 {
            return true;
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ESRCH) => false,
            Some(libc::EPERM) => true,
            _ => true,
        }
    }
}

#[cfg(not(unix))]
fn kill_pid_alive(_pid: u32) -> bool {
    false
}

pub fn terminate_process(pid: u32) -> Result<(), FrameworkError> {
    if pid == 0 {
        return Ok(());
    }
    #[cfg(unix)]
    {
        // Phase 1: SIGTERM with 500ms budget (5 × 100ms)
        send_signal_to_pgrp(pid, libc::SIGTERM)?;
        for _ in 0..5 {
            let _ = reap_child_if_exited(pid);
            if !kill_pid_alive(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        if !kill_pid_alive(pid) {
            return Ok(());
        }
        // Phase 2: SIGKILL with 1.5s budget (15 × 100ms)
        send_signal_to_pgrp(pid, libc::SIGKILL)?;
        for _ in 0..15 {
            let _ = reap_child_if_exited(pid);
            if !kill_pid_alive(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        // Last resort: non-blocking reap so kill(0) won't see a zombie.
        let _ = wait_for_child(pid, false);
        Ok(())
    }
    #[cfg(not(unix))]
    {
        // Windows process termination is not yet implemented.
        // In the future this should use something like:
        //   std::process::Command::new("taskkill")
        //       .args(["/F", "/T", "/PID", &pid.to_string()])
        //       .output()
        //       .ok();
        tracing::warn!("terminate_process called for pid={pid} on non-Unix (no-op)");
        let _ = pid;
        Ok(())
    }
}

pub fn reconcile_process_state(worker: &mut WorkerSessionRecord) {
    let Some(pid) = worker.pid else {
        return;
    };
    if process_is_alive(pid) {
        if worker.status == "launching" || worker.status == "queued" {
            worker.status = "running".to_string();
        }
    } else if matches!(worker.status.as_str(), "running" | "launching" | "queued") {
        worker.status = "completed".to_string();
    } else if matches!(worker.status.as_str(), "blocked_rate_limit") {
        worker.status = "interrupted".to_string();
    }
}

#[cfg(unix)]
fn send_signal_to_pgrp(pid: u32, signal: i32) -> Result<(), FrameworkError> {
    // setsid() in launch_process makes the worker a session leader; prefer pgid kill
    // so shell-spawned children (e.g. smoke-shell's sleep loop) are terminated too.
    // SAFETY: getpgid() reads the kernel's process group table for the given pid.
    // The pid comes from a prior successful spawn in this supervisor. The worst case
    // is that the pid no longer exists, in which case -1 is returned and handled below.
    let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
    let target = if pgid > 0 {
        -(pgid as libc::pid_t)
    } else {
        pid as libc::pid_t
    };
    // SAFETY: pid/pgid come from a prior successful spawn in this supervisor.
    let rc = unsafe { libc::kill(target, signal) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(FrameworkError::session(format!(
        "kill(target={target}, signal={signal}) failed: {err}"
    )))
}

/// Reap a child that has exited (including zombie). Returns true when the pid is gone.
#[cfg(unix)]
fn reap_child_if_exited(pid: u32) -> bool {
    wait_for_child(pid, false)
}

#[cfg(unix)]
fn wait_for_child(pid: u32, block: bool) -> bool {
    let mut status: i32 = 0;
    let flags = if block { 0 } else { libc::WNOHANG };
    loop {
        // SAFETY: pid is from a prior spawn where this process remains the parent until reaped.
        let waited = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, flags) };
        if waited > 0 {
            return true;
        }
        if waited == 0 {
            return false;
        }
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ESRCH) | Some(libc::ECHILD) => return true,
            Some(libc::EINTR) => continue,
            _ => return false,
        }
    }
}

// ── Agent health tracking ─────────────────────────────────────────

const AGENT_HEALTH_REL_PATH: &str = ".framework/agent-health/registry.json";

fn agent_health_path(repo_root: &Path) -> PathBuf {
    repo_root.join(AGENT_HEALTH_REL_PATH)
}

fn load_agent_health_raw(path: &Path) -> Result<AgentHealthStore, FrameworkError> {
    if !path.is_file() {
        return Ok(AgentHealthStore::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_agent_health_raw(path: &Path, store: &AgentHealthStore) -> Result<(), FrameworkError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(store)?;
    write_atomic_text(path, &payload)
}

/// Acquire a cross-process lock on the agent health registry, then call `f`
/// with an `&mut AgentHealthStore`. The lock is held for the entire cycle.
fn with_agent_health<F, T>(repo_root: &Path, f: F) -> Result<T, FrameworkError>
where
    F: FnOnce(&mut AgentHealthStore) -> Result<T, FrameworkError>,
{
    let path = agent_health_path(repo_root);
    let _lock = rt_storage::runtime_storage::acquire_runtime_path_lock(&path)
        .map_err(|e| FrameworkError::lock(e))?;
    let mut store = load_agent_health_raw(&path)?;
    let result = f(&mut store)?;
    save_agent_health_raw(&path, &store)?;
    Ok(result)
}

/// Register an agent as alive in the health registry.
pub fn register_agent_alive(
    repo_root: &Path,
    agent_id: &str,
    host_id: &str,
    spawned_by_tool: &str,
    now: &str,
) -> Result<(), FrameworkError> {
    with_agent_health(repo_root, |store| {
        store.agents.retain(|a| a.agent_id != agent_id);
        store.agents.push(AgentHealthEntry {
            agent_id: agent_id.to_string(),
            host_id: host_id.to_string(),
            status: "running".to_string(),
            spawned_at: now.to_string(),
            completed_at: None,
            error: None,
            spawned_by_tool: AgentSpawnOrigin::from(spawned_by_tool),
        });
        Ok(())
    })
}

/// Mark an agent as completed/failed/interrupted.
pub fn unregister_agent(
    repo_root: &Path,
    agent_id: &str,
    terminal_status: &str,
    error: Option<&str>,
    now: &str,
) -> Result<(), FrameworkError> {
    with_agent_health(repo_root, |store| {
        let mut found = false;
        for agent in &mut store.agents {
            if agent.agent_id == agent_id && agent.is_alive() {
                agent.status = terminal_status.to_string();
                agent.completed_at = Some(now.to_string());
                agent.error = error.map(String::from);
                found = true;
                break;
            }
        }
        if !found {
            store.agents.push(AgentHealthEntry {
                agent_id: agent_id.to_string(),
                host_id: String::new(),
                status: terminal_status.to_string(),
                spawned_at: String::new(),
                completed_at: Some(now.to_string()),
                error: error.map(String::from),
                spawned_by_tool: AgentSpawnOrigin::from("unmanaged"),
            });
        }
        Ok(())
    })
}

/// Reap all stale agents (completed/failed/interrupted) outside a retention window.
pub fn reap_stale_agents(
    repo_root: &Path,
    retention_seconds: i64,
) -> Result<usize, FrameworkError> {
    with_agent_health(repo_root, |store| {
        let before = store.agents.len();
        let deadline = framework_core::time::now_iso();
        store.agents.retain(|a| {
            if !a.is_terminal() {
                return true;
            }
            if let Some(ref completed) = a.completed_at
                && let (Ok(c_dt), Ok(d_dt)) = (
                    chrono::DateTime::parse_from_rfc3339(completed),
                    chrono::DateTime::parse_from_rfc3339(&deadline),
                )
            {
                return d_dt.signed_duration_since(c_dt).num_seconds() < retention_seconds;
            }
            false
        });
        let reaped = before - store.agents.len();
        Ok(reaped)
    })
}

/// List all currently running agents.
#[cfg(test)]
pub fn list_running_agents(repo_root: &Path) -> Result<Vec<AgentHealthEntry>, FrameworkError> {
    with_agent_health(repo_root, |store| {
        let mut running: Vec<_> = store
            .agents
            .iter()
            .filter(|a| a.is_alive())
            .cloned()
            .collect();
        running.sort_by(|a, b| b.spawned_at.cmp(&a.spawned_at));
        Ok(running)
    })
}
