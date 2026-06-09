use super::types::*;
use chrono::{DateTime, Utc};

pub(super) fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Best-effort RFC3339 parse used by the reaper. Non-RFC3339 timestamps
/// (legacy or hand-edited state) are treated as "unknown age" and skipped.
pub(super) fn parse_rfc3339_to_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub(super) fn is_active_status(status: &str) -> bool {
    matches!(
        status,
        "queued" | "running" | "interrupt_requested" | "retry_scheduled" | "retry_claimed"
    )
}

pub(super) fn is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "interrupted" | "retry_exhausted"
    )
}

pub(super) fn validate_transition(previous_status: Option<&str>, next_status: &str) -> Result<(), String> {
    let allowed = match previous_status {
        None => matches!(
            next_status,
            "queued"
                | "running"
                | "interrupt_requested"
                | "retry_scheduled"
                | "retry_claimed"
                | "completed"
                | "failed"
                | "interrupted"
                | "retry_exhausted"
        ),
        Some("queued") => matches!(
            next_status,
            "queued" | "running" | "interrupt_requested" | "interrupted" | "failed"
        ),
        Some("running") => matches!(
            next_status,
            "running"
                | "interrupt_requested"
                | "completed"
                | "failed"
                | "interrupted"
                | "retry_scheduled"
                | "retry_exhausted"
        ),
        Some("interrupt_requested") => matches!(next_status, "interrupt_requested" | "interrupted"),
        Some("retry_scheduled") => matches!(
            next_status,
            "retry_scheduled"
                | "retry_claimed"
                | "interrupt_requested"
                | "interrupted"
                | "retry_exhausted"
        ),
        Some("retry_claimed") => matches!(
            next_status,
            "retry_claimed"
                | "queued"
                | "running"
                | "interrupt_requested"
                | "interrupted"
                | "failed"
                | "retry_scheduled"
                | "retry_exhausted"
        ),
        Some("completed") => next_status == "completed",
        Some("failed") => next_status == "failed",
        Some("interrupted") => next_status == "interrupted",
        Some("retry_exhausted") => next_status == "retry_exhausted",
        // Unknown prior status (legacy / hand-edited / corrupted state).
        // Previously we returned `true` here as a permissive escape hatch,
        // which let zombie or invalid statuses transition to anything and
        // hid storage corruption. Be strict instead: refuse any transition
        // out of an unrecognized state. Operators can still reset such jobs
        // explicitly via reseed (status=None branch above), forcing the
        // problem to the surface.
        Some(_) => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "Invalid background job transition: {:?} -> {:?}",
            previous_status, next_status
        ))
    }
}
impl BackgroundRunStatus {
    pub(super) fn touched(&self) -> Self {
        let mut updated = self.clone();
        updated.updated_at = now_iso();
        updated
    }

    pub(super) fn claimed_placeholder(job_id: &str, session_id: &str) -> Self {
        let now = now_iso();
        BackgroundRunStatus {
            job_id: job_id.to_string(),
            session_id: Some(session_id.to_string()),
            status: "retry_claimed".to_string(),
            parallel_group_id: None,
            lane_id: None,
            parent_job_id: None,
            multitask_strategy: default_multitask_strategy(),
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            attempt: DEFAULT_BACKGROUND_JOB_ATTEMPT,
            retry_count: DEFAULT_BACKGROUND_JOB_RETRY_COUNT,
            max_attempts: DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS,
            timeout_seconds: None,
            claimed_by: Some(job_id.to_string()),
            claimed_at: Some(now.clone()),
            backoff_base_seconds: DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS,
            backoff_multiplier: DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER,
            max_backoff_seconds: None,
            backoff_seconds: None,
            next_retry_at: None,
            retry_scheduled_at: None,
            retry_claimed_at: Some(now),
            interrupt_requested_at: None,
            interrupted_at: None,
            last_attempt_started_at: None,
            last_attempt_finished_at: None,
            last_failure_at: None,
        }
    }
}

