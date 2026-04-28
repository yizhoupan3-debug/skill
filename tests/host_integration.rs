mod common;

use common::{host_integration_json, project_root, read_json, read_text, write_json, write_text};
use serde_json::json;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn install_native_integration_is_idempotent() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    write_text(
        &repo_root.join("skills/SKILL_ROUTING_RUNTIME.json"),
        r#"{"skills":[["systematic-debugging","L0","gate","evidence","required","debug",[],97.0,"P1"]]}"#,
    );
    write_text(
        &repo_root.join("skills/gitx/SKILL.md"),
        "---\nname: gitx\n---\n",
    );
    write_text(
        &repo_root.join("skills/deepinterview/SKILL.md"),
        "---\nname: deepinterview\n---\n",
    );
    write_text(
        &repo_root.join("skills/systematic-debugging/SKILL.md"),
        "---\nname: systematic-debugging\n---\n",
    );
    write_text(
        &repo_root.join("skills/skill-framework-developer/SKILL.md"),
        "---\nname: skill-framework-developer\n---\n",
    );
    write_text(
        &repo_root.join("configs/framework/RUNTIME_REGISTRY.json"),
        r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{"autopilot":{},"team":{}}}"#,
    );
    write_text(
        &repo_root.join("skills/optional-heavy/SKILL.md"),
        "---\nname: optional-heavy\n---\n",
    );
    let plugin_root = repo_root.join("plugins/skill-framework-native/.codex-plugin");
    std::fs::create_dir_all(&plugin_root).unwrap();
    write_text(
        &plugin_root.join("plugin.json"),
        "{\"name\":\"skill-framework-native\"}\n",
    );

    let home_config_path = tmp.path().join("home/.codex/config.toml");
    let home_codex_skills_path = tmp.path().join("home/.codex/skills");
    let bootstrap_output_dir = tmp.path().join("bootstrap");

    let args = vec![
        "install-native-integration".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--home-config-path".to_string(),
        home_config_path.display().to_string(),
        "--home-codex-skills-path".to_string(),
        home_codex_skills_path.display().to_string(),
        "--bootstrap-output-dir".to_string(),
        bootstrap_output_dir.display().to_string(),
    ];
    let refs = string_refs(&args);
    let first = host_integration_json(&refs);
    let second = host_integration_json(&refs);

    let content = read_text(&home_config_path);
    assert_eq!(first["success"], true);
    assert_eq!(second["success"], true);
    assert_eq!(content.matches("[features]").count(), 1);
    assert_eq!(content.matches("codex_hooks = false").count(), 1);
    assert_eq!(content.matches("[mcp_servers.browser-mcp]").count(), 0);
    assert_eq!(content.matches("[mcp_servers.framework-mcp]").count(), 0);
    assert_eq!(
        content.matches("[mcp_servers.openaiDeveloperDocs]").count(),
        0
    );
    assert_eq!(content.matches("[tui]").count(), 1);
    let surface_root = repo_root.join("artifacts/codex-skill-surface/skills");
    assert!(is_symlink_to(&home_codex_skills_path, &surface_root));
    assert!(is_symlink_to(
        &surface_root.join("gitx"),
        &repo_root.join("skills/gitx")
    ));
    assert!(is_symlink_to(
        &surface_root.join("deepinterview"),
        &repo_root.join("skills/deepinterview")
    ));
    assert!(is_symlink_to(
        &surface_root.join("systematic-debugging"),
        &repo_root.join("skills/systematic-debugging")
    ));
    assert_framework_alias_skill(&surface_root, "autopilot");
    assert_framework_alias_skill(&surface_root, "team");
    assert!(!surface_root.join("optional-heavy").exists());
    assert_eq!(first["default_bootstrap"]["status"], "materialized");
    assert!(["already-present", "repaired-stale"]
        .contains(&second["default_bootstrap"]["status"].as_str().unwrap()));
    assert_eq!(first["home_codex_skills_changed"], true);
    assert_eq!(second["home_codex_skills_changed"], false);
}

#[test]
fn install_claude_desktop_mcp_writes_minimal_stdio_server() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("scripts/router-rs")).unwrap();
    write_text(&repo_root.join("scripts/router-rs/run_router_rs.sh"), "#!/bin/sh\n");
    write_text(&repo_root.join("scripts/router-rs/Cargo.toml"), "[package]\n");
    let config_path = tmp.path().join("Claude/claude_desktop_config.json");

    let args = vec![
        "install-claude-desktop-mcp".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let refs = string_refs(&args);
    let first = host_integration_json(&refs);
    let second = host_integration_json(&refs);
    let config = read_json(&config_path);
    let server = &config["mcpServers"]["browser-mcp"];

    assert_eq!(first["success"], true);
    assert_eq!(first["changed"], true);
    assert_eq!(second["success"], true);
    assert_eq!(second["changed"], false);
    assert_eq!(
        server["command"],
        repo_root
            .join("scripts/router-rs/run_router_rs.sh")
            .to_string_lossy()
            .to_string()
    );
    assert_eq!(
        server["args"],
        json!([
            repo_root
                .join("scripts/router-rs/Cargo.toml")
                .to_string_lossy()
                .to_string(),
            "browser",
            "mcp-stdio",
            "--repo-root",
            repo_root.to_string_lossy().to_string(),
        ])
    );
}

