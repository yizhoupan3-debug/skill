//! Session supervisor: native-process worker lifecycle for long-running CLI hosts.

use crate::runtime_storage::acquire_runtime_path_lock;
use serde_json::{json, Value};

mod driver;
mod evolution_idle;
mod process;
mod runtime;
mod types;
mod worker;

#[cfg(test)]
mod tests;

pub use types::{SESSION_SUPERVISOR_AUTHORITY, SESSION_SUPERVISOR_SCHEMA_VERSION};
pub use worker::classify_rate_limit_block;

use process::reconcile_process_state;
use runtime::{load_store, now_from_payload, optional_bool, required_non_empty_string, resolve_state_path, save_store};
use evolution_idle::maybe_trigger_evolution_on_idle;
use worker::{
    launch_worker, mark_worker_blocked, reap_stale_workers, resume_worker, terminate_worker,
    worker_ready_for_resume,
};
use types::DEFAULT_WORKER_STALE_AFTER_SECS;

fn evolution_idle_side_effect(
    payload: &Value,
    workers: &[types::WorkerSessionRecord],
) -> Value {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dry_run = optional_bool(payload, "dry_run").unwrap_or(false);
    let force = optional_bool(payload, "force_evolution_idle").unwrap_or(false);
    let result = maybe_trigger_evolution_on_idle(&cwd, workers, dry_run, force);
    json!({
        "triggered": result.triggered,
        "status": result.status,
    })
}

pub fn handle_session_supervisor_operation(payload: Value) -> Result<Value, String> {
    let operation = required_non_empty_string(&payload, "operation", "session supervisor")?;
    let state_path = resolve_state_path(&payload)?;

    if operation == "classify_block" {
        let host = required_non_empty_string(&payload, "host", "session supervisor")?;
        let evidence_text =
            required_non_empty_string(&payload, "evidence_text", "session supervisor")?;
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
            let worker_id = required_non_empty_string(&payload, "worker_id", "session supervisor")?;
            let worker_snapshot = {
                let worker = store
                    .workers
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| format!("Unknown supervisor worker_id: {worker_id}"))?;
                reconcile_process_state(worker);
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
            let stale_after_secs = runtime::optional_i64(&payload, "stale_after_secs")
                .unwrap_or(DEFAULT_WORKER_STALE_AFTER_SECS);
            reap_stale_workers(&mut store.workers, &now, stale_after_secs)?;
            for worker in &mut store.workers {
                reconcile_process_state(worker);
            }
            save_store(&state_path, &store)?;
            let evolution_idle = evolution_idle_side_effect(&payload, &store.workers);
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "workers": store.workers,
                "evolution_idle": evolution_idle,
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
                match resume_worker(worker, &state_path, dry_run, &now) {
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
                        runtime::push_event(worker, "resume_failed", "failed", &now, Some(err.clone()));
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
            let evolution_idle = evolution_idle_side_effect(&payload, &store.workers);
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "dry_run": dry_run,
                "resumed_workers": resumed_workers,
                "failed_workers": failed_workers,
                "evolution_idle": evolution_idle,
            }))
        }
        other => Err(format!("Unsupported session supervisor operation: {other}")),
    }
}
