use super::control_plane::normalized_backend_family;
use super::persist::{handle_background_state_operation, is_mutating_background_operation};
use super::status::validate_transition;
use super::types::{
    BACKGROUND_STATE_REQUEST_SCHEMA_VERSION, BACKGROUND_STATE_SCHEMA_VERSION, BackgroundRunStatus,
    BackgroundStateStore, STALE_ACTIVE_HEARTBEAT_TTL_SECS, STALE_TERMINAL_JOB_TTL_SECS,
};
use super::*;
use chrono::Utc;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn validate_transition_rejects_unknown_prior_status() {
    // Strict-FSM: any unrecognized prior status (legacy data, hand-edits,
    // disk corruption) must NOT silently transition to anything. Previously
    // the wildcard arm returned `true` and let zombie statuses propagate.
    assert!(validate_transition(Some("ghost_status"), "running").is_err());
    assert!(validate_transition(Some("legacy_v0"), "completed").is_err());
    // Known transitions still work.
    assert!(validate_transition(None, "queued").is_ok());
    assert!(validate_transition(Some("running"), "completed").is_ok());
    assert!(validate_transition(Some("interrupted"), "interrupted").is_ok());
    // Known invalid transitions still rejected.
    assert!(validate_transition(Some("completed"), "running").is_err());
}

fn make_test_store() -> BackgroundStateStore {
    BackgroundStateStore {
        state_path: PathBuf::from("/tmp/router-rs-reaper-test-state.json"),
        backend_family: "filesystem".to_string(),
        sqlite_db_path: None,
        control_plane: Value::Object(serde_json::Map::new()),
        jobs: HashMap::new(),
        active_sessions: HashMap::new(),
        pending_session_takeovers: HashMap::new(),
        reaped_dirty: false,
    }
}

fn make_job(id: &str, status: &str, updated_at: &str) -> BackgroundRunStatus {
    BackgroundRunStatus {
        job_id: id.to_string(),
        session_id: Some(format!("session-{id}")),
        status: status.to_string(),
        updated_at: updated_at.to_string(),
        ..Default::default()
    }
}

#[test]
fn reap_stale_active_jobs_marks_old_running_as_interrupted() {
    // Heartbeat ttl = 3600s; a `running` job last seen 2h ago is stale.
    let mut store = make_test_store();
    let now = Utc::now();
    let stale_ts =
        (now - chrono::Duration::seconds(STALE_ACTIVE_HEARTBEAT_TTL_SECS + 1800)).to_rfc3339();
    let fresh_ts = (now - chrono::Duration::seconds(60)).to_rfc3339();
    store
        .jobs
        .insert("stale".to_string(), make_job("stale", "running", &stale_ts));
    store
        .jobs
        .insert("fresh".to_string(), make_job("fresh", "running", &fresh_ts));
    store
        .active_sessions
        .insert("session-stale".to_string(), "stale".to_string());

    let reaped = store.reap_stale_active_jobs(now);
    assert_eq!(reaped, 1);
    let stale_job = store.jobs.get("stale").expect("stale job kept");
    assert_eq!(stale_job.status, "interrupted");
    assert!(stale_job.interrupted_at.is_some());
    assert!(
        stale_job
            .error
            .as_deref()
            .map(|e| e.contains("heartbeat stale"))
            .unwrap_or(false)
    );
    // session reservation released
    assert!(!store.active_sessions.contains_key("session-stale"));
    // fresh job untouched
    assert_eq!(store.jobs.get("fresh").unwrap().status, "running");
}

#[test]
fn reap_stale_terminal_jobs_drops_old_terminal() {
    // Terminal-TTL = 24h; older terminal jobs should be dropped wholesale.
    let mut store = make_test_store();
    let now = Utc::now();
    let stale_ts = (now - chrono::Duration::seconds(STALE_TERMINAL_JOB_TTL_SECS + 60)).to_rfc3339();
    let fresh_ts = (now - chrono::Duration::seconds(3600)).to_rfc3339();
    store.jobs.insert(
        "old-completed".to_string(),
        make_job("old-completed", "completed", &stale_ts),
    );
    store.jobs.insert(
        "recent-completed".to_string(),
        make_job("recent-completed", "completed", &fresh_ts),
    );
    store.jobs.insert(
        "running".to_string(),
        make_job("running", "running", &stale_ts),
    );

    let reaped = store.reap_stale_terminal_jobs(now);
    assert_eq!(reaped, 1);
    assert!(!store.jobs.contains_key("old-completed"));
    assert!(store.jobs.contains_key("recent-completed"));
    // active job (even if old) untouched by terminal reaper
    assert!(store.jobs.contains_key("running"));
}

