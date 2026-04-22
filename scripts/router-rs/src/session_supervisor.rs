use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub const SESSION_SUPERVISOR_SCHEMA_VERSION: &str = "router-rs-session-supervisor-response-v1";
pub const SESSION_SUPERVISOR_STORE_SCHEMA_VERSION: &str = "router-rs-session-supervisor-store-v1";
pub const SESSION_SUPERVISOR_AUTHORITY: &str = "rust-session-supervisor";
const DEFAULT_BACKOFF_SECONDS: i64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionSupervisorStore {
    schema_version: String,
    version: u64,
    workers: Vec<WorkerSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerEvent {
    event: String,
    status: String,
    timestamp: String,
    detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerSessionRecord {
    worker_id: String,
    host: String,
    driver_id: String,
    cwd: String,
    worktree_path: Option<String>,
    status: String,
    tmux_session: Option<String>,
    tmux_pane: Option<String>,
    attached_session_id: Option<String>,
    resume_target: Option<String>,
    resume_mode: Option<String>,
    blocked_reason: Option<String>,
    next_resume_at: Option<String>,
    retry_policy: Value,
    prompt: Option<String>,
    launch_command: DriverCommandSpec,
    resume_command: Option<DriverCommandSpec>,
    native_tmux_requested: bool,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    metadata: Value,
    events: Vec<WorkerEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCommandSpec {
    driver_id: String,
    binary: String,
    args: Vec<String>,
    shell_command: String,
    supports_resume: bool,
    supports_native_tmux: bool,
    supports_external_tmux: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockClassification {
    host: String,
    blocked_reason: String,
    status: String,
    matched_text: Option<String>,
    backoff_seconds: i64,
}

pub fn handle_session_supervisor_operation(payload: Value) -> Result<Value, String> {
    let operation = required_non_empty_string(&payload, "operation", "session supervisor")?;
    let state_path = resolve_state_path(&payload)?;
    let dry_run = optional_bool(&payload, "dry_run").unwrap_or(false);
    let now = now_from_payload(&payload)?;
    let mut store = load_store(&state_path)?;

    match operation.as_str() {
        "launch" => {
            let worker = launch_worker(&payload, &mut store, dry_run, &now)?;
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "dry_run": dry_run,
                "worker": worker,
            }))
        }
        "inspect" => {
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")?;
            let worker_snapshot = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| format!("Unknown supervisor worker_id: {worker_id}"))?;
                refresh_worker_runtime_state(worker);
                worker.clone()
            };
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "worker": worker_snapshot,
            }))
        }
        "list" => {
            for worker in &mut store.workers {
                refresh_worker_runtime_state(worker);
            }
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "workers": store.workers,
            }))
        }
        "terminate" => {
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")?;
            let (worker_snapshot, terminated) = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| format!("Unknown supervisor worker_id: {worker_id}"))?;
                let terminated = terminate_worker(worker, dry_run, &now)?;
                (worker.clone(), terminated)
            };
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "dry_run": dry_run,
                "worker": worker_snapshot,
                "terminated": terminated,
            }))
        }
        "classify_block" => {
            let host = required_non_empty_string(&payload, "host", "session supervisor")?;
            let evidence_text =
                required_non_empty_string(&payload, "evidence_text", "session supervisor")?;
            let classification = classify_rate_limit_block(&host, &evidence_text)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": false,
                "classification": classification,
            }))
        }
        "mark_blocked" => {
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")?;
            let (worker_snapshot, classification) = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| format!("Unknown supervisor worker_id: {worker_id}"))?;
                let classification = mark_worker_blocked(worker, &payload, &now)?;
                (worker.clone(), classification)
            };
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "worker": worker_snapshot,
                "classification": classification,
            }))
        }
        "resume_due" => {
            let mut resumed_workers = Vec::new();
            let mut failed_workers = Vec::new();
            for worker in &mut store.workers {
                if !worker_ready_for_resume(worker, &now)? {
                    continue;
                }
                match resume_worker(worker, dry_run, &now) {
                    Ok(action) => resumed_workers.push(json!({
                        "worker_id": worker.worker_id,
                        "status": worker.status,
                        "action": action,
                        "worker": worker,
                    })),
                    Err(err) => {
                        worker.status = "failed".to_string();
                        worker.last_error = Some(err.clone());
                        worker.updated_at = now.clone();
                        push_event(worker, "resume_failed", "failed", &now, Some(err.clone()));
                        failed_workers.push(json!({
                            "worker_id": worker.worker_id,
                            "status": worker.status,
                            "error": err,
                            "worker": worker,
                        }));
                    }
                }
            }
            save_store(&state_path, &store)?;
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "dry_run": dry_run,
                "resumed_workers": resumed_workers,
                "failed_workers": failed_workers,
            }))
        }
        other => Err(format!("Unsupported session supervisor operation: {other}")),
    }
}

