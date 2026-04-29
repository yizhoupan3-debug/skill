mod common;

use common::{
    host_integration_json, json_from_output, output_text, project_root, read_json, read_text,
    router_rs_command, router_rs_json, run, write_json, write_text,
};
use serde_json::json;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn install_native_integration_is_idempotent() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    seed_framework_markers(&repo_root);
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
    assert!(!tmp.path().join("home/.codex/prompts/autopilot.md").exists());
    assert!(!tmp.path().join("home/.codex/prompts/gitx.md").exists());
    assert!(!tmp
        .path()
        .join("home/.codex/prompts/systematic-debugging.md")
        .exists());
    assert!(!surface_root.join("optional-heavy").exists());
    assert_eq!(first["default_bootstrap"]["status"], "materialized");
    assert!(["already-present", "repaired-stale"]
        .contains(&second["default_bootstrap"]["status"].as_str().unwrap()));
    assert_eq!(first["home_codex_skills_changed"], true);
    assert_eq!(second["home_codex_skills_changed"], false);
    assert_eq!(first["codex_prompt_entrypoints"]["changed"], false);
    assert_eq!(second["codex_prompt_entrypoints"]["changed"], false);
}

#[test]
fn install_native_integration_preserves_similar_codex_hook_keys_and_dedupes() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills/gitx")).unwrap();
    seed_framework_markers(&repo_root);
    write_text(
        &repo_root.join("skills/gitx/SKILL.md"),
        "---\nname: gitx\n---\n",
    );
    write_text(
        &repo_root.join("skills/SKILL_ROUTING_RUNTIME.json"),
        r#"{"skills":[["gitx","L1","git","git","git","git",[],90.0,"P1"]]}"#,
    );
    let home_config_path = tmp.path().join("home/.codex/config.toml");
    write_text(
        &home_config_path,
        "[features]\ncodex_hooks_extra = true\ncodex_hooks = true\ncodex_hooks = false\n",
    );

    let result = host_integration_json(&[
        "install-native-integration",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home-config-path",
        home_config_path.to_str().unwrap(),
        "--home-codex-skills-path",
        tmp.path().join("home/.codex/skills").to_str().unwrap(),
        "--skip-default-bootstrap",
    ]);

    assert_eq!(result["success"], true);
    let content = read_text(&home_config_path);
    assert!(content.contains("codex_hooks_extra = true"));
    assert_eq!(content.matches("codex_hooks = false").count(), 1);
    assert!(!content.contains("codex_hooks = true"));
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
fn memory_automation_materializes_sqlite_and_continuity_control_plane() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let output_dir = tmp.path().join("memory-automation");
    std::fs::create_dir_all(&repo_root).unwrap();

    let result = host_integration_json(&[
        "run-memory-automation",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--workspace",
        "skill",
        "--output-dir",
        output_dir.to_str().unwrap(),
        "--query",
        "memory audit",
    ]);

    assert_eq!(result["sqlite_result"]["exists"], true);
    assert_eq!(result["sqlite_result"]["memory_items"], 1);
    assert_eq!(result["continuity_seed"]["status"], "materialized");
    assert_eq!(result["continuity_audit"]["status"], "ready");
    assert_eq!(result["continuity_audit"]["control_plane_present"], true);
    assert_eq!(
        result["continuity_audit"]["missing_control_plane_anchors"],
        json!([])
    );
    assert_eq!(result["continuity_audit"]["residual_blocker_count"], 0);

    let health = &result["continuity_health"];
    assert_eq!(health["status"], "ok");

    let active = common::read_json(&repo_root.join("artifacts/current/active_task.json"));
    let task_id = active["task_id"].as_str().unwrap();
    let task_root = repo_root.join("artifacts/current").join(task_id);
    for path in [
        repo_root.join("artifacts/current/active_task.json"),
        repo_root.join("artifacts/current/focus_task.json"),
        repo_root.join("artifacts/current/task_registry.json"),
        repo_root.join(".supervisor_state.json"),
        repo_root.join(".codex/memory/memory.sqlite3"),
        task_root.join("SESSION_SUMMARY.md"),
        task_root.join("NEXT_ACTIONS.json"),
        task_root.join("EVIDENCE_INDEX.json"),
        task_root.join("TRACE_METADATA.json"),
        task_root.join("CONTINUITY_JOURNAL.json"),
        output_dir.join("run_summary.json"),
        output_dir.join("snapshot.json"),
        output_dir.join("storage_audit.json"),
        repo_root.join("artifacts/bootstrap/framework_default_bootstrap.json"),
    ] {
        assert!(path.is_file(), "missing {}", path.display());
    }

    let run_summary = common::read_json(&output_dir.join("run_summary.json"));
    assert_eq!(run_summary["sqlite_result"]["memory_items"], 1);
    assert_eq!(run_summary["continuity_audit"]["status"], "ready");
    let snapshot = read_text(&output_dir.join("snapshot.md"));
    assert!(snapshot.contains("- sqlite_exists: true"));
    assert!(snapshot.contains("- continuity_status: ready"));
    assert!(snapshot.contains("- continuity_residual_blockers: 0"));
}

