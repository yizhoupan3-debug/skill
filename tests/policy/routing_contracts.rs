// Test helper functions that may not all be called from every test.
#![allow(dead_code)]

use crate::common::{
    CANONICAL_HOST_IDS, RETIRED_HOST_IDS, assert_canonical_closed_set_host_ids, project_root,
    read_json, read_text, router_rs_json, seed_framework_markers,
};
use crate::host_platforms;
use crate::policy::policy_helpers::{
    FRAMEWORK_COMMAND_IDS, HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS, key_index,
    runtime_host_platforms_index,
};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Local helper functions
// ---------------------------------------------------------------------------

fn manifest_or_runtime_lane_contains(manifest_slugs: &HashSet<&str>, slug: &str) -> bool {
    slug == "none"
        || manifest_slugs.contains(slug)
        || FRAMEWORK_COMMAND_IDS.contains(&slug)
        || project_root()
            .join("skills")
            .join(slug)
            .join("SKILL.md")
            .is_file()
}

fn parse_skill_md_frontmatter_map(path: &Path) -> Map<String, Value> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let rest = text
        .strip_prefix("---")
        .unwrap_or_else(|| panic!("{}: missing opening ---", path.display()));
    let rest = rest.trim_start_matches(['\n', '\r']);
    let end = rest
        .find("\n---\n")
        .or_else(|| rest.find("\r\n---\r\n"))
        .or_else(|| rest.find("\n---\r\n"))
        .unwrap_or_else(|| panic!("{}: missing closing ---", path.display()));
    let yaml_txt = &rest[..end];
    let yaml_val: serde_yml::Value =
        serde_yml::from_str(yaml_txt).unwrap_or_else(|e| panic!("{}: yaml: {e}", path.display()));
    serde_json::to_value(yaml_val)
        .expect("yaml to json")
        .as_object()
        .expect("frontmatter must be a mapping")
        .clone()
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Some(other) => other
            .as_str()
            .map(|s| vec![s.to_string()])
            .unwrap_or_default(),
    }
}

fn raw_platforms_from_skill_frontmatter(meta: &Map<String, Value>) -> Vec<String> {
    let mut raw = value_string_list(meta.get("platforms"));
    if raw.is_empty()
        && let Some(Value::Object(inner)) = meta.get("metadata")
    {
        raw = value_string_list(inner.get("platforms"));
    }
    raw
}

/// Loads `NL_SIGNAL_REGISTRY` names from the built `router-rs` binary (no regex scan of Rust source).
fn nl_route_registry_signal_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let repo = project_root();
        let manifest = repo.join("core/router-rs/Cargo.toml");
        let output = Command::new("cargo")
            .current_dir(&repo)
            .args([
                "run",
                "-q",
                "--manifest-path",
                manifest.to_str().expect("manifest path utf-8"),
                "--bin",
                "router-rs-cli",
                "--",
                "framework",
                "nl-route-signal-registry-contract",
            ])
            .output()
            .unwrap_or_else(|e| {
                panic!("cargo run router-rs framework nl-route-signal-registry-contract: {e}");
            });
        assert!(
            output.status.success(),
            "nl-route-signal-registry-contract failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let raw = String::from_utf8_lossy(&output.stdout);
        let arr: Vec<String> = serde_json::from_str(raw.trim())
            .expect("nl-route-signal-registry-contract stdout must be a JSON string array");
        assert!(!arr.is_empty(), "NL_SIGNAL_REGISTRY dump must be non-empty");
        arr.into_iter().collect()
    })
}

fn nl_policy_signal_allowed(name: &str) -> bool {
    nl_route_registry_signal_names().contains(name)
}

fn nl_policy_collect_signals_from_when(when: &Value, out: &mut HashSet<String>, ctx: &str) {
    match when {
        Value::Bool(_) => {}
        Value::Object(map) => {
            if map.is_empty() {
                panic!("{ctx}: when must not be an empty object");
            }
            for k in map.keys() {
                assert!(
                    matches!(
                        k.as_str(),
                        "all" | "any" | "not" | "signal" | "query_contains" | "first_turn"
                    ),
                    "{ctx}: when has unknown key `{k}`"
                );
            }
            if let Some(arr) = map.get("all").and_then(Value::as_array) {
                assert_eq!(map.len(), 1, "{ctx}: when.all must be the sole object key");
                for (i, sub) in arr.iter().enumerate() {
                    nl_policy_collect_signals_from_when(sub, out, &format!("{ctx}.all[{i}]"));
                }
                return;
            }
            if let Some(arr) = map.get("any").and_then(Value::as_array) {
                assert_eq!(map.len(), 1, "{ctx}: when.any must be the sole object key");
                for (i, sub) in arr.iter().enumerate() {
                    nl_policy_collect_signals_from_when(sub, out, &format!("{ctx}.any[{i}]"));
                }
                return;
            }
            if map.contains_key("not") {
                assert_eq!(map.len(), 1, "{ctx}: when.not must be the sole object key");
                let inner = map.get("not").expect("not present");
                nl_policy_collect_signals_from_when(inner, out, &format!("{ctx}.not"));
                return;
            }
            assert_eq!(
                map.len(),
                1,
                "{ctx}: when leaf must have exactly one key among signal/query_contains/first_turn"
            );
            if let Some(s) = map.get("signal").and_then(Value::as_str) {
                assert!(
                    nl_policy_signal_allowed(s),
                    "{ctx}: signal `{s}` not in nl_route_adjustments NL_SIGNAL_REGISTRY"
                );
                out.insert(s.to_string());
                return;
            }
            assert!(
                map.get("query_contains").and_then(Value::as_str).is_some()
                    || map.get("first_turn").and_then(Value::as_bool).is_some(),
                "{ctx}: when leaf must be query_contains or first_turn"
            );
        }
        other => panic!("{ctx}: when must be bool or object, got {other:?}"),
    }
}

