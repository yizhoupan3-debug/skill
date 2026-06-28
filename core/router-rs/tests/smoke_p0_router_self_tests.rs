//! `router_self` binary install/validate/dispatch smoke.

use crate::router_self::{
    install_router_rs_for_desktop_mcp_at, install_router_rs_to_bin_dir,
    is_ephemeral_router_rs_path, is_repo_build_router_rs_path, router_rs_desktop_mcp_path_for_home,
    validate_router_rs_binary_runnable,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_home(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-p0-router-self-{label}-{nonce}"))
}

fn try_resolve_router_rs_test_bin() -> Option<PathBuf> {
    if let Some(path) = option_env!("CARGO_BIN_EXE_router-rs-cli") {
        return Some(PathBuf::from(path));
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_router-rs-cli") {
        return Some(PathBuf::from(path));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidate in [
        manifest.join("../../target/debug/router-rs-cli"),
        manifest.join("../target/debug/router-rs-cli"),
        PathBuf::from("/tmp/skill-cargo-target/debug/router-rs-cli"),
        manifest.join("../../target/release/router-rs-cli"),
        manifest.join("../target/release/router-rs-cli"),
        PathBuf::from("/tmp/skill-cargo-target/release/router-rs-cli"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target).join("debug/router-rs-cli");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Installed copy must pass `framework --help` smoke (P0 binary validation path).
#[test]
fn router_self_install_copy_passes_framework_help_smoke() {
    let Some(bin) = try_resolve_router_rs_test_bin() else {
        eprintln!("skip: router-rs binary not built (per-crate test run)");
        return;
    };
    validate_router_rs_binary_runnable(&bin).expect("prebuilt binary smoke");

    #[cfg(unix)]
    {
        let dir = temp_home("install-validate");
        fs::create_dir_all(&dir).expect("mkdir");
        let installed = install_router_rs_to_bin_dir(Some(dir.clone())).expect("install");
        validate_router_rs_binary_runnable(&installed).expect("installed copy smoke");
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Desktop MCP install is idempotent when source bytes are unchanged (P0 distribution path).
#[cfg(unix)]
#[test]
fn router_self_desktop_mcp_idempotent_install_smoke() {
    let home = temp_home("desktop-mcp-idempotent");
    fs::create_dir_all(&home).expect("mkdir home");
    let first = install_router_rs_for_desktop_mcp_at(&home).expect("first install");
    let first_bytes = fs::read(&first).expect("first bytes");
    let second = install_router_rs_for_desktop_mcp_at(&home).expect("second install");
    assert_eq!(first, second);
    assert_eq!(second, router_rs_desktop_mcp_path_for_home(&home));
    assert_eq!(
        fs::read(&second).expect("second bytes"),
        first_bytes,
        "unchanged source must preserve installed bytes"
    );
    let _ = fs::remove_dir_all(&home);
}

/// Ephemeral `/tmp` and sandbox paths must not qualify as stable repo builds (P0 path classification).
#[test]
fn router_self_ephemeral_vs_repo_build_classification_smoke() {
    assert!(is_ephemeral_router_rs_path(
        "/tmp/skill-cargo-target/debug/router-rs"
    ));
    assert!(is_ephemeral_router_rs_path("/tmp/router-rs-probe"));
    assert!(!is_ephemeral_router_rs_path(
        "/Users/joe/.local/bin/router-rs"
    ));

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let framework_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("framework root");
    let repo_debug = framework_root.join("target/debug/router-rs");
    assert!(is_repo_build_router_rs_path(
        &repo_debug.to_string_lossy(),
        framework_root
    ));
    assert!(!is_repo_build_router_rs_path(
        "/tmp/other/target/debug/router-rs",
        framework_root
    ));
}

/// Missing binary path fails closed with an actionable message (P0 validation guard).
#[test]
fn router_self_validate_rejects_missing_binary_smoke() {
    let missing = temp_home("missing-bin");
    let err = validate_router_rs_binary_runnable(&missing).unwrap_err();
    assert!(
        err.to_string().contains("missing"),
        "expected missing-file error, got: {err}"
    );
}