#[test]
fn install_skills_alias_projects_codex_and_claude_code_root_entrypoints() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    seed_framework_markers(&repo_root);
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
    let first = host_integration_json(&[
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "--to",
        "all",
    ]);
    let second = host_integration_json(&[
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "status",
    ]);
    assert_eq!(first["success"], true);
    assert_eq!(first["results"]["codex"]["status"], "installed");
    assert_eq!(first["results"]["claude-code"]["status"], "installed");
    assert_eq!(
        first["results"]["codex"]["prompt_entrypoints"]["changed"],
        false
    );
    assert_eq!(first["invocation"]["deprecated_alias"], true);
    assert!(first["results"].get("agents").is_none());
    let codex_entrypoint = repo_root.join(".codex/prompts/framework.md");
    let claude_entrypoint = repo_root.join(".claude/commands/framework.md");
    assert!(codex_entrypoint.is_file());
    assert!(claude_entrypoint.is_file());
    assert!(!repo_root.join(".codex/prompts/autopilot.md").exists());
    assert!(!repo_root.join(".codex/prompts/gitx.md").exists());
    assert!(!home.join(".codex/skills").exists());
    assert!(!home.join(".claude/skills").exists());
    assert!(read_text(&codex_entrypoint).contains("host_projection: codex-cli"));
    assert!(read_text(&claude_entrypoint).contains("host_projection: claude-code-cli"));
    assert_eq!(
        second["results"]["codex"]["prompts"]["framework"]["project"]["managed"],
        true
    );
    assert_eq!(
        second["results"]["claude-code"]["commands"]["framework"]["project"]["managed"],
        true
    );
    assert_eq!(
        second["results"]["claude-code"]["settings"]["managed"],
        false
    );
}

#[test]
fn install_skills_codex_target_does_not_install_claude_code() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(repo_root.join("skills/gitx")).unwrap();
    seed_framework_markers(&repo_root);
    write_text(
        &repo_root.join("skills/gitx/SKILL.md"),
        "---\nname: gitx\n---\n",
    );

    let result = host_integration_json(&[
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "codex",
    ]);

    assert_eq!(result["success"], true);
    assert_eq!(result["results"]["codex"]["status"], "installed");
    assert!(result["results"].get("claude-code").is_none());
    assert!(repo_root.join(".codex/prompts/framework.md").exists());
    assert!(!repo_root.join(".codex/prompts/autopilot.md").exists());
    assert!(!repo_root.join(".codex/prompts/gitx.md").exists());
    assert!(!repo_root.join(".claude/commands/framework.md").exists());
    assert!(!home.join(".codex/skills").exists());
    assert!(!home.join(".claude/skills").exists());
}

#[test]
fn framework_host_integration_installs_project_local_claude_root_entrypoint() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&project_root).unwrap();

    let first = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);
    let second = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    let command_path = project_root.join(".claude/commands/framework.md");
    let content = read_text(&command_path);
    assert_eq!(first["success"], true);
    assert_eq!(
        first["invocation"]["primary_command"],
        "framework host-integration"
    );
    assert_eq!(first["results"]["claude-code"]["status"], "installed");
    assert_eq!(
        first["results"]["claude-code"]["settings"]["managed"],
        false
    );
    assert!(content.contains("managed_by: skill-framework"));
    assert!(content.contains("host_projection: claude-code-cli"));
    assert!(!content.contains("allowed-tools"));
    assert!(!project_root.join(".codex").exists());
    assert!(artifact_root
        .join("claude-code-surface/entrypoints/framework.md")
        .is_file());
    assert_eq!(
        second["results"]["claude-code"]["commands"]["framework"]["project"]["managed"],
        true
    );
}

#[test]
fn framework_host_integration_remove_skips_user_owned_files() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let user_owned = project_root.join(".claude/commands/framework.md");
    write_text(&user_owned, "# user command\n");

    let result = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);

    assert_eq!(result["results"]["claude-code"]["changed"], false);
    assert_eq!(read_text(&user_owned), "# user command\n");
}

#[test]
fn framework_host_integration_remove_preserves_marker_copy_without_manifest_ownership() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let installed_project = tmp.path().join("installed");
    let copied_project = tmp.path().join("copied");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&installed_project).unwrap();
    std::fs::create_dir_all(copied_project.join(".claude/commands")).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        installed_project.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);
    let copied_command = copied_project.join(".claude/commands/framework.md");
    let copied_content = read_text(&installed_project.join(".claude/commands/framework.md"));
    write_text(&copied_command, &copied_content);

    let result = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        copied_project.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);

    assert_eq!(result["results"]["claude-code"]["changed"], false);
    assert!(copied_command.is_file());
    assert_eq!(read_text(&copied_command), copied_content);
    assert_eq!(
        result["results"]["claude-code"]["skipped_user_owned_paths"],
        json!([copied_command.to_string_lossy()])
    );
}

#[test]
fn framework_host_integration_remove_preserves_files_not_recorded_in_manifest() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&project_root).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "codex",
        "--scope",
        "project",
    ]);
    let command_path = project_root.join(".codex/prompts/framework.md");
    let manifest_path = project_root.join(".codex/.framework-projection.json");
    let original_content = read_text(&command_path);
    write_json(
        &manifest_path,
        &json!({
            "schema_version": "framework-host-projection-v1",
            "managed_by": "skill-framework",
            "host_projection": "codex-cli",
            "scope": "project",
            "files": [project_root.join(".codex/prompts/other.md").to_string_lossy()]
        }),
    );

    let result = router_rs_json(&[
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "codex",
        "--scope",
        "project",
    ]);

    assert_eq!(result["results"]["codex"]["status"], "removed");
    assert!(command_path.is_file());
    assert!(!manifest_path.exists());
    assert_eq!(read_text(&command_path), original_content);
    assert_eq!(
        result["results"]["codex"]["skipped_user_owned_paths"],
        json!([command_path.to_string_lossy()])
    );
}

