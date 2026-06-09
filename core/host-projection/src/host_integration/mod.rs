use router_rs::framework_runtime::{framework_root_from_executable_path, is_framework_root};
use router_rs::framework_error::FrameworkResult;
use router_rs::runtime_registry::{
    load_runtime_registry, load_runtime_registry_payload,
    load_runtime_registry_payload_if_repo_local,
};
use chrono::Local;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CONFIG_SCHEMA_HEADER: &str =
    "#:schema https://developers.openai.com/codex/config-schema.json\n";
const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "fast-mode",
    "context-remaining",
    "git-branch",
];
const CODEX_SKILL_SURFACE_REL: &str = "artifacts/codex-skill-surface/skills";
const CODEX_SKILL_SURFACE_MANIFEST_NAME: &str = ".codex-skill-surface.json";
const FRAMEWORK_PROJECTION_SCHEMA_VERSION: &str = "framework-host-projection-v1";
const HOST_PROJECTION_NARRATIVE_SCHEMA_VERSION: &str = "framework-host-projection-narrative-v2";
const GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION: &str =
    "framework-generated-artifacts-manifest-v1";
const GENERATED_ARTIFACT_GENERATOR_TIMEOUT: Duration = Duration::from_secs(300);
const GENERATED_ARTIFACT_COPY_SKIP_DIR_NAMES: [&str; 10] = [
    ".codex",
    ".git",
    ".mypy_cache",
    ".opencode",
    ".ruff_cache",
    ".serena",
    "artifacts",
    "node_modules",
    "output",
    "target",
];
const FRAMEWORK_PROJECTION_MANIFEST_NAME: &str = ".framework-projection.json";
const FRAMEWORK_PROJECTION_ANTIGRAVITY_MANIFEST_NAME: &str = ".framework-projection-antigravity.json";
const DEFAULT_PROJECT_SCOPE: &str = "project";
const HOST_SKILL_SURFACE_PINNED_SKILLS: [&str; 9] = [
    "discussx",
    "planx",
    "implementx",
    "verifyx",
    "code-review-deep",
    "deepinterview",
    "gitx",
    "plan-mode",
    "update",
];
/// Metadata-only doctor and full drift-gate both use paths declared in
/// `configs/framework/GENERATED_ARTIFACTS.json` (`framework maint update-one-shot`).
const CODEX_SYSTEM_PROVIDED_SKILLS: [&str; 5] = [
    "imagegen",
    "openai-docs",
    "plugin-creator",
    "skill-creator",
    "skill-installer",
];
const CURRENT_ALLOWED_ARTIFACT_NAMES: [&str; 3] =
    ["active_task.json", "focus_task.json", "task_registry.json"];
const TASK_ALLOWED_ARTIFACT_NAMES: [&str; 6] = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    "CONTINUITY_JOURNAL.json",
    ".supervisor_state.json",
];

#[derive(Debug, Clone, Deserialize)]
struct GeneratedArtifactsManifest {
    schema_version: String,
    generated_artifacts: Vec<GeneratedArtifactManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneratedArtifactManifestEntry {
    path: String,
    generator: String,
    compare: String,
}

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ExportRuntimeRegistry {
        #[arg(long, alias = "framework-root")]
        repo_root: Option<PathBuf>,
    },
    ResolveSkillsSource {
        #[arg(long)]
        repo_root: PathBuf,
    },
    ValidateDefaultBootstrap {
        #[arg(long)]
        bootstrap_path: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
    },
    BuildDefaultBootstrap {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long)]
        artifact_source_dir: Option<PathBuf>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        top: usize,
    },
    PlanCurrentArtifactClutter {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        active_task_id: String,
    },
    MigrateCurrentArtifactClutter {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        active_task_id: String,
    },
    EnsureDefaultBootstrap {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    InstallNativeIntegration {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        home_config_path: PathBuf,
        #[arg(long)]
        home_codex_skills_path: PathBuf,
        #[arg(long)]
        bootstrap_output_dir: Option<PathBuf>,
        #[arg(long)]
        skip_home_codex_skills_link: bool,
        #[arg(long)]
        skip_default_bootstrap: bool,
    },
    InstallSkills {
        #[arg(long, alias = "framework-root")]
        repo_root: Option<PathBuf>,
        #[arg(long)]
        project_root: Option<PathBuf>,
        #[arg(long)]
        artifact_root: Option<PathBuf>,
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        codex_home: Option<PathBuf>,
        #[arg(long)]
        cursor_home: Option<PathBuf>,
        #[arg(long)]
        claude_home: Option<PathBuf>,
        #[arg(long)]
        antigravity_home: Option<PathBuf>,
        #[arg(long)]
            #[arg(long)]
        opencode_home: Option<PathBuf>,
        #[arg(long)]
        to: Vec<String>,
        #[arg(long, default_value = DEFAULT_PROJECT_SCOPE)]
        scope: String,
        #[arg(long)]
        bootstrap_output_dir: Option<PathBuf>,
        #[arg(long)]
        skip_default_bootstrap: bool,
        #[arg(default_value = "status")]
        command: String,
        #[arg()]
        tools: Vec<String>,
    },
    Install(ProjectionCommand),
    Status(ProjectionStatusCommand),
    Remove(ProjectionCommand),
    Cleanup(ProjectionCommand),
    CompatibilityAliases,
    GeneratedArtifactsStatus {
        #[arg(long, alias = "repo-root")]
        framework_root: Option<PathBuf>,
        #[arg(long)]
        artifact_root: Option<PathBuf>,
        /// Skip running manifest generators (existence/forbidden/missing-only probe).
        #[arg(long)]
        skip_generator_run: bool,
    },
}