#[test]
fn install_claude_desktop_mcp_preserves_existing_servers() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("scripts/router-rs")).unwrap();
    let config_path = tmp.path().join("claude_desktop_config.json");
    write_json(
        &config_path,
        &json!({
            "mcpServers": {
                "existing": {
                    "command": "/bin/echo",
                    "args": ["ok"]
                }
            },
            "otherSetting": true
        }),
    );

    let args = vec![
        "install-claude-desktop-mcp".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let refs = string_refs(&args);
    let result = host_integration_json(&refs);
    let config = read_json(&config_path);

    assert_eq!(result["success"], true);
    assert_eq!(config["otherSetting"], true);
    assert_eq!(config["mcpServers"]["existing"]["command"], "/bin/echo");
    assert_eq!(config["mcpServers"]["browser-mcp"]["args"][1], "browser");
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
fn current_artifact_clutter_plan_archives_current_mirrors() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let current_root = repo_root.join("artifacts/current");
    let task_root = current_root.join("task-1");
    std::fs::create_dir_all(&task_root).unwrap();
    write_text(
        &current_root.join("SESSION_SUMMARY.md"),
        "stale root mirror\n",
    );
    write_json(
        &current_root.join("NEXT_ACTIONS.json"),
        &json!({"next_actions":["stale"]}),
    );
    write_text(&task_root.join("SESSION_SUMMARY.md"), "task scoped\n");
    write_json(
        &task_root.join("CONTINUITY_JOURNAL.json"),
        &json!({"ok": true}),
    );

    let result = host_integration_json(&[
        "plan-current-artifact-clutter",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--active-task-id",
        "task-1",
    ]);
    let plans = result["plans"].as_array().unwrap();
    let sources = plans
        .iter()
        .map(|plan| plan["source"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert!(sources
        .iter()
        .any(|path| path.ends_with("artifacts/current/SESSION_SUMMARY.md")));
    assert!(sources
        .iter()
        .any(|path| path.ends_with("artifacts/current/NEXT_ACTIONS.json")));
    assert!(!sources
        .iter()
        .any(|path| path.ends_with("artifacts/current/task-1/SESSION_SUMMARY.md")));
    assert!(!sources
        .iter()
        .any(|path| path.ends_with("artifacts/current/task-1/CONTINUITY_JOURNAL.json")));
}

#[test]
fn memory_automation_reports_missing_continuity_control_plane() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();

    let result = host_integration_json(&[
        "run-memory-automation",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--workspace",
        "skill",
        "--output-dir",
        tmp.path().join("memory-automation").to_str().unwrap(),
        "--query",
        "memory audit",
    ]);

    let health = &result["continuity_health"];
    assert_eq!(health["status"], "blocked");
    let blockers = health["blockers"].as_array().unwrap();
    assert!(blockers
        .iter()
        .any(|item| item.as_str().unwrap().contains("current_root missing")));
    assert!(blockers
        .iter()
        .any(|item| item.as_str().unwrap().contains("supervisor_state missing")));
    assert!(blockers.iter().any(|item| item
        .as_str()
        .unwrap()
        .contains("continuity active_task_id is empty")));
}

#[test]
fn install_skills_rust_entrypoint_links_codex_only() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    write_text(
        &repo_root.join("skills/SKILL_ROUTING_RUNTIME.json"),
        r#"{"skills":[["systematic-debugging","L0","gate","evidence","required","debug",[],97.0,"P1"]]}"#,
    );
    write_text(
        &repo_root.join("skills/gitx/SKILL.md"),
        "---\nname: gitx\n---\n",
    );
    write_text(
        &repo_root.join("skills/deepinterview/SKILL.md"),
        "---\nname: deepinterview\n---\n",
    );
    write_text(
        &repo_root.join("skills/systematic-debugging/SKILL.md"),
        "---\nname: systematic-debugging\n---\n",
    );
    write_text(
        &repo_root.join("skills/skill-framework-developer/SKILL.md"),
        "---\nname: skill-framework-developer\n---\n",
    );
    write_text(
        &repo_root.join("configs/framework/RUNTIME_REGISTRY.json"),
        r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{"autopilot":{},"team":{}}}"#,
    );
    write_text(
        &repo_root.join("skills/optional-heavy/SKILL.md"),
        "---\nname: optional-heavy\n---\n",
    );
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
    assert!(first["results"].get("agents").is_none());
    let codex_skills = home.join(".codex/skills");
    let surface_root = repo_root.join("artifacts/codex-skill-surface/skills");
    assert!(is_symlink_to(&codex_skills, &surface_root));
    assert_eq!(second["surface_skills"], 5);
    assert_eq!(second["results"]["codex"]["checks"]["codex_skills"], true);
    assert_eq!(
        second["results"]["codex"]["checks"]["codex_skill_surface"],
        true
    );
    assert_eq!(second["results"]["codex"]["checks"]["config"], true);
    assert_eq!(
        second["results"]["codex"]["status"],
        "native-integration-ready"
    );
}

