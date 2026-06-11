use chrono::Utc;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::OnceLock;

use super::driver::{build_driver_command, default_resume_mode, driver_id_for_host};
use super::process::{launch_process, process_is_alive, terminate_process};
use super::runtime::{
    add_seconds_rfc3339, ensure_lane_contract_metadata, optional_i64,
    optional_non_empty_string, push_event, required_non_empty_string, sanitize_segment,
    upsert_worker, worker_log_path,
};
use super::types::{
    BlockClassification, SessionSupervisorStore, WorkerSessionRecord,
    DEFAULT_BACKOFF_SECONDS,
};

pub fn launch_worker(
    payload: &Value,
    store: &mut SessionSupervisorStore,
    state_path: &Path,
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
    let worktree_name_val = optional_non_empty_string(payload, "worktree_name");
    let worktree_path_val = optional_non_empty_string(payload, "worktree_path");
    let launch_command = build_driver_command(
        &host,
        &cwd,
        prompt.clone(),
        resume_target.clone(),
        &resume_mode,
        false,
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
        worktree_name_val,
        worktree_path_val,
    )?);
    let retry_policy = payload
        .get("retry_policy")
        .cloned()
        .unwrap_or_else(|| {
            json!({"kind": "rate_limit_auto_resume", "default_backoff_seconds": DEFAULT_BACKOFF_SECONDS})
        });
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
        pid: None,
        log_path: None,
        attached_session_id: optional_non_empty_string(payload, "attached_session_id"),
        resume_target,
        resume_mode: Some(resume_mode),
        blocked_reason: None,
        next_resume_at: None,
        retry_policy,
        prompt,
        launch_command,
        resume_command,
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
        let process_cwd = worker.worktree_path.as_deref().unwrap_or(&cwd);
        let log_path = worker_log_path(state_path, &worker.worker_id);
        let spawn = launch_process(&worker.launch_command, process_cwd, &log_path)?;
        worker.pid = Some(spawn.pid);
        worker.log_path = Some(spawn.log_path);
        worker.status = "running".to_string();
        worker.updated_at = now.to_string();
        push_event(
            &mut worker,
            "launched",
            "running",
            now,
            Some(format!("pid {}", spawn.pid)),
        );
    }

    upsert_worker(store, worker.clone());
    Ok(worker)
}

/// Compute exponential backoff based on how many times this worker has been blocked.
/// Steps: 30s → 60s → 120s → 300s (cap).
fn exponential_backoff_seconds(worker: &WorkerSessionRecord) -> i64 {
    let block_count = worker
        .events
        .iter()
        .filter(|e| e.event == "blocked")
        .count() as u32;
    let base = 30i64;
    let backoff = base * 2_i64.pow(block_count.min(3)); // 30, 60, 120, 300
    backoff.min(DEFAULT_BACKOFF_SECONDS)
}

pub fn mark_worker_blocked(
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

    // Use the larger of: parsed duration from error message, or exponential backoff
    let exp_backoff = exponential_backoff_seconds(worker);
    let effective_backoff = classification.backoff_seconds.max(exp_backoff);

    worker.status = classification.status.clone();
    worker.blocked_reason = Some(classification.blocked_reason.clone());
    worker.next_resume_at = Some(add_seconds_rfc3339(now, effective_backoff)?);
    worker.last_error = classification.matched_text.clone();
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "blocked",
        &classification.status,
        now,
        Some(format!(
            "next resume scheduled after {} seconds (attempt {})",
            effective_backoff,
            worker.events.iter().filter(|e| e.event == "blocked").count(),
        )),
    );
    Ok(classification)
}