fn nl_policy_validate_rule(rule: &Value, ctx: &str) {
    let obj = rule
        .as_object()
        .unwrap_or_else(|| panic!("{ctx}: rule must be object"));
    for k in obj.keys() {
        assert!(
            matches!(k.as_str(), "record" | "when" | "action"),
            "{ctx}: unknown rule key `{k}`"
        );
    }
    let action = obj
        .get("action")
        .unwrap_or_else(|| panic!("{ctx}: missing action"));
    let aobj = action
        .as_object()
        .unwrap_or_else(|| panic!("{ctx}: action must be object"));
    for k in aobj.keys() {
        assert!(
            matches!(k.as_str(), "type" | "reason" | "delta"),
            "{ctx}: unknown action key `{k}`"
        );
    }
    let ty = aobj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{ctx}: action.type required"));
    match ty {
        "suppress" | "boost" => {}
        other => panic!("{ctx}: unknown action.type `{other}`"),
    }
    if let Some(rec) = obj.get("record")
        && !rec.is_null()
    {
        let robj = rec
            .as_object()
            .unwrap_or_else(|| panic!("{ctx}: record must be object or null"));
        for k in robj.keys() {
            assert!(
                matches!(k.as_str(), "slug" | "slugs" | "gate_lower"),
                "{ctx}: unknown record key `{k}`"
            );
        }
    }
    let mut signals = HashSet::new();
    match obj.get("when") {
        None => {}
        Some(w) => nl_policy_collect_signals_from_when(w, &mut signals, &format!("{ctx}.when")),
    }
    let _ = signals;
}

