use super::types::*;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct TmuxSpawnResult {
    pub(super) pane_id: String,
}

pub(super) fn launch_in_tmux(
    command: &DriverCommandSpec,
    tmux_session: &str,
    cwd: &str,
) -> Result<TmuxSpawnResult, String> {
    run_tmux([
        "new-session",
        "-d",
        "-s",
        tmux_session,
        "-c",
        cwd,
        command.shell_command.as_str(),
    ])?;
    let pane_id =
        tmux_capture_single_line(["display-message", "-p", "-t", tmux_session, "#{pane_id}"])?;
    Ok(TmuxSpawnResult { pane_id })
}

pub(super) fn send_command_to_tmux(tmux_session: &str, shell_command: &str) -> Result<(), String> {
    run_tmux(["send-keys", "-t", tmux_session, shell_command, "C-m"])
}

pub(super) fn tmux_session_exists(tmux_session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", tmux_session])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Run a single  and return the set of active session names.
/// Returns an empty set if tmux is unavailable or has no sessions.
pub(super) fn batch_tmux_sessions() -> HashSet<String> {
    Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn refresh_worker_runtime_state_with_sessions(
    worker: &mut WorkerSessionRecord,
    active_sessions: &HashSet<String>,
) {
    if let Some(session_name) = worker.tmux_session.clone() {
        if active_sessions.contains(&session_name) {
            if worker.status == "launching" || worker.status == "queued" {
                worker.status = "running".to_string();
            }
            if worker.tmux_pane.is_none() {
                if let Ok(pane_id) = tmux_capture_single_line([
                    "display-message",
                    "-p",
                    "-t",
                    session_name.as_str(),
                    "#{pane_id}",
                ]) {
                    worker.tmux_pane = Some(pane_id);
                }
            }
        } else if matches!(worker.status.as_str(), "running" | "launching") {
            worker.status = "completed".to_string();
        }
    }
}

pub(super) fn run_tmux<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run tmux: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

pub(super) fn tmux_capture_single_line<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run tmux: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn load_store(path: &Path) -> Result<SessionSupervisorStore, String> {
    if !path.is_file() {
        return Ok(SessionSupervisorStore {
            schema_version: SESSION_SUPERVISOR_STORE_SCHEMA_VERSION.to_string(),
            version: 1,
            workers: Vec::new(),
        });
    }
    let payload: SessionSupervisorStore = serde_json::from_str(
        &fs::read_to_string(path).map_err(|err| format!("read supervisor store failed: {err}"))?,
    )
    .map_err(|err| format!("parse supervisor store failed: {err}"))?;
    Ok(payload)
}

pub(super) fn save_store(path: &Path, store: &SessionSupervisorStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create supervisor state dir failed: {err}"))?;
    }
    let payload = serde_json::to_string_pretty(store)
        .map_err(|err| format!("serialize supervisor store failed: {err}"))?
        + "\n";
    // Atomic replace: write to sibling tmp, fsync, rename. Mirrors
    // runtime_storage::filesystem_write_text_inner so the supervisor state
    // file gets the same crash-consistency guarantees as background_state.
    let parent = path.parent().ok_or_else(|| {
        format!(
            "supervisor state path {} has no parent directory",
            path.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("supervisor_state");
    let tmp_path = parent.join(format!(".router-rs.{file_name}.{}.tmp", std::process::id()));
    {
        let mut tmp_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|err| {
                format!(
                    "create supervisor state temp file {} failed: {err}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(payload.as_bytes())
            .and_then(|_| tmp_file.sync_all())
            .map_err(|err| {
                let _ = fs::remove_file(&tmp_path);
                format!(
                    "write supervisor state temp payload failed for {}: {err}",
                    tmp_path.display()
                )
            })?;
    }
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "replace supervisor state failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
}

pub(super) fn upsert_worker(store: &mut SessionSupervisorStore, worker: WorkerSessionRecord) {
    if let Some(existing) = store
        .workers
        .iter_mut()
        .find(|existing| existing.worker_id == worker.worker_id)
    {
        *existing = worker;
    } else {
        store.workers.push(worker);
    }
    store.version += 1;
}

pub(super) fn resolve_state_path(payload: &Value) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| format!("read current_dir failed: {err}"))?;
    let default = cwd.join("artifacts/session_supervisor/state.json");
    if let Some(path) = optional_non_empty_string(payload, "state_path") {
        let pb = PathBuf::from(&path);
        let candidate = if pb.is_absolute() {
            pb
        } else {
            cwd.join(&path)
        };
        let temp = std::env::temp_dir();
        let under_cwd = candidate.strip_prefix(&cwd).is_ok();
        let under_tmp = candidate.strip_prefix(&temp).is_ok();
        if !under_cwd && !under_tmp {
            return Err(format!(
                "state_path must be under cwd {} or system temp {}",
                cwd.display(),
                temp.display()
            ));
        }
        Ok(candidate)
    } else {
        Ok(default)
    }
}

pub(super) fn now_from_payload(payload: &Value) -> Result<String, String> {
    if let Some(now) = optional_non_empty_string(payload, "now") {
        parse_rfc3339(&now)?;
        return Ok(now);
    }
    Ok(Utc::now().to_rfc3339())
}

pub(super) fn add_seconds_rfc3339(now: &str, seconds: i64) -> Result<String, String> {
    let dt = parse_rfc3339(now)?;
    Ok((dt + Duration::seconds(seconds)).to_rfc3339())
}

pub(super) fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| format!("invalid RFC3339 timestamp {value:?}: {err}"))
}

pub(super) fn required_non_empty_string(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    optional_non_empty_string(payload, key)
        .ok_or_else(|| format!("{context} requires a non-empty {key}"))
}

pub(super) fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .and_then(|value| {
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
}

pub(super) fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

pub(super) fn optional_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

pub(super) fn push_event(
    worker: &mut WorkerSessionRecord,
    event: &str,
    status: &str,
    timestamp: &str,
    detail: Option<String>,
) {
    worker.events.push(WorkerEvent {
        event: event.to_string(),
        status: status.to_string(),
        timestamp: timestamp.to_string(),
        detail,
    });
}

pub(super) fn sanitize_segment(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "worker".to_string()
    } else {
        slug
    }
}

pub(super) fn shell_join(binary: &str, args: &[String]) -> String {
    let mut parts = vec![shell_escape(binary)];
    parts.extend(args.iter().map(|arg| shell_escape(arg)));
    parts.join(" ")
}

pub(super) fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}