#[test]
fn reap_ghost_status_jobs_marks_unknown_as_interrupted() {
    // Ghost status (not in active or terminal FSM) must be force-converted
    // to `interrupted` with diagnostic so terminal-TTL eventually drops
    // them and operators can see the corruption.
    let mut store = make_test_store();
    let now = Utc::now();
    let ts = (now - chrono::Duration::seconds(60)).to_rfc3339();
    store.jobs.insert(
        "ghost".to_string(),
        make_job("ghost", "weird_legacy_status", &ts),
    );
    store
        .jobs
        .insert("ok".to_string(), make_job("ok", "running", &ts));
    store
        .active_sessions
        .insert("session-ghost".to_string(), "ghost".to_string());
    store
        .pending_session_takeovers
        .insert("session-x".to_string(), "ghost".to_string());

    let reaped = store.reap_ghost_status_jobs(now);
    assert_eq!(reaped, 1);
    let ghost = store.jobs.get("ghost").expect("ghost job kept after reap");
    assert_eq!(ghost.status, "interrupted");
    assert!(
        ghost
            .error
            .as_deref()
            .map(|e| e.contains("ghost_status") && e.contains("weird_legacy_status"))
            .unwrap_or(false)
    );
    // active maps released
    assert!(!store.active_sessions.contains_key("session-ghost"));
    assert!(!store.pending_session_takeovers.contains_key("session-x"));
    // healthy job untouched
    assert_eq!(store.jobs.get("ok").unwrap().status, "running");
}

#[test]
fn reap_preserves_recent_jobs() {
    // Sanity: nothing should be reaped when all jobs are within TTL.
    let mut store = make_test_store();
    let now = Utc::now();
    let recent = (now - chrono::Duration::seconds(60)).to_rfc3339();
    store
        .jobs
        .insert("a".to_string(), make_job("a", "running", &recent));
    store
        .jobs
        .insert("b".to_string(), make_job("b", "completed", &recent));
    let active_reaped = store.reap_stale_active_jobs(now);
    let terminal_reaped = store.reap_stale_terminal_jobs(now);
    let ghost_reaped = store.reap_ghost_status_jobs(now);
    assert_eq!(active_reaped, 0);
    assert_eq!(terminal_reaped, 0);
    assert_eq!(ghost_reaped, 0);
    assert_eq!(store.jobs.len(), 2);
}

#[test]
fn is_mutating_background_operation_classifies_correctly() {
    // Read-only ops must be classified as non-mutating so they don't
    // trigger reap-flush disk writes.
    for op in [
        "snapshot",
        "get",
        "get_active_job",
        "parallel_group_summary",
        "parallel_group_summaries",
        "health",
    ] {
        assert!(
            !is_mutating_background_operation(op),
            "{op} should be read-only"
        );
    }
    for op in [
        "apply_mutation",
        "arbitrate_session_takeover",
        "reserve",
        "claim",
        "release",
    ] {
        assert!(
            is_mutating_background_operation(op),
            "{op} should be mutating"
        );
    }
}

#[test]
fn snapshot_drops_dangling_session_mappings() {
    let persisted = json!({
        "version": 2,
        "schema_version": BACKGROUND_STATE_SCHEMA_VERSION,
        "control_plane": null,
        "jobs": [],
        "active_sessions": [{"session_id": "s-1", "job_id": "missing-job"}],
        "pending_session_takeovers": [{"session_id": "s-2", "incoming_job_id": "missing-job"}]
    });
    let response = handle_background_state_operation(json!({
        "schema_version": BACKGROUND_STATE_REQUEST_SCHEMA_VERSION,
        "operation": "snapshot",
        "state_path": "/tmp/router-rs-background-state-test.json",
        "backend_family": "memory",
        "state_payload_text": format!("{}\n", persisted)
    }))
    .expect("snapshot should parse persisted state");
    let state = response
        .get("state")
        .expect("snapshot response state payload");
    assert_eq!(
        state
            .get("active_sessions")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        state
            .get("pending_session_takeovers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn concurrent_job_insertions_preserve_all_jobs() {
    // Verify thread safety: multiple threads inserting different jobs
    // should all be present after concurrent access.
    let store = Arc::new(std::sync::Mutex::new(make_test_store()));
    let now = Utc::now().to_rfc3339();
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let store = Arc::clone(&store);
            let now = now.clone();
            std::thread::spawn(move || {
                let mut s = store.lock().unwrap();
                s.jobs.insert(
                    format!("job-{i}"),
                    make_job(&format!("job-{i}"), "running", &now),
                );
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let s = store.lock().unwrap();
    assert_eq!(s.jobs.len(), 8);
    for i in 0..8 {
        assert!(s.jobs.contains_key(&format!("job-{i}")));
    }
}

#[test]
fn state_persistence_roundtrip_via_memory_backend() {
    // Write state via snapshot, then read it back to verify roundtrip
    let now = Utc::now().to_rfc3339();
    let persisted = json!({
        "version": 2,
        "schema_version": BACKGROUND_STATE_SCHEMA_VERSION,
        "control_plane": null,
        "jobs": [
            {
                "job_id": "j1",
                "session_id": "s1",
                "status": "running",
                "created_at": &now,
                "updated_at": &now
            }
        ],
        "active_sessions": [{"session_id": "s1", "job_id": "j1"}],
        "pending_session_takeovers": []
    });
    let response = handle_background_state_operation(json!({
        "schema_version": BACKGROUND_STATE_REQUEST_SCHEMA_VERSION,
        "operation": "snapshot",
        "state_path": "/tmp/router-rs-roundtrip-test.json",
        "backend_family": "memory",
        "state_payload_text": format!("{}\n", persisted)
    }))
    .expect("snapshot should succeed");
    let state = response.get("state").expect("state in response");
    let jobs = state.get("jobs").and_then(Value::as_array).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].get("job_id").and_then(Value::as_str), Some("j1"));
    assert_eq!(
        jobs[0].get("status").and_then(Value::as_str),
        Some("running")
    );
    let sessions = state
        .get("active_sessions")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(sessions.len(), 1);
}