#[test]
fn claude_settings_opt_in_merge_and_cleanup_are_manifest_owned() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let settings_path = project_root.join(".claude/settings.json");
    write_json(
        &settings_path,
        &json!({
            "theme": "keep-me",
            "disableAllHooks": true,
            "env": {"USER_KEY": "keep"}
        }),
    );

    let install = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
        "--enable-claude-settings",
        "--enable-claude-hooks",
        "--enable-claude-statusline",
    ]);
    assert_eq!(
        install["results"]["claude-code"]["settings"]["managed"],
        true
    );
    assert_eq!(install["results"]["claude-code"]["hooks"]["managed"], true);
    assert_eq!(
        install["results"]["claude-code"]["statusLine"]["managed"],
        true
    );

    let settings = read_json(&settings_path);
    assert_eq!(settings["theme"], "keep-me");
    assert_eq!(settings["env"]["USER_KEY"], "keep");
    assert_eq!(
        settings["env"]["SKILL_FRAMEWORK_ROOT"],
        framework_root.to_str().unwrap()
    );
    assert!(
        settings["hooks"]["PreToolUse"].as_array().unwrap()[0]["framework_owned"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(settings["statusLine"]["type"], "command");
    let statusline_command = settings["statusLine"]["command"].as_str().unwrap();
    assert!(statusline_command.contains(&format!(
        "'{}'",
        framework_root
            .join("scripts/router-rs/run_router_rs.sh")
            .to_string_lossy()
    )));
    assert!(statusline_command.contains(&format!(
        "'{}'",
        framework_root
            .join("scripts/router-rs/Cargo.toml")
            .to_string_lossy()
    )));
    assert!(statusline_command.contains(&format!(
        "--repo-root '{}'",
        framework_root.to_string_lossy()
    )));
    let hook_command = settings["hooks"]["PreToolUse"].as_array().unwrap()[0]["hooks"]
        .as_array()
        .unwrap()[0]["command"]
        .as_str()
        .unwrap();
    assert!(hook_command.contains("framework host-integration claude-pre-tool-check"));
    assert!(!hook_command.contains("framework host-integration status"));
    assert!(hook_command.contains(&format!(
        "--framework-root '{}'",
        framework_root.to_string_lossy()
    )));
    assert!(hook_command.contains("--project-root \"$CLAUDE_PROJECT_DIR\""));
    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    assert_eq!(
        status["results"]["claude-code"]["settings"]["hooks_disabled_by_disableAllHooks"],
        true
    );
    assert_eq!(
        status["results"]["claude-code"]["settings"]["statusLine_effective"],
        "not-derived-from-disableAllHooks-without-native-verification"
    );
    assert_eq!(
        status["results"]["claude-code"]["hooks"]["manifest_managed"],
        true
    );
    assert_eq!(status["results"]["claude-code"]["hooks"]["managed"], false);
    assert_eq!(
        status["results"]["claude-code"]["hooks"]["verification"],
        "disabled"
    );
    assert_eq!(
        status["results"]["claude-code"]["hooks"]["reason"],
        "disabled-by-disableAllHooks"
    );
    assert_eq!(
        status["results"]["claude-code"]["hooks"]["schema"]["verified"],
        true
    );
    assert_eq!(
        status["results"]["claude-code"]["statusLine"]["managed"],
        true
    );
    assert_eq!(
        status["results"]["claude-code"]["statusLine"]["schema"]["verified"],
        true
    );
    let hook_check = router_rs_json(&[
        "framework",
        "host-integration",
        "claude-pre-tool-check",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    assert_eq!(hook_check["command"], "claude-pre-tool-check");
    assert_eq!(hook_check["status"], "not-verified");
    assert_eq!(hook_check["hook"]["manifest_managed"], true);
    assert_eq!(hook_check["hook"]["managed"], false);
    assert_eq!(hook_check["hook"]["verification"], "disabled");
    assert_eq!(hook_check["hook"]["reason"], "disabled-by-disableAllHooks");
    assert_eq!(hook_check["hook"]["schema"]["verified"], true);
    assert!(project_root
        .join(".claude/.framework-projection.json")
        .is_file());

    let remove = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);
    assert_eq!(remove["results"]["claude-code"]["changed"], true);
    let cleaned = read_json(&settings_path);
    assert_eq!(cleaned["theme"], "keep-me");
    assert_eq!(cleaned["disableAllHooks"], true);
    assert_eq!(cleaned["env"]["USER_KEY"], "keep");
    assert!(cleaned["env"].get("SKILL_FRAMEWORK_ROOT").is_none());
    assert!(cleaned.get("hooks").is_none());
    assert!(cleaned.get("statusLine").is_none());
}

#[test]
fn claude_settings_cleanup_ignores_unmanaged_manifest() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let settings_path = project_root.join(".claude/settings.json");
    write_json(
        &settings_path,
        &json!({
            "env": {"SKILL_FRAMEWORK_ROOT": "must-stay", "USER_KEY": "keep"},
            "hooks": {"PreToolUse": [{"framework_owned": true}]},
            "statusLine": {"type": "command", "command": "framework statusline"}
        }),
    );
    write_json(
        &project_root.join(".claude/.framework-projection.json"),
        &json!({
            "schema_version": "framework-host-projection-v1",
            "managed_by": "someone-else",
            "host_projection": "claude-code-cli",
            "scope": "project",
            "settings": {
                "path": settings_path.to_string_lossy(),
                "managed_key_paths": ["env.SKILL_FRAMEWORK_ROOT", "hooks.PreToolUse", "statusLine"]
            }
        }),
    );

    let remove = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
        "--scope",
        "project",
    ]);

    assert_eq!(
        remove["results"]["claude-code"]["settings_keys_removed"],
        false
    );
    let settings = read_json(&settings_path);
    assert_eq!(settings["env"]["SKILL_FRAMEWORK_ROOT"], "must-stay");
    assert_eq!(settings["env"]["USER_KEY"], "keep");
    assert!(settings["hooks"]["PreToolUse"].is_array());
    assert_eq!(settings["statusLine"]["type"], "command");
}