fn nl_policy_validate_rule_list(rules: &[Value], label: &str) {
    for (i, rule) in rules.iter().enumerate() {
        nl_policy_validate_rule(rule, &format!("{label}[{i}]"));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn project_host_skill_projection_is_generated_outside_host_entrypoints() {
    assert!(!project_root().join(".codex/skills").exists());
    assert!(!project_root().join("AGENT.md").exists());
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--host-id",
        "codex",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let manifest = read_json(&repo_root.join(".codex/host_entrypoints_sync_manifest.json"));
    assert!(
        sync_report["written"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "expected codex sync to write host entrypoints: {sync_report}"
    );
    let manifest_text = manifest.to_string();
    assert!(!manifest_text.contains(".codex/skills/gitx"));
    assert!(!manifest_text.contains(".codex/skills/autopilot"));
    assert!(!manifest_text.contains(".codex/prompts/"));
    assert!(!repo_root.join(".codex/prompts/autopilot.md").exists());
    assert!(!repo_root.join(".codex/prompts/gitx.md").exists());
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["codex"],
        serde_json::json!("AGENTS.md")
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["cursor"],
        serde_json::json!(["AGENTS.md", ".cursor/rules/*.mdc"])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["claude"],
        serde_json::json!([
            "AGENTS.md",
            ".claude/rules/framework.md",
            ".claude/settings.json"
        ])
    );
    assert_eq!(
        manifest["shared_system"]["host_entrypoints"]["opencode"],
        ".opencode/opencode.json"
    );
    let synced_hosts: Vec<&str> = manifest["shared_system"]["supported_hosts"]
        .as_array()
        .expect("supported_hosts")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_canonical_closed_set_host_ids(&synced_hosts);
    assert_eq!(
        manifest["shared_system"]["policy"],
        "host-specific-agent-policy-v1"
    );
    assert_eq!(
        manifest["shared_system"]["routing_source_of_truth"],
        "skills/"
    );
    assert_eq!(
        manifest["shared_system"]["agent_policy_entrypoint"],
        "AGENTS.md"
    );
    let codex_policy = read_text(&repo_root.join("AGENTS.md"));
    assert!(codex_policy.contains("Codex Agent Policy"));
    assert!(codex_policy.contains("AGENTS.md"));
    assert!(
        manifest["full_sync"]["text_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("AGENTS.md"))
    );
    assert!(
        manifest["full_sync"]["text_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(".codex/README.md"))
    );
    assert!(
        manifest["full_sync"]["json_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(".codex/hooks.json"))
    );
    assert!(
        manifest["partial_sync"]["json_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(
                ".codex/host_entrypoints_sync_manifest.json"
            ))
    );
    assert_eq!(
        manifest["partial_sync"]["text_files"],
        serde_json::json!([])
    );
    assert!(!manifest_text.contains("retired_files"));
    assert!(!manifest_text.contains("retired_directories"));
    assert!(!manifest_text.contains("AGENT.md"));
}

#[test]
fn codex_sync_does_not_write_root_agents_md() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let policy = "custom kernel policy from disk\n";
    std::fs::write(repo_root.join("AGENTS.md"), policy).unwrap();

    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--host-id",
        "codex",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert!(
        !sync_report["written"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("AGENTS.md")),
        "codex sync must not write repo-root AGENTS.md: {sync_report}"
    );
    assert_eq!(read_text(&repo_root.join("AGENTS.md")), policy);
}

#[test]
fn codex_sync_preserves_existing_agents_codex_delta_file() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    seed_framework_markers(&repo_root);
    let delta = "custom codex delta from disk\nReview findings-only\n";
    std::fs::write(repo_root.join("AGENTS.md"), delta).unwrap();

    let sync_report = router_rs_json(&[
        "framework",
        "sync-entrypoints",
        "--host-id",
        "codex",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let written = sync_report["written"].as_array().unwrap();
    assert!(
        !written.contains(&serde_json::json!("AGENTS.md")),
        "sync must not rewrite unchanged AGENTS.md: {sync_report}"
    );
    assert_eq!(read_text(&repo_root.join("AGENTS.md")), delta);
}

#[test]
fn framework_surface_policy_is_the_activation_source_of_truth() {
    let surface =
        read_json(&project_root().join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"));
    let tiers = read_json(&project_root().join("skills/SKILL_TIERS.json"));

    assert_eq!(surface["source_of_truth"], true);
    assert_eq!(
        surface["derived_reports"],
        serde_json::json!(["skills/SKILL_TIERS.json"])
    );
    assert_eq!(
        surface["deprecated_or_foldable_reports"],
        serde_json::json!([])
    );
    assert_eq!(
        surface["kernel"]["canonical_axes"],
        serde_json::json!(["routing", "memory", "continuity", "host_projection"])
    );
    assert_eq!(tiers["source_of_truth"], false);
    assert_eq!(
        tiers["derived_from"],
        "configs/framework/FRAMEWORK_SURFACE_POLICY.json"
    );
    assert_eq!(tiers["report_status"], "generated_debug_report");
    assert_eq!(
        surface["skill_system"]["activation_counts"],
        tiers["summary"]["activation_counts"]
    );
}

#[test]
fn runtime_hot_index_is_minimal() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let runtime_obj = runtime.as_object().expect("runtime object");
    let keys = runtime_obj.keys().cloned().collect::<HashSet<_>>();
    assert_eq!(
        keys,
        HashSet::from([
            "version".to_string(),
            "schema_version".to_string(),
            "scope".to_string(),
            "keys".to_string(),
            "skills".to_string(),
            "default_host_platforms".to_string(),
        ])
    );
    assert!(runtime.get("checklist").is_none());
    assert!(runtime.get("records").is_none());
    assert!(runtime.get("plugin_abi_version").is_none());
    assert!(runtime.get("vnext").is_none());
}

#[test]
fn runtime_hot_index_keeps_capability_gates_explicit() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let keys = runtime["keys"].as_array().expect("runtime keys");
    let slug_idx = key_index(keys, "slug");
    assert_eq!(runtime["version"], 3);
    assert!(
        !keys.iter().any(|key| key == "health"),
        "runtime schema v3 must not expose the retired health column"
    );
    let slugs = runtime["skills"]
        .as_array()
        .expect("runtime skills")
        .iter()
        .map(|skill| skill[slug_idx].as_str().expect("runtime skill slug"))
        .collect::<Vec<_>>();

    assert_eq!(runtime["scope"]["kind"], "hot");
    assert_eq!(
        runtime["scope"]["fallback_manifest"],
        "skills/SKILL_MANIFEST.json"
    );
    for expected in [
        "gh-address-comments",
        "gh-fix-ci",
        "citation-management",
        "paper-workbench",
        "research-discovery",
        "research-execution",
        "deep-research",
        "plan-mode",
        "code-review-deep",
        "statistical-analysis",
        "experiment-reproducibility",
        "math-derivation",
        "scientific-figure-plotting",
        "pdf",
        "skill-framework-developer",
        "visual-review",
    ] {
        assert!(
            slugs.contains(&expected),
            "missing hot runtime slug: {expected}"
        );
    }
    for excluded in [
        "systematic-debugging",
        "idea-to-plan",
        "plan-to-code",
        "plugin-creator",
        "skill-creator",
        "skill-installer",
    ] {
        assert!(
            !slugs.contains(&excluded),
            "broad first-turn owner should stay out of hot runtime: {excluded}"
        );
    }
    assert!(
        slugs.len() <= 46,
        "hot runtime surface should stay bounded; got {}",
        slugs.len()
    );
    assert_eq!(runtime["scope"]["hot_skill_count"], slugs.len());
}

#[test]
fn runtime_hot_index_stays_separate_from_plugin_and_routing_catalogs() {
    let runtime = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME.json"));
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    assert_eq!(runtime["version"], 3);
    assert_eq!(runtime["schema_version"], "skill-routing-runtime-v3");
    let rows = runtime["skills"].as_array().expect("runtime rows");
    let framework_row = rows
        .iter()
        .find(|record| record[0] == "skill-framework-developer")
        .expect("skill-framework-developer runtime row");
    assert_eq!(framework_row[0], "skill-framework-developer");
    assert_eq!(
        plugin_catalog["skills"]["skill-framework-developer"]["kind"],
        "skill"
    );
}

#[test]
fn runtime_host_support_platforms_are_registry_closed_and_match_skill_md() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let plugin_catalog = read_json(&root.join("skills/SKILL_PLUGIN_CATALOG.json"));
    for (slug, record) in plugin_catalog["skills"]
        .as_object()
        .expect("plugin catalog skills")
    {
        let platforms = match record
            .get("host_support")
            .and_then(|hs| hs.get("platforms"))
            .and_then(|p| p.as_array())
        {
            Some(arr) => arr,
            None => continue, // Skills without host_support (e.g. stub entries)
        };
        for p in platforms {
            let id = p.as_str().expect("platform string");
            assert!(
                allowed.contains(id),
                "{slug}: platform `{id}` not in RUNTIME_REGISTRY.host_targets.supported"
            );
        }
        let kind = record["kind"].as_str().expect("plugin.kind");
        if kind != "skill" {
            continue;
        }
        let skill_path = root.join(record["skill_path"].as_str().expect("skill_path"));
        let meta = parse_skill_md_frontmatter_map(&skill_path);
        let raw = raw_platforms_from_skill_frontmatter(&meta);
        let mut supported_ids: Vec<String> = allowed.iter().cloned().collect();
        supported_ids.sort();
        let normalized = host_platforms::normalize_skill_host_platforms(&raw, &supported_ids)
            .unwrap_or_else(|e| panic!("{slug}: normalize_skill_host_platforms: {e}"));
        let from_catalog: Vec<String> = platforms
            .iter()
            .map(|v| v.as_str().expect("platform").to_string())
            .collect();
        let mut from_catalog_sorted = from_catalog.clone();
        from_catalog_sorted.sort();
        assert_eq!(
            normalized,
            from_catalog_sorted,
            "host_support.platforms drift for slug={slug} path={}",
            skill_path.display()
        );
    }
}

#[test]
fn skill_host_platform_aliases_cover_runtime_registry_supported_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();

    let mut supported: Vec<String> = allowed.iter().cloned().collect();
    supported.sort();

    let normalized =
        host_platforms::normalize_skill_host_platforms(&["supported".to_string()], &supported)
            .expect("stock aliases should normalize");
    let normalized_set: HashSet<String> = normalized.into_iter().collect();

    assert_eq!(
        normalized_set, allowed,
        "host_platforms alias coverage must stay aligned with RUNTIME_REGISTRY.host_targets.supported"
    );
}

#[test]
fn hot_runtime_skill_records_cover_all_supported_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let supported: Vec<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let skills = runtime["skills"].as_array().expect("runtime skills");
    for row in skills.iter().filter_map(Value::as_array) {
        let slug = row.first().and_then(|v| v.as_str()).expect("slug");
        if HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS.contains(&slug) {
            continue;
        }
        let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
        let host_idx = runtime_host_platforms_index(runtime_keys);
        let platforms = match row.get(host_idx).and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => continue, // Skills without host platform data (e.g. numeric source_position)
        };
        let set: HashSet<String> = platforms
            .iter()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect();
        for host in &supported {
            assert!(
                set.contains(host),
                "hot runtime skill `{slug}` must include host_platform `{host}` (set `metadata.platforms: [supported]` or list all ids); exempt slugs: {:?}",
                HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS
            );
        }
    }
}

