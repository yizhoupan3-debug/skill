//! Roadmap v5 CG deferred: B1 semantic dispatch smoke for `mcp-codegraph` tools.
//!
//! Verifies RUNTIME_REGISTRY managed MCP wiring and SKILL_MANIFEST allowedTools
//! align with `codegraph_*` tool intents (no live MCP stdio required).

use std::path::PathBuf;

use serde_json::Value;

use crate::framework_host_targets::host_targets_supported_host_ids;
use crate::runtime_registry::{
    load_runtime_registry_json, managed_mcp_server_for_tool, parse_host_mcp_tool_fqn,
    resolves_managed_mcp_tool,
};

const CODEGRAPH_TOOLS: &[&str] = &[
    "codegraph_search",
    "codegraph_callers",
    "codegraph_callees",
    "codegraph_impact",
    "codegraph_node",
    "codegraph_status",
    "codegraph_dead_code",
];

fn framework_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest_allowed_tools_for_slug(manifest: &Value, slug: &str) -> Vec<String> {
    let keys = manifest
        .get("keys")
        .and_then(Value::as_array)
        .expect("manifest keys");
    let idx_slug = keys
        .iter()
        .position(|key| key.as_str() == Some("slug"))
        .expect("slug key");
    let idx_allowed = keys
        .iter()
        .position(|key| key.as_str() == Some("allowedTools"))
        .expect("allowedTools key");
    manifest
        .get("skills")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .find(|row| row.get(idx_slug).and_then(Value::as_str) == Some(slug))
        .and_then(|row| row.get(idx_allowed))
        .and_then(|tools| {
            tools.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Registry `managed_mcp_servers.mcp-codegraph.tools` resolves each `codegraph_*` intent.
#[test]
fn codegraph_registry_semantic_dispatch_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");

    let entry = registry
        .get("managed_mcp_servers")
        .and_then(|v| v.get("mcp-codegraph"))
        .expect("managed_mcp_servers.mcp-codegraph");
    assert_eq!(
        entry.get("server_id").and_then(Value::as_str),
        Some("mcp-codegraph")
    );
    assert_eq!(
        entry.get("crate").and_then(Value::as_str),
        Some("codegraph-rs")
    );

    for tool in CODEGRAPH_TOOLS {
        assert_eq!(
            managed_mcp_server_for_tool(&registry, tool).as_deref(),
            Some("mcp-codegraph"),
            "tool {tool} must resolve to mcp-codegraph"
        );
        assert!(
            resolves_managed_mcp_tool(&registry, tool),
            "bare tool intent {tool} must be recognized"
        );
        let fqn = format!("mcp__mcp-codegraph__{tool}");
        assert!(
            resolves_managed_mcp_tool(&registry, &fqn),
            "host FQN {fqn} must resolve"
        );
        let parsed = parse_host_mcp_tool_fqn(&fqn).expect("parse FQN");
        assert_eq!(parsed.0, "mcp-codegraph");
        assert_eq!(parsed.1, *tool);
    }

    assert!(
        managed_mcp_server_for_tool(&registry, "grep").is_none(),
        "non-codegraph tools must not false-positive"
    );
}

/// Every closed-set host projection registers `mcp-codegraph`.
#[test]
fn codegraph_host_projections_register_mcp_codegraph_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let host_ids = host_targets_supported_host_ids(&registry).expect("supported hosts");
    let projections = registry
        .get("host_projections")
        .and_then(Value::as_object)
        .expect("host_projections");

    for host_id in host_ids {
        let projection = projections
            .get(&host_id)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing projection for {host_id}"));
        let managed = projection
            .get("managed_mcp_server_ids")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{host_id} missing managed_mcp_server_ids"));
        assert!(
            managed.iter().any(|id| id.as_str() == Some("mcp-codegraph")),
            "{host_id} must list mcp-codegraph in managed_mcp_server_ids"
        );
    }
}

const CODEGRAPH_SKILL_SLUGS: &[&str] = &[
    "planx",
    "implementx",
    "verifyx",
    "code-review-deep",
];

fn assert_manifest_codegraph_tools_resolve(registry: &Value, payload: &Value, slug: &str) {
    let allowed = manifest_allowed_tools_for_slug(payload, slug);
    let codegraph_fqns: Vec<&String> = allowed
        .iter()
        .filter(|fqn| fqn.starts_with("mcp__mcp-codegraph__codegraph_"))
        .collect();
    assert_eq!(
        codegraph_fqns.len(),
        CODEGRAPH_TOOLS.len(),
        "{slug} must declare six codegraph allowedTools"
    );

    for fqn in codegraph_fqns {
        assert!(
            resolves_managed_mcp_tool(registry, fqn),
            "manifest tool {fqn} must resolve via registry semantic dispatch"
        );
        let (_, tool_name) = parse_host_mcp_tool_fqn(fqn).expect("parse manifest FQN");
        assert!(
            CODEGRAPH_TOOLS.contains(&tool_name.as_str()),
            "manifest tool {tool_name} not in registry catalog"
        );
    }
}

/// Lifecycle + review skills manifest allowedTools route to the same registry tool catalog.
#[test]
fn codegraph_manifest_allowed_tools_route_to_mcp_codegraph_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let manifest_path = root.join("skills/SKILL_MANIFEST.json");
    let payload: Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");

    for slug in CODEGRAPH_SKILL_SLUGS {
        assert_manifest_codegraph_tools_resolve(&registry, &payload, slug);
    }
}

#[cfg(feature = "codegraph")]
#[test]
fn codegraph_mcp_tool_catalog_matches_registry_smoke() {
    use crate::mcp_common::codegraph::tool_definitions;

    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let registry_tools: Vec<String> = registry
        .get("managed_mcp_servers")
        .and_then(|v| v.get("mcp-codegraph"))
        .and_then(|v| v.get("tools"))
        .and_then(Value::as_array)
        .expect("registry codegraph tools")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();

    let live: Vec<String> = tool_definitions()
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect();

    assert_eq!(live.len(), registry_tools.len());
    for name in &registry_tools {
        assert!(
            live.iter().any(|tool| tool == name),
            "codegraph-rs catalog missing registry tool {name}"
        );
        assert_eq!(
            managed_mcp_server_for_tool(&registry, name).as_deref(),
            Some("mcp-codegraph")
        );
    }
}
