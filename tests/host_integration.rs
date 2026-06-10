mod common;

use common::{
    assert_canonical_closed_set_host_ids, host_integration_json, json_from_output, output_text,
    project_root, read_json, read_text, router_rs_command, router_rs_json, run,
    seed_framework_markers, write_json, write_text, CANONICAL_HOST_IDS, RETIRED_HOST_IDS,
};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Shared fixture for `install-native-integration` tests.
/// The `tmp` field is intentionally kept alive (not read) so the tempdir persists
/// for the duration of the test.
#[allow(dead_code)]
struct NativeInstallFixture {
    tmp: tempfile::TempDir,
    repo_root: std::path::PathBuf,
    home_config_path: std::path::PathBuf,
    home_codex_skills_path: std::path::PathBuf,
}

fn build_native_install_fixture(config_toml: Option<&str>) -> NativeInstallFixture {
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
    if let Some(content) = config_toml {
        write_text(&home_config_path, content);
    }
    let home_codex_skills_path = tmp.path().join("home/.codex/skills");
    NativeInstallFixture {
        tmp,
        repo_root,
        home_config_path,
        home_codex_skills_path,
    }
}

macro_rules! install_native_integration_test {
    ($name:ident, $config:expr, |$f:ident, $result:ident| $body:block) => {
        #[test]
        fn $name() {
            let $f = build_native_install_fixture(Some($config));
            let $result = host_integration_json(&[
                "install-native-integration",
                "--repo-root",
                $f.repo_root.to_str().unwrap(),
                "--home-config-path",
                $f.home_config_path.to_str().unwrap(),
                "--home-codex-skills-path",
                $f.home_codex_skills_path.to_str().unwrap(),
                "--skip-default-bootstrap",
            ]);
            $body
        }
    };
}

/// Like `router_rs_json` but passes `HOME` only to the child process,
/// avoiding mutation of the global environment which causes flaky tests
/// when Rust runs tests in parallel.
fn router_rs_json_with_home(home: &Path, args: &[&str]) -> Value {
    let mut cmd = router_rs_command(args);
    cmd.env("HOME", home);
    json_from_output(&run(cmd))
}

#[test]
fn runtime_registry_review_gate_lane_fields_present_on_disk() {
    let v = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let lanes = common::reviewer_lanes_from_registry(&v);
    common::assert_reviewer_lanes_closed(&lanes);
}

#[test]
fn shell_installer_e2e_writes_expected_files() {
    let codex_home = tempdir().unwrap();
    let status = router_rs_command([
        "framework",
        "maint",
        "install-codex-user-hooks",
        "--codex-home",
        codex_home.path().to_str().unwrap(),
    ])
    .env("HOME", codex_home.path())
    .status()
    .expect("router-rs maint install-codex-user-hooks");
    assert!(status.success());

    let config = std::fs::read_to_string(codex_home.path().join("config.toml")).unwrap();
    assert!(config.contains("[features]"));
    assert!(config.contains("hooks = true"));
    assert!(!config.contains("codex_hooks"));

    let hooks = std::fs::read_to_string(codex_home.path().join("hooks.json")).unwrap();
    assert!(hooks.contains("PreToolUse"));
    assert!(hooks.contains("SessionStart"));
    assert!(hooks.contains("UserPromptSubmit"));
    assert!(hooks.contains("PostToolUse"));
    assert!(hooks.contains("Stop"));
    assert!(hooks.contains("router-rs"));
    assert!(hooks.contains("codex-router-rs-hook.sh"));
    assert!(!hooks.contains("sessionEnd"));
}

