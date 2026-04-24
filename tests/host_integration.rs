mod common;

use common::{host_integration_json, project_root, read_json, read_text, write_text};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn install_native_integration_is_idempotent() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join(".codex")).unwrap();
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    let plugin_root = repo_root.join("plugins/skill-framework-native/.codex-plugin");
    std::fs::create_dir_all(&plugin_root).unwrap();
    write_text(
        &plugin_root.join("plugin.json"),
        "{\"name\":\"skill-framework-native\"}\n",
    );

    let home_config_path = tmp.path().join("home/.codex/config.toml");
    let home_codex_skills_path = tmp.path().join("home/.codex/skills");
    let home_claude_skills_path = tmp.path().join("home/.claude/skills");
    let home_claude_refresh_path = tmp.path().join("home/.claude/commands/refresh.md");
    let home_claude_mcp_config_path = tmp.path().join("home/.claude.json");
    let home_plugin_root = tmp
        .path()
        .join("home/.codex/plugins/skill-framework-native");
    let home_marketplace_path = tmp.path().join("home/.agents/plugins/marketplace.json");
    let bootstrap_output_dir = tmp.path().join("bootstrap");

    let args = vec![
        "install-native-integration".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--home-config-path".to_string(),
        home_config_path.display().to_string(),
        "--home-plugin-root".to_string(),
        home_plugin_root.display().to_string(),
        "--home-marketplace-path".to_string(),
        home_marketplace_path.display().to_string(),
        "--home-codex-skills-path".to_string(),
        home_codex_skills_path.display().to_string(),
        "--home-claude-skills-path".to_string(),
        home_claude_skills_path.display().to_string(),
        "--home-claude-refresh-path".to_string(),
        home_claude_refresh_path.display().to_string(),
        "--home-claude-mcp-config-path".to_string(),
        home_claude_mcp_config_path.display().to_string(),
        "--bootstrap-output-dir".to_string(),
        bootstrap_output_dir.display().to_string(),
        "--skip-home-claude-skills-link".to_string(),
        "--skip-home-claude-refresh".to_string(),
    ];
    let refs = string_refs(&args);
    let first = host_integration_json(&refs);
    let second = host_integration_json(&refs);

    let content = read_text(&home_config_path);
    let claude_mcp_payload = read_json(&home_claude_mcp_config_path);
    let plugin_mcp_payload = read_json(&home_plugin_root.join(".mcp.json"));
    let marketplace = read_json(&home_marketplace_path);

    assert_eq!(first["success"], true);
    assert_eq!(second["success"], true);
    assert_eq!(content.matches("[features]").count(), 1);
    assert_eq!(content.matches("codex_hooks = true").count(), 1);
    assert_eq!(content.matches("[mcp_servers.browser-mcp]").count(), 0);
    assert_eq!(content.matches("[mcp_servers.framework-mcp]").count(), 1);
    assert_eq!(
        content.matches("[mcp_servers.openaiDeveloperDocs]").count(),
        0
    );
    assert_eq!(content.matches("[tui]").count(), 1);
    assert!(home_codex_skills_path.is_symlink());
    assert_eq!(
        home_codex_skills_path.canonicalize().unwrap(),
        repo_root.join("skills").canonicalize().unwrap()
    );
    assert!(!home_claude_skills_path.exists());
    assert!(!home_claude_refresh_path.exists());
    let claude_args = claude_mcp_payload["mcpServers"]["framework-mcp"]["args"]
        .as_array()
        .unwrap();
    assert_eq!(claude_args[0], "--framework-mcp-stdio");
    assert_eq!(claude_args[1], "--repo-root");
    assert_path_eq(
        claude_args[2].as_str().unwrap(),
        &repo_root.canonicalize().unwrap().display().to_string(),
    );
    assert_eq!(
        claude_mcp_payload["mcpServers"].as_object().unwrap().len(),
        1
    );
    assert_path_eq(
        plugin_mcp_payload["mcpServers"]["framework-mcp"]["cwd"]
            .as_str()
            .unwrap(),
        &repo_root.canonicalize().unwrap().display().to_string(),
    );
    assert_eq!(
        plugin_mcp_payload["mcpServers"].as_object().unwrap().len(),
        1
    );
    assert_eq!(
        marketplace["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|plugin| plugin["name"] == "skill-framework-native")
            .count(),
        1
    );
    assert_eq!(first["default_bootstrap"]["status"], "materialized");
    assert!(["already-present", "repaired-stale"]
        .contains(&second["default_bootstrap"]["status"].as_str().unwrap()));
    assert_eq!(first["home_codex_skills_link_changed"], true);
    assert_eq!(second["home_codex_skills_link_changed"], false);
}

#[test]
fn ensure_default_bootstrap_is_idempotent() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let output_dir = tmp.path().join("bootstrap");
    std::fs::create_dir_all(&repo_root).unwrap();
    let first = host_integration_json(&[
        "ensure-default-bootstrap",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--output-dir",
        output_dir.to_str().unwrap(),
    ]);
    let second = host_integration_json(&[
        "ensure-default-bootstrap",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--output-dir",
        output_dir.to_str().unwrap(),
    ]);
    assert_eq!(first["status"], "materialized");
    assert!(["already-present", "repaired-stale"].contains(&second["status"].as_str().unwrap()));
}

