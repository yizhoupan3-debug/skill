use super::super::*;
use chrono::Local;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};



fn unique_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "router-rs-{name}-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ))
}

fn write_test_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
}

fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
}
#[test]
fn build_router_rs_claude_hook_command_sources_optional_env_file() {
        let cmd = build_router_rs_claude_hook_command("Stop");
        assert!(
            cmd.contains("router-rs-hook.env"),
            "expected optional hook env injection path segment: {cmd}"
        );
        assert!(
            cmd.contains("set -a"),
            "expected set -a for env sourcing: {cmd}"
        );
        assert!(
            !cmd.contains("grep -Eq"),
            "Cursor stdin prefilter must not short-circuit before router-rs (see claude_hooks payload_looks_like_cursor_hook_stdin): {cmd}"
        );
}

#[test]
fn canonical_tool_name_reports_registry_supported_tools_and_aliases() {
        let root = repo_root();

        assert_eq!(canonical_tool_name("codex", &root).unwrap(), "codex");
        assert_eq!(canonical_tool_name("claude-code", &root).unwrap(), "claude");

        let err = canonical_tool_name("unknown-host", &root).expect_err("unknown host must fail");
        assert!(
            err.contains("Supported tools: codex, cursor, claude, antigravity, opencode"),
            "{err}"
        );
        assert!(err.contains("codex"), "{err}");
        assert!(err.contains("claude-code"), "{err}");
}

#[test]
fn projection_adapters_are_aligned_with_runtime_registry() {
        let root = repo_root();

        validate_projection_adapters_against_registry(&root).unwrap();
        assert_eq!(
            registry_projection_tools(&root).unwrap(),
            vec![
                "codex".to_string(),
                "cursor".to_string(),
                "claude".to_string(),
                "antigravity".to_string(),
                "opencode".to_string(),
            ]
        );
}

