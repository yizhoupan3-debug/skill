//! `state_manager/atomic_write` coverage
//! (physical module: `core-state/utils/atomic_write.rs`, re-exported as `crate::atomic_write`).

use crate::atomic_write::{write_atomic_json, write_atomic_text};
use serde_json::json;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(label: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-atomic-{label}-{suffix}"))
}

/// Minimal P0 smoke for atomic text/json writes (Roadmap §6.2 #5).
#[test]
fn atomic_write_round_trip_smoke() {
    let path = temp_path("text");
    let _ = fs::remove_file(&path);
    write_atomic_text(&path, "p0-smoke").expect("write text");
    assert_eq!(fs::read_to_string(&path).unwrap(), "p0-smoke");
    assert!(!path.with_extension("tmp").exists());

    let json_path = temp_path("json");
    let _ = fs::remove_file(&json_path);
    let value = json!({"lane": "p0", "n": 1});
    write_atomic_json(&json_path, &value).expect("write json");
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();
    assert_eq!(parsed, value);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&json_path);
}