#[test]
fn install_native_integration_can_opt_into_rust_browser_mcp() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join(".codex")).unwrap();
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    let plugin_root = repo_root.join("plugins/skill-framework-native/.codex-plugin");
    std::fs::create_dir_all(&plugin_root).unwrap();
    write_text(
        &plugin_root.join("plugin.json"),
        "{\"name\":\"skill-framework-native\"}\n",
    );

    let home_config_path = tmp.path().join("home/.codex/config.toml");
    let args = vec![
        "install-native-integration".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--home-config-path".to_string(),
        home_config_path.display().to_string(),
        "--home-plugin-root".to_string(),
        tmp.path()
            .join("home/.codex/plugins/skill-framework-native")
            .display()
            .to_string(),
        "--home-marketplace-path".to_string(),
        tmp.path()
            .join("home/.agents/plugins/marketplace.json")
            .display()
            .to_string(),
        "--home-codex-skills-path".to_string(),
        tmp.path().join("home/.codex/skills").display().to_string(),
        "--home-claude-skills-path".to_string(),
        tmp.path().join("home/.claude/skills").display().to_string(),
        "--home-claude-refresh-path".to_string(),
        tmp.path()
            .join("home/.claude/commands/refresh.md")
            .display()
            .to_string(),
        "--home-claude-mcp-config-path".to_string(),
        tmp.path().join("home/.claude.json").display().to_string(),
        "--with-browser-mcp".to_string(),
        "--skip-personal-plugin".to_string(),
        "--skip-personal-marketplace".to_string(),
        "--skip-home-codex-skills-link".to_string(),
        "--skip-home-claude-skills-link".to_string(),
        "--skip-home-claude-refresh".to_string(),
        "--skip-home-claude-mcp-sync".to_string(),
        "--skip-default-bootstrap".to_string(),
    ];
    let refs = string_refs(&args);
    let result = host_integration_json(&refs);
    let content = read_text(&home_config_path);
    assert_eq!(result["browser_mcp_changed"], true);
    assert!(content.contains("[mcp_servers.browser-mcp]"));
    assert!(content.contains(&format!(
        "command = \"{}\"",
        repo_root
            .join("scripts/router-rs/target/release/router-rs")
            .display()
    )));
    assert!(content.contains("--browser-mcp-stdio"));
    assert!(!content.contains("tools/browser-mcp/dist/index.js"));
    assert!(!content.contains("command = \"node\""));
}

#[test]
fn install_skills_rust_entrypoint_links_supported_tools() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(repo_root.join(".codex")).unwrap();
    std::fs::create_dir_all(repo_root.join("skills/demo")).unwrap();
    let plugin_root = repo_root.join("plugins/skill-framework-native/.codex-plugin");
    std::fs::create_dir_all(&plugin_root).unwrap();
    write_text(
        &plugin_root.join("plugin.json"),
        "{\"name\":\"skill-framework-native\"}\n",
    );

    let first = host_integration_json(&[
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "all",
    ]);
    let second = host_integration_json(&[
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "status",
    ]);
    assert_eq!(first["success"], true);
    assert_eq!(first["results"]["codex"]["status"], "installed");
    assert_eq!(first["results"]["agents"]["status"], "linked");
    assert_eq!(first["results"]["gemini"]["status"], "linked");
    assert_eq!(
        home.join(".agents/skills").canonicalize().unwrap(),
        repo_root.join("skills").canonicalize().unwrap()
    );
    assert_eq!(
        home.join(".gemini/skills").canonicalize().unwrap(),
        repo_root.join("skills").canonicalize().unwrap()
    );
    assert_eq!(
        second["results"]["codex"]["status"],
        "native-integration-incomplete"
    );
    assert_eq!(second["results"]["agents"]["ready"], true);
    assert_eq!(second["results"]["gemini"]["ready"], true);
}