pub fn resume_worker(
    worker: &mut WorkerSessionRecord,
    state_path: &Path,
    dry_run: bool,
    now: &str,
) -> Result<String, String> {
    let command = worker
        .resume_command
        .clone()
        .ok_or_else(|| format!("Worker {} has no resume command", worker.worker_id))?;

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

    if let Some(pid) = worker.pid {
        if process_is_alive(pid) {
            terminate_process(pid)?;
        }
    }

    let process_cwd = worker.worktree_path.as_deref().unwrap_or(&worker.cwd);
    let log_path = worker_log_path(state_path, &worker.worker_id);
    let spawn = launch_process(&command, process_cwd, &log_path)?;
    worker.pid = Some(spawn.pid);
    worker.log_path = Some(spawn.log_path.clone());
    worker.status = "running".to_string();
    worker.blocked_reason = None;
    worker.next_resume_at = None;
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "resumed",
        "running",
        now,
        Some(format!("respawned pid {}", spawn.pid)),
    );
    Ok("respawn".to_string())
}

pub fn terminate_worker(
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

    if let Some(pid) = worker.pid {
        if process_is_alive(pid) {
            terminate_process(pid)?;
        }
    }
    worker.status = "interrupted".to_string();
    worker.updated_at = now.to_string();
    push_event(
        worker,
        "terminated",
        "interrupted",
        now,
        Some("worker process terminated".to_string()),
    );
    Ok(true)
}

/// Reap workers stuck in active statuses without a live PID past `stale_after_secs`.
pub fn reap_stale_workers(
    workers: &mut [WorkerSessionRecord],
    now: &str,
    stale_after_secs: i64,
) -> Result<(), String> {
    if stale_after_secs <= 0 {
        return Ok(());
    }
    let now_dt = super::runtime::parse_rfc3339(now)?;
    for worker in workers.iter_mut() {
        if !matches!(
            worker.status.as_str(),
            "queued" | "launching" | "running" | "resume_scheduled"
        ) {
            continue;
        }
        if let Some(pid) = worker.pid {
            if process_is_alive(pid) {
                continue;
            }
        }
        let updated = super::runtime::parse_rfc3339(&worker.updated_at)?;
        let age = now_dt.signed_duration_since(updated).num_seconds();
        if age <= stale_after_secs {
            continue;
        }
        worker.status = "interrupted".to_string();
        worker.updated_at = now.to_string();
        push_event(
            worker,
            "stale_timeout",
            "interrupted",
            now,
            Some(format!(
                "worker stale after {age}s (threshold {stale_after_secs}s)"
            )),
        );
    }
    Ok(())
}

pub fn worker_ready_for_resume(worker: &WorkerSessionRecord, now: &str) -> Result<bool, String> {
    if !matches!(
        worker.status.as_str(),
        "blocked_rate_limit" | "resume_scheduled"
    ) {
        return Ok(false);
    }
    let Some(next_resume_at) = worker.next_resume_at.as_deref() else {
        return Ok(false);
    };
    let next_time = super::runtime::parse_rfc3339(next_resume_at)?;
    Ok(super::runtime::parse_rfc3339(now)? >= next_time)
}

pub fn classify_rate_limit_block(
    host: &str,
    evidence_text: &str,
) -> Result<BlockClassification, String> {
    let lowered = host.trim().to_ascii_lowercase();
    // All hosts use the same universal rate-limit patterns.
    // No host-specific pattern sets — new hosts get coverage automatically.
    let mut matched = detect_rate_limit(evidence_text, rate_limit_patterns());
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

/// Universal rate-limit patterns that work across all hosts.
/// Matches common HTTP 429 / rate-limit vocabulary shared by all LLM APIs.
/// New hosts automatically get coverage without code changes.
fn rate_limit_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new("(?i)rate limit").expect("valid regex"),
                Regex::new("(?i)too many (?:requests|queries)").expect("valid regex"),
                Regex::new("(?i)\\b429\\b").expect("valid regex"),
                Regex::new("(?i)overloaded").expect("valid regex"),
                Regex::new("(?i)try again (?:later|in|now)").expect("valid regex"),
                Regex::new("(?i)quota exceeded").expect("valid regex"),
                Regex::new("(?i)usage limit").expect("valid regex"),
            ]
        })
        .as_slice()
}
