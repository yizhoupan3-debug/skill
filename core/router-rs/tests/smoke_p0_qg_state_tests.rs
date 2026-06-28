//! `state_manager/rfv_state` coverage at router-rs boundary
//! (physical module: `core/core-state` → `core_state::state_manager`).

use crate::atomic_write::write_atomic_json;
use crate::harness_context_signals::quality_gate_state_signals_math;
use core_state::state_manager::{
    read_rfv_loop_state, rfv_loop_state_path, write_active_task_pointer,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-p0-rfv-{name}-{nonce}"))
}

/// RFV loop state read honors explicit task_id and active_task pointer (core-state P0).
#[test]
fn rfv_state_read_round_trip_smoke() {
    let repo = temp_repo("read-round-trip");
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");

    let path = rfv_loop_state_path(&repo, "rfv-task").expect("path");
    let state = json!({
        "schema_version": "router-rs-quality-gate-v1",
        "loop_status": "active",
        "goal": "prove convergence",
    });
    write_atomic_json(&path, &state).expect("write rfv");

    let read = read_rfv_loop_state(&repo, Some("rfv-task"))
        .expect("read")
        .expect("some");
    assert_eq!(read["loop_status"], json!("active"));

    write_active_task_pointer(&repo, "rfv-task").expect("pointer");
    let via_active = read_rfv_loop_state(&repo, None)
        .expect("read active")
        .expect("some");
    assert_eq!(via_active["goal"], json!("prove convergence"));

    let _ = fs::remove_dir_all(&repo);
}

/// Harness math/formal signal gate over RFV `goal` + `verify_commands` (router-rs P0).
#[test]
fn quality_gate_state_signals_math_smoke() {
    let formal = json!({
        "goal": "lint only",
        "verify_commands": ["python -c \"import sympy\""]
    });
    assert!(quality_gate_state_signals_math(&formal));
    let benign = json!({
        "goal": "cargo fmt",
        "verify_commands": ["cargo test -q"]
    });
    assert!(!quality_gate_state_signals_math(&benign));
}
