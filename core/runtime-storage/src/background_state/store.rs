use super::control_plane::{build_state_control_plane, normalized_backend_family};
use super::persist::{read_persisted_state, write_persisted_state};
use super::status::{
    is_active_status, is_terminal_status, parse_rfc3339_to_utc, validate_transition,
};
use super::types::*;
use super::types::{BackgroundJobStatusMutation, BackgroundRunStatus};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

impl BackgroundStateStore {
    pub(super) fn load(request: &BackgroundStateRequestPayload) -> Result<Self, String> {
        let state_path = request
            .state_path
            .as_ref()
            .ok_or_else(|| "Background state request is missing state_path.".to_string())
            .map(PathBuf::from)?;
        let backend_family = request
            .backend_family
            .clone()
            .unwrap_or_else(|| "filesystem".to_string());
        let sqlite_db_path = request.sqlite_db_path.as_ref().map(PathBuf::from);
        let control_plane = build_state_control_plane(
            request.control_plane_descriptor.as_ref(),
            &backend_family,
            &state_path,
        )?;
        let persisted = read_persisted_state(
            &state_path,
            &backend_family,
            sqlite_db_path.as_deref(),
            request.state_payload_text.as_deref(),
        )?;
        let mut store = Self {
            state_path,
            backend_family: normalized_backend_family(&backend_family),
            sqlite_db_path,
            control_plane,
            jobs: HashMap::new(),
            active_sessions: HashMap::new(),
            pending_session_takeovers: HashMap::new(),
            reaped_dirty: false,
        };
        if let Some(persisted) = persisted {
            store.merge_persisted(persisted)?;
        }
        // Reap zombie / over-aged jobs into the in-memory view so every
        // operation sees a clean snapshot. We deliberately do **not** persist
        // here: that would turn pure-read operations (snapshot/get/health)
        // into silent disk writers, breaking the "read = read-only" contract
        // and forcing every reader through the path-lock + filesystem rename
        // machinery. Instead, `reaped_dirty` is set so mutating handlers
        // (`apply_mutation`, arbitration, reservation) flush the cleanup as
        // part of their normal persist step. Pure readers keep the cleanup
        // in their local view and re-derive it on the next load — cheap,
        // since the reap is an in-memory HashMap scan.
        let now = Utc::now();
        let reaped_active = store.reap_stale_active_jobs(now);
        let reaped_terminal = store.reap_stale_terminal_jobs(now);
        let reaped_ghost = store.reap_ghost_status_jobs(now);
        if reaped_active + reaped_terminal + reaped_ghost > 0 {
            store.reaped_dirty = true;
        }
        store.compact_terminal_over_capacity(request.capacity_limit);
        Ok(store)
    }

    /// Best-effort persist of the in-memory reap cleanup. Called by mutating
    /// handlers right after `load` so reaped state lands on disk together
    /// with the user-driven mutation. Failures are logged loudly to stderr
    /// instead of being silently dropped: an indefinitely-failing reap
    /// persist (full disk, permissions, etc.) is an operational concern that
    /// must surface in logs, not hide behind `let _ =`.
    pub(super) fn flush_reap_if_dirty(&mut self) {
        if !self.reaped_dirty {
            return;
        }
        match self.persist() {
            Ok(_) => {
                self.reaped_dirty = false;
            }
            Err(err) => {
                eprintln!(
                    "[router-rs] background_state reaper persist failed for {} (non-fatal, will retry on next mutation): {err}",
                    self.state_path.display()
                );
            }
        }
    }

