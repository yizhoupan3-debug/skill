//! Integration test: codex user hooks installer preserves existing hooks and updates config.
//! Replaces `.cursor/hook-tests/test_install_codex_cli_hooks.py`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn router_rs_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    // CARGO_TARGET_DIR env (workspace-level target dir) — prefer router-rs-cli (actual binary)
    if let Ok(d) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(PathBuf::from(&d).join("release/router-rs-cli"));
        candidates.push(PathBuf::from(&d).join("debug/router-rs-cli"));
    }
    // Common /tmp target dir
    candidates.push(PathBuf::from("/tmp/skill-cargo-target/release/router-rs-cli"));
    candidates.push(PathBuf::from("/tmp/skill-cargo-target/debug/router-rs-cli"));
    // In-crate target dir
    candidates.push(manifest_dir.join("target/release/router-rs-cli"));
    candidates.push(manifest_dir.join("target/debug/router-rs-cli"));
    // Installed binary (may be the real one even if target has stub)
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local/bin/router-rs"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/router-rs"));
    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    panic!("router-rs binary not found; run cargo build --manifest-path core/router-rs/Cargo.toml --release")
}

fn run_installer(codex_home: &std::path::Path) -> std::process::Output {
    let bin = router_rs_bin();
    Command::new(bin)
        .args([
            "framework",
            "maint",
            "install-codex-user-hooks",
            "--codex-home",
            codex_home.to_str().unwrap(),
        ])
        .env("CODEX_HOME", codex_home)
        .env("HOME", codex_home.parent().unwrap())
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()) // repo root
        .output()
        .expect("failed to run installer")
}

#[test]
fn test_preserves_existing_event_hooks() {
    let tmp = tempfile::tempdir().unwrap();
    let codex_home = tmp.path();
    let hooks_path = codex_home.join("hooks.json");

    // Write existing hooks with a custom Stop hook
    let existing_hooks = serde_json::json!({
        "hooks": {
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": "/usr/bin/env echo existing",
                    "timeout": 5,
                    "statusMessage": "existing"
                }]
            }]
        }
    });
    fs::write(&hooks_path, serde_json::to_string_pretty(&existing_hooks).unwrap() + "\n").unwrap();

    let result = run_installer(codex_home);
    assert!(result.status.success(), "installer failed: {}", String::from_utf8_lossy(&result.stderr));

    let data: serde_json::Value = serde_json::from_str(&fs::read_to_string(&hooks_path).unwrap()).unwrap();
    let stop_entries = data["hooks"]["Stop"].as_array().unwrap();

    // Collect all commands from Stop hooks
    let commands: Vec<String> = stop_entries
        .iter()
        .flat_map(|entry| {
            entry["hooks"]
                .as_array()
                .map(|hooks| {
                    hooks
                        .iter()
                        .filter_map(|h| h["command"].as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();

    assert!(
        commands.iter().any(|c| c.contains("/usr/bin/env echo existing")),
        "existing stop hook should be preserved"
    );

    // Find the managed router hook
    let router_hooks: Vec<&String> = commands
        .iter()
        .filter(|c| {
            c.contains("codex hook --event=Stop")
                || (c.contains("codex-router-rs-hook.sh") && c.contains(" Stop"))
        })
        .collect();

    assert_eq!(router_hooks.len(), 1, "expected exactly one managed Stop command hook");

    let gate_cmd = router_hooks[0];
    assert!(
        gate_cmd.contains("git rev-parse --show-toplevel")
            || gate_cmd.contains("CODEX_PROJECT_ROOT")
            || gate_cmd.contains("SKILL_FRAMEWORK_ROOT"),
        "hook should resolve repo root at runtime"
    );
}

#[test]
fn test_updates_features_scoped_codex_hooks_only() {
    let tmp = tempfile::tempdir().unwrap();
    let codex_home = tmp.path();
    let config_path = codex_home.join("config.toml");

    // Write config with non-features codex_hooks = false
    fs::write(
        &config_path,
        "[custom]\ncodex_hooks = false\n\n[features]\nother_flag = true\n",
    )
    .unwrap();

    let result = run_installer(codex_home);
    assert!(result.status.success(), "installer failed: {}", String::from_utf8_lossy(&result.stderr));

    let text = fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("[custom]\ncodex_hooks = false"), "non-features codex_hooks should be untouched");
    assert!(text.contains("[features]") && text.contains("hooks = true"), "features hooks should be enabled");
    assert!(!text.contains("codex_hooks = true"), "deprecated features codex_hooks should not be emitted");
}