#[derive(clap::Args, Debug, Clone)]
pub struct ProjectionCommand {
    #[arg(long, alias = "repo-root")]
    framework_root: Option<PathBuf>,
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    artifact_root: Option<PathBuf>,
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    cursor_home: Option<PathBuf>,
    #[arg(long)]
    claude_home: Option<PathBuf>,
    #[arg(long)]
    antigravity_home: Option<PathBuf>,
    #[arg(long)]
    #[arg(long)]
    opencode_home: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_PROJECT_SCOPE)]
    scope: String,
    #[arg(long)]
    to: Vec<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ProjectionStatusCommand {
    #[arg(long, alias = "repo-root")]
    framework_root: Option<PathBuf>,
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    artifact_root: Option<PathBuf>,
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    cursor_home: Option<PathBuf>,
    #[arg(long)]
    claude_home: Option<PathBuf>,
    #[arg(long)]
    antigravity_home: Option<PathBuf>,
    #[arg(long)]
    #[arg(long)]
    opencode_home: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ResolvedProjectionRoots {
    pub framework_root: PathBuf,
    pub project_root: PathBuf,
    pub artifact_root: PathBuf,
    /// OS account home for Desktop official config paths and stable MCP binary (not `CLAUDE_HOME` parent).
    pub account_home_root: PathBuf,
    pub codex_home_root: PathBuf,
    pub cursor_home_root: PathBuf,
    pub claude_home_root: PathBuf,
    pub antigravity_home_root: PathBuf,
    pub opencode_home_root: PathBuf,
}

pub fn run_host_integration_from_args(args: &[String]) -> FrameworkResult<Value> {
    let forwarded_args = if matches!(args.first().map(String::as_str), Some("--")) {
        &args[1..]
    } else {
        args
    };
    let iter = std::iter::once("router-rs-host-integration".to_string())
        .chain(forwarded_args.iter().cloned());
    run_host_integration_payload(Cli::parse_from(iter))
}


mod artifacts;
mod projection;
mod roots;

pub use artifacts::*;
pub use projection::*;
pub use roots::*;

#[cfg(test)]
mod tests {
    use super::*;

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

    // ── Race condition / concurrent install tests (#63) ──────────────

    #[test]
    fn parallel_atomic_write_to_same_path_last_writer_wins() {
        // 验证 concurrent atomic_write 不会导致文件损坏（内容始终是完整字符串）。
        use std::sync::{Arc, Barrier};
        let dir = unique_test_root("parallel-atomic-write");
        fs::create_dir_all(&dir).unwrap();
        let final_path = dir.join("output.json");
        let barrier = Arc::new(Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let path = final_path.clone();
                let tmp = dir.join(format!("output.{i}.tmp"));
                let b = barrier.clone();
                std::thread::spawn(move || {
                    b.wait();
                    let content = format!(r#"{{"thread": {i}}}"#);
                    router_rs::atomic_write::write_atomic_text_to_temp(
                        &path, &content, &tmp,
                    )
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        // 文件存在且内容是有效 JSON（某个线程的输出）。
        let content = fs::read_to_string(&final_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("thread").is_some(), "content: {content}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parallel_install_different_tools_no_conflict() {
        // 两个线程同时安装不同 tool 的投影不会互相干扰。
        use std::sync::{Arc, Barrier};
        let root = repo_root();
        let dir = unique_test_root("parallel-install");
        fs::create_dir_all(&dir).unwrap();

        let roots_codex = resolve_projection_roots(
            Some(&root),
            Some(&dir.join("project")),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&dir.join("home-codex")),
        )
        .unwrap();
        let roots_cursor = resolve_projection_roots(
            Some(&root),
            Some(&dir.join("project")),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&dir.join("home-cursor")),
        )
        .unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let b1 = barrier.clone();
        let b2 = barrier.clone();

        let h1 = std::thread::spawn(move || {
            b1.wait();
            install_projection_tool(&roots_codex, "codex", "project")
        });
        let h2 = std::thread::spawn(move || {
            b2.wait();
            install_projection_tool(&roots_cursor, "cursor", "project")
        });
        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();
        // 两者应各自成功（或因路径不存在而合理失败，但不 panic）。
        // 关键断言：不因并发而返回 Poisoned lock 或 corrupted data。
        let ok_or_path_err = |r: &Result<serde_json::Value, String>| -> bool {
            r.is_ok() || r.as_ref().unwrap_err().contains("not found") || r.as_ref().unwrap_err().contains("directory")
        };
        assert!(ok_or_path_err(&r1), "codex: {r1:?}");
        assert!(ok_or_path_err(&r2), "cursor: {r2:?}");

        let _ = fs::remove_dir_all(dir);
    }
}
