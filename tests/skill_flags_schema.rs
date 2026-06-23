//! Skill flags schema validation: ensure all referenced flag types
//! are defined in RUNTIME_REGISTRY.json skill_flag_types.

use serde_json::Value;
use std::collections::HashSet;
use std::fs;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_json(path: &std::path::Path) -> Value {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

#[test]
fn all_referenced_skill_flags_are_defined() {
    let root = repo_root();
    let registry_path = root.join("configs/framework/RUNTIME_REGISTRY.json");
    let runtime_path = root.join("skills/SKILL_ROUTING_RUNTIME.json");

    let registry = load_json(&registry_path);
    let runtime = load_json(&runtime_path);

    // Get defined flag types from registry
    let defined_flags: HashSet<String> = registry
        .get("skill_flag_types")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    // Extract all skill_flags from runtime records
    let keys = runtime
        .get("keys")
        .and_then(|k| k.as_array())
        .expect("missing keys array");
    let flags_idx = keys
        .iter()
        .position(|k| k.as_str() == Some("skill_flags"));

    let records = runtime
        .get("skills")
        .or_else(|| runtime.get("records"))
        .and_then(|r| r.as_array())
        .expect("missing skills/records array");

    let mut referenced_flags: HashSet<String> = HashSet::new();
    for row in records {
        if let Some(arr) = row.as_array() {
            if let Some(idx) = flags_idx {
                if let Some(flags_val) = arr.get(idx) {
                    if let Some(flags) = flags_val.as_array() {
                        for flag in flags {
                            if let Some(s) = flag.as_str() {
                                referenced_flags.insert(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Validate: each referenced flag must be defined
    // (or be a parameterized variant of a defined flag like "behavior:low_score_override:20.0")
    let mut undefined = Vec::new();
    for flag in &referenced_flags {
        if defined_flags.contains(flag) {
            continue;
        }
        // Check parameterized variants: "foo:bar:baz" -> try "foo:bar" then "foo"
        let parts: Vec<&str> = flag.split(':').collect();
        let mut found = false;
        for i in (1..parts.len()).rev() {
            let prefix = parts[..i].join(":");
            if defined_flags.contains(&prefix) {
                found = true;
                break;
            }
        }
        if !found {
            undefined.push(flag.clone());
        }
    }

    assert!(
        undefined.is_empty(),
        "Skill flags referenced in SKILL_ROUTING_RUNTIME.json but not defined \
         in RUNTIME_REGISTRY.json skill_flag_types: {:?}",
        undefined
    );
}
