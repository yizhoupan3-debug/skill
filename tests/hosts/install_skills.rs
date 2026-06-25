use crate::common;
use crate::common::{
    host_integration_json, json_from_output, project_root, read_json, read_text,
    router_rs_command, router_rs_json, run, seed_framework_markers, write_json, write_text,
};
use serde_json::{Value, json};
use std::process::Command;
use tempfile::tempdir;

#[test]
fn install_skills_rejects_retired_codex_app_host_id() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(repo_root.join("skills/gitx")).unwrap();
    seed_framework_markers(&repo_root);
    write_text(
        &repo_root.join("skills/gitx/SKILL.md"),
        "---\nname: gitx\n---\n",
    );

    let output = run(router_rs_command([
        "framework",
        "host-integration",
        "install-skills",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--bootstrap-output-dir",
        tmp.path().join("bootstrap").to_str().unwrap(),
        "claude-desktop",
    ]));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("claude-desktop") || stderr.contains("supported"),
        "retired claude-desktop install must fail closed: {stderr}"
    );
    assert!(!repo_root.join(".codex/prompts/framework.md").exists());
}

#[test]
fn install_skills_cursor_target_installs_only_cursor() {
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
        "cursor",
    ]);

    assert_eq!(result["success"], true);
    assert_eq!(result["results"]["cursor"]["status"], "installed");
    assert!(home.join(".cursor/rules/framework.mdc").exists());
    let framework_rule = read_text(&home.join(".cursor/rules/framework.mdc"));
    assert!(framework_rule.contains("跨宿主内核"));
    assert!(framework_rule.contains("AGENTS.md"));
    assert!(!repo_root.join(".cursor/rules/framework.mdc").exists());
    assert!(!repo_root.join(".codex/prompts/framework.md").exists());
}

#[test]
fn install_skills_claude_target_installs_only_claude() {
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
        "claude",
    ]);

    assert_eq!(result["success"], true);
    assert_eq!(result["results"]["claude"]["status"], "installed");
    assert!(repo_root.join(".claude/rules/framework.md").exists());
    let framework_rule = read_text(&repo_root.join(".claude/rules/framework.md"));
    assert!(framework_rule.contains("跨宿主内核"));
    assert!(framework_rule.contains("AGENTS.md"));
    let settings_path = repo_root.join(".claude/settings.json");
    assert!(settings_path.exists());
    let settings = read_json(&settings_path);
    for event in ["PreToolUse", "UserPromptSubmit", "PostToolUse", "Stop"] {
        let entries = settings["hooks"][event].as_array().unwrap_or_else(|| {
            panic!("expected Claude settings hook entries for {event}: {settings:?}")
        });
        assert!(
            entries.iter().any(|entry| {
                entry.to_string().contains("claude-router-rs-hook.sh")
                    && entry.to_string().contains(event)
            }),
            "expected managed Claude launcher hook for {event}: {entries:?}"
        );
    }
    let pre_tool_command = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .expect("claude hook command");
    assert!(
        pre_tool_command.contains("claude-router-rs-hook.sh")
            && pre_tool_command.contains("PreToolUse"),
        "Claude hook command must invoke launcher script: {pre_tool_command}"
    );
    let fallback = Command::new("/bin/sh")
        .arg("-c")
        .arg(pre_tool_command)
        .env("CLAUDE_PROJECT_ROOT", &repo_root)
        .env("SKILL_FRAMEWORK_ROOT", project_root())
        .env("CARGO_TARGET_DIR", "/nonexistent")
        .env("ROUTER_RS_BIN", "/nonexistent/router-rs")
        .env("ROUTER_RS_HOOK_FAIL_OPEN", "0")
        .env("PATH", "/bin:/usr/bin")
        .output()
        .expect("run claude fallback command");
    assert!(
        !fallback.status.success(),
        "fallback should fail closed when router-rs is unavailable"
    );
    let fallback_json: Value = serde_json::from_slice(&fallback.stdout).unwrap_or_else(|err| {
        panic!(
            "fallback stdout must be valid JSON: {err}; stdout={}; stderr={}",
            String::from_utf8_lossy(&fallback.stdout),
            String::from_utf8_lossy(&fallback.stderr)
        )
    });
    assert_eq!(fallback_json["decision"], "block");
    assert_eq!(fallback_json["suppressOutput"], true);
    assert!(
        repo_root
            .join(".claude/.framework-projection.json")
            .exists()
    );
    let manifest = read_json(&repo_root.join(".claude/.framework-projection.json"));
    assert!(manifest["files"].as_array().unwrap().iter().any(|path| {
        path.as_str()
            .unwrap_or_default()
            .ends_with(".claude/settings.json")
    }));
    assert!(!repo_root.join(".cursor/rules/framework.mdc").exists());
    assert!(!repo_root.join(".codex/prompts/framework.md").exists());
}

