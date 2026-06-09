use super::driver::{
    build_driver_command, default_resume_mode, driver_id_for_host, ensure_lane_contract_metadata,
};
use super::runtime::{
    add_seconds_rfc3339, launch_in_tmux, optional_bool, optional_i64, optional_non_empty_string,
    parse_rfc3339, push_event, required_non_empty_string, run_tmux, sanitize_segment,
    send_command_to_tmux, tmux_session_exists, upsert_worker,
};
use super::types::*;
use chrono::Utc;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

pub(super) fn launch_worker(
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
    let worktree_name_val = optional_non_empty_string(payload, "worktree_name");
    let worktree_path_val = optional_non_empty_string(payload, "worktree_path");
    let launch_command = build_driver_command(
        &host,
        &cwd,
        prompt.clone(),
        resume_target.clone(),
        &resume_mode,
        false,
        native_tmux_requested,
        worktree_name_val.clone(),
        worktree_path_val.clone(),
    )?;
    let resume_command = Some(build_driver_command(
        &host,
        &cwd,
        None,
        resume_target.clone(),
        &resume_mode,
        true,
        native_tmux_requested,
        worktree_name_val,
        worktree_path_val,
    )?);
    let retry_policy = payload
        .get("retry_policy")
        .cloned()
        .unwrap_or_else(|| json!({"kind": "rate_limit_auto_resume", "default_backoff_seconds": DEFAULT_BACKOFF_SECONDS}));
    let mut metadata = payload
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    metadata = ensure_lane_contract_metadata(
        metadata,
        &worker_id,
        &host,
        &cwd,
        prompt.as_deref(),
        payload.get("lane_contract").cloned(),
    );

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
        let tmux_cwd = worker.worktree_path.as_deref().unwrap_or(&cwd);
        let spawn = launch_in_tmux(&worker.launch_command, &tmux_session, tmux_cwd)?;
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

pub(super) fn mark_worker_blocked(
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

pub(super) fn resume_worker(
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

pub(super) fn terminate_worker(
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

pub(super) fn worker_ready_for_resume(worker: &WorkerSessionRecord, now: &str) -> Result<bool, String> {
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
        "codex" => detect_rate_limit(evidence_text, codex_rate_limit_patterns()),
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