impl BackgroundJobStatusMutation {
    pub(super) fn apply(&self, job_id: &str, existing: Option<&BackgroundRunStatus>) -> BackgroundRunStatus {
        match existing {
            None => BackgroundRunStatus {
                job_id: job_id.to_string(),
                session_id: self.session_id.clone(),
                status: self.status.clone(),
                parallel_group_id: self.parallel_group_id.clone(),
                lane_id: self.lane_id.clone(),
                parent_job_id: self.parent_job_id.clone(),
                multitask_strategy: self
                    .multitask_strategy
                    .clone()
                    .unwrap_or_else(default_multitask_strategy),
                result: self.result.clone(),
                error: self.error.clone(),
                created_at: now_iso(),
                updated_at: now_iso(),
                attempt: self.attempt.unwrap_or(DEFAULT_BACKGROUND_JOB_ATTEMPT),
                retry_count: self
                    .retry_count
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_RETRY_COUNT),
                max_attempts: self
                    .max_attempts
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS),
                timeout_seconds: self.timeout_seconds,
                claimed_by: self.claimed_by.clone(),
                claimed_at: self.claimed_at.clone(),
                backoff_base_seconds: self
                    .backoff_base_seconds
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS),
                backoff_multiplier: self
                    .backoff_multiplier
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER),
                max_backoff_seconds: self.max_backoff_seconds,
                backoff_seconds: self.backoff_seconds,
                next_retry_at: self.next_retry_at.clone(),
                retry_scheduled_at: self.retry_scheduled_at.clone(),
                retry_claimed_at: self.retry_claimed_at.clone(),
                interrupt_requested_at: self.interrupt_requested_at.clone(),
                interrupted_at: self.interrupted_at.clone(),
                last_attempt_started_at: self.last_attempt_started_at.clone(),
                last_attempt_finished_at: self.last_attempt_finished_at.clone(),
                last_failure_at: self.last_failure_at.clone(),
            },
            Some(existing) => {
                let mut updated = existing.touched();
                updated.status = self.status.clone();
                updated.session_id = self.session_id.clone();
                if self.parallel_group_id.is_some() {
                    updated.parallel_group_id = self.parallel_group_id.clone();
                }
                if self.lane_id.is_some() {
                    updated.lane_id = self.lane_id.clone();
                }
                if self.parent_job_id.is_some() {
                    updated.parent_job_id = self.parent_job_id.clone();
                }
                if self.multitask_strategy.is_some() {
                    updated.multitask_strategy =
                        self.multitask_strategy.clone().unwrap_or_default();
                }
                updated.result = self.result.clone();
                updated.error = self.error.clone();
                if self.attempt.is_some() {
                    updated.attempt = self.attempt.unwrap_or(DEFAULT_BACKGROUND_JOB_ATTEMPT);
                }
                if self.retry_count.is_some() {
                    updated.retry_count = self
                        .retry_count
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_RETRY_COUNT);
                }
                if self.max_attempts.is_some() {
                    updated.max_attempts = self
                        .max_attempts
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS);
                }
                if self.timeout_seconds.is_some() {
                    updated.timeout_seconds = self.timeout_seconds;
                }
                if self.claimed_by.is_some() {
                    updated.claimed_by = self.claimed_by.clone();
                }
                if self.claimed_at.is_some() {
                    updated.claimed_at = self.claimed_at.clone();
                }
                if self.backoff_base_seconds.is_some() {
                    updated.backoff_base_seconds = self
                        .backoff_base_seconds
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS);
                }
                if self.backoff_multiplier.is_some() {
                    updated.backoff_multiplier = self
                        .backoff_multiplier
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER);
                }
                if self.max_backoff_seconds.is_some() {
                    updated.max_backoff_seconds = self.max_backoff_seconds;
                }
                updated.backoff_seconds = self.backoff_seconds;
                updated.next_retry_at = self.next_retry_at.clone();
                updated.retry_scheduled_at = self.retry_scheduled_at.clone();
                updated.retry_claimed_at = self.retry_claimed_at.clone();
                if self.interrupt_requested_at.is_some() {
                    updated.interrupt_requested_at = self.interrupt_requested_at.clone();
                }
                if self.interrupted_at.is_some() {
                    updated.interrupted_at = self.interrupted_at.clone();
                }
                if self.last_attempt_started_at.is_some() {
                    updated.last_attempt_started_at = self.last_attempt_started_at.clone();
                }
                if self.last_attempt_finished_at.is_some() {
                    updated.last_attempt_finished_at = self.last_attempt_finished_at.clone();
                }
                if self.last_failure_at.is_some() {
                    updated.last_failure_at = self.last_failure_at.clone();
                }
                updated
            }
        }
    }
}
