#![deny(clippy::unwrap_used, clippy::expect_used)]
//! Session supervisor: native-process worker lifecycle for long-running CLI hosts.

use core_errors::FrameworkError;
use rt_storage::runtime_storage::acquire_runtime_path_lock;
use serde_json::{Value, json};
use tracing::{debug, instrument};

mod driver;
mod process;
mod runtime;
pub mod team_manager;
mod types;
mod worker;

#[cfg(test)]
mod tests;

pub mod router_env_flags;

pub use types::AgentHealthEntry;
pub use types::AgentHealthStore;
pub use types::WorkerSessionRecord;
pub use types::{SESSION_SUPERVISOR_AUTHORITY, SESSION_SUPERVISOR_SCHEMA_VERSION};
pub use worker::classify_rate_limit_block;

use process::reconcile_process_state;
use runtime::{
    load_store, now_from_payload, optional_bool, required_non_empty_string, resolve_state_path,
    save_store,
};
use types::DEFAULT_WORKER_STALE_AFTER_SECS;
use worker::{
    launch_worker, mark_worker_blocked, reap_stale_workers, resume_worker, terminate_worker,
    worker_ready_for_resume,
};

#[instrument(level = "info", skip_all, fields(operation))]
pub fn handle_session_supervisor_operation(payload: Value) -> Result<Value, FrameworkError> {
    let operation = required_non_empty_string(&payload, "operation", "session supervisor")
        .map_err(FrameworkError::validation)?;
    debug!(%operation, "session supervisor operation");
    let state_path = resolve_state_path(&payload)?;

    if operation == "classify_block" {
        let host = required_non_empty_string(&payload, "host", "session supervisor")
            .map_err(FrameworkError::validation)?;
        let evidence_text =
            required_non_empty_string(&payload, "evidence_text", "session supervisor")
                .map_err(FrameworkError::validation)?;
        let classification = classify_rate_limit_block(&host, &evidence_text)?;
        return Ok(json!({
            "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
            "authority": SESSION_SUPERVISOR_AUTHORITY,
            "operation": operation,
            "state_path": state_path.display().to_string(),
            "changed": false,
            "classification": classification,
        }));
    }

    let dry_run = optional_bool(&payload, "dry_run").unwrap_or(false);
    let now = now_from_payload(&payload)?;

    let _store_lock = acquire_runtime_path_lock(&state_path)?;
    let mut store = load_store(&state_path)?;

    match operation.as_str() {
        "launch" => {
            let worker = launch_worker(&payload, &mut store, &state_path, dry_run, &now)?;
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
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")
                .map_err(FrameworkError::validation)?;
            let worker_snapshot = {
                let worker = store
                    .workers
                    .iter()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| {
                        FrameworkError::not_found(format!(
                            "Unknown supervisor worker_id: {worker_id}"
                        ))
                    })?;
                worker.clone()
            };
            // inspect is read-only — no save_store, no reconcile_process_state.
            // Call `list` to trigger state reconciliation and cleanup.
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": false,
                "worker": worker_snapshot,
            }))
        }
        // NOTE: `list` intentionally has write side effects — it reconciles
        // process states, reaps stale workers/agents, compacts terminated
        // records, and cleans up orphaned logs. This is by design: `list`
        // is the primary housekeeping entry point.
        "list" => {
            let stale_after_secs = runtime::optional_i64(&payload, "stale_after_secs")
                .unwrap_or(DEFAULT_WORKER_STALE_AFTER_SECS);
            reap_stale_workers(&mut store.workers, &now, stale_after_secs)?;
            for worker in &mut store.workers {
                reconcile_process_state(worker, &now);
            }
            // Compact after reconcile so newly-terminated workers are cleaned up
            // in the same pass.
            runtime::compact_terminated_workers(&mut store.workers, &now);
            save_store(&state_path, &store)?;
            // Side-effect: reap stale agent health entries (use the stale-after
            // value as retention — agents terminal longer than the worker stale
            // threshold are removed)
            if let Ok(cwd) = std::env::current_dir() {
                let _ = process::reap_stale_agents(&cwd, stale_after_secs);
            }
            // Side-effect: clean up orphaned worker log files
            observability_core::cleanup_stale_logs(&state_path, &store.workers.iter().map(|w| w.worker_id.clone()).collect::<Vec<_>>());
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
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")
                .map_err(FrameworkError::validation)?;
            let (worker_snapshot, terminated) = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| {
                        FrameworkError::not_found(format!(
                            "Unknown supervisor worker_id: {worker_id}"
                        ))
                    })?;
                let terminated = terminate_worker(worker, dry_run, &now)?;
                (worker.clone(), terminated)
            };
            runtime::compact_terminated_workers(&mut store.workers, &now);
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
        "mark_blocked" => {
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")
                .map_err(FrameworkError::validation)?;
            let (worker_snapshot, classification) = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| {
                        FrameworkError::not_found(format!(
                            "Unknown supervisor worker_id: {worker_id}"
                        ))
                    })?;
                let classification = mark_worker_blocked(worker, &payload, &now)?;
                (worker.clone(), classification)
            };
            // Compact terminated workers opportunistically — the just-modified
            // worker is blocked_rate_limit (not terminal), but other workers
            // may have aged out.
            runtime::compact_terminated_workers(&mut store.workers, &now);
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

            // Phase 1: identify due workers under the outer lock.
            let due_ids: Vec<String> = store
                .workers
                .iter()
                .filter_map(|w| {
                    worker_ready_for_resume(w, &now)
                        .ok()
                        .filter(|&ready| ready)
                        .map(|_| w.worker_id.clone())
                })
                .collect();

            // Release the outer lock before process spawning.
            // resume_worker → launch_process calls fork+exec, which would
            // block all other supervisor operations if held. Each worker
            // gets its own short lock transaction.
            drop(_store_lock);
            drop(store);

            for worker_id in &due_ids {
                let _lock = acquire_runtime_path_lock(&state_path)?;
                let mut store = load_store(&state_path)?;

                let Some(worker) = store
                    .workers
                    .iter_mut()
                    .find(|w| w.worker_id == *worker_id)
                else {
                    continue; // removed concurrently — skip
                };

                // Re-check readiness under the per-worker lock to prevent
                // double-resume from concurrent resume_due callers.
                if !worker_ready_for_resume(worker, &now)? {
                    continue;
                }

                match resume_worker(worker, &state_path, dry_run, &now) {
                    Ok(action) => resumed_workers.push(json!({
                        "worker_id": worker.worker_id,
                        "status": worker.status,
                        "action": action,
                        "worker": worker,
                    })),
                    Err(err) => {
                        worker.status = "failed".to_string();
                        worker.last_error = Some(err.to_string());
                        worker.updated_at = now.clone();
                        runtime::push_event(
                            worker,
                            "resume_failed",
                            "failed",
                            &now,
                            Some(err.to_string()),
                        );
                        failed_workers.push(json!({
                            "worker_id": worker.worker_id,
                            "status": worker.status,
                            "error": err.to_string(),
                            "worker": worker,
                        }));
                    }
                }
                save_store(&state_path, &store)?;
                // _lock dropped — per-worker transaction released
            }

            // Final compact pass: clean up terminated workers that accumulated
            // during the per-worker processing.
            {
                let _lock = acquire_runtime_path_lock(&state_path)?;
                let mut store = load_store(&state_path)?;
                runtime::compact_terminated_workers(&mut store.workers, &now);
                save_store(&state_path, &store)?;
            }

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

        // ═══ Agent health operations ═══
        "agent_register" => {
            let agent_id = required_non_empty_string(&payload, "agent_id", "agent health")
                .map_err(FrameworkError::validation)?;
            let host_id = required_non_empty_string(&payload, "host_id", "agent health")
                .map_err(FrameworkError::validation)?;
            let tool_type = required_non_empty_string(&payload, "tool_type", "agent health")
                .unwrap_or_else(|_| {
                    tracing::warn!("agent_register missing tool_type, defaulting to 'agent'");
                    "agent".to_string()
                });
            // Prefer explicit repo_root from payload; fall back to cwd for backward compat.
            let cwd = payload
                .get("repo_root")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            process::register_agent_alive(&cwd, &agent_id, &host_id, &tool_type, &now)?;
            Ok(json!({
                "operation": operation,
                "agent_id": agent_id,
                "registered": true,
            }))
        }
        "agent_unregister" => {
            let agent_id = required_non_empty_string(&payload, "agent_id", "agent health")
                .map_err(FrameworkError::validation)?;
            let terminal_status = payload
                .get("terminal_status")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| "completed".to_string());
            let error = payload
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let cwd = payload
                .get("repo_root")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            process::unregister_agent(&cwd, &agent_id, &terminal_status, error.as_deref(), &now)?;
            Ok(json!({
                "operation": operation,
                "agent_id": agent_id,
                "terminal_status": terminal_status,
            }))
        }
        // agent_list_running: removed (no external callers)
        // agent_reap_stale: removed (no external callers)
        // team_* operations: removed (no external callers)

        other => Err(FrameworkError::unsupported(format!(
            "Unsupported session supervisor operation: {other}"
        ))),
    }
}
