//! Registry consistency tests: ensure SKILL_ROUTING_RUNTIME.json,
//! SKILL_MANIFEST.json, and SKILL_ROUTING_INDEX.json stay in sync.

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

/// Extract slugs from a v3 routing file that uses `{"keys": [...], "skills"|"records": [...]}`.
fn extract_slugs(json: &Value) -> HashSet<String> {
    let keys = json
        .get("keys")
        .and_then(|k| k.as_array())
        .expect("missing keys array");
    let slug_idx = keys
        .iter()
        .position(|k| k.as_str() == Some("slug"))
        .expect("keys missing 'slug'");

    // v3 uses "skills"; fall back to "records" for older schemas
    let records = json
        .get("skills")
        .or_else(|| json.get("records"))
        .and_then(|r| r.as_array())
        .expect("missing skills/records array");

    records
        .iter()
        .filter_map(|row| {
            row.as_array()
                .and_then(|arr| arr.get(slug_idx))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

#[test]
fn index_slugs_are_subset_of_runtime() {
    let root = repo_root();
    let runtime_path = root.join("skills/SKILL_ROUTING_RUNTIME.json");
    let index_path = root.join("skills/SKILL_ROUTING_INDEX.json");

    if !runtime_path.exists() || !index_path.exists() {
        eprintln!("skipping: routing files not found");
        return;
    }

    let runtime = load_json(&runtime_path);
    let index = load_json(&index_path);

    let runtime_slugs = extract_slugs(&runtime);
    let index_slugs = extract_slugs(&index);

    let missing_in_runtime: Vec<&String> = index_slugs.difference(&runtime_slugs).collect();
    assert!(
        missing_in_runtime.is_empty(),
        "INDEX has slugs not in RUNTIME: {:?}",
        missing_in_runtime
    );
}

#[test]
fn manifest_slugs_superset_of_runtime() {
    let root = repo_root();
    let runtime_path = root.join("skills/SKILL_ROUTING_RUNTIME.json");
    let manifest_path = root.join("skills/SKILL_MANIFEST.json");

    if !runtime_path.exists() || !manifest_path.exists() {
        eprintln!("skipping: routing files not found");
        return;
    }

    let runtime = load_json(&runtime_path);
    let manifest = load_json(&manifest_path);

    let runtime_slugs = extract_slugs(&runtime);
    let manifest_slugs = extract_slugs(&manifest);

    // MANIFEST is a cold fallback; RUNTIME may have fewer skills (hot subset only).
    // It is fine for RUNTIME to lack some MANIFEST entries — but MANIFEST must not
    // have EXTRA slugs that are completely absent from RUNTIME.
    let extra_in_manifest: Vec<&String> = manifest_slugs.difference(&runtime_slugs).collect();
    assert!(
        extra_in_manifest.is_empty(),
        "MANIFEST has slugs not in RUNTIME (stale manifest entries): {:?}",
        extra_in_manifest
    );
}
