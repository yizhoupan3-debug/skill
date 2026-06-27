use chrono::Local;
use clap::{Parser, Subcommand};
use core_errors::FrameworkError;
use framework_kernel::repo_roots::{framework_root_from_executable_path, is_framework_root};
use framework_kernel::runtime_registry::{load_runtime_registry, load_runtime_registry_payload};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

type Result<T> = std::result::Result<T, FrameworkError>;
use std::time::{Duration, Instant};


const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "fast-mode",
    "context-remaining",
    "git-branch",
];
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
const DEFAULT_PROJECT_SCOPE: &str = "project";
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
struct GeneratedArtifactManifestEntry {
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
        bootstrap_output_dir: Option<PathBuf>,
        #[arg(long)]
        skip_default_bootstrap: bool,
        #[arg(long)]
        host_id: String,
    },
    InstallSkills {
        #[arg(long, alias = "framework-root")]
        repo_root: Option<PathBuf>,
        #[arg(long)]
        project_root: Option<PathBuf>,
        #[arg(long)]
        artifact_root: Option<PathBuf>,
        /// Per-host home overrides as `<host_id>=<path>`, e.g. `--host-home cursor=~/.cursor`.
        #[arg(long = "host-home")]
        host_homes: Vec<String>,
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
    /// Per-host home overrides as `<host_id>=<path>`, e.g. `--host-home cursor=~/.cursor`.
    #[arg(long = "host-home")]
    host_homes: Vec<String>,
    #[arg(long)]
    home: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_PROJECT_SCOPE)]
    scope: String,
    #[arg(long)]
    to: Vec<String>,
    #[arg(long)]
    dry_run: bool,
}

impl ProjectionCommand {
    /// Check if a host-specific home override is present in the --host-home vec.
    pub fn host_home_is_set(&self, host_id: &str) -> bool {
        let prefix = format!("{}=", host_id);
        self.host_homes.iter().any(|h| h.starts_with(&prefix))
    }