#[test]
fn hot_runtime_codex_only_slugs_have_no_extra_hosts() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let allowed: HashSet<String> = registry["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .map(|v| v.as_str().expect("host id").to_string())
        .collect();
    let mut supported_ids: Vec<String> = allowed.iter().cloned().collect();
    supported_ids.sort();

    let runtime = read_json(&root.join("skills/SKILL_ROUTING_RUNTIME.json"));
    let skills = runtime["skills"].as_array().expect("runtime skills");
    for row in skills.iter().filter_map(Value::as_array) {
        let slug = row.first().and_then(|v| v.as_str()).expect("slug");
        if !HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS.contains(&slug) {
            continue;
        }
        let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
        let skill_path_idx = key_index(runtime_keys, "skill_path");
        let skill_path = row
            .get(skill_path_idx)
            .and_then(|v| v.as_str())
            .map(|rel| root.join(rel))
            .expect("skill_path");
        let meta = parse_skill_md_frontmatter_map(&skill_path);
        let raw = raw_platforms_from_skill_frontmatter(&meta);
        let allowed_platforms =
            host_platforms::normalize_skill_host_platforms(&raw, &supported_ids)
                .unwrap_or_else(|e| panic!("{slug}: normalize_skill_host_platforms: {e}"));
        let allowed_set: HashSet<String> = allowed_platforms.into_iter().collect();

        let runtime_keys = runtime["keys"].as_array().expect("runtime keys");
        let host_idx = runtime_host_platforms_index(runtime_keys);
        let runtime_platforms = row
            .get(host_idx)
            .and_then(|v| v.as_array())
            .expect("host_platforms");
        for platform in runtime_platforms {
            let id = platform.as_str().expect("host platform");
            assert!(
                allowed_set.contains(id),
                "codex-only hot runtime skill `{slug}` must not list extra host `{id}` in runtime host_platforms; allowed={allowed_set:?}"
            );
        }
    }
}