#[test]
fn compatibility_alias_inventory_and_generated_artifacts_status_are_reported() {
    let framework_root = project_root();
    let aliases = router_rs_json(&["framework", "host-integration", "compatibility-aliases"]);
    assert_eq!(
        aliases["schema_version"],
        "framework-compatibility-alias-inventory-v1"
    );
    let alias_entries = aliases["aliases"].as_array().unwrap();
    let expected_aliases = [
        "codex host-integration ...",
        "framework host-integration install-skills",
        "--repo-root",
    ];
    for expected_alias in expected_aliases {
        let alias = alias_entries
            .iter()
            .find(|alias| alias["alias"] == expected_alias)
            .unwrap_or_else(|| {
                panic!("missing compatibility alias inventory entry: {expected_alias}")
            });
        for field in [
            "owner",
            "reason",
            "primary_command",
            "kept_policy",
            "removal_condition",
        ] {
            assert!(
                alias[field].as_str().is_some_and(|value| !value.is_empty()),
                "alias {expected_alias} missing non-empty {field}"
            );
        }
        assert_eq!(alias["independent_behavior"], false);
    }
    let repo_root_alias = alias_entries
        .iter()
        .find(|alias| alias["alias"] == "--repo-root")
        .unwrap();
    assert!(repo_root_alias["kept_policy"]
        .as_str()
        .unwrap()
        .contains("never resolves or fills project_root"));

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]);
    assert_eq!(
        status["schema_version"],
        "framework-generated-artifacts-status-v1"
    );
    assert_eq!(
        status["manifest_status"]["mode"],
        "manifest-backed-byte-for-byte-drift-gate"
    );
    assert_eq!(status["drift_gate"]["enabled"], true);
    assert_eq!(status["drift_gate"]["compare"], "byte-for-byte");
    assert_eq!(
        status["manifest_status"]["missing_required_generated_artifacts"],
        json!([])
    );
    for required in [
        "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
        "skills/SKILL_ROUTING_REGISTRY.md",
        "skills/SKILL_ROUTING_INDEX.md",
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "skills/SKILL_SHADOW_MAP.json",
        "skills/SKILL_APPROVAL_POLICY.json",
        "skills/SKILL_LOADOUTS.json",
        "skills/SKILL_TIERS.json",
        "AGENTS.md",
        "CLAUDE.md",
        ".codex/host_entrypoints_sync_manifest.json",
    ] {
        assert!(
            status["generated_artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|artifact| artifact["path"] == required
                    && artifact["drifted"].is_boolean()
                    && artifact["regenerated_exists"].is_boolean()),
            "missing generated artifact status for {required}"
        );
    }
}

#[test]
fn generated_artifacts_status_reports_missing_required_manifest_entries() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "sh scripts/generate-surface.sh",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh"}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
printf '%s\n' '{"status":"fresh"}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    assert_eq!(status["ok"], false);
    let missing = status["manifest_status"]["missing_required_generated_artifacts"]
        .as_array()
        .unwrap();
    assert!(missing.contains(&json!("skills/SKILL_ROUTING_RUNTIME.json")));
    assert!(missing.contains(&json!("skills/SKILL_TIERS.json")));
    assert!(
        !artifact_root
            .join("generated-artifacts-drift-check")
            .exists(),
        "generated-artifacts-status should clean temporary drift-check copies"
    );
}

#[test]
fn generated_artifacts_status_rejects_missing_or_unsupported_manifest_schema() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    seed_framework_markers(&framework_root);

    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "generated_artifacts": []
        }),
    );
    let missing_schema = run(router_rs_command([
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]));
    assert!(!missing_schema.status.success());
    let (_, stderr) = output_text(&missing_schema);
    assert!(
        stderr.contains("invalid generated artifact manifest"),
        "unexpected stderr for missing schema: {stderr}"
    );

    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v0",
            "generated_artifacts": []
        }),
    );
    let unsupported_schema = run(router_rs_command([
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]));
    assert!(!unsupported_schema.status.success());
    let (_, stderr) = output_text(&unsupported_schema);
    assert!(
        stderr.contains("unsupported generated artifact manifest schema_version"),
        "unexpected stderr for unsupported schema: {stderr}"
    );
}