#[test]
fn runtime_registry_missing_in_repo_root_returns_actionable_error() {
        let root = unique_test_root("runtime-registry-missing");
        fs::create_dir_all(&root).unwrap();

        let err =
            load_runtime_registry_payload(&root).expect_err("expected missing registry error");
        let expected_registry = root.join("configs/framework/RUNTIME_REGISTRY.json");
        assert!(
            err.contains(expected_registry.to_string_lossy().as_ref()),
            "error should include expected repo-local registry path: {err}"
        );
        assert!(
            err.contains("framework-root"),
            "error should mention framework-root / --framework-root: {err}"
        );

        let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_registry_repo_root_registry_is_used() {
        use router_rs::runtime_registry::RUNTIME_REGISTRY_SCHEMA_VERSION;
        let root = unique_test_root("runtime-registry-repo-local");
        let repo_registry = root.join("configs/framework/RUNTIME_REGISTRY.json");
        write_test_file(
            &repo_registry,
            r#"{
  "schema_version": "framework-runtime-registry-v1",
  "runtime_profiles": []
}"#,
        );

        let payload =
            load_runtime_registry_payload(&root).expect("expected repo-local registry to load");
        assert_eq!(
            payload["schema_version"],
            json!(RUNTIME_REGISTRY_SCHEMA_VERSION)
        );

        let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_artifact_copy_skips_local_state_and_dependency_dirs() {
        let root = unique_test_root("copy-skip");
        let source = root.join("source");
        let destination = root.join("destination");
        write_test_file(&source.join("Cargo.toml"), "[package]\n");
        for skipped in [
            ".codex/cache/cache.json",
            ".git/config",
            ".mypy_cache/state",
            ".opencode/state",
            ".ruff_cache/cache",
            ".serena/state",
            "artifacts/current/state.json",
            "core/router-rs/target/debug/router-rs",
            "target/debug/root",
            "tools/browser-mcp/node_modules/package/index.js",
            "output/image.png",
            "skills/.system/.codex-system-skills.marker",
        ] {
            write_test_file(&source.join(skipped), "local state");
        }

        copy_framework_tree_for_generation(&source, &destination).unwrap();

        assert!(destination.join("Cargo.toml").is_file());
        for skipped in [
            ".codex",
            ".git",
            ".mypy_cache",
            ".opencode",
            ".ruff_cache",
            ".serena",
            "artifacts",
            "core/router-rs/target",
            "target",
            "tools/browser-mcp/node_modules",
            "output",
            "skills/.system/.codex-system-skills.marker",
        ] {
            assert!(
                !destination.join(skipped).exists(),
                "copied skipped generated-artifact dir: {skipped}"
            );
        }

        let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_artifact_temp_root_is_removed_on_drop() {
        let root = unique_test_root("temp-drop");
        let framework_root = root.join("framework");
        let artifact_root = root.join("artifacts");
        write_test_file(&framework_root.join("Cargo.toml"), "[package]\n");

        let temp_path = {
            let guard =
                prepare_generated_artifact_temp_root(&framework_root, &artifact_root).unwrap();
            let temp_path = guard.path().to_path_buf();
            assert!(temp_path.exists());
            temp_path
        };

        assert!(
            !temp_path.exists(),
            "generated artifact temp root was not cleaned"
        );
        let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_artifact_generator_success_and_failure_paths_are_reported() {
        let root = unique_test_root("generator-success-failure");
        fs::create_dir_all(&root).unwrap();

        let ok = run_generated_artifact_generator("printf 'ok\\n'", &root, &root);
        assert!(ok.is_ok(), "expected generator success");

        let fail = run_generated_artifact_generator("printf 'boom\\n' 1>&2; exit 23", &root, &root);
        assert!(fail.is_err(), "expected generator failure");
        let fail_msg = fail.err().unwrap();
        assert!(
            fail_msg.to_string().contains("generated artifact generator failed"),
            "failure message should include generator failed marker: {fail_msg}"
        );
        assert!(
            fail_msg.to_string().contains("boom"),
            "failure message should include stderr output: {fail_msg}"
        );

        let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_artifact_generator_timeout_kills_process() {
        let root = unique_test_root("generator-timeout");
        fs::create_dir_all(&root).unwrap();
        std::env::set_var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS", "1");

        let timeout = run_generated_artifact_generator("sleep 5", &root, &root);
        assert!(timeout.is_err(), "expected timeout failure");
        let timeout_msg = timeout.err().unwrap();
        assert!(
            timeout_msg.to_string().contains("timed out after 1s"),
            "timeout message should include configured timeout: {timeout_msg}"
        );

        std::env::remove_var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS");
        let _ = fs::remove_dir_all(root);
}

#[test]
fn is_ephemeral_executable_path_flags_sandbox_and_tmp_builds() {
        assert!(is_ephemeral_executable_path(
            "/var/folders/xx/T/cursor-sandbox-cache/abc/cargo-target/debug/router-rs"
        ));
        assert!(is_ephemeral_executable_path(
            "/tmp/skill-cargo-target/debug/router-rs"
        ));
        assert!(!is_ephemeral_executable_path(
            "/Users/joe/.local/bin/router-rs"
        ));
}

#[test]
fn validate_mcp_command_binary_rejects_repo_target_artifacts() {
        let root = unique_test_root("mcp-validate-repo-target");
        let framework_root = root.join("framework");
        let artifact = framework_root
            .join("core/router-rs/target/release/router-rs");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        write_test_file(&artifact, "#!/bin/sh\n");

        let err = validate_mcp_command_binary(
            &artifact.to_string_lossy(),
            Some(&framework_root),
        )
        .expect_err("repo target artifact must be rejected");
        assert!(
            err.to_string().contains("ephemeral build path") || err.to_string().contains("repo build artifact"),
            "unexpected error: {err}"
        );

        let _ = fs::remove_dir_all(root);
}

#[test]
fn install_cursor_mcp_server_rewrites_stale_browser_mcp_command() {
        let root = unique_test_root("cursor-mcp-install");
        let home = root.join("home");
        let framework_root = root.join("framework");
        let cursor_home = root.join("cursor-home");
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::create_dir_all(&cursor_home).unwrap();
        write_test_file(
            &home.join(".local/bin/router-rs"),
            "#!/bin/sh\n",
        );
        write_test_file(
            &framework_root.join("core/router-rs/Cargo.toml"),
            "[package]\nname = \"router-rs\"\n",
        );
        write_test_file(
            &cursor_home.join("mcp.json"),
            &format!(
                r#"{{
  "mcp_servers": {{
"browser-mcp": {{
  "command": "cargo",
  "args": ["run", "--release", "--quiet", "--manifest-path", "{}/stale/Cargo.toml", "--", "browser", "mcp-stdio", "--repo-root", "{}"]
}}
  }}
}}"#,
                framework_root.display(),
                framework_root.display()
            ),
        );

        let roots = ResolvedProjectionRoots {
            framework_root: framework_root.clone(),
            project_root: framework_root.clone(),
            artifact_root: framework_root.join("artifacts"),
            account_home_root: home.clone(),
            codex_home_root: root.join("codex"),
            cursor_home_root: cursor_home.clone(),
            claude_home_root: root.join("claude"),
            antigravity_home_root: root.join("gemini"),
            opencode_home_root: root.join("opencode"),
        };
        std::env::set_var("HOME", &home);
        let outcome =
            install_cursor_mcp_server(&roots, &cursor_home.join("mcp.json")).expect("install");
        assert!(outcome.changed, "expected stale browser-mcp entry to be rewritten");
        assert!(!outcome.skipped_user_owned);

        let payload = read_json_if_exists(&cursor_home.join("mcp.json"))
            .expect("read mcp")
            .expect("mcp exists");
        let command = payload["mcp_servers"]["browser-mcp"]["command"]
            .as_str()
            .expect("command");
        assert!(
            command == "router-rs" || command.ends_with("/.local/bin/router-rs"),
            "expected stable PATH or install-path command, got {command}"
        );

        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_mcp_router_rs_command_prefers_path_over_repo_target() {
        let root = unique_test_root("mcp-resolve");
        let home = root.join("home");
        let framework_root = root.join("framework");
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::create_dir_all(framework_root.join("core/router-rs/target/release")).unwrap();
        write_test_file(&home.join(".local/bin/router-rs"), "#!/bin/sh\n");
        write_test_file(
            &framework_root.join("core/router-rs/target/release/router-rs"),
            "#!/bin/sh\n",
        );
        std::env::set_var("HOME", &home);
        std::env::remove_var("ROUTER_RS_BIN");

        let command = resolve_mcp_router_rs_command(&framework_root);
        assert_ne!(command, McpRouterRsCommand::CargoBootstrap);
        match command {
            McpRouterRsCommand::OnPath | McpRouterRsCommand::Absolute(_) => {}
            McpRouterRsCommand::CargoBootstrap => unreachable!(),
        }

        std::env::remove_var("HOME");
        let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_projection_roots_uses_os_home_not_claude_home_parent_for_account_home() {
        let root = unique_test_root("account-home-root");
        let os_home = root.join("os-home");
        let custom_claude = root.join("custom-claude/.claude");
        fs::create_dir_all(&custom_claude).unwrap();
        // Use explicit shared_home to avoid parallel-test HOME env var pollution.
        let roots = resolve_projection_roots(
            None,
            Some(&root.join("project")),
            None,
            None,
            None,
            Some(&custom_claude),
            None,
            None,
            Some(&os_home),
        )
        .expect("resolve roots");
        assert_eq!(roots.account_home_root, os_home);
        assert_eq!(roots.claude_home_root, custom_claude);
        assert_eq!(roots.opencode_home_root, os_home.join(".opencode"));

        let _ = fs::remove_dir_all(root);
}

#[test]
fn continuity_journal_under_task_dir_is_not_clutter() {
        let root = unique_test_root("continuity-journal-allowed");
        let task_root = root.join("artifacts/current/task-1");
        fs::create_dir_all(&task_root).unwrap();
        let journal = task_root.join("CONTINUITY_JOURNAL.json");
        write_test_file(&journal, "{}");
        assert!(
            destination_for_current_artifact(&root, &journal, "task-1").is_none(),
            "CONTINUITY_JOURNAL.json is a task-scoped continuity board"
        );
        let plans =
            plan_current_artifact_clutter_migrations(&root, "task-1").expect("plan migrations");
        assert!(
            !plans.iter().any(|plan| plan.source.ends_with("CONTINUITY_JOURNAL.json")),
            "journal must not appear in clutter plan: {plans:?}"
        );
        let _ = fs::remove_dir_all(root);
}