fn launch_worker(
    payload: &Value,
    store: &mut SessionSupervisorStore,
    dry_run: bool,
    now: &str,
) -> Result<WorkerSessionRecord, String> {
    let host = required_non_empty_string(payload, "host", "session supervisor")?;
    let cwd = required_non_empty_string(payload, "cwd", "session supervisor")?;
    let prompt = optional_non_empty_string(payload, "prompt");
    let resume_target = optional_non_empty_string(payload, "resume_target");
    let resume_mode = optional_non_empty_string(payload, "resume_mode")
        .unwrap_or_else(|| default_resume_mode(&host).to_string());
    let worker_id = optional_non_empty_string(payload, "worker_id").unwrap_or_else(|| {
        format!(
            "{}-{}",
            sanitize_segment(&host),
            Utc::now().timestamp_millis()
        )
    });
    let tmux_session = optional_non_empty_string(payload, "tmux_session")
        .unwrap_or_else(|| format!("supervisor-{}", sanitize_segment(&worker_id)));
    let native_tmux_requested = optional_bool(payload, "native_tmux").unwrap_or(false);
    let launch_command = build_driver_command(
        &host,
        &cwd,
        prompt.clone(),
        resume_target.clone(),
        &resume_mode,
        false,
        native_tmux_requested,
        optional_non_empty_string(payload, "worktree_name"),
    )?;
    let resume_command = Some(build_driver_command(
        &host,
        &cwd,
        None,
        resume_target.clone(),
        &resume_mode,
        true,
        native_tmux_requested,
        optional_non_empty_string(payload, "worktree_name"),
    )?);
    let retry_policy = payload
        .get("retry_policy")
        .cloned()
        .unwrap_or_else(|| json!({"kind": "rate_limit_auto_resume", "default_backoff_seconds": DEFAULT_BACKOFF_SECONDS}));
    let metadata = payload
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    let mut worker = WorkerSessionRecord {
        worker_id,
        host: host.clone(),
        driver_id: driver_id_for_host(&host).to_string(),
        cwd: cwd.clone(),
        worktree_path: optional_non_empty_string(payload, "worktree_path"),
        status: "launching".to_string(),
        tmux_session: Some(tmux_session.clone()),
        tmux_pane: None,
        attached_session_id: optional_non_empty_string(payload, "attached_session_id"),
        resume_target,
        resume_mode: Some(resume_mode),
        blocked_reason: None,
        next_resume_at: None,
        retry_policy,
        prompt,
        launch_command,
        resume_command,
        native_tmux_requested,
        last_error: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        metadata,
        events: Vec::new(),
    };

    if dry_run {
        worker.status = "queued".to_string();
        push_event(
            &mut worker,
            "launch_planned",
            "queued",
            now,
            Some("dry_run launch planned".to_string()),
        );
    } else {
        let spawn = launch_in_tmux(&worker.launch_command, &tmux_session, &cwd)?;
        worker.tmux_pane = Some(spawn.pane_id);
        worker.status = "running".to_string();
        worker.updated_at = now.to_string();
        push_event(
            &mut worker,
            "launched",
            "running",
            now,
            Some(format!("tmux session {}", tmux_session)),
        );
    }

    upsert_worker(store, worker.clone());
    Ok(worker)
}

