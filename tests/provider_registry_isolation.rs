//! Provider registry isolation tests.
//!
//! Ensures `RUNTIME_PROVIDER_REGISTRY.json` is document-only: it must NOT be
//! imported or referenced by the routing engine, and `hook_policy.rs` must
//! explicitly declare its document-only status.

use std::fs;

/// Grep the routing-engine source tree for any reference to RUNTIME_PROVIDER_REGISTRY.
/// This file is document-only and must not drive routing decisions.
#[test]
fn provider_registry_not_used_by_routing_engine() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let routing_engine_dir = std::path::PathBuf::from(manifest_dir).join("core/routing-engine/src");

    if !routing_engine_dir.is_dir() {
        // routing-engine not present in this build — skip gracefully.
        eprintln!(
            "SKIP: routing-engine dir not found at {}",
            routing_engine_dir.display()
        );
        return;
    }

    let output = std::process::Command::new("grep")
        .args([
            "-rn",
            "RUNTIME_PROVIDER_REGISTRY",
            routing_engine_dir.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run grep");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "RUNTIME_PROVIDER_REGISTRY.json is referenced in routing-engine:\n{stdout}\n\
         This file is document-only and must not drive routing decisions."
    );
}

/// Verify that `hook_policy.rs` documents that RUNTIME_PROVIDER_REGISTRY is document-only.
#[test]
fn provider_registry_document_only_in_hook_policy() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let hook_policy_path =
        std::path::PathBuf::from(manifest_dir).join("core/core-policy/src/hook_policy.rs");

    let hook_policy = fs::read_to_string(&hook_policy_path)
        .expect("failed to read hook_policy.rs — core-policy crate must be present");

    assert!(
        hook_policy.contains("document-only") || hook_policy.contains("does not drive"),
        "hook_policy.rs should document that RUNTIME_PROVIDER_REGISTRY is document-only.\n\
         Expected to find 'document-only' or 'does not drive' in hook_policy.rs."
    );
}

/// Verify that `tool_safety_rules.rs` declares lifecycle constants for auxiliary files.
#[test]
fn tool_safety_rules_declares_lifecycle_constants() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let tsr_path =
        std::path::PathBuf::from(manifest_dir).join("core/core-policy/src/tool_safety_rules.rs");

    let tsr = fs::read_to_string(&tsr_path).expect("failed to read tool_safety_rules.rs");

    assert!(
        tsr.contains("WRITE_ONLY_AUXILIARY_FILES"),
        "tool_safety_rules.rs must declare WRITE_ONLY_AUXILIARY_FILES constant"
    );
    assert!(
        tsr.contains("DOCUMENT_ONLY_AUXILIARY_FILES"),
        "tool_safety_rules.rs must declare DOCUMENT_ONLY_AUXILIARY_FILES constant"
    );
}

/// Verify the auxiliary JSON files on disk carry a `lifecycle` field.
#[test]
fn auxiliary_json_files_have_lifecycle_field() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = std::path::PathBuf::from(manifest_dir);

    let cases: &[(&str, &str)] = &[
        ("skills/SKILL_TIERS.json", "write-only"),
        ("skills/SKILL_HEALTH_MANIFEST.json", "write-only"),
        ("skills/SKILL_PLUGIN_CATALOG.json", "write-only"),
        (
            "configs/framework/RUNTIME_PROVIDER_REGISTRY.json",
            "document-only",
        ),
    ];

    for (rel_path, expected_lifecycle) in cases {
        let full = repo_root.join(rel_path);
        if !full.is_file() {
            eprintln!("SKIP: {rel_path} not found on disk");
            continue;
        }
        let content = fs::read_to_string(&full).unwrap_or_else(|e| {
            panic!("failed to read {rel_path}: {e}");
        });
        let val: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("{rel_path} is not valid JSON: {e}"));
        let actual: &str = val
            .get("lifecycle")
            .and_then(|v| v.as_str())
            .unwrap_or("MISSING");
        assert_eq!(
            actual, *expected_lifecycle,
            "{rel_path} lifecycle field: expected {expected_lifecycle}, got {actual}"
        );
    }
}