#[test]
fn project_scope_all_does_not_install_claude_projection() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    seed_framework_markers(&repo_root);

    let result = host_integration_json(&[
        "install",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "all",
    ]);

    assert_eq!(result["success"], true);
    assert_eq!(result["results"]["codex"]["status"], "installed");
    assert_eq!(result["results"]["cursor"]["status"], "installed");
    // `claude` is excluded from project-scope batch install.
    assert!(result["results"].get("claude").is_none());
    assert!(repo_root.join(".codex/prompts/framework.md").exists());
    assert!(home.join(".cursor/rules/framework.mdc").exists());
    assert!(!repo_root.join(".cursor/rules/framework.mdc").exists());
    assert!(!repo_root.join(".claude/rules/framework.md").exists());
    assert!(
        !repo_root.join(".claude/mcp.json").exists(),
        "retired claude-desktop must not write .claude/mcp.json on batch install"
    );
}

#[test]
fn codex_project_install_does_not_write_project_mcp_surfaces() {
    // §1.1: MCP 配置统一到 user-level，install 不再写 project-level .mcp.json / .codex/config.toml
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    seed_framework_markers(&repo_root);

    let install = host_integration_json(&[
        "install",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "codex",
    ]);
    assert_eq!(install["success"], true);
    assert_eq!(install["results"]["codex"]["status"], "installed");

    // §1.1: project-level MCP configs no longer written by install
    assert!(
        !repo_root.join(".mcp.json").exists(),
        "§1.1: install must not write project-level .mcp.json"
    );
}

#[test]
fn remove_claude_projection_removes_managed_settings_hooks() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    seed_framework_markers(&repo_root);

    let install = host_integration_json(&[
        "install",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "claude",
    ]);
    assert_eq!(install["results"]["claude"]["status"], "installed");

    let settings_path = repo_root.join(".claude/settings.json");
    let mut settings = read_json(&settings_path);
    settings["theme"] = json!("dark");
    write_json(&settings_path, &settings);

    let removed = host_integration_json(&[
        "remove",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "claude",
    ]);

    assert_eq!(removed["results"]["claude"]["status"], "removed");
    assert_eq!(
        removed["results"]["claude"]["settings"]["removed_events"],
        json!([
            "SessionStart",
            "PreToolUse",
            "UserPromptSubmit",
            "PostToolUse",
            "Stop",
            "SubagentStart",
            "SubagentStop"
        ])
    );
    assert!(!repo_root.join(".claude/rules/framework.md").exists());
    assert!(
        !repo_root
            .join(".claude/.framework-projection.json")
            .exists()
    );
    let settings = read_json(&settings_path);
    assert_eq!(settings["theme"], "dark");
    assert!(settings.get("hooks").is_none());
}

#[test]
fn remove_claude_projection_deletes_settings_when_only_managed_hooks_remain() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    seed_framework_markers(&repo_root);

    let install = host_integration_json(&[
        "install",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "claude",
    ]);
    assert_eq!(install["results"]["claude"]["status"], "installed");

    let removed = host_integration_json(&[
        "remove",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "claude",
    ]);

    assert_eq!(
        removed["results"]["claude"]["settings"]["removed_file"],
        true
    );
    assert!(!repo_root.join(".claude/settings.json").exists());
}