fn mark_worker_blocked(
    worker: &mut WorkerSessionRecord,
    payload: &Value,
    now: &str,
) -> Result<BlockClassification, String> {
    let classification =
        if let Some(evidence_text) = optional_non_empty_string(payload, "evidence_text") {
            classify_rate_limit_block(&worker.host, &evidence_text)?
        } else {
            BlockClassification {
                host: worker.host.clone(),
                blocked_reason: optional_non_empty_string(payload, "blocked_reason")
                    .unwrap_or_else(|| "rate_limit".to_string()),
                status: "blocked_rate_limit".to_string(),
                matched_text: None,
                backoff_seconds: optional_i64(payload, "backoff_seconds")
                    .unwrap_or(DEFAULT_BACKOFF_SECONDS),
            }
        };

    worker.status = classification.status.clone();
    worker.blocked_reason = Some(classification.blocked_reason.clone());
    worker.next_resume_at = Some(add_seconds_rfc3339(now, classification.backoff_seconds)?);
    worker.last_error = classification.matched_text.clone();
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "blocked",
        &classification.status,
        now,
        Some(format!(
            "next resume scheduled after {} seconds",
            classification.backoff_seconds
        )),
    );
    Ok(classification)
}

fn resume_worker(
    worker: &mut WorkerSessionRecord,
    dry_run: bool,
    now: &str,
) -> Result<String, String> {
    let command = worker
        .resume_command
        .clone()
        .ok_or_else(|| format!("Worker {} has no resume command", worker.worker_id))?;
    let session_name = worker
        .tmux_session
        .clone()
        .unwrap_or_else(|| format!("supervisor-{}", sanitize_segment(&worker.worker_id)));

    if dry_run {
        worker.status = "resume_scheduled".to_string();
        worker.updated_at = now.to_string();
        push_event(
            worker,
            "resume_planned",
            "resume_scheduled",
            now,
            Some("dry_run resume planned".to_string()),
        );
        return Ok("dry_run".to_string());
    }

    if tmux_session_exists(&session_name) {
        send_command_to_tmux(&session_name, &command.shell_command)?;
        worker.status = "running".to_string();
        worker.blocked_reason = None;
        worker.next_resume_at = None;
        worker.updated_at = now.to_string();
        push_event(
            worker,
            "resumed",
            "running",
            now,
            Some("reused existing tmux session".to_string()),
        );
        return Ok("send_keys".to_string());
    }

    let spawn = launch_in_tmux(&command, &session_name, &worker.cwd)?;
    worker.tmux_session = Some(session_name.clone());
    worker.tmux_pane = Some(spawn.pane_id);
    worker.status = "running".to_string();
    worker.blocked_reason = None;
    worker.next_resume_at = None;
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "resumed",
        "running",
        now,
        Some(format!("created tmux session {}", session_name)),
    );
    Ok("new_session".to_string())
}

fn terminate_worker(
    worker: &mut WorkerSessionRecord,
    dry_run: bool,
    now: &str,
) -> Result<bool, String> {
    if dry_run {
        worker.status = "interrupted".to_string();
        worker.updated_at = now.to_string();
        push_event(
            worker,
            "terminate_planned",
            "interrupted",
            now,
            Some("dry_run terminate planned".to_string()),
        );
        return Ok(true);
    }

    if let Some(session_name) = worker.tmux_session.clone() {
        if tmux_session_exists(&session_name) {
            run_tmux(["kill-session", "-t", session_name.as_str()])?;
        }
    }
    worker.status = "interrupted".to_string();
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "terminated",
        "interrupted",
        now,
        Some("tmux session terminated".to_string()),
    );
    Ok(true)
}

fn worker_ready_for_resume(worker: &WorkerSessionRecord, now: &str) -> Result<bool, String> {
    if !matches!(
        worker.status.as_str(),
        "blocked_rate_limit" | "resume_scheduled"
    ) {
        return Ok(false);
    }
    let Some(next_resume_at) = worker.next_resume_at.as_deref() else {
        return Ok(false);
    };
    let next_time = parse_rfc3339(next_resume_at)?;
    Ok(parse_rfc3339(now)? >= next_time)
}

pub fn classify_rate_limit_block(
    host: &str,
    evidence_text: &str,
) -> Result<BlockClassification, String> {
    let lowered = host.trim().to_ascii_lowercase();
    let mut matched = match lowered.as_str() {
        "claude" | "claude-code" => detect_rate_limit(evidence_text, claude_rate_limit_patterns()),
        "codex" | "codex-cli" => detect_rate_limit(evidence_text, codex_rate_limit_patterns()),
        other => {
            return Err(format!(
                "Unsupported session supervisor host for rate-limit classification: {other}"
            ))
        }
    };
    if let Some(classification) = matched.as_mut() {
        classification.host = lowered;
    }
    matched.ok_or_else(|| {
        format!(
            "Could not classify a rate-limit block for host {} from the provided evidence.",
            host
        )
    })
}