#[test]
fn plugin_catalog_routing_metadata_and_health_manifest_form_closed_loop() {
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    let routing_metadata = read_json(&project_root().join("skills/SKILL_ROUTING_METADATA.json"));
    let explain = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json"));
    let health = read_json(&project_root().join("skills/SKILL_HEALTH_MANIFEST.json"));

    assert_eq!(plugin_catalog["schema_version"], "skill-plugin-catalog-v1");
    assert_eq!(plugin_catalog["source_of_truth"], false);
    assert_eq!(plugin_catalog["derived_from"], "skills/SKILL_MANIFEST.json");
    assert_eq!(
        routing_metadata["schema_version"],
        "skill-routing-metadata-v1"
    );
    assert_eq!(routing_metadata["source_of_truth"], false);
    assert_eq!(
        explain["schema_version"],
        "skill-routing-runtime-explain-v1"
    );
    assert_eq!(explain["source_of_truth"], false);
    assert_eq!(health["schema_version"], "skill-health-manifest-v1");
    assert_eq!(health["source_of_truth"], false);
    assert!(health["skills"].as_object().is_some());

    let catalog_skills = plugin_catalog["skills"]
        .as_object()
        .expect("plugin catalog skills");
    let metadata_skills = routing_metadata["skills"]
        .as_object()
        .expect("routing metadata skills");
    assert!(!catalog_skills.is_empty());
    for (slug, record) in catalog_skills {
        assert!(
            metadata_skills.contains_key(slug),
            "routing metadata missing slug {slug}"
        );
        assert_eq!(record["kind"], "skill");
        assert!(record["skill_path"].as_str().is_some());
        assert!(record["host_support"]["platforms"].as_array().is_some());
    }

    let skill = "skill-framework-developer";
    assert!(catalog_skills.contains_key(skill));
    assert!(metadata_skills.contains_key(skill));
    if explain["selected"][skill].is_object() {
        assert_eq!(
            explain["selected"][skill]["plugin_kind"],
            catalog_skills[skill]["kind"]
        );
    }
}

#[test]
fn plugin_catalog_routing_metadata_companion_schemas_contract() {
    let plugin_catalog = read_json(&project_root().join("skills/SKILL_PLUGIN_CATALOG.json"));
    let routing_metadata = read_json(&project_root().join("skills/SKILL_ROUTING_METADATA.json"));
    let explain = read_json(&project_root().join("skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json"));
    let health = read_json(&project_root().join("skills/SKILL_HEALTH_MANIFEST.json"));

    assert_eq!(plugin_catalog["schema_version"], "skill-plugin-catalog-v1");
    assert!(
        plugin_catalog["skills"].is_object(),
        "companion plugin catalog must list skills"
    );
    assert_eq!(
        routing_metadata["schema_version"],
        "skill-routing-metadata-v1"
    );
    assert_eq!(
        explain["schema_version"],
        "skill-routing-runtime-explain-v1"
    );
    assert_eq!(health["schema_version"], "skill-health-manifest-v1");
    assert!(
        routing_metadata["skills"].is_object(),
        "routing metadata companion must list skills"
    );
    assert_eq!(
        explain.get("source_of_truth").and_then(|v| v.as_bool()),
        Some(false),
        "RUNTIME_EXPLAIN is a refresh stub, not router hot-path truth"
    );
}