#[test]
fn cursor_hooks_template_matches_repo_hook_events_and_timeouts() {
    let root = project_root();
    let repo_hooks = read_json(&root.join(".cursor/hooks.json"));
    let template_hooks =
        read_json(&root.join("configs/framework/cursor-hooks.workspace-template.json"));
    let repo_hooks = repo_hooks["hooks"].as_object().expect("repo hooks object");
    let template_hooks = template_hooks["hooks"]
        .as_object()
        .expect("template hooks object");

    let mut repo_keys = repo_hooks.keys().cloned().collect::<Vec<_>>();
    let mut template_keys = template_hooks.keys().cloned().collect::<Vec<_>>();
    repo_keys.sort();
    template_keys.sort();
    assert_eq!(
        repo_keys, template_keys,
        "repo .cursor/hooks.json and cursor workspace template must bind the same events"
    );

    for key in repo_keys {
        let repo_timeout = first_cursor_hook_timeout(repo_hooks.get(&key).unwrap());
        let template_timeout = first_cursor_hook_timeout(template_hooks.get(&key).unwrap());
        assert_eq!(
            repo_timeout, template_timeout,
            "cursor hook timeout drift for event {key}"
        );
        let repo_command = repo_hooks[&key][0]["command"].as_str().unwrap_or_default();
        let template_command = template_hooks[&key][0]["command"]
            .as_str()
            .unwrap_or_default();
        assert!(
            repo_command.contains("cursor-router-rs-hook.sh"),
            "repo Cursor hook {key} must use the router-rs launcher: {repo_command}"
        );
        assert!(
            template_command.contains("cursor-router-rs-hook.sh"),
            "template Cursor hook {key} must use the router-rs launcher: {template_command}"
        );
        assert_eq!(
            normalize_cursor_hook_command(repo_command),
            normalize_cursor_hook_command(template_command),
            "repo and template command must match after SKILL_FRAMEWORK_ROOT normalization for {key}"
        );
    }
}

fn normalize_cursor_hook_command(command: &str) -> String {
    command
        .replace(
            "${SKILL_FRAMEWORK_ROOT:-${CURSOR_WORKSPACE_ROOT:-$PWD}}",
            "${ROOT}",
        )
        .replace("${CURSOR_WORKSPACE_ROOT:-$PWD}", "${ROOT}")
}

fn first_cursor_hook_timeout(value: &Value) -> Option<i64> {
    value
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(Value::as_object)
        .and_then(|entry| entry.get("timeout"))
        .and_then(Value::as_i64)
}

#[test]
fn install_native_integration_idempotent() {
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
        r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{"implementx":{"canonical_owner":"implementx","skill_path":"skills/implementx/SKILL.md","host_entrypoints":{"codex":"/implementx"}}}}"#,
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

    assert_eq!(first["success"], true);
    assert_eq!(second["success"], true);

    let content = read_text(&home_config_path);
    assert_eq!(content.matches("[features]").count(), 1);
    assert_eq!(content.matches("hooks = false").count(), 1);
    assert_eq!(content.matches("codex_hooks = true").count(), 0);

    assert_eq!(first["hooks_enabled"], false);
    assert_eq!(first["hooks_disabled_changed"], true);
    assert_eq!(first["deprecated_codex_hooks_removed"], false);
    assert_eq!(second["hooks_enabled"], false);
    assert_eq!(second["hooks_disabled_changed"], false);
    assert_eq!(second["deprecated_codex_hooks_removed"], false);
    assert_eq!(first["default_bootstrap"]["status"], "materialized");
    assert!(["already-present", "repaired-stale"]
        .contains(&second["default_bootstrap"]["status"].as_str().unwrap()));
    assert_eq!(first["home_codex_skills_changed"], true);
    assert_eq!(second["home_codex_skills_changed"], false);
    assert_eq!(first["codex_prompt_entrypoints"]["changed"], false);
    assert_eq!(second["codex_prompt_entrypoints"]["changed"], false);
}

#[test]
fn install_native_integration_symlink_structure() {
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
        r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{"implementx":{"canonical_owner":"implementx","skill_path":"skills/implementx/SKILL.md","host_entrypoints":{"codex":"/implementx"}}}}"#,
    );

    let home_codex_skills_path = tmp.path().join("home/.codex/skills");
    host_integration_json(&[
        "install-native-integration",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home-config-path",
        tmp.path().join("home/.codex/config.toml").to_str().unwrap(),
        "--home-codex-skills-path",
        home_codex_skills_path.to_str().unwrap(),
        "--skip-default-bootstrap",
    ]);

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
    assert_framework_alias_skill(&surface_root, "implementx");
    assert!(
        !surface_root.join("gsd").exists(),
        "legacy gsd must not be published to codex skill surface"
    );
    assert!(
        !surface_root.join("team/SKILL.md").exists(),
        "retired team slug must not be a visible Codex skill surface"
    );
    assert!(
        !surface_root.join("workflow/SKILL.md").exists(),
        "workflow orchestration must not be a visible Codex skill surface"
    );
}