#[test]
fn generated_artifacts_status_reports_undeclared_markers_across_reverse_reference_surfaces() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "sh scripts/generate-surface.sh",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"fresh","marker":"generated-by-test"}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
printf '%s\n' '{"status":"fresh","marker":"generated-by-test"}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );
    write_text(
        &framework_root.join("skills/SKILL_EXTRA.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(
        &framework_root.join("docs/generated.md"),
        "generated-by-test\n",
    );
    write_text(
        &framework_root.join(".codex/generated.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(&framework_root.join("AGENTS.md"), "generated-by-test\n");
    write_text(
        &framework_root.join("tests/source.rs"),
        r#"let fixture = "generated-by-test";"#,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    assert_eq!(status["ok"], false);
    let undeclared = status["manifest_status"]["undeclared_generated_artifacts"]
        .as_array()
        .unwrap();
    for expected in [
        ".codex/generated.json",
        "AGENTS.md",
        "docs/generated.md",
        "skills/SKILL_EXTRA.json",
    ] {
        assert!(
            undeclared.contains(&json!(expected)),
            "missing undeclared generated artifact marker: {expected}; got {undeclared:?}"
        );
    }
    assert!(!undeclared.contains(&json!("tests/source.rs")));
}

#[test]
fn generated_artifacts_status_reports_manifest_backed_drift() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "sh scripts/generate-surface.sh",
                "compare": "byte-for-byte"
            }]
        }),
    );
    write_text(
        &framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json"),
        r#"{"status":"stale","marker":"generated-by-test","bad":"/Users/joe/.claude /Users/joe/.codex /Users/joe/Documents/skill"}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
printf '%s\n' '{"status":"fresh","marker":"generated-by-test"}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );
    write_text(
        &artifact_root.join("undeclared/root/IGNORED.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);

    assert_eq!(status["ok"], false);
    assert_eq!(
        status["manifest_status"]["mode"],
        "manifest-backed-byte-for-byte-drift-gate"
    );
    assert_eq!(
        status["manifest_status"]["drifted_artifacts"],
        json!([{
            "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
            "generator": "sh scripts/generate-surface.sh",
            "compare": "byte-for-byte"
        }])
    );
    assert_eq!(
        status["generated_artifacts"][0]["forbidden_markers"],
        json!([
            "expanded-claude-home",
            "expanded-codex-home",
            "expanded-consuming-project-root"
        ])
    );
    assert_eq!(
        status["manifest_status"]["undeclared_generated_artifacts"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn external_consuming_repo_matrix_keeps_framework_project_artifact_and_homes_separate() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_a = tmp.path().join("project-a");
    let project_b = tmp.path().join("project-b");
    let artifact_root = tmp.path().join("artifact-root");
    let home = tmp.path().join("home");
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    std::fs::create_dir_all(&outside).unwrap();

    let claude = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_a.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--claude-home",
        home.join(".claude").to_str().unwrap(),
        "--codex-home",
        home.join(".codex").to_str().unwrap(),
        "--to",
        "claude-code",
    ]);
    let codex = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_b.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--claude-home",
        home.join(".claude").to_str().unwrap(),
        "--codex-home",
        home.join(".codex").to_str().unwrap(),
        "--to",
        "codex",
    ]);
    let mut status_cmd = router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_a.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--claude-home",
        home.join(".claude").to_str().unwrap(),
        "--codex-home",
        home.join(".codex").to_str().unwrap(),
    ]);
    status_cmd.current_dir(&outside);
    let status = json_from_output(&run(status_cmd));

    assert_eq!(claude["results"]["claude-code"]["status"], "installed");
    assert_eq!(codex["results"]["codex"]["status"], "installed");
    assert!(project_a.join(".claude/commands/framework.md").is_file());
    assert!(!project_a.join(".codex").exists());
    assert!(project_b.join(".codex/prompts/framework.md").is_file());
    assert!(!project_b.join(".claude").exists());
    assert!(artifact_root
        .join("claude-code-surface/entrypoints/framework.md")
        .is_file());
    assert_eq!(
        status["resolved_roots"]["framework_root"],
        framework_root.to_str().unwrap()
    );
    assert_eq!(
        status["resolved_roots"]["project_root"],
        project_a.to_str().unwrap()
    );
    assert!(read_text(&project_a.join(".claude/commands/framework.md"))
        .contains(framework_root.to_str().unwrap()));
}

#[test]
fn install_skills_repo_root_alias_does_not_fill_project_root() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut command = router_rs_command([
        "codex",
        "host-integration",
        "install-skills",
        "--repo-root",
        framework_root.to_str().unwrap(),
        "status",
    ]);
    command.env("SKILL_PROJECT_ROOT", &project_root);
    let output = json_from_output(&run(command));

    assert_eq!(
        output["resolved_roots"]["framework_root"],
        framework_root.to_str().unwrap()
    );
    assert_eq!(
        output["resolved_roots"]["project_root"],
        project_root.to_str().unwrap()
    );
}

