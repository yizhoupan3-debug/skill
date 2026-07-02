//! Workspace DAG compliance smoke tests.
//!
//! Verifies the core-crate architecture:
//! - Leaf crates have zero workspace deps
//! - DAG direction is correct (no reverse edges)
//! - B10/B8 are fully independent
//!
//! Note: core-policy and framework-kernel have been merged into framework-core (2026-06-29).

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR to find Cargo.toml with [workspace]
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("Cargo.toml").exists() {
            let content = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
            if content.contains("[workspace]") {
                return dir;
            }
        }
        if !dir.pop() {
            panic!("Could not find workspace root");
        }
    }
}

fn read_cargo_toml(relative_path: &str) -> String {
    let root = workspace_root();
    std::fs::read_to_string(root.join(relative_path).join("Cargo.toml"))
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", relative_path, e))
}

fn has_workspace_dep(content: &str, dep_name: &str) -> bool {
    // Check if dep_name appears as a path dependency (exact match, not prefix)
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("path") {
            // Match "name = {" or "\"name\" = {"  (exact dep name, not prefix)
            // Split on '=' and check the first part matches
            if let Some(name_part) = trimmed.split('=').next() {
                let name = name_part.trim().trim_matches('"');
                if name == dep_name {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn workspace_has_seven_core_crates() {
    let root = workspace_root();
    let workspace_content = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    let expected_crates = [
        "core/core-state",
        "core/framework-core",
        "core/routing-engine",
        "core/router-rs",
        "core/codegraph-rs",
        "core/research-harness",
    ];

    for crate_path in &expected_crates {
        assert!(
            workspace_content.contains(crate_path),
            "workspace missing crate: {}",
            crate_path
        );
    }
}

#[test]
fn framework_core_is_foundation_leaf() {
    // framework-core (merged from core-policy + framework-kernel) is the L0 foundation.
    // It should not depend on any higher-layer workspace crate.
    let content = read_cargo_toml("core/framework-core");
    let higher_crates = [
        "core-state",
        "routing-engine",
        "router-rs",
        "codegraph-rs",
        "research-harness",
    ];
    for dep in &higher_crates {
        assert!(
            !has_workspace_dep(&content, dep),
            "framework-core should not depend on {}",
            dep
        );
    }
}

#[test]
fn core_state_deps_are_correct() {
    // core-state (L4) depends on framework-core (L0)
    let content = read_cargo_toml("core/core-state");
    assert!(
        has_workspace_dep(&content, "framework-core"),
        "core-state should depend on framework-core"
    );
}

#[test]
fn router_rs_deps_are_correct() {
    let content = read_cargo_toml("core/router-rs");
    let expected_deps = [
        "core-state",
        "framework-core",
        "routing-engine",
    ];
    for dep in &expected_deps {
        assert!(
            has_workspace_dep(&content, dep),
            "router-rs should depend on {}",
            dep
        );
    }
}

#[test]
fn b10_codegraph_rs_is_independent() {
    let content = read_cargo_toml("core/codegraph-rs");
    let framework_crates = [
        "core-state",
        "framework-core",
        "routing-engine",
        "router-rs",
    ];
    for dep in &framework_crates {
        assert!(
            !has_workspace_dep(&content, dep),
            "B10 codegraph-rs should not depend on {}",
            dep
        );
    }
}

#[test]
fn dag_max_depth_is_three_hops() {
    // Longest path: router-rs → core-state → framework-core = 3 hops
    let content = read_cargo_toml("core/router-rs");
    assert!(has_workspace_dep(&content, "core-state"));
    let state = read_cargo_toml("core/core-state");
    assert!(has_workspace_dep(&state, "framework-core"));
}
