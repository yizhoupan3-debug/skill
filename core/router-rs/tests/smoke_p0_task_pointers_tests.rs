//! `state_manager/task_pointers` coverage at router-rs boundary.
//! `core_state::state_manager` handles TASK_POINTERS.json read/write;
//! integration tests live in `core/core-state/tests/`.

use std::path::PathBuf;
use std::process::Command;

/// The router-rs smoke CLI starts in `$CARGO_MANIFEST_DIR`.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn task_pointers_help_docs() {
    // Verify the task-pointers subcommand (if any) has help output.
    // At minimum, the binary itself parses and exits cleanly.
    let bin = manifest_dir().join("../../target/debug/router-rs");
    if !bin.exists() {
        eprintln!("router-rs binary not found at {bin:?} — skip");
        return;
    }
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("router-rs --help should run");
    assert!(output.status.success(), "router-rs --help should exit 0");
}