#[test]
fn install_native_integration_surface_runtime_contract() {
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
        r#"{"schema_version":"framework-runtime-registry-v1","framework_commands":{"implementx":{"canonical_owner":"implementx","skill_path":"skills/implementx/SKILL.md","host_entrypoints":{"codex":"/implementx"}}}}"#,
    );
    write_text(
        &repo_root.join("skills/optional-heavy/SKILL.md"),
        "---\nname: optional-heavy\n---\n",
    );

    host_integration_json(&[
        "install-native-integration",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home-config-path",
        tmp.path().join("home/.codex/config.toml").to_str().unwrap(),
        "--home-codex-skills-path",
        tmp.path().join("home/.codex/skills").to_str().unwrap(),
        "--skip-default-bootstrap",
    ]);

    let surface_root = repo_root.join("artifacts/codex-skill-surface/skills");
    let surface_runtime = read_json(&surface_root.join("SKILL_ROUTING_RUNTIME.json"));
    let surface_runtime_text = serde_json::to_string(&surface_runtime).unwrap();
    assert!(!surface_runtime_text.contains("review-fix-verify-loop"));
    assert!(!surface_runtime_text.contains("artifacts/codex-skill-surface"));
    assert_eq!(
        surface_runtime["skills"][0][8],
        "skills/systematic-debugging/SKILL.md"
    );
    assert!(surface_root
        .parent()
        .unwrap()
        .join(surface_runtime["skills"][0][8].as_str().unwrap())
        .is_file());
    assert!(!surface_root.join("optional-heavy").exists());
}

#[test]
fn install_native_integration_prompt_entrypoints_clean() {
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
        &repo_root.join("skills/systematic-debugging/SKILL.md"),
        "---\nname: systematic-debugging\n---\n",
    );

    host_integration_json(&[
        "install-native-integration",
        "--repo-root",
        repo_root.to_str().unwrap(),
        "--home-config-path",
        tmp.path().join("home/.codex/config.toml").to_str().unwrap(),
        "--home-codex-skills-path",
        tmp.path().join("home/.codex/skills").to_str().unwrap(),
        "--skip-default-bootstrap",
    ]);

    assert!(!tmp.path().join("home/.codex/prompts/gsd.md").exists());
    assert!(!tmp.path().join("home/.codex/prompts/gitx.md").exists());
    assert!(!tmp
        .path()
        .join("home/.codex/prompts/systematic-debugging.md")
        .exists());
}

install_native_integration_test!(
    install_native_integration_forces_hooks_false_when_deprecated_key_is_true,
    "[features]\ncodex_hooks_extra = true\ncodex_hooks = true\n",
    |f, result| {
        assert_eq!(result["success"], true);
        let content = read_text(&f.home_config_path);
        assert!(content.contains("codex_hooks_extra = true"));
        assert_eq!(content.matches("codex_hooks = true").count(), 0);
        assert_eq!(content.matches("hooks = false").count(), 1);
        assert_eq!(result["hooks_enabled"], false);
        assert_eq!(result["deprecated_codex_hooks_removed"], true);
    }
);

install_native_integration_test!(
    install_native_integration_adds_hooks_false_when_missing_in_features,
    "[features]\ncodex_hooks_extra = true\n",
    |f, result| {
        assert_eq!(result["success"], true);
        let content = read_text(&f.home_config_path);
        assert!(content.contains("[features]"));
        assert!(content.contains("codex_hooks_extra = true"));
        assert_eq!(content.matches("hooks = false").count(), 1);
        assert_eq!(result["hooks_enabled"], false);
        assert_eq!(result["deprecated_codex_hooks_removed"], false);
    }
);

install_native_integration_test!(
    install_native_integration_adds_features_block_when_missing,
    "[tui]\nstatus_line = [\"model\", \"tokens\"]\n",
    |f, result| {
        assert_eq!(result["success"], true);
        let content = read_text(&f.home_config_path);
        assert!(content.contains("[tui]"));
        assert!(content.contains("[features]"));
        assert_eq!(content.matches("hooks = false").count(), 1);
        assert_eq!(result["hooks_enabled"], false);
        assert_eq!(result["deprecated_codex_hooks_removed"], false);
    }
);

