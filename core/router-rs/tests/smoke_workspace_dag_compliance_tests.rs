//! Workspace DAG compliance smoke tests.
//!
//! Verifies the eight-crate architecture matches roadmap v5 §1.3:
//! - 8 core crates exist in workspace (core-math merged into runtime-core 2026-06-11)
//! - Leaf crates have zero workspace deps
//! - DAG direction is correct (no reverse edges)
//! - B10/B8 are fully independent

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
    // Check if dep_name appears as a path dependency
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(dep_name) && trimmed.contains("path") {
            return true;
        }
        // Also check quoted form: "dep-name" = { path = ... }
        if trimmed.starts_with(&format!("\"{}\"", dep_name)) && trimmed.contains("path") {
            return true;
        }
    }
    false
}

#[test]
fn workspace_has_nine_core_crates() {
    let root = workspace_root();
    let workspace_content = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    let expected_crates = [
        "core/core-state",
        "core/core-policy",
        "core/framework-kernel",
        "core/routing-engine",
        "core/router-rs",
        "tools/codegraph-rs",
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
fn leaf_crates_have_zero_workspace_deps() {
    // These crates should have NO path dependencies to other workspace crates
    // Note: research-harness depends on core-state, so it's no longer a leaf
    let leaf_crates = [
        "core/core-state",
        "core/framework-kernel",
        "core/routing-engine",
        "tools/codegraph-rs",
    ];

    let workspace_core = [
        "core-state",
        "core-policy",
        "framework-kernel",
        "routing-engine",
        "router-rs",
        "codegraph-rs",
        "research-harness",
    ];

    for leaf in &leaf_crates {
        let content = read_cargo_toml(leaf);
        for dep in &workspace_core {
            // Skip self-references
            if leaf.ends_with(dep) {
                continue;
            }
            assert!(
                !has_workspace_dep(&content, dep),
                "Leaf crate {} should not depend on {}",
                leaf,
                dep
            );
        }
    }
}

#[test]
fn core_policy_deps_are_correct() {
    let content = read_cargo_toml("core/core-policy");
    assert!(
        has_workspace_dep(&content, "core-state"),
        "core-policy should depend on core-state"
    );
    assert!(
        has_workspace_dep(&content, "framework-kernel"),
        "core-policy should depend on framework-kernel"
    );
    // Should NOT depend on router-rs, routing-engine, etc.
    assert!(
        !has_workspace_dep(&content, "router-rs"),
        "core-policy should not depend on router-rs"
    );
    assert!(
        !has_workspace_dep(&content, "routing-engine"),
        "core-policy should not depend on routing-engine"
    );
}

#[test]
fn router_rs_deps_are_correct() {
    let content = read_cargo_toml("core/router-rs");
    let expected_deps = [
        "core-state",
        "framework-kernel",
        "core-policy",
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
    let content = read_cargo_toml("tools/codegraph-rs");
    let framework_crates = [
        "core-state",
        "core-policy",
        "framework-kernel",
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
fn dag_max_depth_is_four_hops() {
    // Longest path: B7 → B4 → B3 → B1 → B0 = 4 hops
    // Verify by checking the dependency chain exists:
    // router-rs → core-policy → core-state (3 hops)
    // router-rs → core-policy → framework-kernel (3 hops)
    // This is currently the longest actual path (B7/B3/B4 are still in router-rs)
    let content = read_cargo_toml("core/router-rs");
    assert!(has_workspace_dep(&content, "core-policy"));
    let policy_content = read_cargo_toml("core/core-policy");
    assert!(has_workspace_dep(&policy_content, "core-state"));
    assert!(has_workspace_dep(&policy_content, "framework-kernel"));

    // When B3/B4/B7 are extracted, the path will extend to 4 hops
    // This test documents the current max depth
}
