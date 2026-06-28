#![deny(clippy::unwrap_used, clippy::expect_used)]
//! Session supervisor: native-process worker lifecycle for long-running CLI hosts.

use core_errors::FrameworkError;
use rt_storage::runtime_storage::acquire_runtime_path_lock;
use serde_json::{Value, json};
use tracing::{debug, instrument};

mod driver;
mod idle_observer;
mod process;
mod runtime;
pub mod team_manager;
mod types;
mod worker;

#[cfg(test)]
mod tests;

pub mod hooks;
pub mod router_env_flags;

pub use types::AgentHealthEntry;
pub use types::AgentHealthStore;
pub use types::WorkerSessionRecord;
pub use types::{SESSION_SUPERVISOR_AUTHORITY, SESSION_SUPERVISOR_SCHEMA_VERSION};
pub use worker::classify_rate_limit_block;

use idle_observer::maybe_trigger_idle_observation;
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

fn idle_observation_side_effect(payload: &Value, workers: &[types::WorkerSessionRecord]) -> Value {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dry_run = optional_bool(payload, "dry_run").unwrap_or(false);
    let force = optional_bool(payload, "force_idle_observation").unwrap_or(false);
    let result = maybe_trigger_idle_observation(&cwd, workers, dry_run, force);
    json!({
        "triggered": result.triggered,
        "status": result.status,
    })
}

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
                    .iter_mut()
                    .find(|worker| worker.worker_id == worker_id)
                    .ok_or_else(|| {
                        FrameworkError::not_found(format!(
                            "Unknown supervisor worker_id: {worker_id}"
                        ))
                    })?;
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
            // Side-effect: reap stale agent health entries
            if let Ok(cwd) = std::env::current_dir() {
                let _ = process::reap_stale_agents(&cwd, stale_after_secs);
            }
            let idle_observation = idle_observation_side_effect(&payload, &store.workers);
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "workers": store.workers,
                "observation_idle": idle_observation,
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
            }
            save_store(&state_path, &store)?;
            let idle_observation = idle_observation_side_effect(&payload, &store.workers);
            Ok(json!({
                "schema_version": SESSION_SUPERVISOR_SCHEMA_VERSION,
                "authority": SESSION_SUPERVISOR_AUTHORITY,
                "operation": operation,
                "state_path": state_path.display().to_string(),
                "changed": true,
                "dry_run": dry_run,
                "resumed_workers": resumed_workers,
                "failed_workers": failed_workers,
                "observation_idle": idle_observation,
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
            let cwd = std::env::current_dir()?;
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
            let cwd = std::env::current_dir()?;
            process::unregister_agent(&cwd, &agent_id, &terminal_status, error.as_deref(), &now)?;
            Ok(json!({
                "operation": operation,
                "agent_id": agent_id,
                "terminal_status": terminal_status,
            }))
        }
        "agent_list_running" => {
            let cwd = std::env::current_dir()?;
            let agents = process::list_running_agents(&cwd)?;
            Ok(json!({
                "operation": operation,
                "running_agents": agents,
                "count": agents.len(),
            }))
        }
        "agent_reap_stale" => {
            let retention_secs = runtime::optional_i64(&payload, "retention_seconds").unwrap_or(0);
            let cwd = std::env::current_dir()?;
            let reaped = process::reap_stale_agents(&cwd, retention_secs)?;
            Ok(json!({
                "operation": operation,
                "reaped_count": reaped,
            }))
        }

        // ═══ Team operations ═══
        "team_create" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| team_id.clone());
            let supervisor = payload
                .get("supervisor_agent_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let cwd = std::env::current_dir()?;
            let team =
                team_manager::create_team(&cwd, &team_id, &name, supervisor.as_deref(), &now)?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "team": team,
            }))
        }
        "team_add_member" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let agent_id = required_non_empty_string(&payload, "agent_id", "team")
                .map_err(FrameworkError::validation)?;
            let role = payload
                .get("role")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| "worker".to_string());
            let host_id = required_non_empty_string(&payload, "host_id", "team")
                .map_err(FrameworkError::validation)?;
            let cwd = std::env::current_dir()?;
            let member =
                team_manager::add_team_member(&cwd, &team_id, &agent_id, &role, &host_id, &now)?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "agent_id": agent_id,
                "member": member,
            }))
        }
        "team_remove_member" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let agent_id = required_non_empty_string(&payload, "agent_id", "team")
                .map_err(FrameworkError::validation)?;
            let terminal_status = payload
                .get("terminal_status")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| "interrupted".to_string());
            let error = payload
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let cwd = std::env::current_dir()?;
            team_manager::remove_team_member(
                &cwd,
                &team_id,
                &agent_id,
                &terminal_status,
                error.as_deref(),
                &now,
            )?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "agent_id": agent_id,
                "removed": true,
            }))
        }
        "team_complete" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let cwd = std::env::current_dir()?;
            let team = team_manager::complete_team(&cwd, &team_id, &now)?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "team": team,
            }))
        }
        "team_send_message" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let from_agent = required_non_empty_string(&payload, "from_agent", "team")
                .map_err(FrameworkError::validation)?;
            let to_agent = payload
                .get("to_agent")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let msg_kind = payload
                .get("kind")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| "command".to_string());
            let msg_payload = payload.get("payload").cloned().unwrap_or_default();
            let cwd = std::env::current_dir()?;
            let msg = team_manager::send_message(
                &cwd,
                &team_id,
                &from_agent,
                to_agent.as_deref(),
                &msg_kind,
                msg_payload,
                &now,
            )?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "message": msg,
            }))
        }
        "team_read_messages" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let agent_id = required_non_empty_string(&payload, "agent_id", "team")
                .map_err(FrameworkError::validation)?;
            let cwd = std::env::current_dir()?;
            let messages = team_manager::read_my_messages(&cwd, &team_id, &agent_id)?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "agent_id": agent_id,
                "messages": messages,
                "count": messages.len(),
            }))
        }
        "team_alive_members" => {
            let team_id = required_non_empty_string(&payload, "team_id", "team")
                .map_err(FrameworkError::validation)?;
            let cwd = std::env::current_dir()?;
            let alive = team_manager::team_alive_members(&cwd, &team_id)?;
            Ok(json!({
                "operation": operation,
                "team_id": team_id,
                "alive_members": alive,
            }))
        }
        "team_list" => {
            let cwd = std::env::current_dir()?;
            let team_id_filter = payload
                .get("team_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let teams = team_manager::team_list(&cwd, team_id_filter)?;
            Ok(json!({
                "operation": operation,
                "teams": teams,
                "count": teams.len(),
            }))
        }
        "team_reap_stale" => {
            let retention_secs = runtime::optional_i64(&payload, "retention_seconds").unwrap_or(0);
            let cwd = std::env::current_dir()?;
            let reaped = team_manager::reap_stale_teams(&cwd, retention_secs)?;
            Ok(json!({
                "operation": operation,
                "reaped_teams": reaped,
            }))
        }

        other => Err(FrameworkError::unsupported(format!(
            "Unsupported session supervisor operation: {other}"
        ))),
    }
}