#[test]
fn validation_subcommands_cover_install_skills_contract() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    let bootstrap_path = tmp.path().join("framework_default_bootstrap.json");
    host_integration_json(&[
        "ensure-default-bootstrap",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--output-dir",
        tmp.path().to_str().unwrap(),
    ]);
    let marketplace_path = tmp.path().join("marketplace.json");
    write_text(
        &marketplace_path,
        &serde_json::to_string(&json!({"plugins": [{"name": "skill-framework-native"}]})).unwrap(),
    );
    let bootstrap_ok = host_integration_json(&[
        "validate-default-bootstrap",
        "--bootstrap-path",
        bootstrap_path.to_str().unwrap(),
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let marketplace_ok = host_integration_json(&[
        "validate-marketplace-plugin",
        "--marketplace-path",
        marketplace_path.to_str().unwrap(),
        "--plugin-name",
        "skill-framework-native",
    ]);
    let source_path = host_integration_json(&[
        "resolve-skill-bridge-source",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert!(bootstrap_ok["ok"].as_bool().is_some());
    assert_eq!(marketplace_ok["ok"], true);
    assert_path_eq(
        source_path["path"].as_str().unwrap(),
        &repo_root
            .join("skills")
            .canonicalize()
            .unwrap()
            .display()
            .to_string(),
    );
}

#[test]
fn python_runtime_package_is_retired() {
    assert!(!project_root().join("framework_runtime").exists());
}

#[test]
fn runtime_registry_missing_file_uses_default_registry() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    let payload = runtime_registry(&repo_root);
    assert_eq!(payload["schema_version"], "framework-runtime-registry-v1");
    assert_eq!(
        payload["shared_project_mcp_servers"],
        json!(["framework-mcp"])
    );
}

#[test]
fn runtime_registry_prefers_repo_local_registry_for_explicit_repo_root() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    write_text(
        &registry_path,
        &serde_json::to_string_pretty(&json!({
            "schema_version": "framework-runtime-registry-v1",
            "default_host_peer_set": ["repo-host"],
            "shared_project_mcp_servers": [],
            "workspace_bootstrap_defaults": {"skill_bridge": {"source_rel": "repo-skills"}},
            "framework_native_aliases": {"autopilot": {"canonical_owner": "repo-owner"}},
            "omc_retirement_contract": {"runtime_authority": "repo-rust"},
            "plugins": [{"plugin_name": "repo-plugin", "source_rel": "repo-plugin"}],
            "host_adapters": []
        }))
        .unwrap(),
    );
    let payload = runtime_registry(&repo_root);
    assert_eq!(payload["plugins"][0]["plugin_name"], "repo-plugin");
    assert_eq!(payload["shared_project_mcp_servers"], json!([]));
    assert_eq!(
        payload["framework_native_aliases"]["autopilot"]["canonical_owner"],
        "repo-owner"
    );
}

#[test]
fn runtime_registry_exposes_framework_native_aliases_and_omc_retirement_contract() {
    let payload = runtime_registry(&project_root());
    let aliases = &payload["framework_native_aliases"];
    assert_eq!(
        aliases["autopilot"]["canonical_owner"],
        "execution-controller-coding"
    );
    assert_eq!(
        aliases["autopilot"]["host_entrypoints"]["codex-cli"],
        "$autopilot"
    );
    assert_eq!(
        aliases["autopilot"]["interaction_invariants"]["implicit_route_policy"],
        "never"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["claude-code"],
        "/deepinterview"
    );
    assert_eq!(aliases["team"]["host_entrypoints"]["claude-code"], "/team");
    assert_eq!(aliases["team"]["route_mode"], "team-orchestration");
    let retirement = &aliases["autopilot"];
    assert_eq!(retirement["external_runtime_dependency"], false);
    assert_eq!(retirement["omc_dependency"], false);
    assert_eq!(retirement["lineage"]["source"], "repo-native");
    assert!(retirement["implementation_bar"]
        .as_array()
        .unwrap()
        .contains(&json!("resume-and-recovery-required")));
}

#[test]
fn runtime_registry_exposes_shared_project_mcp_servers() {
    assert_eq!(
        runtime_registry(&project_root())["shared_project_mcp_servers"],
        json!(["framework-mcp"])
    );
}

#[test]
fn runtime_registry_host_records_expose_supervisor_capabilities() {
    let payload = runtime_registry(&project_root());
    let records = payload["host_adapters"].as_array().unwrap();
    let codex = records
        .iter()
        .find(|row| row["adapter_id"] == "codex_cli_adapter")
        .unwrap();
    let claude = records
        .iter()
        .find(|row| row["adapter_id"] == "claude_code_adapter")
        .unwrap();
    for (record, expected_driver) in [(codex, "codex_driver"), (claude, "claude_driver")] {
        let capabilities = record["host_capabilities"].as_array().unwrap();
        for capability in [
            "external_session_supervisor",
            "rate_limit_auto_resume",
            "host_resume_entrypoint",
            "host_tmux_worker_management",
        ] {
            assert!(capabilities.contains(&json!(capability)));
        }
        assert_eq!(
            record["protocol_hints"]["session_supervisor_driver"],
            expected_driver
        );
    }
    assert_eq!(
        codex["protocol_hints"]["framework_alias_entrypoints"]["autopilot"],
        "$autopilot"
    );
    assert_eq!(
        claude["protocol_hints"]["framework_alias_entrypoints"]["deepinterview"],
        "/deepinterview"
    );
}

fn runtime_registry(repo_root: &std::path::Path) -> serde_json::Value {
    host_integration_json(&[
        "export-runtime-registry",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ])
}

fn string_refs(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

fn assert_path_eq(left: &str, right: &str) {
    assert_eq!(
        normalize_macos_private_var(left),
        normalize_macos_private_var(right)
    );
}

fn normalize_macos_private_var(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/private/") {
        format!("/{rest}")
    } else {
        path.to_string()
    }
}