fn detect_rate_limit(evidence_text: &str, patterns: &[Regex]) -> Option<BlockClassification> {
    let duration_re = duration_pattern();
    for regex in patterns {
        if let Some(matched) = regex.find(evidence_text) {
            let backoff_seconds = duration_re
                .captures(evidence_text)
                .and_then(|caps| parse_duration_caps(&caps))
                .unwrap_or(DEFAULT_BACKOFF_SECONDS);
            return Some(BlockClassification {
                host: String::new(),
                blocked_reason: "rate_limit".to_string(),
                status: "blocked_rate_limit".to_string(),
                matched_text: Some(matched.as_str().to_string()),
                backoff_seconds,
            });
        }
    }
    None
}

fn build_driver_command(
    host: &str,
    cwd: &str,
    prompt: Option<String>,
    resume_target: Option<String>,
    resume_mode: &str,
    resume_only: bool,
    native_tmux_requested: bool,
    worktree_name: Option<String>,
) -> Result<DriverCommandSpec, String> {
    let lowered = host.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "claude" | "claude-code" => {
            let mut args = Vec::new();
            if native_tmux_requested && !resume_only {
                args.push("--tmux=classic".to_string());
            }
            if let Some(name) = worktree_name.filter(|_| !resume_only) {
                args.push("--worktree".to_string());
                args.push(name);
            }
            if resume_only {
                if let Some(target) = resume_target.or_else(|| Some("".to_string())) {
                    if target.is_empty() || resume_mode == "continue" {
                        args.push("--continue".to_string());
                    } else {
                        args.push("--resume".to_string());
                        args.push(target);
                    }
                } else {
                    args.push("--continue".to_string());
                }
            } else if let Some(prompt) = prompt {
                args.push(prompt);
            }
            Ok(DriverCommandSpec {
                driver_id: "claude_driver".to_string(),
                binary: "claude".to_string(),
                shell_command: shell_join("claude", &args),
                args,
                supports_resume: true,
                supports_native_tmux: true,
                supports_external_tmux: true,
            })
        }
        "codex" | "codex-cli" => {
            let mut args = vec!["-C".to_string(), cwd.to_string()];
            if resume_only {
                args.push("resume".to_string());
                if let Some(target) = resume_target {
                    if target == "last" || resume_mode == "last" {
                        args.push("--last".to_string());
                    } else {
                        args.push(target);
                    }
                } else {
                    args.push("--last".to_string());
                }
            } else if let Some(prompt) = prompt {
                args.push(prompt);
            }
            Ok(DriverCommandSpec {
                driver_id: "codex_driver".to_string(),
                binary: "codex".to_string(),
                shell_command: shell_join("codex", &args),
                args,
                supports_resume: true,
                supports_native_tmux: false,
                supports_external_tmux: true,
            })
        }
        other => Err(format!("Unsupported session supervisor host: {other}")),
    }
}

fn driver_id_for_host(host: &str) -> &'static str {
    match host.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude-code" => "claude_driver",
        "codex" | "codex-cli" => "codex_driver",
        _ => "unknown_driver",
    }
}

fn default_resume_mode(host: &str) -> &'static str {
    match host.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude-code" => "continue",
        _ => "last",
    }
}

#[derive(Debug)]
struct TmuxSpawnResult {
    pane_id: String,
}