#[test]
fn cursor_user_scope_projection_manages_browser_mcp_server() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();

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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install["success"], true);
    assert_eq!(install["results"]["cursor"]["status"], "installed");
    assert_eq!(install["results"]["cursor"]["mcp"]["managed"], true);
    assert_eq!(
        install["results"]["cursor"]["mcp"]["reason"],
        json!("installed")
    );

    let mcp_path = cursor_home.join("mcp.json");
    let mcp_payload = common::read_json(&mcp_path);
    let cmd = mcp_payload["mcp_servers"]["browser-mcp"]["command"]
        .as_str()
        .expect("browser-mcp command");
    assert!(
        cmd == "router-rs" || cmd.ends_with("/router-rs"),
        "unexpected command: {cmd}"
    );
    let args = mcp_payload["mcp_servers"]["browser-mcp"]["args"]
        .as_array()
        .expect("browser-mcp args");
    assert_eq!(args[0], json!("browser"));
    assert_eq!(args[1], json!("mcp-stdio"));
    assert_eq!(args[2], json!("--repo-root"));
    assert_eq!(args[3], json!(framework_root.to_string_lossy()));
    let manifest_path = cursor_home.join(".framework-projection.json");
    let manifest_payload = common::read_json(&manifest_path);
    assert!(
        manifest_payload["settings"]["managed_key_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("mcp_servers.browser-mcp"))
    );
    assert_eq!(
        mcp_payload["mcp_servers"]["paperplain"]["command"],
        json!("npx")
    );
    assert_eq!(
        mcp_payload["mcp_servers"]["paperplain"]["args"],
        json!(["-y", "paperplain-mcp"])
    );
    // §1.1: project-level .mcp.json no longer written by install
    assert!(
        !project_root.join(".mcp.json").exists(),
        "§1.1: install must not write project-level .mcp.json"
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(remove["success"], true);
    let removed_payload = common::read_json(&mcp_path);
    assert!(
        removed_payload
            .get("mcp_servers")
            .and_then(|servers| servers.get("browser-mcp"))
            .is_none()
    );
}

#[test]
fn cursor_user_scope_install_preserves_user_owned_browser_mcp_server() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();

    let mcp_path = cursor_home.join("mcp.json");
    write_json(
        &mcp_path,
        &json!({
            "mcp_servers": {
                "browser-mcp": {
                    "command": "custom-browser-mcp",
                    "args": ["--local"]
                }
            }
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install["success"], true);
    assert_eq!(install["results"]["cursor"]["status"], "installed");
    assert_eq!(install["results"]["cursor"]["mcp"]["changed"], false);
    assert_eq!(install["results"]["cursor"]["mcp"]["managed"], false);
    assert_eq!(
        install["results"]["cursor"]["mcp"]["reason"],
        json!("skipped_user_owned")
    );
    assert_eq!(
        install["results"]["cursor"]["mcp"]["skipped_user_owned"],
        json!(true)
    );

    let mcp_payload = common::read_json(&mcp_path);
    assert_eq!(
        mcp_payload["mcp_servers"]["browser-mcp"]["command"],
        json!("custom-browser-mcp")
    );
    let manifest_path = cursor_home.join(".framework-projection.json");
    let manifest_payload = common::read_json(&manifest_path);
    assert_eq!(manifest_payload["settings"]["managed_key_paths"], json!([]));
    assert!(
        !manifest_payload["files"]
            .as_array()
            .unwrap()
            .contains(&json!(mcp_path.to_string_lossy().to_string()))
    );
}

#[test]
fn cursor_user_scope_install_marks_equivalent_browser_mcp_server_managed() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();

    let mcp_path = cursor_home.join("mcp.json");
    write_json(
        &mcp_path,
        &json!({
            "mcp_servers": {
                "browser-mcp": common::browser_mcp_server_payload_like_host(&framework_root)
            }
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install["success"], true);
    assert_eq!(install["results"]["cursor"]["mcp"]["managed"], true);
    assert_eq!(
        install["results"]["cursor"]["mcp"]["skipped_user_owned"],
        json!(false)
    );
    assert!(
        install["results"]["cursor"]["mcp"]["reason"] == json!("already-managed-equivalent")
            || install["results"]["cursor"]["mcp"]["reason"] == json!("installed"),
        "unexpected mcp reason: {:?}",
        install["results"]["cursor"]["mcp"]["reason"]
    );
    assert_eq!(
        install["results"]["cursor"]["mcp"]["skipped_user_owned"],
        json!(false)
    );

    let manifest_path = cursor_home.join(".framework-projection.json");
    let manifest_payload = common::read_json(&manifest_path);
    assert!(
        manifest_payload["settings"]["managed_key_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("mcp_servers.browser-mcp"))
    );
}

#[test]
fn cursor_user_scope_equivalence_check_requires_matching_repo_root_arg() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();

    let mcp_path = cursor_home.join("mcp.json");
    let fake_framework_root = tmp.path().join("not-framework-root");
    write_json(
        &mcp_path,
        &json!({
            "mcp_servers": {
                "browser-mcp": json!({
                    "command": common::router_rs_binary().expect("router-rs").to_string_lossy(),
                    "args": [
                        "browser",
                        "mcp-stdio",
                        "--repo-root",
                        fake_framework_root.to_string_lossy(),
                    ]
                })
            }
        }),
    );

    let install_with_fake_suffix_match = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install_with_fake_suffix_match["success"], true);
    assert_eq!(
        install_with_fake_suffix_match["results"]["cursor"]["mcp"]["managed"],
        false
    );
    assert_eq!(
        install_with_fake_suffix_match["results"]["cursor"]["mcp"]["reason"],
        json!("skipped_user_owned")
    );

    write_json(
        &mcp_path,
        &json!({
            "mcp_servers": {
                "browser-mcp": common::browser_mcp_server_payload_like_host(&framework_root)
            }
        }),
    );

    let install_with_real_framework_root = router_rs_json(&[
        "framework",
        "host-integration",
        "install",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install_with_real_framework_root["success"], true);
    assert_eq!(
        install_with_real_framework_root["results"]["cursor"]["mcp"]["managed"],
        true
    );
    let reason = &install_with_real_framework_root["results"]["cursor"]["mcp"]["reason"];
    assert!(
        reason == &json!("already-managed-equivalent") || reason == &json!("installed"),
        "matching framework repo-root should mark browser-mcp managed: {reason}"
    );
}

#[test]
fn cursor_user_scope_remove_preserves_user_owned_browser_mcp_server() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();

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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(install["success"], true);

    let mcp_path = cursor_home.join("mcp.json");
    write_json(
        &mcp_path,
        &json!({
            "mcp_servers": {
                "browser-mcp": {
                    "command": "custom-browser-mcp",
                    "args": ["--local"]
                }
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    assert_eq!(remove["success"], true);
    assert_eq!(remove["results"]["cursor"]["mcp"]["changed"], false);
    assert_eq!(
        remove["results"]["cursor"]["mcp"]["skipped_user_owned"],
        json!(true)
    );
    let removed_payload = common::read_json(&mcp_path);
    assert_eq!(
        removed_payload["mcp_servers"]["browser-mcp"]["command"],
        json!("custom-browser-mcp")
    );
}

#[test]
fn framework_host_integration_remove_preserves_files_not_recorded_in_manifest() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let artifact_root = tmp.path().join("artifacts");
    let cursor_home = tmp.path().join("cursor-home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&cursor_home).unwrap();
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);
    let command_path = cursor_home.join("rules/framework.mdc");
    let manifest_path = cursor_home.join(".framework-projection.json");
    let original_content = read_text(&command_path);
    write_json(
        &manifest_path,
        &json!({
            "schema_version": "framework-host-projection-v1",
            "managed_by": "skill-framework",
            "host_projection": "cursor",
            "scope": "project",
            "files": [project_root.join(".cursor/rules/other.mdc").to_string_lossy()]
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
        "--cursor-home",
        cursor_home.to_str().unwrap(),
        "--to",
        "cursor",
        "--scope",
        "user",
    ]);

    assert_eq!(result["results"]["cursor"]["status"], "removed");
    assert!(command_path.is_file());
    assert_eq!(read_text(&command_path), original_content);
    assert_eq!(
        result["results"]["cursor"]["skipped_user_owned_paths"],
        json!([command_path.to_string_lossy()])
    );
}

#[test]
fn install_skills_repo_root_alias_does_not_fill_project_root() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut command = router_rs_command([
        "host",
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
fn install_and_remove_opencode_projection() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    seed_framework_markers(&repo_root);

    let install = host_integration_json(&[
        "install",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "opencode",
    ]);
    assert_eq!(install["success"], true);
    assert_eq!(install["results"]["opencode"]["status"], "installed");

    // .opencode/opencode.json must contain mcpServers.router-rs-framework
    let config_path = repo_root.join(".opencode/opencode.json");
    assert!(config_path.exists(), "missing {}", config_path.display());
    let config = read_json(&config_path);
    assert!(
        config["mcpServers"]["router-rs-framework"].is_object(),
        "opencode.json must contain mcpServers.router-rs-framework"
    );

    // .opencode/.framework-projection.json must exist
    let manifest_path = repo_root.join(".opencode/.framework-projection.json");
    assert!(
        manifest_path.exists(),
        "missing opencode projection manifest"
    );
    let manifest = read_json(&manifest_path);
    assert_eq!(manifest["host_projection"].as_str(), Some("opencode"));

    // status subcommand must report the projection
    let status = host_integration_json(&[
        "status",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
    ]);
    assert_eq!(status["success"], true);

    // remove
    let removed = host_integration_json(&[
        "remove",
        "--framework-root",
        repo_root.to_str().unwrap(),
        "--project-root",
        repo_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
        "--scope",
        "project",
        "--to",
        "opencode",
    ]);
    assert_eq!(removed["success"], true);
    assert_eq!(removed["results"]["opencode"]["status"], "removed");

    // mcpServers.router-rs-framework must be gone from config
    let config_after = read_json(&config_path);
    assert!(
        config_after
            .get("mcpServers")
            .and_then(|s| s.get("router-rs-framework"))
            .is_none(),
        "router-rs-framework must be removed from opencode.json after remove"
    );
    assert!(
        !manifest_path.exists(),
        "opencode projection manifest must be deleted after remove"
    );
}

#[test]
fn install_claude_script_help_exits_zero() {
    let root = project_root();
    let script = root.join("scripts/install-claude.sh");
    let status = Command::new("bash")
        .arg(script)
        .arg("--help")
        .status()
        .expect("install-claude.sh --help");
    assert!(status.success());
}