#[test]
fn runtime_provider_registry_declares_component_plugin_lanes() {
    let registry =
        read_json(&project_root().join("configs/framework/RUNTIME_PROVIDER_REGISTRY.json"));
    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert_eq!(registry["schema_version"], "runtime-provider-registry-v1");
    assert_eq!(registry["plugin_abi_version"], "skill-plugin-abi-v1");
    for lane in [
        "execution_providers",
        "storage_providers",
        "trace_replay_providers",
        "observability_providers",
        "sandbox_profile_providers",
        "host_projection_providers",
        "governance_eval_loop",
    ] {
        assert!(
            registry.get(lane).is_some(),
            "missing provider registry lane: {lane}"
        );
    }
    assert_eq!(
        registry["execution_providers"]["local_rust"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["storage_providers"]["sqlite"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["trace_replay_providers"]["human_intervention"]["status"],
        "declared"
    );
    assert_eq!(
        registry["host_projection_providers"]["codex"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["host_projection_providers"]["cursor"]["status"],
        "implemented"
    );
    assert_eq!(
        registry["host_projection_providers"]["mcp"]["status"],
        "declared"
    );
    let supported_hosts = runtime["host_targets"]["supported"]
        .as_array()
        .expect("runtime supported hosts")
        .iter()
        .map(|host| host.as_str().expect("host id").to_string())
        .collect::<BTreeSet<_>>();
    let projected_hosts = runtime["host_projections"]
        .as_object()
        .expect("runtime host_projections")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provider_hosts = registry["host_projection_providers"]
        .as_object()
        .expect("provider host_projection_providers")
        .keys()
        .filter(|host| supported_hosts.contains(*host))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        projected_hosts, supported_hosts,
        "RUNTIME_REGISTRY host_targets.supported and host_projections must match"
    );
    assert_eq!(
        provider_hosts, supported_hosts,
        "RUNTIME_PROVIDER_REGISTRY must cover every supported host projection"
    );
    assert_eq!(
        registry["governance_eval_loop"]["metrics"][0],
        "route_expected_owner_accuracy"
    );
    assert!(
        !registry.to_string().contains("/Users/joe"),
        "provider registry must stay portable"
    );
}

#[test]
fn routing_signal_markers_json_unique_nonempty_lists() {
    let v = read_json(&project_root().join("configs/framework/ROUTING_SIGNAL_MARKERS.json"));
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("routing-signal-markers-v1")
    );
    fn assert_no_dupes(arr: &Value, ctx: &str) {
        let a = arr
            .as_array()
            .unwrap_or_else(|| panic!("{ctx} must be array"));
        let mut seen = HashSet::new();
        for item in a {
            let s = item
                .as_str()
                .unwrap_or_else(|| panic!("{ctx} must be string list"));
            assert!(!s.is_empty(), "{ctx} empty string");
            assert!(
                seen.insert(s.to_string()),
                "{ctx} duplicate substring `{s}`"
            );
        }
    }
    let m = v.get("meta_routing_task").expect("meta_routing_task");
    assert_no_dupes(
        m.get("anchor_any_of_substrings")
            .expect("anchor_any_of_substrings"),
        "meta_routing_task.anchor_any_of_substrings",
    );
    assert_no_dupes(
        m.get("marker_any_of_substrings")
            .expect("marker_any_of_substrings"),
        "meta_routing_task.marker_any_of_substrings",
    );
    assert_no_dupes(
        &v["completion_execution_markers"],
        "completion_execution_markers",
    );
    assert_no_dupes(
        &v["supervisor_execution_markers"],
        "supervisor_execution_markers",
    );
}

#[test]
fn hook_observation_rules_json_schema_version() {
    let v =
        read_json(&project_root().join("configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json"));
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("router-rs-hook-observation-rules-v1")
    );
}

#[test]
fn nl_route_adjustments_json_schema_version() {
    let v = read_json(&project_root().join("configs/framework/NL_ROUTE_ADJUSTMENTS.json"));
    let root = v
        .as_object()
        .expect("NL_ROUTE_ADJUSTMENTS root must be object");
    for k in root.keys() {
        assert!(
            matches!(
                k.as_str(),
                "schema_version"
                    | "docs"
                    | "pre_framework_alias_rules"
                    | "post_framework_alias_rules"
                    | "visual_evidence_markers"
            ),
            "NL_ROUTE_ADJUSTMENTS: unknown root key `{k}`"
        );
    }
    assert_eq!(
        v.get("schema_version").and_then(Value::as_str),
        Some("nl-route-adjustments-v1")
    );
    let pre = v["pre_framework_alias_rules"]
        .as_array()
        .expect("pre_framework_alias_rules must be array");
    let post = v["post_framework_alias_rules"]
        .as_array()
        .expect("post_framework_alias_rules must be array");
    nl_policy_validate_rule_list(pre, "pre_framework_alias_rules");
    nl_policy_validate_rule_list(post, "post_framework_alias_rules");

    let mut used_signals = HashSet::new();
    for (label, arr) in [("pre", pre.as_slice()), ("post", post.as_slice())] {
        for (ri, rule) in arr.iter().enumerate() {
            if let Some(w) = rule.get("when") {
                nl_policy_collect_signals_from_when(
                    w,
                    &mut used_signals,
                    &format!("{label}_rules[{ri}].when"),
                );
            }
        }
    }
    let allow = nl_route_registry_signal_names();
    for s in &used_signals {
        assert!(
            allow.contains(s.as_str()),
            "used signal `{s}` must appear in nl_route_adjustments NL_SIGNAL_REGISTRY"
        );
    }
    for reg in allow.iter() {
        assert!(
            used_signals.contains(reg),
            "NL_SIGNAL_REGISTRY entry `{reg}` is never referenced in NL_ROUTE_ADJUSTMENTS.json"
        );
    }
}

