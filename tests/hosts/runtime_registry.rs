use crate::common;
use crate::common::{
    CANONICAL_HOST_IDS, RETIRED_HOST_IDS, assert_canonical_closed_set_host_ids,
    host_integration_json, output_text, project_root, read_json, router_rs_command, router_rs_json,
    run, write_json, write_text,
};
use serde_json::{Value, json};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn runtime_registry_review_gate_lane_fields_present_on_disk() {
    let v = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let lanes = common::reviewer_lanes_from_registry(&v);
    common::assert_reviewer_lanes_closed(&lanes);
}

#[test]
fn runtime_registry_closed_set_is_canonical_five_hosts() {
    let payload = runtime_registry(&project_root());
    let supported = payload["host_targets"]["supported"]
        .as_array()
        .expect("supported hosts");
    let supported_ids: Vec<&str> = supported.iter().filter_map(|v| v.as_str()).collect();
    assert_canonical_closed_set_host_ids(&supported_ids);

    let metadata = payload["host_targets"]["metadata"]
        .as_object()
        .expect("host metadata");
    for id in CANONICAL_HOST_IDS {
        assert!(
            metadata.contains_key(*id),
            "canonical host `{id}` must appear in host_targets.metadata"
        );
    }
    for retired in RETIRED_HOST_IDS {
        assert!(
            !metadata.contains_key(*retired),
            "retired host `{retired}` must not appear in host_targets.metadata"
        );
    }
}

#[test]
fn route_search_host_id_filters_skill_body_platforms_but_keeps_framework_commands() {
    let tmp = tempdir().unwrap();
    let runtime_path = tmp.path().join("SKILL_ROUTING_RUNTIME.json");
    write_json(
        &runtime_path,
        &json!({
            "version": 3,
            "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "priority", "skill_path", "host_platforms", "kind"],
            "skills": [
                ["opencode-only", "L1", "owner", "none", "n/a", "Opencode-only skill", ["opencode only"], "P1", "skills/opencode-only/SKILL.md", ["opencode"], "skill"],
                ["cursor-skill", "L1", "owner", "none", "n/a", "Cursor skill", ["cursor task"], "P1", "skills/cursor-skill/SKILL.md", ["cursor"], "skill"],
                ["gitx", "L1", "owner", "none", "n/a", "Git command", ["gitx", "/gitx"], "P1", "skills/gitx/SKILL.md", ["cursor"], "framework_command"]
            ]
        }),
    );

    let filtered = router_rs_json(&[
        "search",
        "opencode only",
        "--runtime",
        runtime_path.to_str().unwrap(),
        "--host-id",
        "cursor",
        "--json",
    ]);
    assert!(
        filtered["matches"].as_array().unwrap().iter().all(|entry| {
            entry["record"]["name"].as_str().unwrap_or_default() != "opencode-only"
        }),
        "cursor host search must not return opencode-only skill: {filtered}"
    );

    let alias = router_rs_json(&[
        "search",
        "/gitx",
        "--runtime",
        runtime_path.to_str().unwrap(),
        "--host-id",
        "cursor",
        "--json",
    ]);
    assert!(
        alias["matches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["record"]["name"].as_str().unwrap_or_default() == "gitx" }),
        "cursor host search must keep framework command aliases available: {alias}"
    );
}

#[test]
fn framework_runtime_package_stays_absent() {
    assert!(!project_root().join("framework_runtime").exists());
}

#[test]
fn runtime_registry_missing_file_fails_closed_with_actionable_error() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    let output = run(router_rs_command([
        "framework",
        "host-integration",
        "export-runtime-registry",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]));
    assert!(!output.status.success());
    let (_stdout, stderr) = output_text(&output);
    assert!(stderr.contains("runtime registry not found"));
    assert!(stderr.contains("--framework-root"));
}

#[test]
fn runtime_registry_prefers_repo_local_registry_for_explicit_repo_root() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    write_text(
        &registry_path,
        &serde_json::to_string_pretty(&json!({
            "schema_version": "framework-runtime-registry-v2",
            "framework_core": {
                "authority": "rust",
                "source": "framework-root-native",
                "host_policy": "closed-set-explicit-projections"
            },
            "host_projections": {"cursor": {"profile_id": "repo-cursor"}},
            "workspace_bootstrap_defaults": {"skills": {"source_rel": "repo-skills"}},
            "framework_commands": {"gitx": {"canonical_owner": "repo-owner"}}
        }))
        .unwrap(),
    );
    let payload = runtime_registry(&repo_root);
    assert_eq!(
        payload["host_projections"]["cursor"]["profile_id"],
        "repo-cursor"
    );
    assert_eq!(
        payload["framework_commands"]["gitx"]["canonical_owner"],
        "repo-owner"
    );
}