    /// Transition active jobs whose `updated_at` heartbeat is older than
    /// `STALE_ACTIVE_HEARTBEAT_TTL_SECS` to `interrupted` so they release
    /// session reservations and become eligible for garbage collection.
    /// Returns the number of jobs reaped.
    pub(super) fn reap_stale_active_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let cutoff = now - chrono::Duration::seconds(STALE_ACTIVE_HEARTBEAT_TTL_SECS);
        let stale_ids: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, job)| is_active_status(&job.status))
            .filter(|(_, job)| {
                parse_rfc3339_to_utc(&job.updated_at)
                    .map(|ts| ts < cutoff)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let now_iso = now.to_rfc3339();
        for job_id in &stale_ids {
            if let Some(job) = self.jobs.get_mut(job_id) {
                let reaper_msg = format!(
                    "reaped: {} heartbeat stale > {STALE_ACTIVE_HEARTBEAT_TTL_SECS}s",
                    &job.status
                );
                job.status = "interrupted".to_string();
                job.interrupted_at = Some(now_iso.clone());
                job.updated_at = now_iso.clone();
                job.error = Some(match job.error.as_deref() {
                    Some(prev) if !prev.is_empty() => format!("{prev}; {reaper_msg}"),
                    _ => reaper_msg,
                });
            }
        }
        if !stale_ids.is_empty() {
            self.active_sessions
                .retain(|_, owner| !stale_ids.iter().any(|id| id == owner));
            self.pending_session_takeovers
                .retain(|_, incoming| !stale_ids.iter().any(|id| id == incoming));
        }
        stale_ids.len()
    }

    /// Drop terminal jobs older than `STALE_TERMINAL_JOB_TTL_SECS` so the
    /// persisted file stays bounded across days/weeks of long-running use.
    pub(super) fn reap_stale_terminal_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let cutoff = now - chrono::Duration::seconds(STALE_TERMINAL_JOB_TTL_SECS);
        let drop_ids: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, job)| is_terminal_status(&job.status))
            .filter(|(_, job)| {
                parse_rfc3339_to_utc(&job.updated_at)
                    .map(|ts| ts < cutoff)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let count = drop_ids.len();
        for id in drop_ids {
            self.jobs.remove(&id);
        }
        count
    }

    /// Sweep "ghost" jobs whose `status` is neither active nor terminal.
    ///
    /// After we tightened `validate_transition` to refuse transitions out of
    /// unrecognized prior statuses, any pre-existing ghost (e.g. a status
    /// string introduced by a future schema, hand-edited, or persisted from
    /// a corrupted run) becomes permanently uncoverable: the FSM rejects
    /// every mutation and the active/terminal reapers never see it. Force
    /// such jobs into `interrupted` with a diagnostic `error` so:
    ///   - the next terminal-TTL pass can drop them on schedule,
    ///   - operators see exactly which ghost status was observed,
    ///   - session/takeover maps releasing the slot follow the same flow as
    ///     stale-active reaping.
    ///
    /// Returns the number of jobs converted.
    pub(super) fn reap_ghost_status_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let ghost_pairs: Vec<(String, String)> = self
            .jobs
            .iter()
            .filter(|(_, job)| !is_active_status(&job.status) && !is_terminal_status(&job.status))
            .map(|(id, job)| (id.clone(), job.status.clone()))
            .collect();
        if ghost_pairs.is_empty() {
            return 0;
        }
        let now_iso = now.to_rfc3339();
        for (job_id, prev_status) in &ghost_pairs {
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.status = "interrupted".to_string();
                job.interrupted_at = Some(now_iso.clone());
                job.updated_at = now_iso.clone();
                let reaper_msg =
                    format!("reaped: ghost_status={prev_status:?} not in active/terminal FSM");
                job.error = Some(match job.error.as_deref() {
                    Some(prev) if !prev.is_empty() => format!("{prev}; {reaper_msg}"),
                    _ => reaper_msg,
                });
            }
        }
        let ghost_ids: Vec<&str> = ghost_pairs.iter().map(|(id, _)| id.as_str()).collect();
        self.active_sessions
            .retain(|_, owner| !ghost_ids.iter().any(|id| *id == owner));
        self.pending_session_takeovers
            .retain(|_, incoming| !ghost_ids.iter().any(|id| *id == incoming));
        ghost_pairs.len()
    }

    pub(super) fn merge_persisted(
        &mut self,
        persisted: PersistedBackgroundState,
    ) -> Result<(), String> {
        if let Some(Value::Object(persisted_control_plane)) = persisted.control_plane
            && let Value::Object(ref mut current) = self.control_plane {
                for (key, value) in persisted_control_plane {
                    if !value.is_null() {
                        current.insert(key, value);
                    }
                }
            }
        self.jobs = persisted
            .jobs
            .into_iter()
            .map(|job| (job.job_id.clone(), job))
            .collect();
        self.active_sessions = if persisted.active_sessions.is_empty() {
            self.rebuild_active_sessions()
        } else {
            persisted
                .active_sessions
                .into_iter()
                .map(|row| (row.session_id, row.job_id))
                .collect()
        };
        self.active_sessions.retain(|_, job_id| {
            self.jobs
                .get(job_id)
                .map(|job| is_active_status(&job.status))
                .unwrap_or(false)
        });
        self.pending_session_takeovers = persisted
            .pending_session_takeovers
            .into_iter()
            .filter(|row| {
                // Keep pending takeover when either:
                // 1) incoming job exists and is still active, or
                // 2) the target session is still known in persisted jobs
                //    (including recently completed owners) so a follow-up claim
                //    can finish the handoff.
                self.jobs
                    .get(&row.incoming_job_id)
                    .map(|job| is_active_status(&job.status))
                    .unwrap_or(false)
                    || self
                        .jobs
                        .values()
                        .any(|job| job.session_id.as_deref() == Some(row.session_id.as_str()))
            })
            .map(|row| (row.session_id, row.incoming_job_id))
            .collect();
        Ok(())
    }

    pub(super) fn rebuild_active_sessions(&self) -> HashMap<String, String> {
        let mut rows = self
            .jobs
            .values()
            .filter(|job| job.session_id.is_some() && is_active_status(&job.status))
            .map(|job| {
                (
                    job.updated_at.clone(),
                    job.job_id.clone(),
                    job.session_id.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        let mut rebuilt = HashMap::new();
        for (_, job_id, session_id) in rows {
            rebuilt.insert(session_id, job_id);
        }
        rebuilt
    }

    pub(super) fn serialized_payload(&self) -> Result<String, String> {
        let persisted = PersistedBackgroundState {
            version: 2,
            schema_version: BACKGROUND_STATE_SCHEMA_VERSION.to_string(),
            control_plane: Some(self.control_plane.clone()),
            jobs: sorted_jobs(&self.jobs),
            active_sessions: sorted_string_pairs(&self.active_sessions)
                .into_iter()
                .map(|(session_id, job_id)| PersistedActiveSession { session_id, job_id })
                .collect(),
            pending_session_takeovers: sorted_string_pairs(&self.pending_session_takeovers)
                .into_iter()
                .map(|(session_id, incoming_job_id)| PersistedPendingTakeover {
                    session_id,
                    incoming_job_id,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&persisted)
            .map(|payload| payload + "\n")
            .map_err(|err| err.to_string())
    }

    pub(super) fn persist(&self) -> Result<Option<String>, String> {
        let payload = self.serialized_payload()?;
        if self.backend_family == "memory" {
            return Ok(Some(payload));
        }
        write_persisted_state(
            &self.state_path,
            &self.backend_family,
            self.sqlite_db_path.as_deref(),
            &payload,
        )?;
        Ok(None)
    }

    pub(super) fn apply_mutation(
        &mut self,
        job_id: &str,
        mutation: &BackgroundJobStatusMutation,
    ) -> Result<(BackgroundRunStatus, Option<String>), String> {
        let existing = self.jobs.get(job_id).cloned();
        let previous_status = existing.as_ref().map(|job| job.status.as_str());
        validate_transition(previous_status, &mutation.status)?;
        let previous_session_id = existing.as_ref().and_then(|job| job.session_id.clone());
        let resolved_session_id = mutation
            .session_id
            .clone()
            .or_else(|| previous_session_id.clone());
        self.reserve_session(job_id, resolved_session_id.as_deref(), &mutation.status)?;
        let resolved_mutation = BackgroundJobStatusMutation {
            status: mutation.status.clone(),
            session_id: resolved_session_id,
            parallel_group_id: mutation.parallel_group_id.clone(),
            lane_id: mutation.lane_id.clone(),
            parent_job_id: mutation.parent_job_id.clone(),
            multitask_strategy: mutation.multitask_strategy.clone(),
            result: mutation.result.clone(),
            error: mutation.error.clone(),
            timeout_seconds: mutation.timeout_seconds,
            claimed_by: mutation.claimed_by.clone(),
            attempt: mutation.attempt,
            retry_count: mutation.retry_count,
            max_attempts: mutation.max_attempts,
            claimed_at: mutation.claimed_at.clone(),
            backoff_base_seconds: mutation.backoff_base_seconds,
            backoff_multiplier: mutation.backoff_multiplier,
            max_backoff_seconds: mutation.max_backoff_seconds,
            backoff_seconds: mutation.backoff_seconds,
            next_retry_at: mutation.next_retry_at.clone(),
            retry_scheduled_at: mutation.retry_scheduled_at.clone(),
            retry_claimed_at: mutation.retry_claimed_at.clone(),
            interrupt_requested_at: mutation.interrupt_requested_at.clone(),
            interrupted_at: mutation.interrupted_at.clone(),
            last_attempt_started_at: mutation.last_attempt_started_at.clone(),
            last_attempt_finished_at: mutation.last_attempt_finished_at.clone(),
            last_failure_at: mutation.last_failure_at.clone(),
        };
        let resolved_session_ref = resolved_mutation.session_id.as_deref();
        let updated = resolved_mutation.apply(job_id, existing.as_ref());
        self.jobs.insert(job_id.to_string(), updated.clone());
        self.release_previous_session(job_id, previous_session_id.as_deref(), resolved_session_ref);
        self.finalize_session(job_id, resolved_session_ref, &mutation.status);
        let persisted_payload_text = self.persist()?;
        Ok((updated, persisted_payload_text))
    }

    pub(super) fn reserve_session(
        &mut self,
        job_id: &str,
        session_id: Option<&str>,
        status: &str,
    ) -> Result<(), String> {
        let Some(session_id) = session_id else {
            return Ok(());
        };
        if !is_active_status(status) {
            return Ok(());
        }
        if let Some(owner) = self.active_sessions.get(session_id)
            && owner != job_id {
                return Err(format!(
                    "Session {session_id:?} is already active in job {owner:?}."
                ));
            }
        self.active_sessions
            .insert(session_id.to_string(), job_id.to_string());
        Ok(())
    }

    pub(super) fn release_previous_session(
        &mut self,
        job_id: &str,
        previous_session_id: Option<&str>,
        next_session_id: Option<&str>,
    ) {
        let Some(previous_session_id) = previous_session_id else {
            return;
        };
        if Some(previous_session_id) == next_session_id {
            return;
        }
        if self
            .active_sessions
            .get(previous_session_id)
            .map(String::as_str)
            == Some(job_id)
        {
            self.active_sessions.remove(previous_session_id);
        }
    }

    pub(super) fn finalize_session(
        &mut self,
        job_id: &str,
        session_id: Option<&str>,
        status: &str,
    ) {
        let Some(session_id) = session_id else {
            return;
        };
        if !is_terminal_status(status) {
            return;
        }
        if self.active_sessions.get(session_id).map(String::as_str) == Some(job_id) {
            self.active_sessions.remove(session_id);
        }
    }

    pub(super) fn get(&self, job_id: &str) -> Option<BackgroundRunStatus> {
        self.jobs.get(job_id).cloned()
    }

    pub(super) fn active_job(&self, session_id: &str) -> Option<String> {
        self.active_sessions.get(session_id).cloned()
    }

    pub(super) fn active_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| is_active_status(&job.status))
            .count()
    }

    pub(super) fn terminal_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| is_terminal_status(&job.status))
            .count()
    }

    pub(super) fn resolved_capacity_limit(&self, override_limit: Option<usize>) -> usize {
        override_limit
            .or_else(|| {
                self.control_plane
                    .get("max_background_jobs")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
            })
            .unwrap_or(DEFAULT_MAX_BACKGROUND_JOBS)
            .clamp(1, MAX_BACKGROUND_JOBS_LIMIT)
    }

    pub(super) fn compact_terminal_over_capacity(&mut self, capacity_limit: Option<usize>) {
        let limit = self.resolved_capacity_limit(capacity_limit);
        // Loop in case a single pass cannot remove enough terminal jobs to
        // reach the limit (e.g. 200 terminal + 800 active, limit=500 →
        // first pass removes 200, totals 800, still over).
        loop {
            if self.jobs.len() <= limit {
                break;
            }
            let mut terminal_jobs = self
                .jobs
                .values()
                .filter(|job| is_terminal_status(&job.status))
                .map(|job| (job.updated_at.clone(), job.job_id.clone()))
                .collect::<Vec<_>>();
            if terminal_jobs.is_empty() {
                break; // No more terminal jobs to remove.
            }
            terminal_jobs.sort();
            for (_, job_id) in terminal_jobs {
                self.jobs.remove(&job_id);
            }
            // Re-check; may need another pass if active jobs transitioned
            // to terminal between now and the next load cycle.
        }
    }

    pub(super) fn pending_session_takeovers(&self) -> usize {
        self.pending_session_takeovers.len()
    }

    pub(super) fn parallel_group_summary(
        &self,
        parallel_group_id: &str,
    ) -> Option<BackgroundParallelGroupSummary> {
        let jobs = self
            .jobs
            .values()
            .filter(|job| job.parallel_group_id.as_deref() == Some(parallel_group_id))
            .cloned()
            .collect::<Vec<_>>();
        if jobs.is_empty() {
            return None;
        }
        Some(build_parallel_group_summary(parallel_group_id, &jobs))
    }

    pub(super) fn parallel_group_summaries(&self) -> Vec<BackgroundParallelGroupSummary> {
        let mut grouped: HashMap<String, Vec<BackgroundRunStatus>> = HashMap::new();
        for job in self.jobs.values() {
            if let Some(ref group_id) = job.parallel_group_id {
                grouped
                    .entry(group_id.clone())
                    .or_default()
                    .push(job.clone());
            }
        }
        let mut group_ids = grouped.keys().cloned().collect::<Vec<_>>();
        group_ids.sort();
        group_ids
            .into_iter()
            .filter_map(|group_id| {
                grouped
                    .get(&group_id)
                    .map(|jobs| build_parallel_group_summary(&group_id, jobs))
            })
            .collect()
    }

    pub(super) fn arbitrate_session_takeover(
        &mut self,
        operation: &str,
        session_id: &str,
        incoming_job_id: &str,
    ) -> Result<(BackgroundSessionTakeoverArbitration, Option<String>), String> {
        let previous_active_job_id = self.active_sessions.get(session_id).cloned();
        let previous_pending_job_id = self.pending_session_takeovers.get(session_id).cloned();
        let mut changed = false;
        let outcome = match operation {
            "reserve" => {
                if let Some(previous_pending) = previous_pending_job_id.as_deref()
                    && previous_pending != incoming_job_id {
                        return Err(format!(
                            "Session {session_id:?} already has a pending takeover for job {previous_pending:?}."
                        ));
                    }
                match previous_active_job_id.as_deref() {
                    None => {
                        if previous_pending_job_id.as_deref() == Some(incoming_job_id) {
                            "pending".to_string()
                        } else {
                            "available".to_string()
                        }
                    }
                    Some(active_job_id) if active_job_id == incoming_job_id => "owned".to_string(),
                    Some(_) => {
                        if previous_pending_job_id.as_deref() != Some(incoming_job_id) {
                            self.pending_session_takeovers
                                .insert(session_id.to_string(), incoming_job_id.to_string());
                            changed = true;
                        }
                        "pending".to_string()
                    }
                }
            }
            "claim" => {
                if previous_pending_job_id.as_deref() != Some(incoming_job_id) {
                    return Err(format!(
                        "Session {session_id:?} is not reserved for incoming job {incoming_job_id:?}."
                    ));
                }
                if let Some(active_job_id) = previous_active_job_id.as_deref()
                    && active_job_id != incoming_job_id {
                        return Err(format!(
                            "Session {session_id:?} is still active in job {active_job_id:?}."
                        ));
                    }
                if previous_active_job_id.as_deref() != Some(incoming_job_id) {
                    self.active_sessions
                        .insert(session_id.to_string(), incoming_job_id.to_string());
                    changed = true;
                }
                if !self.jobs.contains_key(incoming_job_id) {
                    self.jobs.insert(
                        incoming_job_id.to_string(),
                        BackgroundRunStatus::claimed_placeholder(incoming_job_id, session_id),
                    );
                    changed = true;
                }
                if previous_pending_job_id.is_some() {
                    self.pending_session_takeovers.remove(session_id);
                    changed = true;
                }
                "claimed".to_string()
            }
            "release" => {
                if previous_pending_job_id.as_deref() == Some(incoming_job_id) {
                    self.pending_session_takeovers.remove(session_id);
                    changed = true;
                }
                if self.active_sessions.get(session_id).map(String::as_str) == Some(incoming_job_id)
                    && !self.jobs.contains_key(incoming_job_id)
                {
                    self.active_sessions.remove(session_id);
                    changed = true;
                }
                if changed {
                    "released".to_string()
                } else {
                    "noop".to_string()
                }
            }
            other => {
                return Err(format!(
                    "Unsupported takeover arbitration operation: {:?}",
                    other
                ));
            }
        };
        let persisted_payload_text = if changed { self.persist()? } else { None };
        Ok((
            BackgroundSessionTakeoverArbitration {
                schema_version: BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION.to_string(),
                operation: operation.to_string(),
                session_id: session_id.to_string(),
                incoming_job_id: incoming_job_id.to_string(),
                previous_active_job_id,
                previous_pending_job_id,
                active_job_id: self.active_sessions.get(session_id).cloned(),
                pending_job_id: self.pending_session_takeovers.get(session_id).cloned(),
                outcome,
                changed,
            },
            persisted_payload_text,
        ))
    }

    pub(super) fn snapshot_payload(&self) -> Value {
        json!({
            "control_plane": self.control_plane,
            "jobs": sorted_jobs(&self.jobs),
            "active_sessions": sorted_string_pairs(&self.active_sessions)
                .into_iter()
                .map(|(session_id, job_id)| json!({"session_id": session_id, "job_id": job_id}))
                .collect::<Vec<_>>(),
            "pending_session_takeovers": sorted_string_pairs(&self.pending_session_takeovers)
                .into_iter()
                .map(|(session_id, incoming_job_id)| json!({"session_id": session_id, "incoming_job_id": incoming_job_id}))
                .collect::<Vec<_>>(),
        })
    }

    pub(super) fn health_payload(&self) -> Value {
        json!({
            "control_plane_authority": self.control_plane.get("authority").cloned().unwrap_or(Value::Null),
            "control_plane_role": self.control_plane.get("role").cloned().unwrap_or(Value::Null),
            "control_plane_projection": self.control_plane.get("projection").cloned().unwrap_or(Value::Null),
            "control_plane_delegate_kind": self.control_plane.get("delegate_kind").cloned().unwrap_or(Value::Null),
            "runtime_control_plane_authority": self.control_plane.get("runtime_control_plane_authority").cloned().unwrap_or(Value::Null),
            "runtime_control_plane_schema_version": self.control_plane.get("runtime_control_plane_schema_version").cloned().unwrap_or(Value::Null),
            "backend_family": self.control_plane.get("backend_family").cloned().unwrap_or(Value::Null),
            "supports_atomic_replace": self.control_plane.get("supports_atomic_replace").cloned().unwrap_or(Value::Bool(false)),
            "supports_compaction": self.control_plane.get("supports_compaction").cloned().unwrap_or(Value::Bool(false)),
            "supports_snapshot_delta": self.control_plane.get("supports_snapshot_delta").cloned().unwrap_or(Value::Bool(false)),
            "supports_remote_event_transport": self.control_plane.get("supports_remote_event_transport").cloned().unwrap_or(Value::Bool(false)),
            "supports_consistent_append": self.control_plane.get("supports_consistent_append").cloned().unwrap_or(Value::Bool(false)),
            "supports_sqlite_wal": self.control_plane.get("supports_sqlite_wal").cloned().unwrap_or(Value::Bool(false)),
            "state_path": self.control_plane.get("state_path").cloned().unwrap_or(Value::Null),
            "job_count": self.jobs.len(),
            "active_job_count": self.active_job_count(),
            "terminal_job_count": self.terminal_job_count(),
            "max_background_jobs": self.resolved_capacity_limit(None),
            "max_background_jobs_limit": MAX_BACKGROUND_JOBS_LIMIT,
            "parallel_group_count": self.parallel_group_summaries().len(),
            "pending_session_takeovers": self.pending_session_takeovers(),
        })
    }
}

