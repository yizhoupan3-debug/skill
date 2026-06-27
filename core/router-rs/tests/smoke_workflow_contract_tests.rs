//! Team artifact static contract smoke (team registry + agent health schema).
//!
//! Replaces the previous workflow_scripts smoke tests (`.claude/workflows/*.js` scripts removed).

use std::fs;
use std::path::PathBuf;

fn framework_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn team_registry_path() -> PathBuf {
    framework_repo_root().join("artifacts/teams/registry.json")
}

fn agent_health_registry_path() -> PathBuf {
    framework_repo_root().join(".framework/agent-health/registry.json")
}

/// Team registry artifact exists with valid schema.
#[test]
fn team_registry_artifact_exists() {
    let path = team_registry_path();
    assert!(
        path.is_file(),
        "team registry artifact must exist at {}",
        path.display()
    );
    let raw = fs::read_to_string(&path).expect("read team registry");
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("team registry must be valid JSON");
    assert_eq!(
        value["schema_version"],
        "v1",
        "team registry schema_version must be v1"
    );
    assert!(
        value["teams"].is_array(),
        "team registry must have a 'teams' array"
    );
}

/// Agent health registry artifact exists with valid schema.
#[test]
fn agent_health_registry_artifact_exists() {
    let path = agent_health_registry_path();
    assert!(
        path.is_file(),
        "agent health registry artifact must exist at {}",
        path.display()
    );
    let raw = fs::read_to_string(&path).expect("read agent health registry");
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("agent health registry must be valid JSON");
    assert!(
        value["agents"].is_array(),
        "agent health registry must have an 'agents' array"
    );
}

/// Validate team_manager module is linked via session-supervisor dependency.
#[test]
fn team_manager_module_linked() {
    let raw = fs::read_to_string(framework_repo_root().join("Cargo.toml"))
        .expect("read workspace Cargo.toml");
    assert!(
        raw.contains("session-supervisor"),
        "workspace must include session-supervisor crate (team model dependency)"
    );
}

/// Artifacts directory for teams has expected subdirectory structure.
#[test]
fn team_artifacts_directory_structure() {
    let teams_dir = framework_repo_root().join("artifacts/teams");
    assert!(
        teams_dir.is_dir(),
        "artifacts/teams/ directory must exist"
    );
    assert!(
        teams_dir.join("registry.json").is_file(),
        "artifacts/teams/registry.json must exist"
    );

    let agent_health_dir = framework_repo_root().join(".framework/agent-health");
    assert!(
        agent_health_dir.is_dir(),
        ".framework/agent-health/ directory must exist"
    );
}