#[test]
fn runtime_registry_on_disk_closed_set_is_canonical_five_hosts() {
    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let supported: Vec<&str> = runtime["host_targets"]["supported"]
        .as_array()
        .expect("host_targets.supported")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_canonical_closed_set_host_ids(&supported);

    let metadata = runtime["host_targets"]["metadata"]
        .as_object()
        .expect("host_targets.metadata");
    for id in CANONICAL_HOST_IDS {
        assert!(
            metadata.contains_key(*id),
            "canonical host `{id}` missing from RUNTIME_REGISTRY metadata"
        );
    }
    for retired in RETIRED_HOST_IDS {
        assert!(
            !metadata.contains_key(*retired),
            "retired host `{retired}` must not appear in RUNTIME_REGISTRY metadata"
        );
    }

    let projections = runtime["host_projections"]
        .as_object()
        .expect("host_projections");
    for id in CANONICAL_HOST_IDS {
        assert!(
            projections.contains_key(*id),
            "canonical host `{id}` missing from RUNTIME_REGISTRY host_projections"
        );
    }

    assert!(
        project_root().join("AGENTS.md").is_file(),
        "missing host agent policy: AGENTS.md"
    );
}

#[test]
fn document_only_provider_lanes_do_not_become_installable_hosts() {
    let registry =
        read_json(&project_root().join("configs/framework/RUNTIME_PROVIDER_REGISTRY.json"));
    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let host_metadata = runtime["host_targets"]["metadata"]
        .as_object()
        .expect("runtime host metadata");
    let host_projection_providers = registry["host_projection_providers"]
        .as_object()
        .expect("provider host projections");

    for (host_id, provider) in host_projection_providers {
        let status = provider["status"].as_str().unwrap_or_default();
        let runtime_installable = host_metadata
            .get(host_id)
            .and_then(|meta| meta.get("installable"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if status != "implemented" {
            assert!(
                !runtime_installable,
                "document-only provider `{host_id}` must not be installable in RUNTIME_REGISTRY"
            );
        }
    }

    for id in CANONICAL_HOST_IDS {
        assert!(
            host_metadata.contains_key(*id),
            "canonical host `{id}` must appear in RUNTIME_REGISTRY metadata"
        );
    }
    for retired in RETIRED_HOST_IDS {
        assert!(
            !host_metadata.contains_key(*retired),
            "retired host `{retired}` must not appear in RUNTIME_REGISTRY metadata"
        );
    }
}

#[test]
fn manifest_and_runtime_skill_paths_are_loadable() {
    for relative in [
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
    ] {
        let payload = read_json(&project_root().join(relative));
        let keys = payload["keys"].as_array().expect("keys");
        let slug_idx = key_index(keys, "slug");
        let skill_path_idx = key_index(keys, "skill_path");
        for row in payload["skills"].as_array().expect("skills") {
            let row = row.as_array().expect("skill row");
            let slug = row[slug_idx].as_str().expect("slug");
            let skill_path = row[skill_path_idx].as_str().expect("skill_path");
            assert!(
                !skill_path.starts_with('/') && !skill_path.contains(".."),
                "{relative} has unsafe skill_path for {slug}: {skill_path}"
            );
            assert!(
                project_root().join(skill_path).is_file(),
                "{relative} missing skill_path for {slug}: {skill_path}"
            );
        }
    }
}

#[test]
fn skill_manifest_excludes_retired_autopilot_slug() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let keys = manifest["keys"].as_array().expect("manifest keys");
    let slug_idx = key_index(keys, "slug");
    let slugs = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .map(|row| row[slug_idx].as_str().expect("manifest slug"))
        .collect::<Vec<_>>();
    assert!(
        !slugs.contains(&"autopilot"),
        "retired autopilot must not appear in SKILL_MANIFEST.json (stub remains on disk only)"
    );
}

#[test]
fn routing_eval_cases_reference_existing_manifest_skills() {
    let manifest = read_json(&project_root().join("skills/SKILL_MANIFEST.json"));
    let manifest_keys = manifest["keys"].as_array().expect("manifest keys");
    let manifest_slug_idx = key_index(manifest_keys, "slug");
    let manifest_slugs = manifest["skills"]
        .as_array()
        .expect("manifest skills")
        .iter()
        .map(|row| row[manifest_slug_idx].as_str().expect("manifest slug"))
        .collect::<std::collections::HashSet<_>>();
    let eval_cases = read_json(&project_root().join("tests/routing_eval_cases.json"));
    for case in eval_cases["cases"].as_array().expect("eval cases") {
        let id = case["id"].as_str().unwrap_or("<missing id>");
        for key in ["focus_skill", "expected_owner", "expected_overlay"] {
            if let Some(slug) = case.get(key).and_then(|value| value.as_str()) {
                assert!(
                    manifest_or_runtime_lane_contains(&manifest_slugs, slug),
                    "case {id} {key} references missing slug {slug}"
                );
            }
        }
        for slug in case
            .get("forbidden_owners")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
        {
            assert!(
                manifest_or_runtime_lane_contains(&manifest_slugs, slug),
                "case {id} forbidden_owners references missing slug {slug}"
            );
        }
    }
}

#[test]
fn paper_prose_quality_hook_txt_exists_and_nl_signal_registered() {
    let root = project_root();
    let prose_txt = root.join("configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");
    assert!(
        prose_txt.is_file(),
        "missing PAPER_PROSE_QUALITY_HOOK.txt at {}",
        prose_txt.display()
    );
    let body = read_text(&prose_txt);
    assert!(
        body.contains("PAPER_PROSE_QUALITY_HOOK") || body.contains("language_register"),
        "prose hook txt must contain actionable prose gate hints"
    );
    let nl = read_json(&root.join("configs/framework/NL_ROUTE_ADJUSTMENTS.json"));
    let post_rules = nl["post_framework_alias_rules"]
        .as_array()
        .expect("nl post_framework_alias_rules array");
    let has_prose_boost = post_rules.iter().any(|rule| {
        rule.get("when")
            .and_then(|w| w.get("signal"))
            .and_then(Value::as_str)
            == Some("has_paper_prose_edit_context")
            && rule
                .get("record")
                .and_then(|r| r.get("slug"))
                .and_then(Value::as_str)
                == Some("paper-workbench")
            && rule
                .get("action")
                .and_then(|a| a.get("type"))
                .and_then(Value::as_str)
                == Some("boost")
    });
    assert!(
        has_prose_boost,
        "NL_ROUTE_ADJUSTMENTS must boost paper-workbench on has_paper_prose_edit_context"
    );
    let has_writing_boost = post_rules.iter().any(|rule| {
        rule.get("when")
            .and_then(|w| w.get("signal"))
            .and_then(Value::as_str)
            == Some("has_paper_writing_context")
            && rule
                .get("record")
                .and_then(|r| r.get("slug"))
                .and_then(Value::as_str)
                == Some("paper-workbench")
            && rule
                .get("action")
                .and_then(|a| a.get("type"))
                .and_then(Value::as_str)
                == Some("boost")
    });
    assert!(
        has_writing_boost,
        "NL_ROUTE_ADJUSTMENTS must boost paper-workbench on has_paper_writing_context"
    );
    for rule in post_rules {
        let slug = rule
            .get("record")
            .and_then(|r| r.get("slug"))
            .and_then(Value::as_str);
        if let Some(s) = slug {
            assert!(
                s != "paper-reviewer" && s != "paper-reviser",
                "post_framework_alias_rules must not target dead hot-route slugs: {s}"
            );
        }
    }
    let signals_rs = read_text(&root.join("core/routing-engine/src/route/nl_route_adjustments.rs"));
    assert!(
        signals_rs.contains("has_paper_prose_negation_context"),
        "nl_route_adjustments must register has_paper_prose_negation_context"
    );
    let hooks_rs = read_text(&root.join("core/host-projection/src/hooks.rs"));
    // Collect paper_prose_env values from RUNTIME_REGISTRY.json → host_targets.metadata
    let registry_path = root.join("configs/framework/RUNTIME_REGISTRY.json");
    let registry_text = fs::read_to_string(&registry_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", registry_path.display()));
    let registry: serde_json::Value =
        serde_json::from_str(&registry_text).expect("parse RUNTIME_REGISTRY.json");
    let metadata = registry
        .get("host_targets")
        .and_then(|ht| ht.get("metadata"))
        .and_then(|m| m.as_object())
        .expect("host_targets.metadata");
    let paper_prose_envs: Vec<String> = metadata
        .values()
        .filter_map(|h| h.get("paper_prose_env"))
        .filter_map(|v| v.as_str())
        .map(String::from)
        .collect();
    assert!(
        !paper_prose_envs.is_empty(),
        "RUNTIME_REGISTRY.json host_targets.metadata must declare at least one paper_prose_env"
    );
    for env in &paper_prose_envs {
        assert!(
            hooks_rs.contains(env),
            "host-projection/src/hooks.rs must declare {env} (v6: moved from paper_prose_hook.rs)"
        );
    }
}
