//! `state_manager/task_pointers` coverage at router-rs boundary
//! (physical module: `core_state::state_manager`).

use core_state::state_manager::{
    neutralize_task_pointers_for_task, read_task_pointer_pair, read_primary_task_id,
    sync_task_pointers_after_goal_drive, write_active_task_pointer,
};
use core_state::task_state::read_task_pointers;
use crate::framework_runtime::{
    build_framework_runtime_snapshot_envelope, write_framework_session_artifacts,
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
    std::env::temp_dir().join(format!("router-rs-p0-pointers-{name}-{nonce}"))
}

/// core-state goal-drive sync writes TASK_POINTERS.json; router runtime snapshot reads it.
#[test]
fn task_pointers_read_sync_smoke() {
    let repo = temp_repo("read-sync");
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

    write_active_task_pointer(&repo, "ptr-read").expect("active");
    sync_task_pointers_after_goal_drive(
        &repo,
        "ptr-read",
        "read label",
        &json!({"set_focus": true}),
    )
    .expect("sync");

    let (active, focus) = read_task_pointer_pair(&repo);
    assert_eq!(active.as_deref(), Some("ptr-read"));
    assert_eq!(focus.as_deref(), Some("ptr-read"));
    let pointers = read_task_pointers(&repo);
    assert_eq!(pointers.active_task_id.as_deref(), Some("ptr-read"));
    assert_eq!(pointers.focus_task_id.as_deref(), Some("ptr-read"));
    assert_eq!(read_primary_task_id(&repo).as_deref(), Some("ptr-read"));

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo, None, None).expect("snapshot");
    let runtime = &snapshot["runtime_snapshot"];
    assert_eq!(runtime["active_task_id"], json!("ptr-read"));
    assert_eq!(runtime["focus_task_id"], json!("ptr-read"));
    assert!(
        runtime["known_task_ids"]
            .as_array()
            .is_some_and(|ids| ids.contains(&json!("ptr-read"))),
        "registry row visible in snapshot"
    );

    let pointers_path = repo.join("artifacts/current/TASK_POINTERS.json");
    assert!(pointers_path.is_file(), "consolidated pointers file");

    let _ = fs::remove_dir_all(&repo);
}

/// router-rs session artifact write updates TASK_POINTERS registry; core-state read stays aligned.
#[test]
fn task_pointers_write_sync_smoke() {
    let repo = temp_repo("write-sync");
    let _ = fs::remove_dir_all(&repo);
    let output_dir = repo.join("artifacts/current");
    fs::create_dir_all(&output_dir).expect("mkdir");

    sync_task_pointers_after_goal_drive(
        &repo,
        "ptr-write",
        "seed label",
        &json!({"set_focus": true}),
    )
    .expect("seed pointers");

    write_framework_session_artifacts(json!({
        "repo_root": repo,
        "output_dir": output_dir,
        "task_id": "ptr-write",
        "task": "router write sync",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "P0 task_pointers write sync smoke.",
        "focus": true,
        "next_actions": ["Verify pointers"],
        "evidence": [],
    }))
    .expect("session write");

    let (active, focus) = read_task_pointer_pair(&repo);
    assert_eq!(active.as_deref(), Some("ptr-write"));
    assert_eq!(focus.as_deref(), Some("ptr-write"));

    let consolidated: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo.join("artifacts/current/TASK_POINTERS.json")).expect("read"),
    )
    .expect("parse");
    let task_row = consolidated["tasks"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.get("task_id")
                    .and_then(serde_json::Value::as_str)
                    == Some("ptr-write")
            })
        })
        .expect("registry row");
    assert_eq!(task_row["task"], json!("router write sync"));
    assert_eq!(task_row["phase"], json!("implementation"));

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo, None, None).expect("snapshot");
    assert_eq!(
        snapshot["runtime_snapshot"]["active_task_id"],
        json!("ptr-write")
    );

    let _ = fs::remove_dir_all(&repo);
}

/// core-state neutralize clears pointers; router snapshot reflects empty active/focus.
#[test]
fn task_pointers_neutralize_sync_smoke() {
    let repo = temp_repo("neutralize");
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

    sync_task_pointers_after_goal_drive(
        &repo,
        "ptr-done",
        "done",
        &json!({"set_focus": true}),
    )
    .expect("sync");
    neutralize_task_pointers_for_task(&repo, "ptr-done").expect("neutralize");

    let pointers = read_task_pointers(&repo);
    assert!(pointers.active_task_id.is_none());
    assert!(pointers.focus_task_id.is_none());

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo, None, None).expect("snapshot");
    let runtime = &snapshot["runtime_snapshot"];
    assert!(
        runtime["active_task_id"].is_null()
            || runtime["active_task_id"].as_str().is_some_and(str::is_empty)
    );

    let _ = fs::remove_dir_all(&repo);
}

/// `set_focus: false` keeps active pointer but clears focus; router snapshot must agree.
#[test]
fn task_pointers_set_focus_false_router_snapshot_smoke() {
    let repo = temp_repo("focus-false");
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

    write_active_task_pointer(&repo, "ptr-active-only").expect("active");
    sync_task_pointers_after_goal_drive(
        &repo,
        "ptr-active-only",
        "no focus",
        &json!({"set_focus": false}),
    )
    .expect("sync without focus");

    let (active, focus) = read_task_pointer_pair(&repo);
    assert_eq!(active.as_deref(), Some("ptr-active-only"));
    assert!(focus.is_none());
    let pointers = read_task_pointers(&repo);
    assert_eq!(pointers.active_task_id.as_deref(), Some("ptr-active-only"));
    assert!(pointers.focus_task_id.is_none());

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo, None, None).expect("snapshot");
    let runtime = &snapshot["runtime_snapshot"];
    assert_eq!(runtime["active_task_id"], json!("ptr-active-only"));
    assert!(
        runtime["focus_task_id"].is_null()
            || runtime["focus_task_id"].as_str().is_some_and(str::is_empty)
    );

    let _ = fs::remove_dir_all(&repo);
}