install_native_integration_test!(
    install_native_integration_dedupes_deprecated_codex_hooks_and_forces_hooks_false,
    "[features]\ncodex_hooks_extra = true\ncodex_hooks = true\ncodex_hooks = false\n",
    |f, result| {
        assert_eq!(result["success"], true);
        let content = read_text(&f.home_config_path);
        assert!(content.contains("codex_hooks_extra = true"));
        assert_eq!(content.matches("codex_hooks = true").count(), 0);
        assert_eq!(content.matches("hooks = false").count(), 1);
        assert_eq!(result["hooks_enabled"], false);
        assert_eq!(result["deprecated_codex_hooks_removed"], true);
    }
);

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
    assert!(framework_rule.contains("AGENTS_CURSOR.md"));
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
    assert!(framework_rule.contains("AGENTS_CLAUDE.md"));
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
    assert!(repo_root
        .join(".claude/.framework-projection.json")
        .exists());
    let manifest = read_json(&repo_root.join(".claude/.framework-projection.json"));
    assert!(manifest["files"].as_array().unwrap().iter().any(|path| path
        .as_str()
        .unwrap_or_default()
        .ends_with(".claude/settings.json")));
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
    // `claude` (claude-code) is excluded from project-scope batch install.
    assert!(result["results"].get("claude").is_none());
    assert!(
        result["results"].get("claude-desktop").is_none(),
        "retired claude-desktop must not appear in project-scope batch install"
    );
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
fn codex_project_install_writes_research_mcp_surfaces() {
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

    let project_mcp = read_json(&repo_root.join(".mcp.json"));
    assert_eq!(
        project_mcp["mcpServers"]["paperplain"]["command"],
        json!("npx")
    );
    assert_eq!(
        project_mcp["mcpServers"]["paperplain"]["args"],
        json!(["-y", "paperplain-mcp"])
    );

    let codex_toml = read_text(&repo_root.join(".codex/config.toml"));
    assert!(
        codex_toml.contains("[mcp_servers.paperplain]"),
        "expected paperplain MCP section in .codex/config.toml"
    );
    assert!(
        codex_toml.contains("paperplain-mcp"),
        "expected paperplain-mcp args in .codex/config.toml"
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
    assert!(!repo_root
        .join(".claude/.framework-projection.json")
        .exists());
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
    assert!(manifest_payload["settings"]["managed_key_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("mcp_servers.browser-mcp")));
    assert_eq!(
        mcp_payload["mcp_servers"]["paperplain"]["command"],
        json!("npx")
    );
    assert_eq!(
        mcp_payload["mcp_servers"]["paperplain"]["args"],
        json!(["-y", "paperplain-mcp"])
    );
    let project_mcp = common::read_json(&project_root.join(".mcp.json"));
    assert_eq!(
        project_mcp["mcpServers"]["paperplain"]["command"],
        json!("npx")
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
    assert!(removed_payload
        .get("mcp_servers")
        .and_then(|servers| servers.get("browser-mcp"))
        .is_none());
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
    assert!(!manifest_payload["files"]
        .as_array()
        .unwrap()
        .contains(&json!(mcp_path.to_string_lossy().to_string())));
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
    assert_eq!(install["results"]["cursor"]["mcp"]["skipped_user_owned"], json!(false));
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
    assert!(manifest_payload["settings"]["managed_key_paths"]
        .as_array()
        .unwrap()
        .contains(&json!("mcp_servers.browser-mcp")));
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
        "--skip-generator-run",
    ]);
    assert_eq!(
        status["schema_version"],
        "framework-generated-artifacts-status-v1"
    );
    assert_eq!(
        status["manifest_status"]["mode"],
        "manifest-backed-generated-artifact-metadata-only"
    );
    assert_eq!(status["manifest_status"]["skip_generator_run"], true);
    assert_eq!(status["drift_gate"]["enabled"], true);
    assert_eq!(
        status["drift_gate"]["compare"],
        json!(["byte-for-byte", "normalized-text"])
    );
    assert!(
        status["manifest_status"]
            .get("missing_required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose missing_required_generated_artifacts"
    );
    assert!(
        status["manifest_status"]
            .get("required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose required_generated_artifacts"
    );
    let declared_paths = status["manifest_status"]["declared_generated_artifact_paths"]
        .as_array()
        .unwrap();
    assert!(!declared_paths.is_empty());
    for required in declared_paths {
        let required = required.as_str().unwrap();
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
fn generated_artifacts_status_fails_when_declared_artifact_missing_on_disk() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    let artifact_root = tmp.path().join("artifacts");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [
                {
                    "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                    "generator": "sh scripts/generate-surface.sh",
                    "compare": "byte-for-byte"
                },
                {
                    "path": "skills/SKILL_ROUTING_RUNTIME.json",
                    "generator": "sh scripts/generate-surface.sh",
                    "compare": "byte-for-byte"
                }
            ]
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
        "--skip-generator-run",
    ]);

    assert_eq!(status["ok"], false);
    let runtime = status["generated_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"] == "skills/SKILL_ROUTING_RUNTIME.json")
        .expect("manifest-declared runtime artifact must be reported");
    assert_eq!(runtime["exists"], false);
    assert_eq!(runtime["clean"], false);
    assert!(
        status["manifest_status"]
            .get("missing_required_generated_artifacts")
            .is_none(),
        "manifest-only status must not expose missing_required_generated_artifacts"
    );
    assert!(
        !artifact_root
            .join("generated-artifacts-drift-check")
            .exists(),
        "generated-artifacts-status should clean temporary drift-check copies"
    );
}

#[test]
fn generated_artifacts_status_fails_when_manifest_omits_checked_in_projection() {
    let tmp = tempdir().unwrap();
    let framework_root = tmp.path().join("framework");
    seed_framework_markers(&framework_root);
    write_json(
        &framework_root.join("configs/framework/GENERATED_ARTIFACTS.json"),
        &json!({
            "schema_version": "framework-generated-artifacts-manifest-v1",
            "generated_artifacts": [{
                "path": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
                "generator": "true",
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
        &framework_root.join(".claude/rules/framework.md"),
        "---\ndescription: test\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: claude-code -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: framework-host-projection-v1 -->\n<!-- install_scope: project -->\n\nprojection\n",
    );

    let status = router_rs_json(&[
        "framework",
        "host-integration",
        "generated-artifacts-status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--skip-generator-run",
    ]);

    assert_eq!(status["ok"], false);
    let undeclared = status["manifest_status"]["undeclared_generated_artifacts"]
        .as_array()
        .unwrap();
    assert!(
        undeclared.iter().any(|path| path == ".claude/rules/framework.md"),
        "expected undeclared projection, got {undeclared:?}"
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
        r#"{"status":"fresh","marker":"generated-by-test","derived_reports":["skills/SKILL_TIERS.json"]}
"#,
    );
    write_text(
        &framework_root.join("scripts/generate-surface.sh"),
        r##"mkdir -p configs/framework
printf '%s\n' '{"status":"fresh","marker":"generated-by-test","derived_reports":["skills/SKILL_TIERS.json"]}' > configs/framework/FRAMEWORK_SURFACE_POLICY.json
"##,
    );
    write_text(
        &framework_root.join("skills/SKILL_EXTRA.json"),
        r#"{"marker":"generated-by-test"}
"#,
    );
    write_text(
        &framework_root.join("skills/SKILL_TIERS.json"),
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
    assert!(
        !undeclared.contains(&json!("skills/SKILL_TIERS.json")),
        "derived reports declared by FRAMEWORK_SURFACE_POLICY.json should not be flagged"
    );
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
        r#"{"status":"stale","marker":"generated-by-test","bad":"/Users/joe/.codex ${HOME}/Documents/skill"}
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
        "manifest-backed-generated-artifact-drift-gate"
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
        json!(["expanded-codex-home", "expanded-consuming-project-root"])
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
    let manifest_path = tmp.path().join("SKILL_MANIFEST.json");
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
    write_json(
        &manifest_path,
        &json!({
            "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "source", "source_position", "skill_path", "host_platforms", "kind"],
            "skills": [
                ["opencode-only", "L1", "owner", "none", "P1", "Opencode-only skill", "n/a", ["opencode only"], "project", 3, "skills/opencode-only/SKILL.md", ["opencode"], "skill"],
                ["cursor-skill", "L1", "owner", "none", "P1", "Cursor skill", "n/a", ["cursor task"], "project", 3, "skills/cursor-skill/SKILL.md", ["cursor"], "skill"],
                ["gitx", "L1", "owner", "none", "P1", "Git command", "n/a", ["gitx", "/gitx"], "project", 3, "skills/gitx/SKILL.md", ["cursor"], "framework_command"]
            ]
        }),
    );
    let filtered = router_rs_json(&[
        "search",
        "opencode only",
        "--runtime",
        runtime_path.to_str().unwrap(),
        "--manifest",
        manifest_path.to_str().unwrap(),
        "--host-id",
        "cursor",
        "--json",
    ]);
    assert!(
        filtered["matches"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| { entry["record"]["name"].as_str().unwrap_or_default() != "opencode-only" }),
        "cursor host search must not return opencode-only skill: {filtered}"
    );

    let alias = router_rs_json(&[
        "search",
        "/gitx",
        "--runtime",
        runtime_path.to_str().unwrap(),
        "--manifest",
        manifest_path.to_str().unwrap(),
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
    let env_codex_home = tmp.path().join("env/.codex");
    let flag_codex_home = tmp.path().join("flag/.codex");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut env_status = router_rs_command(["framework", "host-integration", "status"]);
    env_status
        .env("SKILL_FRAMEWORK_ROOT", &framework_root)
        .env("SKILL_PROJECT_ROOT", &project_root)
        .env("SKILL_ARTIFACT_ROOT", &artifact_root)
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
        env_payload["resolved_roots"]["host_home_roots"]["codex"],
        env_codex_home.to_str().unwrap()
    );

    let mut flag_status = router_rs_command([
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--codex-home",
        flag_codex_home.to_str().unwrap(),
    ]);
    flag_status.env("CODEX_HOME", &env_codex_home);
    let flag_payload = json_from_output(&run(flag_status));
    assert_eq!(
        flag_payload["resolved_roots"]["host_home_roots"]["codex"],
        flag_codex_home.to_str().unwrap()
    );
}

#[test]
fn project_discovery_ignores_host_private_projection_directories() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let host_private_only = tmp.path().join("host-private-only");
    std::fs::create_dir_all(host_private_only.join(".codex/prompts")).unwrap();

    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--manifest-path",
        framework_root
            .join("core/router-rs-cli/Cargo.toml")
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
            .join("core/router-rs-cli/Cargo.toml")
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
fn compatibility_alias_outputs_are_normalized_equivalent() {
    let tmp = tempdir().unwrap();
    let framework_root = project_root();
    let project_root = tmp.path().join("consumer");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&project_root).unwrap();
    std::fs::create_dir_all(&home).unwrap();

    let framework_status = router_rs_json_with_home(&home, &[
        "framework",
        "host-integration",
        "status",
        "--framework-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
    ]);
    let framework_status_with_repo_root = router_rs_json_with_home(&home, &[
        "framework",
        "host-integration",
        "status",
        "--repo-root",
        framework_root.to_str().unwrap(),
        "--project-root",
        project_root.to_str().unwrap(),
        "--home",
        home.to_str().unwrap(),
    ]);
    assert_eq!(
        normalize_alias_equivalence(framework_status_with_repo_root),
        normalize_alias_equivalence(router_rs_json_with_home(&home, &[
            "framework",
            "host-integration",
            "status",
            "--framework-root",
            framework_root.to_str().unwrap(),
            "--project-root",
            project_root.to_str().unwrap(),
            "--home",
            home.to_str().unwrap(),
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

fn normalize_alias_equivalence(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove("invocation");
        object.remove("resolved_roots");
        // Collect host keys dynamically from `host_targets` and `results` so that
        // newly-added hosts are covered without manual list maintenance.
        let host_keys: Vec<String> = {
            let mut keys = Vec::new();
            if let Some(ht) = object.get("host_targets").and_then(Value::as_object) {
                keys.extend(ht.keys().cloned());
            }
            if let Some(r) = object.get("results").and_then(Value::as_object) {
                for k in r.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
            keys
        };
        if let Some(host_targets) = object.get_mut("host_targets").and_then(Value::as_object_mut) {
            for key in &host_keys {
                host_targets.remove(key.as_str());
            }
        }
        if let Some(results) = object.get_mut("results").and_then(Value::as_object_mut) {
            for key in &host_keys {
                results.remove(key.as_str());
            }
        }
    }
    payload
}

fn assert_framework_alias_skill(surface_root: &Path, slug: &str) {
    let content = read_text(&surface_root.join(slug).join("SKILL.md"));
    assert!(content.contains(&format!("name: {slug}")));
    assert!(
        content.contains("generated lightweight Codex CLI alias")
            || content.contains("generated-codex-skill-surface"),
        "expected generated alias marker in surface SKILL.md"
    );
    assert!(content.contains(&format!("`/{slug}`")));
    assert!(!content.contains(&format!("`${slug}`")));
    assert!(content.contains("skills/skill-framework-developer/SKILL.md"));
}

#[test]
fn validation_subcommands_cover_install_skills_contract() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(repo_root.join("skills")).unwrap();
    seed_framework_markers(&repo_root);
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
fn runtime_registry_missing_file_fails_closed_with_actionable_error() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_root).unwrap();
    let output = run(router_rs_command([
        "host",
        "codex",
        "host-integration",
        "export-runtime-registry",
        "--repo-root",
        repo_root.to_str().unwrap(),
    ]));
    assert!(!output.status.success());
    let (_stdout, stderr) = output_text(&output);
    assert!(stderr.contains("Runtime registry not found"));
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
            "schema_version": "framework-runtime-registry-v1",
            "framework_core": {
                "authority": "rust",
                "source": "framework-root-native",
                "host_policy": "closed-set-explicit-projections"
            },
            "host_projections": {"cursor": {"profile_id": "repo-cursor"}},
            "workspace_bootstrap_defaults": {"skills": {"source_rel": "repo-skills"}},
            "framework_commands": {"implementx": {"canonical_owner": "repo-owner"}}
        }))
        .unwrap(),
    );
    let payload = runtime_registry(&repo_root);
    assert_eq!(
        payload["host_projections"]["cursor"]["profile_id"],
        "repo-cursor"
    );
    assert_eq!(
        payload["framework_commands"]["implementx"]["canonical_owner"],
        "repo-owner"
    );
}

#[test]
fn runtime_registry_exposes_framework_commands_and_native_runtime_contract() {
    let payload = runtime_registry(&project_root());
    let aliases = &payload["framework_commands"];
    assert!(aliases.get("autopilot").is_none());
    assert!(aliases.get("gsd").is_none());
    assert_eq!(
        aliases["implementx"]["canonical_owner"],
        "implementx"
    );
    assert_eq!(
        aliases["implementx"]["host_entrypoints"]["codex"],
        "/implementx"
    );
    assert_eq!(
        aliases["implementx"]["host_entrypoints"]["cursor"],
        "/implementx"
    );
    let implementx_eps = aliases["implementx"]["goal_persistence"]["execution_entrypoints"]
        .as_array()
        .expect("implementx execution_entrypoints should be an array");
    assert!(implementx_eps.contains(&json!("/implementx")));
    assert!(implementx_eps.contains(&json!("/verifyx")));
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
        json!([
            "cursor",
            "claude-code",
            "opencode",
            "antigravity",
            "codex"
        ])
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["codex"]["install_tool"],
        "codex"
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["cursor"]["host_entrypoints"],
        json!(["AGENTS_CURSOR.md", ".cursor/rules/*.mdc"])
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["claude-code"]["install_tool"],
        "claude"
    );
    assert_eq!(
        payload["host_targets"]["metadata"]["claude-code"]["host_entrypoints"],
        json!([
            "AGENTS_CLAUDE.md",
            ".claude/rules/framework.md",
            ".claude/settings.json"
        ])
    );
    assert!(payload.get("mcp_clients").is_none());
    let implementx = &aliases["implementx"];
    assert_eq!(implementx["canonical_owner"], "implementx");
    let gp = implementx["goal_persistence"]
        .as_object()
        .expect("goal_persistence");
    let leader = gp
        .get("continuation_hook_leader")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        leader.contains("framework_goal_drive") && !leader.contains("GOAL_CONTINUE"),
        "continuation_hook_leader: {leader}"
    );
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
    // Lock the contract-with-code alignment: session_supervisor.rs only
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