fn is_symlink_to(path: &Path, expected_target: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(target) = std::fs::read_link(path).map(|target| {
        if target.is_absolute() {
            target
        } else {
            path.parent().unwrap_or_else(|| Path::new(".")).join(target)
        }
    }) else {
        return false;
    };
    target.canonicalize().ok() == expected_target.canonicalize().ok()
}

fn assert_framework_alias_skill(surface_root: &Path, slug: &str) {
    let content = read_text(&surface_root.join(slug).join("SKILL.md"));
    let dollar_entrypoint = format!("${slug}");
    assert!(content.contains(&format!("name: {slug}")));
    assert!(content.contains("generated lightweight Codex CLI/App alias"));
    assert!(content.contains(&format!("`{dollar_entrypoint}`")));
    assert!(content.contains(&format!("`/{slug}`")));
    assert!(!content.contains("claude-code="));
    assert!(content.contains("skills/skill-framework-developer/SKILL.md"));
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
    let bootstrap_ok = host_integration_json(&[
        "validate-default-bootstrap",
        "--bootstrap-path",
        bootstrap_path.to_str().unwrap(),
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    let source_path = host_integration_json(&[
        "resolve-skills-source",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]);
    assert!(bootstrap_ok["ok"].as_bool().is_some());
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
fn framework_runtime_package_stays_absent() {
    assert!(!project_root().join("framework_runtime").exists());
}

#[test]
fn runtime_registry_missing_file_uses_default_registry() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    let payload = runtime_registry(&repo_root);
    assert_eq!(payload["schema_version"], "framework-runtime-registry-v1");
    assert_eq!(payload["codex_host"]["profile_id"], "codex_profile");
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
            "codex_host": {"profile_id": "repo-codex"},
            "workspace_bootstrap_defaults": {"skills": {"source_rel": "repo-skills"}},
            "framework_commands": {"autopilot": {"canonical_owner": "repo-owner"}}
        }))
        .unwrap(),
    );
    let payload = runtime_registry(&repo_root);
    assert_eq!(payload["codex_host"]["profile_id"], "repo-codex");
    assert_eq!(
        payload["framework_commands"]["autopilot"]["canonical_owner"],
        "repo-owner"
    );
}

#[test]
fn runtime_registry_exposes_framework_commands_and_native_runtime_contract() {
    let payload = runtime_registry(&project_root());
    let aliases = &payload["framework_commands"];
    assert_eq!(
        aliases["autopilot"]["canonical_owner"],
        "execution-controller-coding"
    );
    assert_eq!(
        aliases["autopilot"]["host_entrypoints"]["codex-cli"],
        "$autopilot"
    );
    assert_eq!(
        aliases["autopilot"]["host_entrypoints"]["codex-app"],
        "$autopilot"
    );
    assert_eq!(
        aliases["autopilot"]["interaction_invariants"]["implicit_route_policy"],
        "never"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["codex-cli"],
        "$deepinterview"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["codex-app"],
        "$deepinterview"
    );
    assert_eq!(aliases["team"]["host_entrypoints"]["codex-cli"], "$team");
    assert_eq!(aliases["team"]["host_entrypoints"]["codex-app"], "$team");
    assert_eq!(
        payload["host_targets"]["supported"],
        json!(["codex-cli", "codex-app"])
    );
    assert_eq!(
        payload["mcp_clients"]["claude-desktop"]["uses_runtime_surfaces"],
        json!(["router-rs", "browser-mcp"])
    );
    assert_eq!(
        payload["mcp_clients"]["claude-desktop"]["uses_skill_surface"],
        false
    );
    assert_eq!(
        payload["mcp_clients"]["claude-desktop"]["image_required"],
        false
    );
    assert_eq!(aliases["team"]["route_mode"], "team-orchestration");
    let autopilot = &aliases["autopilot"];
    assert_eq!(autopilot["lineage"]["source"], "repo-native");
    assert!(autopilot["implementation_bar"]
        .as_array()
        .unwrap()
        .contains(&json!("resume-and-recovery-required")));
}

#[test]
fn runtime_registry_codex_host_exposes_supervisor_capabilities() {
    let payload = runtime_registry(&project_root());
    let codex = &payload["codex_host"];
    assert_eq!(codex["profile_id"], "codex_profile");
    let capabilities = codex["capabilities"].as_array().unwrap();
    for capability in [
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
    ] {
        assert!(capabilities.contains(&json!(capability)));
    }
    assert_eq!(codex["session_supervisor_driver"], "codex_driver");
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