#[test]
fn projection_root_resolution_fails_closed_for_missing_framework_root() {
    let tmp = tempdir().unwrap();
    let bad_framework = tmp.path().join("missing-framework");
    let project = tmp.path().join("consumer");
    std::fs::create_dir_all(&project).unwrap();
    let output = run(router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        bad_framework.to_str().unwrap(),
        "--project-root",
        project.to_str().unwrap(),
    ]));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("stale or missing framework_root"));
    assert!(stderr.contains("Repair by passing --framework-root"));
}

#[test]
fn projection_root_resolution_honors_env_fallbacks_and_cli_home_overrides() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let env_claude_home = tmp.path().join("env/.claude");
    let env_codex_home = tmp.path().join("env/.codex");
    let flag_claude_home = tmp.path().join("flag/.claude");
    let flag_codex_home = tmp.path().join("flag/.codex");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut env_status = router_rs_command(["framework", "host-integration", "status"]);
    env_status
        .env("SKILL_FRAMEWORK_ROOT", &framework_root)
        .env("SKILL_PROJECT_ROOT", &project_root)
        .env("SKILL_ARTIFACT_ROOT", &artifact_root)
        .env("CLAUDE_HOME", &env_claude_home)
        .env("CODEX_HOME", &env_codex_home);
    let env_payload = json_from_output(&run(env_status));
    assert_eq!(
        env_payload["resolved_roots"]["framework_root"],
        framework_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["project_root"],
        project_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["artifact_root"],
        artifact_root.to_str().unwrap()
    );
    assert_eq!(
        env_payload["resolved_roots"]["host_home_roots"]["claude-code-cli"],
        env_claude_home.to_str().unwrap()
    );

    let mut flag_status = router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--claude-home",
        flag_claude_home.to_str().unwrap(),
        "--codex-home",
        flag_codex_home.to_str().unwrap(),
    ]);
    flag_status
        .env("CLAUDE_HOME", &env_claude_home)
        .env("CODEX_HOME", &env_codex_home);
    let flag_payload = json_from_output(&run(flag_status));
    assert_eq!(
        flag_payload["resolved_roots"]["host_home_roots"]["claude-code-cli"],
        flag_claude_home.to_str().unwrap()
    );
    assert_eq!(
        flag_payload["resolved_roots"]["host_home_roots"]["codex-cli"],
        flag_codex_home.to_str().unwrap()
    );
}

#[test]
fn project_discovery_ignores_host_private_projection_directories() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let host_private_only = tmp.path().join("host-private-only");
    std::fs::create_dir_all(host_private_only.join(".claude/commands")).unwrap();
    std::fs::create_dir_all(host_private_only.join(".codex/prompts")).unwrap();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        framework_root
            .join("scripts/router-rs/Cargo.toml")
            .to_str()
            .unwrap(),
        "--",
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]);
    command.current_dir(&host_private_only);
    let output = run(command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing project_root"));
    assert!(stderr.contains("pass --project-root or set SKILL_PROJECT_ROOT"));
}

#[test]
fn project_discovery_rejects_ambiguous_framework_like_candidate() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let other_framework = tmp.path().join("other-framework");
    seed_framework_markers(&other_framework);
    std::fs::create_dir_all(other_framework.join(".git")).unwrap();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        framework_root
            .join("scripts/router-rs/Cargo.toml")
            .to_str()
            .unwrap(),
        "--",
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
    ]);
    command.current_dir(&other_framework);
    let output = run(command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ambiguous project_root discovery"));
    assert!(stderr.contains("Pass both --framework-root and --project-root explicitly"));
}

#[test]
fn claude_command_status_reports_frontmatter_allowlist_and_stale_project_metadata() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_a = tmp.path().join("project-a");
    let project_b = tmp.path().join("project-b");
    let artifact_root = tmp.path().join("artifact-root");
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b.join(".claude/commands")).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_a.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
    ]);
    let copied = read_text(&project_a.join(".claude/commands/framework.md"))
        .replace("argument-hint:", "allowed-tools: Bash\nargument-hint:");
    write_text(&project_b.join(".claude/commands/framework.md"), &copied);

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_b.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    let command_status = &status["results"]["claude-code"]["commands"]["framework"]["project"];
    assert_eq!(command_status["managed"], false);
    assert_eq!(command_status["verification"], "unknown");
    assert_eq!(command_status["marker_managed"], true);
    assert_eq!(command_status["stale_project_metadata"], true);
    assert_eq!(command_status["frontmatter"]["allowed"], false);
    assert!(command_status["frontmatter"]["unknown_keys"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("allowed-tools")));
    assert_eq!(command_status["command_file"]["valid"], true);
    assert_eq!(command_status["copied_skill_bodies"]["detected"], true);
}

#[test]
fn claude_command_status_verifies_generated_command_shape_without_skill_body_copy() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&project_root).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
    ]);

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    let command_status = &status["results"]["claude-code"]["commands"]["framework"]["project"];
    assert_eq!(command_status["managed"], true);
    assert_eq!(command_status["verification"], "verified");
    assert_eq!(command_status["frontmatter"]["allowed"], true);
    assert_eq!(
        command_status["frontmatter"]["allowed_keys"],
        json!(["argument-hint", "description", "name"])
    );
    assert_eq!(command_status["command_file"]["valid"], true);
    assert_eq!(command_status["command_file"]["has_arguments"], true);
    assert_eq!(command_status["copied_skill_bodies"]["detected"], false);
}

