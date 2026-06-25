use crate::common::{
    host_integration_json, project_root, read_json, read_text, router_rs_command,
    seed_framework_markers, write_json, write_text,
};
use serde_json::{Value, json};
use tempfile::tempdir;

/// Shared fixture for `install-native-integration` tests.
/// The `tmp` field is intentionally kept alive (not read) so the tempdir persists
/// for the duration of the test.
struct NativeInstallFixture {
    #[allow(dead_code)]
    tmp: tempfile::TempDir,
    repo_root: std::path::PathBuf,
    home_config_path: std::path::PathBuf,
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
    NativeInstallFixture {
        tmp,
        repo_root,
        home_config_path,
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
                "--skip-default-bootstrap",
            ]);
            $body
        }
    };
}

#[test]
#[ignore = "install-codex-user-hooks subcommand was removed; functionality covered by install-native-integration tests"]
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
    .env_remove("SKILL_FRAMEWORK_ROOT")
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
    let template = read_json(&root.join("configs/framework/cursor-hooks.workspace-template.json"));
    let hooks = template["hooks"].as_object().expect("template hooks object");

    // Template must have the expected events: beforeSubmitPrompt, stop, sessionStart, sessionEnd, postToolUse, subagentStart, subagentStop
    let expected_events = [
        "beforeSubmitPrompt",
        "stop",
        "sessionStart",
        "sessionEnd",
        "postToolUse",
        "subagentStart",
        "subagentStop",
    ];
    for event in &expected_events {
        assert!(
            hooks.contains_key(*event),
            "template missing expected hook event: {event}"
        );
        let entries = hooks[*event].as_array().expect("hook entries array");
        assert!(!entries.is_empty(), "template event {event} has no entries");
        let command = entries[0]["command"].as_str().expect("hook command");
        assert!(
            command.contains("cursor-router-rs-hook.sh"),
            "template Cursor hook {event} must use the router-rs launcher: {command}"
        );
        let timeout = entries[0]["timeout"].as_i64().expect("hook timeout");
        assert!(
            timeout > 0,
            "template Cursor hook {event} must have positive timeout"
        );
    }

    // No extra events beyond the expected set
    for key in hooks.keys() {
        assert!(
            expected_events.contains(&key.as_str()),
            "template has unexpected hook event: {key}"
        );
    }
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
        r#"{"schema_version":"framework-runtime-registry-v2","framework_commands":{"gitx":{"canonical_owner":"gitx","skill_path":"skills/gitx/SKILL.md","host_entrypoints":{"codex":"/gitx"}}}}"#,
    );
    write_text(
        &repo_root.join("skills/optional-heavy/SKILL.md"),
        "---\nname: optional-heavy\n---\n",
    );

    let home_config_path = tmp.path().join("home/.codex/config.toml");
    let bootstrap_output_dir = tmp.path().join("bootstrap");

    let args = vec![
        "install-native-integration".to_string(),
        "--repo-root".to_string(),
        repo_root.display().to_string(),
        "--home-config-path".to_string(),
        home_config_path.display().to_string(),
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
    assert!(
        ["already-present", "repaired-stale"]
            .contains(&second["default_bootstrap"]["status"].as_str().unwrap())
    );
    assert_eq!(first["codex_prompt_entrypoints"]["changed"], false);
    assert_eq!(second["codex_prompt_entrypoints"]["changed"], false);
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
        "--skip-default-bootstrap",
    ]);

    assert!(!tmp.path().join("home/.codex/prompts/gsd.md").exists());
    assert!(!tmp.path().join("home/.codex/prompts/gitx.md").exists());
    assert!(
        !tmp.path()
            .join("home/.codex/prompts/systematic-debugging.md")
            .exists()
    );
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

fn string_refs(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
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

    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("artifacts/current/SESSION_SUMMARY.md"))
    );
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("artifacts/current/NEXT_ACTIONS.json"))
    );
    assert!(
        !sources
            .iter()
            .any(|path| path.ends_with("artifacts/current/task-1/SESSION_SUMMARY.md"))
    );
    assert!(
        !sources
            .iter()
            .any(|path| path.ends_with("artifacts/current/task-1/CONTINUITY_JOURNAL.json"))
    );
}