fn sorted_jobs(jobs: &HashMap<String, BackgroundRunStatus>) -> Vec<BackgroundRunStatus> {
    let mut rows = jobs.values().cloned().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.job_id.cmp(&right.job_id));
    rows
}

fn sorted_string_pairs(rows: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut entries = rows
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn build_parallel_group_summary(
    parallel_group_id: &str,
    jobs: &[BackgroundRunStatus],
) -> BackgroundParallelGroupSummary {
    let mut status_counts = Map::new();
    let mut session_ids = Vec::new();
    let mut lane_ids = Vec::new();
    let mut parent_job_ids = Vec::new();
    let mut active_job_count = 0usize;
    let mut terminal_job_count = 0usize;
    let mut latest_updated_at: Option<String> = None;
    let mut job_ids = jobs
        .iter()
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    job_ids.sort();
    for job in jobs {
        let current = status_counts
            .get(&job.status)
            .and_then(Value::as_u64)
            .unwrap_or(0);
        status_counts.insert(job.status.clone(), Value::from(current + 1));
        if let Some(session_id) = job.session_id.clone() {
            session_ids.push(session_id);
        }
        if let Some(lane_id) = job.lane_id.clone() {
            lane_ids.push(lane_id);
        }
        if let Some(parent_job_id) = job.parent_job_id.clone() {
            parent_job_ids.push(parent_job_id);
        }
        if is_active_status(&job.status) {
            active_job_count += 1;
        }
        if is_terminal_status(&job.status) {
            terminal_job_count += 1;
        }
        if latest_updated_at
            .as_ref()
            .map(|current| job.updated_at > *current)
            .unwrap_or(true)
        {
            latest_updated_at = Some(job.updated_at.clone());
        }
    }
    session_ids.sort();
    session_ids.dedup();
    lane_ids.sort();
    lane_ids.dedup();
    parent_job_ids.sort();
    parent_job_ids.dedup();
    BackgroundParallelGroupSummary {
        parallel_group_id: parallel_group_id.to_string(),
        job_ids,
        session_ids,
        lane_ids,
        parent_job_ids,
        status_counts,
        active_job_count,
        terminal_job_count,
        total_job_count: jobs.len(),
        latest_updated_at,
    }
}