    /// Parse `--host-home` pairs into `(host_id, path)` tuples.
    pub fn parsed_host_homes(&self) -> Vec<(String, PathBuf)> {
        self.host_homes.iter().filter_map(|s| {
            s.split_once('=').map(|(id, path)| (id.to_string(), PathBuf::from(path)))
        }).collect()
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct ProjectionStatusCommand {
    #[arg(long, alias = "repo-root")]
    framework_root: Option<PathBuf>,
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    artifact_root: Option<PathBuf>,
    /// Per-host home overrides as `<host_id>=<path>`, e.g. `--host-home cursor=~/.cursor`.
    #[arg(long = "host-home")]
    host_homes: Vec<String>,
    #[arg(long)]
    home: Option<PathBuf>,
}

impl ProjectionStatusCommand {
    /// Parse `--host-home` pairs into `(host_id, path)` tuples.
    pub fn parsed_host_homes(&self) -> Vec<(String, PathBuf)> {
        self.host_homes.iter().filter_map(|s| {
            s.split_once('=').map(|(id, path)| (id.to_string(), PathBuf::from(path)))
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedProjectionRoots {
    pub framework_root: PathBuf,
    pub project_root: PathBuf,
    pub artifact_root: PathBuf,
    /// OS account home for Desktop official config paths and stable MCP binary (not `CLAUDE_HOME` parent).
    pub account_home_root: PathBuf,
    /// Per-host home roots, keyed by host_id from the runtime registry.
    pub host_home_roots: BTreeMap<String, PathBuf>,
}

impl ResolvedProjectionRoots {
    /// Get the home root for a specific host. Returns None if host_id not found.
    pub fn host_home_root(&self, host_id: &str) -> Option<&PathBuf> {
        self.host_home_roots.get(host_id)
    }
}

pub fn run_host_integration_from_args(args: &[String]) -> Result<Value> {
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
pub use roots::{
    McpRouterRsCommand, McpCodegraphCommand, cargo_router_rs_executable,
    codegraph_mcp_cargo_bootstrap_args, ensure_router_rs_installed_for_mcp_with_roots,
    is_ephemeral_executable_path, is_repo_build_executable_path,
    mcp_codegraph_command_value, mcp_router_rs_command_value, nearest_marker_root,
    normalize_path, resolve_artifact_root, resolve_framework_root, resolve_host_home,
    resolve_maint_roots, resolve_mcp_codegraph_command, resolve_mcp_router_rs_command,
    resolve_projection_framework_root, resolve_projection_roots, resolve_project_root,
    resolve_stable_router_rs_executable, router_rs_cargo_bootstrap_args,
    run_host_integration_payload, try_framework_root_from_current_exe,
    try_framework_root_from_workspace_env, validate_mcp_command_binary,
    workspace_mcp_codegraph_release_binary,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

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
        let cmd = build_router_rs_hook_command("Stop", "claude");
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
            "Cursor stdin prefilter must not short-circuit before router-rs (see host_extensions dispatch pattern): {cmd}"
        );
    }

    #[test]
    fn canonical_tool_name_reports_registry_supported_tools_and_aliases() {
        let root = repo_root();

        assert_eq!(canonical_tool_name("claude", &root).unwrap(), "claude");

        let err = canonical_tool_name("unknown-host", &root).expect_err("unknown host must fail");
        for &tool in framework_kernel::runtime_registry::ALL_HOST_IDS {
            assert!(
                err.contains(tool),
                "expected supported tool {tool} in error: {err}"
            );
        }
        assert!(err.contains("claude"), "{err}");
    }

    #[test]
    fn runtime_registry_missing_in_repo_root_returns_actionable_error() {
        let root = unique_test_root("runtime-registry-missing");
        fs::create_dir_all(&root).unwrap();

        let err =
            load_runtime_registry_payload(&root).expect_err("expected missing registry error");
        let expected_registry = root.join("configs/framework/RUNTIME_REGISTRY.json");
        assert!(
            err.to_string().contains(expected_registry.to_string_lossy().as_ref()),
            "error should include expected repo-local registry path: {err}"
        );
        assert!(
            err.to_string().contains("framework-root"),
            "error should mention framework-root / --framework-root: {err}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_registry_repo_root_registry_is_used() {
        use framework_kernel::runtime_registry::RUNTIME_REGISTRY_SCHEMA_VERSION;
        let root = unique_test_root("runtime-registry-repo-local");
        let repo_registry = root.join("configs/framework/RUNTIME_REGISTRY.json");
        write_test_file(
            &repo_registry,
            r#"{
  "schema_version": "framework-runtime-registry-v2",
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
            fail_msg.contains("generated artifact generator failed"),
            "failure message should include generator failed marker: {fail_msg}"
        );
        assert!(
            fail_msg.contains("boom"),
            "failure message should include stderr output: {fail_msg}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generated_artifact_generator_timeout_kills_process() {
        let root = unique_test_root("generator-timeout");
        fs::create_dir_all(&root).unwrap();
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS", "1") };

        let timeout = run_generated_artifact_generator("sleep 5", &root, &root);
        assert!(timeout.is_err(), "expected timeout failure");
        let timeout_msg = timeout.err().unwrap();
        assert!(
            timeout_msg.contains("timed out after 1s"),
            "timeout message should include configured timeout: {timeout_msg}"
        );

        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS") };
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
        let artifact = framework_root.join("core/router-rs/target/release/router-rs-cli");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        write_test_file(&artifact, "#!/bin/sh\n");

        let err = validate_mcp_command_binary(&artifact.to_string_lossy(), Some(&framework_root))
            .expect_err("repo target artifact must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("ephemeral build path") || msg.contains("repo build artifact"),
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
        write_test_file(&home.join(".local/bin/router-rs"), "#!/bin/sh\n");
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
            host_home_roots: [
                ("codex".to_string().into(), root.join("codex")),
                ("cursor".to_string().into(), cursor_home.clone()),
                ("claude".to_string().into(), root.join("claude")),
                ("opencode".to_string().into(), root.join("opencode")),
            ]
            .into_iter()
            .collect(),
        };
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::set_env("HOME", &home) };
        let outcome =
            install_mcp_server(&roots, &cursor_home.join("mcp.json"), "cursor", "user").expect("install");
        assert!(
            outcome.changed,
            "expected stale browser-mcp entry to be rewritten"
        );
        assert!(!outcome.skipped_user_owned);

        let payload = read_json_if_exists(&cursor_home.join("mcp.json"))
            .expect("read mcp")
            .expect("mcp exists");
        let command = payload["mcp_servers"]["browser-mcp"]["command"]
            .as_str()
            .expect("command");
        assert!(
            command == "router-rs-cli" || command.ends_with("/.local/bin/router-rs-cli"),
            "expected stable PATH or install-path command, got {command}"
        );

        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env("HOME") };
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
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::set_env("HOME", &home) };
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_BIN") };

        let command = resolve_mcp_router_rs_command(&framework_root);
        assert_ne!(command, McpRouterRsCommand::CargoBootstrap);
        match command {
            McpRouterRsCommand::OnPath | McpRouterRsCommand::Absolute(_) => {}
            McpRouterRsCommand::CargoBootstrap => unreachable!(),
        }

        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe { core_state_utils::env_sync::remove_env("HOME") };
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[serial]
    fn resolve_projection_roots_uses_os_home_not_claude_home_parent_for_account_home() {
        let root = unique_test_root("account-home-root");
        let os_home = root.join("os-home");
        let custom_claude = root.join("custom-claude/.claude");
        fs::create_dir_all(&custom_claude).unwrap();
        let prior_home = std::env::var_os("HOME");
        let prior_claude = std::env::var_os("CLAUDE_HOME");
        // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env("HOME", &os_home) };
        // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env("CLAUDE_HOME", &custom_claude) };

        let host_homes = vec![("claude".to_string(), custom_claude.clone())];
        let roots = resolve_projection_roots(
            None,
            Some(&root.join("project")),
            None,
            &host_homes,
            None,
        )
        .expect("resolve roots");
        assert_eq!(roots.account_home_root, os_home);
        assert_eq!(
            roots.host_home_root("claude").unwrap().as_path(),
            custom_claude.as_path()
        );
        assert_eq!(
            roots.host_home_root("opencode").unwrap().as_path(),
            os_home.join(".opencode").as_path()
        );

        if let Some(h) = prior_home {
            // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
            unsafe { core_state_utils::env_sync::set_env("HOME", &h) };
        } else {
            // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
            unsafe { core_state_utils::env_sync::remove_env("HOME") };
        }
        if let Some(c) = prior_claude {
            // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
            unsafe { core_state_utils::env_sync::set_env("CLAUDE_HOME", &c) };
        } else {
            // SAFETY: test-only; the #[serial] attribute prevents concurrent env access from other tests.
            unsafe { core_state_utils::env_sync::remove_env("CLAUDE_HOME") };
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ensure_codex_research_mcp_toml_writes_paperplain_section() {
        let root = unique_test_root("codex-research-mcp");
        let home = root.join("home");
        let framework_root = root.join("framework");
        let project_root = root.join("project");
        fs::create_dir_all(&framework_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();

        let roots = ResolvedProjectionRoots {
            framework_root: framework_root.clone(),
            project_root: project_root.clone(),
            artifact_root: project_root.join("artifacts"),
            account_home_root: home.clone(),
            host_home_roots: [
                ("codex".to_string().into(), home.join(".codex")),
                ("cursor".to_string().into(), home.join(".cursor")),
                ("claude".to_string().into(), home.join(".claude")),
                ("opencode".to_string().into(), home.join(".opencode")),
            ]
            .into_iter()
            .collect(),
        };

        let changed =
            super::projection::ensure_research_mcp_toml(&roots, "codex").expect("codex mcp toml");
        assert!(changed);
        let text = fs::read_to_string(project_root.join(".codex/config.toml")).unwrap();
        assert!(text.contains("[mcp_servers.paperplain]"));
        assert!(text.contains("paperplain-mcp"));
        assert!(text.contains("[mcp_servers.mcp-codegraph]"));
        assert!(
            text.contains("--repo-root"),
            "codegraph MCP section must pass repo root: {text}"
        );

        let changed_again =
            super::projection::ensure_research_mcp_toml(&roots, "codex").expect("idempotent");
        assert!(!changed_again);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ensure_project_research_mcp_json_registers_mcp_codegraph() {
        let root = unique_test_root("project-research-mcp");
        let home = root.join("home");
        let framework_root = root.join("framework");
        let project_root = root.join("project");
        fs::create_dir_all(&framework_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        write_test_file(
            &framework_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"tools/codegraph-rs\"]\n",
        );

        let roots = ResolvedProjectionRoots {
            framework_root: framework_root.clone(),
            project_root: project_root.clone(),
            artifact_root: project_root.join("artifacts"),
            account_home_root: home.clone(),
            host_home_roots: [
                ("codex".to_string().into(), home.join(".codex")),
                ("cursor".to_string().into(), home.join(".cursor")),
                ("claude".to_string().into(), home.join(".claude")),
                ("opencode".to_string().into(), home.join(".opencode")),
            ]
            .into_iter()
            .collect(),
        };

        let changed =
            super::projection::ensure_project_research_mcp_json(&roots, "claude").expect("project mcp json");
        assert!(changed);

        let payload: Value =
            serde_json::from_str(&fs::read_to_string(project_root.join(".mcp.json")).unwrap())
                .unwrap();
        let servers = payload
            .get("mcpServers")
            .and_then(Value::as_object)
            .expect("mcpServers object");
        let codegraph = servers.get("mcp-codegraph").expect("mcp-codegraph entry");
        assert_eq!(codegraph.get("type").and_then(Value::as_str), Some("stdio"));
        assert!(
            codegraph.get("command").is_some(),
            "codegraph payload must include command: {codegraph}"
        );
        let args = codegraph
            .get("args")
            .and_then(Value::as_array)
            .expect("args array");
        assert!(
            args.iter().any(|v| v.as_str() == Some("--repo-root")),
            "args must include --repo-root: {args:?}"
        );

        let _ = fs::remove_dir_all(root);
    }
}
