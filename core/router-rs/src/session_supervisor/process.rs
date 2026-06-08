use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use super::types::DriverCommandSpec;
use super::types::WorkerSessionRecord;

pub struct ProcessLaunchResult {
    pub pid: u32,
    pub log_path: String,
}

pub fn launch_process(
    command: &DriverCommandSpec,
    cwd: &str,
    log_path: &Path,
) -> Result<ProcessLaunchResult, String> {
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create worker log dir failed: {err}"))?;
    }
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| format!("open worker log {} failed: {err}", log_path.display()))?;
    let stderr_log = log_file
        .try_clone()
        .map_err(|err| format!("dup worker log handle failed: {err}"))?;

    let mut cmd = Command::new(&command.binary);
    cmd.args(&command.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_log));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    let child = cmd
        .spawn()
        .map_err(|err| format!("spawn {} failed: {err}", command.binary))?;
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
    true
}

pub fn terminate_process(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Ok(());
    }
    #[cfg(unix)]
    {
        send_signal(pid, libc::SIGTERM)?;
        for _ in 0..50 {
            if !process_is_alive(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        if process_is_alive(pid) {
            send_signal(pid, libc::SIGKILL)?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
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
    } else if matches!(worker.status.as_str(), "running" | "launching") {
        worker.status = "completed".to_string();
    }
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: i32) -> Result<(), String> {
    // SAFETY: pid is stored from a prior successful spawn in this supervisor.
    let rc = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(format!("kill(pid={pid}, signal={signal}) failed: {err}"))
}