fn launch_in_tmux(
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

fn send_command_to_tmux(tmux_session: &str, shell_command: &str) -> Result<(), String> {
    run_tmux(["send-keys", "-t", tmux_session, shell_command, "C-m"])
}

fn tmux_session_exists(tmux_session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", tmux_session])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn refresh_worker_runtime_state(worker: &mut WorkerSessionRecord) {
    if let Some(session_name) = worker.tmux_session.clone() {
        if tmux_session_exists(&session_name) {
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

fn run_tmux<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run tmux: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

fn tmux_capture_single_line<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run tmux: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn load_store(path: &Path) -> Result<SessionSupervisorStore, String> {
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

fn save_store(path: &Path, store: &SessionSupervisorStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create supervisor state dir failed: {err}"))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(store)
            .map_err(|err| format!("serialize supervisor store failed: {err}"))?
            + "\n",
    )
    .map_err(|err| format!("write supervisor store failed: {err}"))?;
    Ok(())
}

fn upsert_worker(store: &mut SessionSupervisorStore, worker: WorkerSessionRecord) {
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

fn resolve_state_path(payload: &Value) -> Result<PathBuf, String> {
    if let Some(path) = optional_non_empty_string(payload, "state_path") {
        return Ok(PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|err| format!("read current_dir failed: {err}"))?;
    Ok(cwd.join("codex_agno_runtime/data/session_supervisor_state.json"))
}

fn now_from_payload(payload: &Value) -> Result<String, String> {
    if let Some(now) = optional_non_empty_string(payload, "now") {
        parse_rfc3339(&now)?;
        return Ok(now);
    }
    Ok(Utc::now().to_rfc3339())
}

fn add_seconds_rfc3339(now: &str, seconds: i64) -> Result<String, String> {
    let dt = parse_rfc3339(now)?;
    Ok((dt + Duration::seconds(seconds)).to_rfc3339())
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| format!("invalid RFC3339 timestamp {value:?}: {err}"))
}

fn required_non_empty_string(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    optional_non_empty_string(payload, key)
        .ok_or_else(|| format!("{context} requires a non-empty {key}"))
}

fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
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

fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn optional_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn push_event(
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

fn sanitize_segment(value: &str) -> String {
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

fn shell_join(binary: &str, args: &[String]) -> String {
    let mut parts = vec![shell_escape(binary)];
    parts.extend(args.iter().map(|arg| shell_escape(arg)));
    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn duration_pattern() -> &'static Regex {
    static DURATION: OnceLock<Regex> = OnceLock::new();
    DURATION.get_or_init(|| {
        Regex::new(r"(?i)(\d+)\s*(second|sec|minute|min|hour|hr)s?").expect("valid duration regex")
    })
}

fn parse_duration_caps(caps: &regex::Captures<'_>) -> Option<i64> {
    let amount = caps.get(1)?.as_str().parse::<i64>().ok()?;
    let unit = caps.get(2)?.as_str().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "second" | "sec" => 1,
        "minute" | "min" => 60,
        "hour" | "hr" => 3600,
        _ => return None,
    };
    Some(amount * multiplier)
}

fn claude_rate_limit_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new("(?i)rate limit").expect("valid regex"),
                Regex::new("(?i)try again in").expect("valid regex"),
                Regex::new("(?i)usage limit").expect("valid regex"),
                Regex::new("(?i)overloaded").expect("valid regex"),
            ]
        })
        .as_slice()
}

fn codex_rate_limit_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new("(?i)rate limit").expect("valid regex"),
                Regex::new("(?i)try again").expect("valid regex"),
                Regex::new("(?i)too many requests").expect("valid regex"),
                Regex::new("(?i)429").expect("valid regex"),
                Regex::new("(?i)overloaded").expect("valid regex"),
            ]
        })
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn claude_resume_command_prefers_continue_without_explicit_target() {
        let command = build_driver_command(
            "claude",
            "/tmp/project",
            None,
            None,
            "continue",
            true,
            false,
            None,
        )
        .expect("build claude resume command");
        assert_eq!(command.driver_id, "claude_driver");
        assert_eq!(command.binary, "claude");
        assert_eq!(command.args, vec!["--continue".to_string()]);
    }

    #[test]
    fn classify_claude_rate_limit_extracts_backoff() {
        let result = classify_rate_limit_block(
            "claude",
            "Claude hit a rate limit. Please try again in 12 minutes.",
        )
        .expect("classify claude rate limit");
        assert_eq!(result.host, "claude");
        assert_eq!(result.blocked_reason, "rate_limit");
        assert_eq!(result.status, "blocked_rate_limit");
        assert_eq!(result.backoff_seconds, 720);
    }

    #[test]
    fn claude_resume_command_skips_tmux_and_worktree_flags() {
        let command = build_driver_command(
            "claude",
            "/tmp/project",
            None,
            Some("session-123".to_string()),
            "resume",
            true,
            true,
            Some("feature-lane".to_string()),
        )
        .expect("build claude resume command");
        assert_eq!(
            command.args,
            vec!["--resume".to_string(), "session-123".to_string()]
        );
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
}
