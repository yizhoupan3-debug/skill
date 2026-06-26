//! Idle-window side-effect: spawn observer-rs analyze when no workers are running (EV-5).

use crate::types::WorkerSessionRecord;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const COOLDOWN_SECS: u64 = 300;
const COOLDOWN_REL_PATH: &str = "artifacts/observer/.last_idle_trigger";

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn workers_are_idle(workers: &[WorkerSessionRecord]) -> bool {
    workers.iter().all(|worker| worker.status != "running")
}

pub struct IdleTriggerResult {
    pub triggered: bool,
    pub status: String,
}

pub fn maybe_trigger_idle_observation(
    repo_cwd: &Path,
    workers: &[WorkerSessionRecord],
    dry_run: bool,
    force: bool,
) -> IdleTriggerResult {
    if !workers_are_idle(workers) {
        return IdleTriggerResult {
            triggered: false,
            status: "workers_active".to_string(),
        };
    }
    if !force && !cooldown_elapsed(repo_cwd) {
        return IdleTriggerResult {
            triggered: false,
            status: "cooldown".to_string(),
        };
    }
    if dry_run {
        return IdleTriggerResult {
            triggered: true,
            status: "dry_run".to_string(),
        };
    }
    match spawn_observer_analyze(repo_cwd) {
        Ok(pid) => {
            let _ = stamp_cooldown(repo_cwd);
            IdleTriggerResult {
                triggered: true,
                status: format!("spawned:{pid}"),
            }
        }
        Err(err) => IdleTriggerResult {
            triggered: false,
            status: format!("spawn_failed:{err}"),
        },
    }
}

fn cooldown_path(repo_cwd: &Path) -> std::path::PathBuf {
    repo_cwd.join(COOLDOWN_REL_PATH)
}

fn cooldown_elapsed(repo_cwd: &Path) -> bool {
    let path = cooldown_path(repo_cwd);
    let Ok(raw) = fs::read_to_string(&path) else {
        return true;
    };
    let Ok(stamp) = raw.trim().parse::<u64>() else {
        return true;
    };
    epoch_seconds().saturating_sub(stamp) >= COOLDOWN_SECS
}

fn stamp_cooldown(repo_cwd: &Path) -> Result<(), String> {
    let path = cooldown_path(repo_cwd);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create observer cooldown dir: {e}"))?;
    }
    let now = epoch_seconds();
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("open cooldown stamp {}: {e}", path.display()))?;
    writeln!(file, "{now}").map_err(|e| format!("write cooldown stamp: {e}"))?;
    Ok(())
}

fn spawn_observer_analyze(repo_cwd: &Path) -> Result<u32, String> {
    let journal = repo_cwd.join("artifacts/telemetry/events.jsonl");
    let output_dir = repo_cwd.join("artifacts/observer");
    let config = repo_cwd.join("configs/observer/observer.toml");
    let mut cmd = Command::new("observer-rs");
    cmd.current_dir(repo_cwd)
        .arg("analyze")
        .arg("--journal")
        .arg(&journal)
        .arg("--output-dir")
        .arg(&output_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if config.is_file() {
        cmd.arg("--config").arg(&config);
    }
    if let Ok(child) = cmd.spawn() {
        return Ok(child.id());
    }
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(repo_cwd)
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            "tools/observer-rs/Cargo.toml",
            "--",
            "analyze",
            "--journal",
        ])
        .arg(&journal)
        .args(["--output-dir"])
        .arg(&output_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if config.is_file() {
        cargo.arg("--config").arg(&config);
    }
    cargo
        .spawn()
        .map(|child| child.id())
        .map_err(|e| format!("spawn observer-rs analyze failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::build_driver_command;
    use serde_json::json;

    fn sample_worker(status: &str) -> WorkerSessionRecord {
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

    #[test]
    fn idle_when_no_running_workers() {
        assert!(workers_are_idle(&[
            sample_worker("queued"),
            sample_worker("blocked_rate_limit"),
        ]));
        assert!(!workers_are_idle(&[
            sample_worker("queued"),
            sample_worker("running"),
        ]));
    }

    #[test]
    fn dry_run_idle_trigger_reports_dry_run() {
        let dir = std::env::temp_dir().join(format!(
            "obs-idle-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let result = maybe_trigger_idle_observation(&dir, &[], true, true);
        assert!(result.triggered);
        assert_eq!(result.status, "dry_run");
        let _ = fs::remove_dir_all(dir);
    }
}
