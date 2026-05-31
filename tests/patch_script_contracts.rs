//! Contract tests for Claude Desktop maintenance shell scripts.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn framework_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn patch_scripts_pass_bash_syntax_check() {
    for name in [
        "patch-claude-desktop-3p-cowork-egress.sh",
        "patch-claude-desktop-permission-mode.sh",
        "install-claude-desktop.sh",
        "install-claude.sh",
    ] {
        let path = framework_root().join("scripts").join(name);
        let status = Command::new("bash")
            .args(["-n", path.to_str().unwrap()])
            .status()
            .expect("bash -n");
        assert!(status.success(), "bash -n failed for {name}");
    }
}

#[test]
fn patch_permission_mode_requires_account_id() {
    let script = framework_root().join("scripts/patch-claude-desktop-permission-mode.sh");
    let output = Command::new("bash")
        .env_remove("CLAUDE_DESKTOP_ACCOUNT_ID")
        .arg(&script)
        .output()
        .expect("run patch script");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CLAUDE_DESKTOP_ACCOUNT_ID"),
        "expected account id error, got: {stderr}"
    );
}

#[test]
fn patch_permission_mode_merges_config_without_hardcoded_paths() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("claude_desktop_config.json");
    fs::write(
        &cfg,
        r#"{"preferences":{"epitaxyPrefs":{}}}"#,
    )
    .unwrap();
    let script = framework_root().join("scripts/patch-claude-desktop-permission-mode.sh");
    let output = Command::new("bash")
        .env("CLAUDE_DESKTOP_ACCOUNT_ID", "test-account-uuid")
        .env("CLAUDE_3P_DESKTOP_CONFIG", &cfg)
        .env("CLAUDE_DESKTOP_FOLDER_PATHS", "/tmp/cowork-test-folder")
        .env("COWORK_USER_FILES", "/tmp/cowork-root")
        .arg(&script)
        .arg("acceptEdits")
        .output()
        .expect("run patch");
    assert!(
        output.status.success(),
        "patch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(&cfg).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    let folders = payload["preferences"]["epitaxyPrefs"]
        .as_object()
        .and_then(|ep| ep.get("epitaxy-folder-permission-mode.test-account-uuid"))
        .and_then(|v| v.as_object())
        .expect("folder map");
    assert_eq!(
        folders.get("/tmp/cowork-test-folder").and_then(|v| v.as_str()),
        Some("acceptEdits")
    );
    assert_eq!(
        folders.get("/tmp/cowork-root").and_then(|v| v.as_str()),
        Some("acceptEdits")
    );
    assert!(!folders.contains_key("/Users/joe/Developer/skill"));
}

#[test]
fn patch_egress_updates_config_library_fixture() {
    let tmp = tempdir().unwrap();
    let lib = tmp.path().join("configLibrary");
    fs::create_dir_all(&lib).unwrap();
    let applied_id = "applied-test";
    fs::write(lib.join("_meta.json"), format!(r#"{{"appliedId":"{applied_id}"}}"#)).unwrap();
    fs::write(lib.join(format!("{applied_id}.json")), r#"{}"#).unwrap();

    let script = framework_root().join("scripts/patch-claude-desktop-3p-cowork-egress.sh");
    let output = Command::new("bash")
        .arg(&script)
        .arg("--config-library")
        .arg(&lib)
        .arg("--allow-all")
        .output()
        .expect("run egress patch");
    assert!(
        output.status.success(),
        "egress patch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(lib.join(format!("{applied_id}.json"))).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        payload["coworkEgressAllowedHosts"],
        serde_json::json!(["*"])
    );
}