#[test]
fn runtime_registry_exposes_framework_commands_and_native_runtime_contract() {
    let payload = runtime_registry(&project_root());
    let aliases = &payload["framework_commands"];
    assert!(aliases.get("autopilot").is_none());
    assert!(aliases.get("gsd").is_none());
    assert_eq!(aliases["gitx"]["canonical_owner"], "gitx");
    assert_eq!(aliases["gitx"]["host_entrypoints"]["codex"], "/gitx");
    assert_eq!(aliases["gitx"]["host_entrypoints"]["cursor"], "/gitx");
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["codex"],
        "/deepinterview"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["cursor"],
        "/deepinterview"
    );
    assert_eq!(aliases["gitx"]["host_entrypoints"]["codex"], "/gitx");
    assert_eq!(aliases["gitx"]["host_entrypoints"]["cursor"], "/gitx");
    assert_eq!(
        aliases["gitx"]["interaction_invariants"]["implicit_route_policy"],
        "never"
    );
    assert_eq!(
        aliases["gitx"]["interaction_invariants"]["requires_explicit_entrypoint"],
        true
    );
    let gitx_entrypoints = aliases["gitx"]["interaction_invariants"]["explicit_entrypoints"]
        .as_array()
        .expect("gitx explicit_entrypoints should be an array");
    assert!(gitx_entrypoints.contains(&json!("/gitx")));
    assert!(gitx_entrypoints.contains(&json!("gitx")));
    assert!(
        aliases.get("team").is_none(),
        "retired framework_commands.team must not be exposed (fail-closed; workflow via NL routing)"
    );
    assert_eq!(
        payload["host_targets"]["policy"],
        "shared-rust-core-explicit-host-projections"
    );
    assert_eq!(
        payload["host_targets"]["supported"],
        json!(["cursor", "claude", "opencode", "codex"])
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["codex"]["install_tool"],
        "codex"
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["cursor"]["host_entrypoints"],
        json!(["AGENTS.md", ".cursor/rules/*.mdc"])
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["claude"]["install_tool"],
        "claude"
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["claude"]["host_entrypoints"],
        json!([
            "AGENTS.md",
            ".claude/rules/framework.md",
            ".claude/settings.json"
        ])
    );
    assert!(payload.get("mcp_clients").is_none());
    assert_eq!(aliases["gitx"]["canonical_owner"], "gitx");
}

#[test]
fn runtime_registry_host_projections_expose_supervisor_capabilities() {
    let payload = runtime_registry(&project_root());
    let codex = &payload["host_projections"]["codex"];
    assert_eq!(codex["profile_id"], "codex_profile");
    let codex_capabilities = codex["capabilities"].as_array().unwrap();
    for capability in [
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
    ] {
        assert!(codex_capabilities.contains(&json!(capability)));
    }
    assert_eq!(codex["session_supervisor_driver"], "codex_driver");

    let cursor = &payload["host_projections"]["cursor"];
    assert_eq!(cursor["profile_id"], "cursor_profile");
    // Lock the contract-with-code alignment: agent-orchestrator only
    // accepts codex/codex, so the registry must mark the cursor driver
    // as unsupported (was previously misdeclared as "cursor_driver").
    assert_eq!(cursor["session_supervisor_driver"], "unsupported");
    assert_eq!(cursor["session_supervisor_status"]["supported"], false);
}

#[test]
fn hook_policy_save_optimize_operations_are_exposed() {
    let category_payload = router_rs_json(&[
        "hook-policy",
        "evaluate",
        "--input-json",
        r#"{"operation":"save-optimize-category","path":"src/main.rs"}"#,
    ]);
    assert_eq!(category_payload["operation"], "save-optimize-category");
    assert_eq!(category_payload["category"], "balanced");

    let guard_payload = router_rs_json(&[
        "hook-policy",
        "evaluate",
        "--input-json",
        r#"{"operation":"save-optimize-guard","path":"README.md"}"#,
    ]);
    assert_eq!(guard_payload["operation"], "save-optimize-guard");
    assert_eq!(guard_payload["blocked"], true);
    assert_eq!(guard_payload["category"], "skip");
}

fn runtime_registry(repo_root: &Path) -> Value {
    host_integration_json(&[
        "export-runtime-registry",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ])
}