#[test]
fn claude_command_status_marks_marker_only_unverified_shape_as_unknown() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let command_path = project_root.join(".claude/commands/framework.md");
    write_text(
        &command_path,
        "<!-- managed_by: skill-framework -->\n<!-- framework_schema_version: framework-host-projection-v1 -->\n",
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    let command_status = &status["results"]["claude-code"]["commands"]["framework"]["project"];
    assert_eq!(command_status["managed"], false);
    assert_eq!(command_status["verification"], "unknown");
    assert_eq!(command_status["marker_managed"], true);
    assert_eq!(command_status["frontmatter"]["present"], false);
    assert_eq!(command_status["command_file"]["valid"], false);
}

#[test]
fn claude_settings_status_marks_manifest_only_invalid_schema_as_unknown() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let settings_path = project_root.join(".claude/settings.json");
    write_json(
        &settings_path,
        &json!({
            "hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "echo unmanaged"}]}]},
            "statusLine": {"type": "command", "command": "echo unmanaged"}
        }),
    );
    write_json(
        &project_root.join(".claude/.framework-projection.json"),
        &json!({
            "schema_version": "framework-host-projection-v1",
            "managed_by": "skill-framework",
            "host_projection": "claude-code-cli",
            "scope": "project",
            "files": [project_root.join(".claude/commands/framework.md").to_string_lossy()],
            "settings": {
                "path": settings_path.to_string_lossy(),
                "managed_key_paths": ["hooks.PreToolUse", "statusLine"]
            }
        }),
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    let claude = &status["results"]["claude-code"];
    assert_eq!(claude["hooks"]["managed"], false);
    assert_eq!(claude["hooks"]["manifest_managed"], true);
    assert_eq!(claude["hooks"]["verification"], "unknown");
    assert_eq!(claude["statusLine"]["managed"], false);
    assert_eq!(claude["statusLine"]["manifest_managed"], true);
    assert_eq!(claude["statusLine"]["verification"], "unknown");
}

#[test]
fn cleanup_supports_dry_run_idempotency_and_manifest_owned_removal() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let claude_home = tmp.path().join("home/.claude");
    std::fs::create_dir_all(&project_root).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "all",
    ]);

    let dry_run = router_rs_json(&[
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "all",
        "--dry-run",
    ]);
    assert_eq!(dry_run["command"], "cleanup");
    assert_eq!(dry_run["scope"], "project");
    assert_eq!(dry_run["results"]["codex"]["status"], "would-remove");
    assert!(project_root.join(".codex/prompts/framework.md").is_file());
    assert!(project_root.join(".claude/commands/framework.md").is_file());

    let missing_home = run(router_rs_command([
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--scope",
        "user",
        "--to",
        "claude-code",
    ]));
    assert!(!missing_home.status.success());
    assert!(String::from_utf8_lossy(&missing_home.stderr)
        .contains("user-scope cleanup for claude-code requires explicit host-home resolution"));

    let cleanup = router_rs_json(&[
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "all",
    ]);
    assert_eq!(cleanup["results"]["codex"]["status"], "removed");
    assert_eq!(cleanup["results"]["claude-code"]["status"], "removed");
    assert!(!project_root.join(".codex/prompts/framework.md").exists());
    assert!(!project_root.join(".claude/commands/framework.md").exists());
    assert!(!project_root
        .join(".codex/.framework-projection.json")
        .exists());
    assert!(!project_root
        .join(".claude/.framework-projection.json")
        .exists());

    let idempotent = router_rs_json(&[
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "all",
    ]);
    assert_eq!(
        idempotent["results"]["codex"]["status"],
        "not-installed-or-user-owned"
    );
    assert_eq!(
        idempotent["results"]["claude-code"]["status"],
        "not-installed-or-user-owned"
    );

    let explicit_user_cleanup = router_rs_json(&[
        "framework",
        "host-integration",
        "cleanup",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--scope",
        "user",
        "--claude-home",
        claude_home.to_str().unwrap(),
        "--to",
        "claude-code",
    ]);
    assert_eq!(explicit_user_cleanup["scope"], "user");
}

#[test]
fn remove_one_host_projection_preserves_the_other_host() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&project_root).unwrap();
    router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "all",
    ]);

    let removed_codex = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "codex",
    ]);
    assert_eq!(removed_codex["results"]["codex"]["status"], "removed");
    assert!(!project_root.join(".codex/prompts/framework.md").exists());
    assert!(project_root.join(".claude/commands/framework.md").is_file());

    let removed_claude = router_rs_json(&[
        "framework",
        "host-integration",
        "remove",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--to",
        "claude-code",
    ]);
    assert_eq!(
        removed_claude["results"]["claude-code"]["status"],
        "removed"
    );
    assert!(!project_root.join(".claude/commands/framework.md").exists());
}

