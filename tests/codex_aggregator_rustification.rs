mod common;

use common::{project_root, read_text, run_ok};
use std::process::Command;

#[test]
fn codex_aggregator_retired_python_and_shell_entrypoints() {
    let root = project_root().join("codex-aggregator");
    assert!(!root.join("health_monitor.py").exists());
    assert!(!root.join("extract_tokens.sh").exists());
    assert!(!root.join("dashboard/server.py").exists());
    assert!(root.join("src/bin/dashboard.rs").is_file());
    assert!(root.join("src/bin/health_monitor.rs").is_file());
    assert!(root.join("src/bin/extract_tokens.rs").is_file());
}

#[test]
fn codex_aggregator_docker_dashboard_uses_rust_binary() {
    let root = project_root().join("codex-aggregator");
    let dockerfile = read_text(&root.join("dashboard/Dockerfile"));
    let compose = read_text(&root.join("docker-compose.yml"));
    assert!(dockerfile.contains("FROM rust:"));
    assert!(dockerfile.contains("cargo build --release --bin codex-dashboard"));
    assert!(dockerfile.contains(r#"CMD ["codex-dashboard"]"#));
    assert!(!dockerfile.to_lowercase().contains("python"));
    assert!(!dockerfile.contains("pip install"));
    assert!(compose.contains("context: ."));
    assert!(compose.contains("dockerfile: dashboard/Dockerfile"));
}

#[test]
fn codex_aggregator_rust_manifest_checks() {
    run_ok({
        let mut command = Command::new("cargo");
        command
            .args(["check", "--manifest-path"])
            .arg(project_root().join("codex-aggregator/Cargo.toml"))
            .current_dir(project_root());
        command
    });
}

#[test]
fn codex_extract_tokens_supports_non_secret_smoke_run() {
    let output = run_ok({
        let mut command = Command::new("cargo");
        command
            .args(["run", "--quiet", "--manifest-path"])
            .arg(project_root().join("codex-aggregator/Cargo.toml"))
            .args([
                "--bin",
                "codex-extract-tokens",
                "--",
                "--skip-gh",
                "--vscode-hosts",
                "/tmp/nonexistent-hosts.json",
            ])
            .current_dir(project_root());
        command
    });
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GitHub Copilot Token Extractor"));
    assert!(stdout.contains("[Skip] VS Code config not found."));
    assert!(!stdout.contains("GH_TOKEN:"));
}