#[test]
fn compatibility_alias_outputs_are_normalized_equivalent() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    std::fs::create_dir_all(&project_root).unwrap();

    let framework_status = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
    ]);
    let codex_status = router_rs_json(&[
        "codex",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
    ]);
    assert_eq!(
        normalize_alias_equivalence(framework_status),
        normalize_alias_equivalence(codex_status)
    );

    let framework_status_with_repo_root = router_rs_json(&[
        "framework",
        "host-integration",
        "status",
        "--repo-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
    ]);
    assert_eq!(
        normalize_alias_equivalence(framework_status_with_repo_root),
        normalize_alias_equivalence(router_rs_json(&[
            "framework",
            "host-integration",
            "status",
            "--framework-root",
            framework_root.to_str().unwrap(),
            "--project-root",
            project_root.to_str().unwrap(),
        ]))
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

fn seed_framework_markers(root: &Path) {
    write_text(
        &root.join("configs/framework/RUNTIME_REGISTRY.json"),
        r#"{"schema_version":"framework-runtime-registry-v1","framework_core":{"authority":"rust","source":"framework-root-native","host_policy":"closed-set-explicit-projections"},"host_projections":{"codex-cli":{"profile_id":"codex_profile"},"claude-code-cli":{"profile_id":"claude_code_profile"}}}"#,
    );
    write_text(
        &root.join("scripts/router-rs/Cargo.toml"),
        "[package]\nname = \"router-rs-marker\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    );
}

fn normalize_alias_equivalence(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove("invocation");
    }
    payload
}

fn assert_framework_alias_skill(surface_root: &Path, slug: &str) {
    let content = read_text(&surface_root.join(slug).join("SKILL.md"));
    let dollar_entrypoint = format!("${slug}");
    assert!(content.contains(&format!("name: {slug}")));
    assert!(content.contains("generated lightweight Codex CLI/Claude Code alias"));
    assert!(content.contains(&format!("`{dollar_entrypoint}`")));
    assert!(content.contains(&format!("`/{slug}`")));
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
    assert_eq!(payload["framework_core"]["authority"], "rust");
    assert_eq!(
        payload["host_projections"]["codex-cli"]["profile_id"],
        "codex_profile"
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
            "framework_core": {
                "authority": "rust",
                "source": "framework-root-native",
                "host_policy": "closed-set-explicit-projections"
            },
            "host_projections": {"codex-cli": {"profile_id": "repo-codex"}},
            "workspace_bootstrap_defaults": {"skills": {"source_rel": "repo-skills"}},
            "framework_commands": {"autopilot": {"canonical_owner": "repo-owner"}}
        }))
        .unwrap(),
    );
    let payload = runtime_registry(&repo_root);
    assert_eq!(
        payload["host_projections"]["codex-cli"]["profile_id"],
        "repo-codex"
    );
    assert_eq!(
        payload["framework_commands"]["autopilot"]["canonical_owner"],
        "repo-owner"
    );
}

#[test]
fn runtime_registry_exposes_framework_commands_and_native_runtime_contract() {
    let payload = runtime_registry(&project_root());
    let aliases = &payload["framework_commands"];
    assert_eq!(aliases["autopilot"]["canonical_owner"], "plan-to-code");
    assert_eq!(
        aliases["autopilot"]["host_entrypoints"]["codex-cli"],
        "/autopilot"
    );
    assert_eq!(
        aliases["autopilot"]["interaction_invariants"]["implicit_route_policy"],
        "never"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["codex-cli"],
        "/deepinterview"
    );
    assert_eq!(
        aliases["autopilot"]["host_entrypoints"]["claude-code-cli"],
        "/autopilot"
    );
    assert_eq!(
        aliases["deepinterview"]["host_entrypoints"]["claude-code-cli"],
        "/deepinterview"
    );
    assert_eq!(aliases["team"]["host_entrypoints"]["codex-cli"], "/team");
    assert_eq!(
        aliases["team"]["host_entrypoints"]["claude-code-cli"],
        "/team"
    );
    assert_eq!(
        payload["host_targets"]["policy"],
        "shared-rust-core-explicit-host-projections"
    );
    assert_eq!(
        payload["host_targets"]["supported"],
        json!(["codex-cli", "claude-code-cli"])
    );
    assert!(payload.get("mcp_clients").is_none());
    assert_eq!(aliases["team"]["route_mode"], "team-orchestration");
    let autopilot = &aliases["autopilot"];
    assert_eq!(autopilot["lineage"]["source"], "repo-native");
    assert!(autopilot["implementation_bar"]
        .as_array()
        .unwrap()
        .contains(&json!("resume-and-recovery-required")));
}

#[test]
fn runtime_registry_host_projections_expose_supervisor_capabilities() {
    let payload = runtime_registry(&project_root());
    let codex = &payload["host_projections"]["codex-cli"];
    assert_eq!(codex["profile_id"], "codex_profile");
    let codex_capabilities = codex["capabilities"].as_array().unwrap();
    for capability in [
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
    ] {
        assert!(codex_capabilities.contains(&json!(capability)));
    }
    assert_eq!(codex["session_supervisor_driver"], "codex_driver");

    let claude = &payload["host_projections"]["claude-code-cli"];
    assert_eq!(claude["profile_id"], "claude_code_profile");
    let claude_capabilities = claude["capabilities"].as_array().unwrap();
    for capability in [
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
        "slash_commands",
    ] {
        assert!(claude_capabilities.contains(&json!(capability)));
    }
    assert_eq!(claude["session_supervisor_driver"], "claude_code_driver");
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
