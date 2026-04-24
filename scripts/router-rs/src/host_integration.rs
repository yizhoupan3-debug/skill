use chrono::Local;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

const CONFIG_SCHEMA_HEADER: &str =
    "#:schema https://developers.openai.com/codex/config-schema.json\n";
const FRAMEWORK_START_MARKER: &str = "<!-- FRAMEWORK_DEFAULT_RUNTIME_START -->";
const FRAMEWORK_END_MARKER: &str = "<!-- FRAMEWORK_DEFAULT_RUNTIME_END -->";
const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v1";
const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
const RETIRED_CODEX_MODEL_INSTRUCTIONS_PATH: &str = ".codex/model_instructions.md";
const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "fast-mode",
    "context-remaining",
    "git-branch",
];
const DEFAULT_SHARED_PROJECT_MCP_SERVERS: [&str; 3] =
    ["browser-mcp", "framework-mcp", "openaiDeveloperDocs"];
const OPENAI_DEVELOPER_DOCS_MCP_URL: &str = "https://developers.openai.com/mcp";
const PERSONAL_PLUGIN_LIVE_PROJECTION_EXCLUDES: [&str; 2] = ["skills", ".mcp.json"];
const CURRENT_ALLOWED_ARTIFACT_NAMES: [&str; 7] = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    "active_task.json",
    "focus_task.json",
    "task_registry.json",
];
const TASK_ALLOWED_ARTIFACT_NAMES: [&str; 5] = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
];

#[derive(Deserialize)]
struct SyncSectionManifest {
    text_files: Vec<String>,
    json_files: Vec<String>,
    managed_directories: Vec<String>,
    #[serde(default)]
    retired_paths: Vec<String>,
}

#[derive(Deserialize)]
struct SyncManifest {
    full_sync: SyncSectionManifest,
    partial_sync: SyncSectionManifest,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeRegistry {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    #[serde(default)]
    shared_project_mcp_servers: Vec<String>,
    #[serde(default)]
    plugins: Vec<RuntimePluginRegistration>,
    #[serde(default)]
    workspace_bootstrap_defaults: RuntimeWorkspaceBootstrapDefaults,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimePluginRegistration {
    plugin_name: String,
    source_rel: String,
    #[serde(default)]
    marketplace_name: Option<String>,
    #[serde(default)]
    marketplace_category: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RuntimeWorkspaceBootstrapDefaults {
    #[serde(default)]
    skill_bridge: RuntimeSkillBridgeDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RuntimeSkillBridgeDefaults {
    #[serde(default)]
    source_rel: Option<String>,
}

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    SyncHostEntrypoints {
        #[arg(long)]
        template_root: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long, conflicts_with = "check")]
        apply: bool,
        #[arg(long, conflicts_with = "apply")]
        check: bool,
    },
    ExportRuntimeRegistry {
        #[arg(long)]
        repo_root: PathBuf,
    },
    ResolveSkillBridgeSource {
        #[arg(long)]
        repo_root: PathBuf,
    },
    ValidateDefaultBootstrap {
        #[arg(long)]
        bootstrap_path: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
    },
    ValidateMarketplacePlugin {
        #[arg(long)]
        marketplace_path: PathBuf,
        #[arg(long)]
        plugin_name: String,
    },
    ValidateHomeClaudeMcp {
        #[arg(long)]
        config_path: PathBuf,
        #[arg(long)]
        repo_root: PathBuf,
    },
    ValidatePersonalPluginMcp {
        #[arg(long)]
        config_path: PathBuf,
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
        memory_root: Option<PathBuf>,
        #[arg(long)]
        artifact_source_dir: Option<PathBuf>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        top: usize,
    },
    RunMemoryAutomation {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        memory_root: Option<PathBuf>,
        #[arg(long)]
        artifact_source_dir: Option<PathBuf>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 8)]
        top: usize,
        #[arg(long)]
        apply_artifact_migrations: bool,
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
    PlanLegacyArtifactRoots {
        #[arg(long)]
        repo_root: PathBuf,
    },
    MigrateLegacyArtifactRoots {
        #[arg(long)]
        repo_root: PathBuf,
    },
    EnsureDefaultBootstrap {
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    InstallNativeIntegration {
        #[arg(long)]
        template_root: Option<PathBuf>,
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        home_config_path: PathBuf,
        #[arg(long)]
        home_plugin_root: PathBuf,
        #[arg(long)]
        home_marketplace_path: PathBuf,
        #[arg(long)]
        home_codex_skills_path: PathBuf,
        #[arg(long)]
        home_claude_skills_path: PathBuf,
        #[arg(long)]
        home_claude_refresh_path: PathBuf,
        #[arg(long)]
        home_claude_mcp_config_path: PathBuf,
        #[arg(long)]
        bootstrap_output_dir: Option<PathBuf>,
        #[arg(long)]
        skip_browser_mcp: bool,
        #[arg(long)]
        skip_framework_mcp: bool,
        #[arg(long)]
        skip_openai_developer_docs_mcp: bool,
        #[arg(long)]
        skip_framework_overlay_retirement: bool,
        #[arg(long)]
        skip_personal_plugin: bool,
        #[arg(long)]
        skip_personal_marketplace: bool,
        #[arg(long)]
        skip_home_codex_skills_link: bool,
        #[arg(long)]
        skip_home_claude_skills_link: bool,
        #[arg(long)]
        skip_home_claude_refresh: bool,
        #[arg(long)]
        skip_home_claude_mcp_sync: bool,
        #[arg(long)]
        skip_default_bootstrap: bool,
    },
}

#[derive(Default, Serialize)]
struct SyncReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
    synced_worktrees: Vec<String>,
    skipped_worktrees: Vec<String>,
}

#[derive(Default)]
struct SingleSyncReport {
    written: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
}

pub fn run_host_integration_from_args(args: &[String]) -> Result<Value, String> {
    let forwarded_args = if matches!(args.first().map(String::as_str), Some("--")) {
        &args[1..]
    } else {
        args
    };
    let iter = std::iter::once("router-rs-host-integration".to_string())
        .chain(forwarded_args.iter().cloned());
    run_host_integration_payload(Cli::parse_from(iter))
}

fn run_host_integration_payload(cli: Cli) -> Result<Value, String> {
    let payload = match cli.command {
        Commands::SyncHostEntrypoints {
            template_root,
            repo_root,
            apply,
            check: _,
        } => serde_json::to_value(sync_host_entrypoints(&template_root, &repo_root, apply)?)
            .map_err(|err| err.to_string())?,
        Commands::ExportRuntimeRegistry { repo_root } => {
            serde_json::to_value(load_runtime_registry_payload(&repo_root)?)
                .map_err(|err| err.to_string())?
        }
        Commands::ResolveSkillBridgeSource { repo_root } => json!({
            "path": normalize_path(&repo_root)?
                .join(skill_bridge_source_rel(&repo_root)?)
                .to_string_lossy(),
        }),
        Commands::ValidateDefaultBootstrap {
            bootstrap_path,
            repo_root,
        } => json!({
            "ok": validate_default_bootstrap(&bootstrap_path, &repo_root)?,
        }),
        Commands::ValidateMarketplacePlugin {
            marketplace_path,
            plugin_name,
        } => json!({
            "ok": validate_marketplace_plugin(&marketplace_path, &plugin_name)?,
        }),
        Commands::ValidateHomeClaudeMcp {
            config_path,
            repo_root,
        } => json!({
            "ok": validate_home_claude_mcp(&config_path, &repo_root)?,
        }),
        Commands::ValidatePersonalPluginMcp {
            config_path,
            repo_root,
        } => json!({
            "ok": validate_personal_plugin_mcp(&config_path, &repo_root)?,
        }),
        Commands::BuildDefaultBootstrap {
            repo_root,
            output_dir,
            query,
            memory_root,
            artifact_source_dir,
            workspace,
            top,
        } => build_default_bootstrap_payload(
            &repo_root,
            output_dir.as_deref(),
            &query,
            memory_root.as_deref(),
            artifact_source_dir.as_deref(),
            workspace.as_deref(),
            top,
        )?,
        Commands::RunMemoryAutomation {
            repo_root,
            output_dir,
            memory_root,
            artifact_source_dir,
            workspace,
            query,
            top,
            apply_artifact_migrations,
        } => run_memory_automation(
            &repo_root,
            output_dir.as_deref(),
            memory_root.as_deref(),
            artifact_source_dir.as_deref(),
            workspace.as_deref(),
            &query,
            top,
            apply_artifact_migrations,
        )?,
        Commands::PlanCurrentArtifactClutter {
            repo_root,
            active_task_id,
        } => json!({
            "plans": migration_plan_values(&plan_current_artifact_clutter_migrations(
                &normalize_path(&repo_root)?,
                &active_task_id,
            )?),
        }),
        Commands::MigrateCurrentArtifactClutter {
            repo_root,
            active_task_id,
        } => json!({
            "moved": migrate_current_artifact_clutter(&normalize_path(&repo_root)?, &active_task_id)?,
        }),
        Commands::PlanLegacyArtifactRoots { repo_root } => json!({
            "plans": migration_plan_values(&plan_legacy_artifact_root_migrations(
                &normalize_path(&repo_root)?,
            )?),
        }),
        Commands::MigrateLegacyArtifactRoots { repo_root } => json!({
            "moved": migrate_legacy_artifact_roots(&normalize_path(&repo_root)?)?,
        }),
        Commands::EnsureDefaultBootstrap {
            repo_root,
            output_dir,
        } => ensure_default_bootstrap(&repo_root, output_dir.as_deref())?,
        Commands::InstallNativeIntegration {
            template_root: _,
            repo_root,
            home_config_path,
            home_plugin_root,
            home_marketplace_path,
            home_codex_skills_path,
            home_claude_skills_path,
            home_claude_refresh_path,
            home_claude_mcp_config_path,
            bootstrap_output_dir,
            skip_browser_mcp,
            skip_framework_mcp,
            skip_openai_developer_docs_mcp,
            skip_framework_overlay_retirement,
            skip_personal_plugin,
            skip_personal_marketplace,
            skip_home_codex_skills_link,
            skip_home_claude_skills_link,
            skip_home_claude_refresh,
            skip_home_claude_mcp_sync,
            skip_default_bootstrap,
        } => install_native_integration(
            &repo_root,
            &home_config_path,
            &home_plugin_root,
            &home_marketplace_path,
            &home_codex_skills_path,
            &home_claude_skills_path,
            &home_claude_refresh_path,
            &home_claude_mcp_config_path,
            bootstrap_output_dir.as_deref(),
            !skip_browser_mcp,
            !skip_framework_mcp,
            !skip_openai_developer_docs_mcp,
            !skip_framework_overlay_retirement,
            !skip_personal_plugin,
            !skip_personal_marketplace,
            !skip_home_codex_skills_link,
            !skip_home_claude_skills_link,
            !skip_home_claude_refresh,
            !skip_home_claude_mcp_sync,
            !skip_default_bootstrap,
        )?,
    };
    Ok(payload)
}

fn sync_host_entrypoints(
    template_root: &Path,
    repo_root: &Path,
    apply: bool,
) -> Result<SyncReport, String> {
    let root = normalize_path(repo_root)?;
    let template = normalize_path(template_root)?;
    let sync_manifest = load_sync_manifest(&template)?;
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = SyncReport {
        skipped_worktrees,
        ..SyncReport::default()
    };
    let mut targets = vec![root.clone()];
    targets.extend(matched_worktrees);

    for target_root in targets {
        let section = if target_root == root {
            &sync_manifest.full_sync
        } else {
            &sync_manifest.partial_sync
        };
        let single = sync_single_root(&template, &target_root, &root, apply, section)?;
        report.written.extend(single.written);
        report.unchanged.extend(single.unchanged);
        report.created_dirs.extend(single.created_dirs);
        if target_root != root {
            report
                .synced_worktrees
                .push(target_root.to_string_lossy().into_owned());
        }
    }

    report.written.sort();
    report.unchanged.sort();
    report.created_dirs.sort();
    report.synced_worktrees.sort();
    report.skipped_worktrees.sort();
    Ok(report)
}

fn sync_single_root(
    template_root: &Path,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    section: &SyncSectionManifest,
) -> Result<SingleSyncReport, String> {
    let mut report = SingleSyncReport::default();
    for relative in &section.managed_directories {
        let directory = target_root.join(relative);
        if !directory.exists() {
            if apply {
                fs::create_dir_all(&directory).map_err(|err| err.to_string())?;
            }
            report
                .created_dirs
                .push(describe_path(report_root, target_root, &directory));
        }
    }

    for relative in &section.text_files {
        sync_template_file(
            &template_root.join(relative),
            &target_root.join(relative),
            report_root,
            target_root,
            apply,
            &mut report,
        )?;
    }

    for relative in &section.json_files {
        sync_template_file(
            &template_root.join(relative),
            &target_root.join(relative),
            report_root,
            target_root,
            apply,
            &mut report,
        )?;
    }
    for relative in &section.retired_paths {
        let path = target_root.join(relative);
        let exists = path.exists() || symlink_exists(&path);
        if exists && apply {
            remove_path(&path).map_err(|err| err.to_string())?;
        }
        if exists {
            report
                .written
                .push(describe_path(report_root, target_root, &path));
        }
    }

    Ok(report)
}

fn load_sync_manifest(template_root: &Path) -> Result<SyncManifest, String> {
    let manifest_path = template_root.join(HOST_ENTRYPOINT_SYNC_MANIFEST_PATH);
    let payload = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "failed to read host-entrypoint sync manifest {}: {}",
            manifest_path.to_string_lossy(),
            err
        )
    })?;
    serde_json::from_str(&payload).map_err(|err| {
        format!(
            "failed to parse host-entrypoint sync manifest {}: {}",
            manifest_path.to_string_lossy(),
            err
        )
    })
}

fn sync_template_file(
    source: &Path,
    destination: &Path,
    report_root: &Path,
    target_root: &Path,
    apply: bool,
    report: &mut SingleSyncReport,
) -> Result<(), String> {
    let desired = fs::read(source).map_err(|err| err.to_string())?;
    let existing = fs::read(destination).ok();
    let changed = existing.as_ref() != Some(&desired);
    if changed && apply {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(destination, desired).map_err(|err| err.to_string())?;
    }
    let bucket = if changed {
        &mut report.written
    } else {
        &mut report.unchanged
    };
    bucket.push(describe_path(report_root, target_root, destination));
    Ok(())
}

fn discover_matching_worktrees(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let worktree_listing = read_git_stdout(root, &["worktree", "list", "--porcelain"]);
    if worktree_listing.is_none() {
        return (Vec::new(), Vec::new());
    }

    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut worktrees: Vec<BTreeMap<String, String>> = Vec::new();
    for raw_line in worktree_listing.unwrap_or_default().lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                worktrees.push(current);
                current = BTreeMap::new();
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default().to_string();
        let value = parts.next().unwrap_or_default().to_string();
        current.insert(key, value);
    }
    if !current.is_empty() {
        worktrees.push(current);
    }

    let mut matches = Vec::new();
    let mut skipped = Vec::new();
    for entry in worktrees {
        let Some(worktree_path) = entry.get("worktree") else {
            continue;
        };
        let candidate = normalize_path(Path::new(worktree_path))
            .unwrap_or_else(|_| PathBuf::from(worktree_path));
        if candidate == root {
            continue;
        }
        if !candidate.exists() {
            skipped.push(format!("{} (missing)", candidate.to_string_lossy()));
            continue;
        }
        matches.push(candidate);
    }
    (matches, skipped)
}

fn read_git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn runtime_registry_path(repo_root: &Path) -> PathBuf {
    let repo_candidate = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if repo_candidate.is_file() {
        return repo_candidate;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/framework/RUNTIME_REGISTRY.json")
}

fn load_runtime_registry_payload(repo_root: &Path) -> Result<Value, String> {
    let path = runtime_registry_path(repo_root);
    let payload = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let parsed = serde_json::from_str::<Value>(&payload).map_err(|err| err.to_string())?;
    let schema_version = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "Runtime registry missing schema_version at {}",
                path.to_string_lossy()
            )
        })?;
    if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported runtime registry schema_version {:?} at {}",
            schema_version,
            path.to_string_lossy()
        ));
    }
    Ok(parsed)
}

fn load_runtime_registry(repo_root: &Path) -> Result<RuntimeRegistry, String> {
    let payload = load_runtime_registry_payload(repo_root)?;
    serde_json::from_value::<RuntimeRegistry>(payload).map_err(|err| err.to_string())
}

fn primary_plugin_registration(repo_root: &Path) -> Result<RuntimePluginRegistration, String> {
    let registry = load_runtime_registry(repo_root)?;
    registry
        .plugins
        .into_iter()
        .next()
        .ok_or_else(|| "Runtime registry must define at least one plugin.".to_string())
}

fn skill_bridge_source_rel(repo_root: &Path) -> Result<String, String> {
    let registry = load_runtime_registry(repo_root)?;
    Ok(registry
        .workspace_bootstrap_defaults
        .skill_bridge
        .source_rel
        .unwrap_or_else(|| "skills".to_string()))
}

fn shared_project_mcp_servers(repo_root: &Path) -> Result<Vec<String>, String> {
    let registry = load_runtime_registry(repo_root)?;
    if registry.shared_project_mcp_servers.is_empty() {
        return Ok(DEFAULT_SHARED_PROJECT_MCP_SERVERS
            .iter()
            .map(|server| server.to_string())
            .collect());
    }
    Ok(registry.shared_project_mcp_servers)
}

fn router_rs_crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../router-rs")
}

fn router_rs_binary_candidates() -> Vec<PathBuf> {
    let crate_root = router_rs_crate_root();
    vec![
        crate_root.join("target/release/router-rs"),
        crate_root.join("target/debug/router-rs"),
    ]
}

fn run_router_rs_json(repo_root: &Path, args: &[String]) -> Result<Value, String> {
    for candidate in router_rs_binary_candidates() {
        if !candidate.is_file() {
            continue;
        }
        let output = Command::new(&candidate)
            .args(args)
            .arg("--repo-root")
            .arg(repo_root)
            .output()
            .map_err(|err| err.to_string())?;
        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
            return serde_json::from_str(stdout.trim()).map_err(|err| err.to_string());
        }
    }

    let crate_root = router_rs_crate_root();
    let manifest_path = crate_root.join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--release")
        .arg("--")
        .args(args)
        .arg("--repo-root")
        .arg(repo_root)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
        return serde_json::from_str(stdout.trim()).map_err(|err| err.to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        "missing required router-rs binary and cargo fallback failed".to_string()
    } else {
        stderr
    })
}

fn load_claude_refresh_command_text(repo_root: &Path) -> Result<String, String> {
    let args = vec!["--claude-hook-projection-json".to_string()];
    let payload = run_router_rs_json(repo_root, &args)?;
    payload
        .get("claude_commands")
        .and_then(Value::as_object)
        .and_then(|commands| commands.get("refresh"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "router-rs hook projection missing claude_commands.refresh".to_string())
}

fn describe_path(report_root: &Path, target_root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(report_root) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(target_root) {
        return format!(
            "{}::{}",
            target_root.to_string_lossy(),
            relative.to_string_lossy()
        );
    }
    path.to_string_lossy().into_owned()
}

fn install_native_integration(
    repo_root: &Path,
    home_config_path: &Path,
    home_plugin_root: &Path,
    home_marketplace_path: &Path,
    home_codex_skills_path: &Path,
    home_claude_skills_path: &Path,
    home_claude_refresh_path: &Path,
    home_claude_mcp_config_path: &Path,
    bootstrap_output_dir: Option<&Path>,
    install_browser_mcp: bool,
    install_framework_mcp: bool,
    install_openai_developer_docs_mcp: bool,
    retire_framework_overlay_file: bool,
    install_personal_plugin: bool,
    install_personal_marketplace_entry: bool,
    install_home_codex_skills_link: bool,
    install_home_claude_skills_link: bool,
    install_home_claude_refresh_command: bool,
    install_home_claude_mcp_sync: bool,
    install_default_bootstrap: bool,
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let plugin_registration = primary_plugin_registration(&repo_root)?;
    let plugin_name = plugin_registration
        .marketplace_name
        .clone()
        .unwrap_or_else(|| plugin_registration.plugin_name.clone());
    let plugin_category = plugin_registration
        .marketplace_category
        .clone()
        .unwrap_or_else(|| "Developer Tools".to_string());
    let home_config_path = normalize_path(home_config_path)?;
    let home_plugin_root = normalize_path(home_plugin_root)?;
    let home_marketplace_path = normalize_path(home_marketplace_path)?;
    let home_codex_skills_path = normalize_path(home_codex_skills_path)?;
    let home_claude_skills_path = normalize_path(home_claude_skills_path)?;
    let home_claude_refresh_path = normalize_path(home_claude_refresh_path)?;
    let home_claude_mcp_config_path = normalize_path(home_claude_mcp_config_path)?;
    let bootstrap_output_dir = bootstrap_output_dir.map(normalize_path).transpose()?;
    let claude_refresh_command = load_claude_refresh_command_text(&repo_root)?;

    let created_config = ensure_config_file(&home_config_path)?;
    let codex_hooks_feature_changed = ensure_codex_hooks_feature(&home_config_path)?;
    let browser_changed = if install_browser_mcp {
        install_mcp_block(
            &home_config_path,
            "[mcp_servers.browser-mcp]",
            &build_browser_server_block(&repo_root),
        )?
    } else {
        false
    };
    let framework_changed = if install_framework_mcp {
        install_mcp_block(
            &home_config_path,
            "[mcp_servers.framework-mcp]",
            &build_framework_server_block(&repo_root),
        )?
    } else {
        false
    };
    let openai_developer_docs_changed = if install_openai_developer_docs_mcp {
        install_mcp_block(
            &home_config_path,
            "[mcp_servers.openaiDeveloperDocs]",
            &build_openai_developer_docs_server_block(),
        )?
    } else {
        false
    };
    let tui_changed = ensure_tui_status_line(&home_config_path)?;
    let personal_plugin_changed = if install_personal_plugin {
        ensure_personal_plugin_live_projection(
            &repo_root,
            &repo_root.join(&plugin_registration.source_rel),
            &home_plugin_root,
        )?
    } else {
        false
    };
    let personal_marketplace_changed = if install_personal_marketplace_entry {
        install_personal_marketplace(
            &home_marketplace_path,
            &home_plugin_root,
            &plugin_name,
            &plugin_category,
        )?
    } else {
        false
    };
    let home_codex_skills_link_changed = if install_home_codex_skills_link {
        ensure_home_skills_link(&repo_root, &home_codex_skills_path)?
    } else {
        false
    };
    let home_claude_skills_link_changed = if install_home_claude_skills_link {
        ensure_home_skills_link(&repo_root, &home_claude_skills_path)?
    } else {
        retire_home_skills_link(&repo_root, &home_claude_skills_path)?
    };
    let home_claude_refresh_changed = if install_home_claude_refresh_command {
        ensure_home_claude_refresh_command(&claude_refresh_command, &home_claude_refresh_path)?
    } else {
        retire_home_claude_refresh_command(&claude_refresh_command, &home_claude_refresh_path)?
    };
    let home_claude_mcp_config_changed = if install_home_claude_mcp_sync {
        ensure_home_claude_mcp_servers(&repo_root, &home_claude_mcp_config_path)?
    } else {
        false
    };
    let framework_overlay_result = if retire_framework_overlay_file {
        retire_overlay(&repo_root.join(RETIRED_CODEX_MODEL_INSTRUCTIONS_PATH))?
    } else {
        Value::Null
    };
    let default_bootstrap = if install_default_bootstrap {
        ensure_default_bootstrap(&repo_root, bootstrap_output_dir.as_deref())?
    } else {
        Value::Null
    };

    Ok(json!({
        "success": true,
        "repo_root": repo_root.to_string_lossy(),
        "home_config_path": home_config_path.to_string_lossy(),
        "home_plugin_root": home_plugin_root.to_string_lossy(),
        "home_marketplace_path": home_marketplace_path.to_string_lossy(),
        "home_codex_skills_path": home_codex_skills_path.to_string_lossy(),
        "home_claude_skills_path": home_claude_skills_path.to_string_lossy(),
        "home_claude_refresh_path": home_claude_refresh_path.to_string_lossy(),
        "home_claude_mcp_config_path": home_claude_mcp_config_path.to_string_lossy(),
        "repo_marketplace_path": repo_root.join(".agents/plugins/marketplace.json").to_string_lossy(),
        "created_config": created_config,
        "codex_hooks_feature_changed": codex_hooks_feature_changed,
        "browser_mcp_changed": browser_changed,
        "framework_mcp_changed": framework_changed,
        "openai_developer_docs_mcp_changed": openai_developer_docs_changed,
        "tui_status_line_changed": tui_changed,
        "personal_plugin_changed": personal_plugin_changed,
        "personal_marketplace_changed": personal_marketplace_changed,
        "home_codex_skills_link_changed": home_codex_skills_link_changed,
        "home_claude_skills_link_changed": home_claude_skills_link_changed,
        "home_claude_refresh_changed": home_claude_refresh_changed,
        "home_claude_mcp_config_changed": home_claude_mcp_config_changed,
        "framework_overlay_retirement": framework_overlay_result,
        "default_bootstrap": default_bootstrap,
    }))
}

fn default_bootstrap_output_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("artifacts").join("bootstrap")
}

fn default_bootstrap_mirror_path(output_dir: &Path) -> PathBuf {
    output_dir.join("framework_default_bootstrap.json")
}

fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn safe_slug(label: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in label.chars().flat_map(|ch| ch.to_lowercase()) {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '\\' | '.') {
            Some('-')
        } else {
            None
        };
        if let Some(value) = normalized {
            if value == '-' {
                if slug.is_empty() || previous_dash {
                    continue;
                }
                previous_dash = true;
                slug.push(value);
            } else {
                previous_dash = false;
                slug.push(value);
            }
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed
    }
}

fn build_framework_task_id(label: &str) -> String {
    let stamp = current_local_timestamp()
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .collect::<String>();
    let slug = safe_slug(label);
    if stamp.is_empty() {
        slug
    } else {
        let suffix = if stamp.len() > 14 {
            &stamp[stamp.len() - 14..]
        } else {
            &stamp
        };
        format!("{slug}-{suffix}")
    }
}

fn compact_evolution_proposals(payload: &Value) -> Value {
    json!({
        "proposal_count": payload.get("proposal_count").and_then(Value::as_u64).unwrap_or(0),
        "proposals": payload
            .get("proposals")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    })
}

fn build_default_bootstrap_payload(
    repo_root: &Path,
    output_dir: Option<&Path>,
    query: &str,
    memory_root: Option<&Path>,
    artifact_source_dir: Option<&Path>,
    workspace_override: Option<&str>,
    top: usize,
) -> Result<Value, String> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let resolved_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_bootstrap_output_dir(&repo_root));
    fs::create_dir_all(&resolved_output_dir).map_err(|err| err.to_string())?;
    let mut memory_args = vec![
        "--framework-memory-recall-json".to_string(),
        "--framework-memory-mode".to_string(),
        "active".to_string(),
        "--limit".to_string(),
        top.to_string(),
    ];
    if !query.trim().is_empty() {
        memory_args.push("--query".to_string());
        memory_args.push(query.to_string());
    }
    if let Some(path) = memory_root {
        memory_args.push("--framework-memory-root".to_string());
        memory_args.push(path.to_string_lossy().into_owned());
    }
    if let Some(path) = artifact_source_dir {
        memory_args.push("--framework-artifact-source-dir".to_string());
        memory_args.push(path.to_string_lossy().into_owned());
    }
    let memory = run_router_rs_json(&repo_root, &memory_args)?;
    let memory_recall = memory
        .get("memory_recall")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "router-rs memory recall payload missing memory_recall object".to_string()
        })?;
    let prompt_payload = memory_recall
        .get("prompt_payload")
        .cloned()
        .ok_or_else(|| "router-rs memory recall payload missing prompt_payload".to_string())?;
    let continuity_decision = prompt_payload
        .get("continuity_decision")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let workspace = workspace_override
        .map(str::to_owned)
        .or_else(|| {
            prompt_payload
                .get("workspace")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| workspace_name_from_root(&repo_root));
    let created_at = current_local_timestamp();
    let task_id = continuity_decision
        .get("task_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            build_framework_task_id(if query.trim().is_empty() {
                &workspace
            } else {
                query
            })
        });
    let runtime = json!({
        "skills": [],
        "count": 0,
        "source": "skills/SKILL_ROUTING_RUNTIME.json",
    });
    let proposals = compact_evolution_proposals(&json!({
        "proposal_count": 0,
        "proposals": [],
    }));
    let payload = json!({
        "skills-export": runtime,
        "memory-bootstrap": prompt_payload,
        "evolution-proposals": proposals,
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": repo_root.to_string_lossy(),
            "task_id": task_id,
            "created_at": created_at,
            "source_task": continuity_decision.get("source_task").cloned().unwrap_or(Value::Null),
            "query_matches_active_task": continuity_decision
                .get("query_matches_active_task")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "ignored_root_continuity": continuity_decision
                .get("ignored_root_continuity")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    });
    let task_output_dir = resolved_output_dir.join(&task_id);
    fs::create_dir_all(&task_output_dir).map_err(|err| err.to_string())?;
    let bootstrap_path = task_output_dir.join("framework_default_bootstrap.json");
    let mirror_bootstrap_path = default_bootstrap_mirror_path(&resolved_output_dir);
    write_json_if_changed(&bootstrap_path, &payload)?;
    write_json_if_changed(&mirror_bootstrap_path, &payload)?;
    Ok(json!({
        "bootstrap_path": bootstrap_path.to_string_lossy(),
        "paths": {
            "output_dir": resolved_output_dir.to_string_lossy(),
            "task_output_dir": task_output_dir.to_string_lossy(),
            "repo_root": repo_root.to_string_lossy(),
            "memory_root": memory_recall
                .get("memory_root")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        },
        "memory_items": memory_recall
            .get("retrieval")
            .and_then(Value::as_object)
            .and_then(|retrieval| retrieval.get("items"))
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0),
        "proposal_count": payload
            .get("evolution-proposals")
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("proposal_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "payload": payload,
    }))
}

fn run_memory_automation(
    repo_root: &Path,
    output_dir: Option<&Path>,
    memory_root: Option<&Path>,
    artifact_source_dir: Option<&Path>,
    workspace_override: Option<&str>,
    query: &str,
    top: usize,
    apply_artifact_migrations: bool,
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let resolved_memory_root = memory_root
        .map(normalize_path)
        .transpose()?
        .unwrap_or_else(|| repo_root.join(".codex").join("memory"));
    let resolved_artifact_source_dir = artifact_source_dir.map(normalize_path).transpose()?;
    let workspace = workspace_override
        .map(str::to_owned)
        .unwrap_or_else(|| workspace_name_from_root(&repo_root));

    let mut runtime_args = vec!["--framework-runtime-snapshot-json".to_string()];
    if let Some(path) = resolved_artifact_source_dir.as_ref() {
        runtime_args.push("--framework-artifact-source-dir".to_string());
        runtime_args.push(path.to_string_lossy().into_owned());
    }
    let runtime_payload = run_router_rs_json(&repo_root, &runtime_args)?;
    let runtime_snapshot = runtime_payload
        .get("runtime_snapshot")
        .and_then(Value::as_object)
        .ok_or_else(|| "router-rs runtime snapshot missing runtime_snapshot object".to_string())?;
    let active_task_id = runtime_snapshot
        .get("active_task_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let planned_current_artifact_migrations = if resolved_artifact_source_dir.is_some() {
        Vec::new()
    } else {
        plan_current_artifact_clutter_migrations(&repo_root, &active_task_id)?
    };
    let planned_legacy_root_migrations = if resolved_artifact_source_dir.is_some() {
        Vec::new()
    } else {
        plan_legacy_artifact_root_migrations(&repo_root)?
    };
    let moved_current_artifacts =
        if apply_artifact_migrations && resolved_artifact_source_dir.is_none() {
            migrate_current_artifact_clutter(&repo_root, &active_task_id)?
        } else {
            Vec::new()
        };
    let moved_legacy_roots = if apply_artifact_migrations && resolved_artifact_source_dir.is_none()
    {
        migrate_legacy_artifact_roots(&repo_root)?
    } else {
        Vec::new()
    };

    let consolidation = run_router_rs_json(
        &repo_root,
        &[
            "--claude-hook-command".to_string(),
            "session-end".to_string(),
            "--claude-hook-max-lines".to_string(),
            "4".to_string(),
        ],
    )?;
    let consolidation_payload = consolidation
        .get("consolidation")
        .and_then(Value::as_object)
        .ok_or_else(|| "router-rs session-end payload missing consolidation object".to_string())?;
    let changed_files = consolidation_payload
        .get("changed_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let archive = consolidation_payload
        .get("archive")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let sqlite_result = consolidation_payload
        .get("sqlite_result")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let report = collect_storage_report(&default_codex_root(), top)?;
    let retrieval = run_framework_memory_recall(
        &repo_root,
        query,
        top,
        "stable",
        Some(&resolved_memory_root),
        resolved_artifact_source_dir.as_deref(),
    )?;

    let generated_at = current_local_timestamp();
    let run_id = build_framework_task_id(&format!("{workspace}-memory-automation"));
    let resolved_output_dir = output_dir
        .map(normalize_path)
        .transpose()?
        .unwrap_or_else(|| ops_memory_automation_root(&repo_root).join(&run_id));
    fs::create_dir_all(&resolved_output_dir).map_err(|err| err.to_string())?;

    let changed_file_list = changed_files
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let report_object = report
        .as_object()
        .ok_or_else(|| "storage report must be an object".to_string())?;
    let sqlite_object = sqlite_result
        .as_object()
        .ok_or_else(|| "sqlite_result must be an object".to_string())?;
    let archive_object = archive
        .as_object()
        .ok_or_else(|| "archive result must be an object".to_string())?;
    let snapshot_md = render_memory_automation_snapshot(
        &workspace,
        &generated_at,
        &resolved_memory_root,
        &default_codex_root(),
        report_object,
        sqlite_object,
        &changed_file_list,
        archive_object,
        &planned_current_artifact_migrations,
        &planned_legacy_root_migrations,
        apply_artifact_migrations,
    );

    write_json_if_changed(&resolved_output_dir.join("storage_audit.json"), &report)?;
    write_text_if_changed(&resolved_output_dir.join("snapshot.md"), &snapshot_md)?;
    write_json_if_changed(
        &resolved_output_dir.join("snapshot.json"),
        &json!({
            "workspace": workspace,
            "generated_at": generated_at,
            "archive": archive,
            "changed_files": changed_files,
            "planned_current_artifact_migrations": migration_plan_values(&planned_current_artifact_migrations),
            "planned_legacy_root_migrations": migration_plan_values(&planned_legacy_root_migrations),
            "moved_current_artifacts": moved_current_artifacts,
            "moved_legacy_roots": moved_legacy_roots,
            "retrieval": retrieval,
            "apply_artifact_migrations": apply_artifact_migrations,
        }),
    )?;

    let bootstrap = build_default_bootstrap_payload(
        &repo_root,
        None,
        query,
        Some(&resolved_memory_root),
        resolved_artifact_source_dir.as_deref(),
        Some(&workspace),
        top,
    )?;
    let run_summary = json!({
        "workspace": workspace,
        "generated_at": generated_at,
        "run_date": current_local_date(),
        "run_id": run_id,
        "sqlite_path": sqlite_result.get("db_path").cloned().unwrap_or(Value::Null),
        "memory_root": resolved_memory_root.to_string_lossy(),
        "output_dir": resolved_output_dir.to_string_lossy(),
        "changed_files": changed_files,
        "archive": archive,
        "planned_current_artifact_migrations": migration_plan_values(&planned_current_artifact_migrations),
        "planned_legacy_root_migrations": migration_plan_values(&planned_legacy_root_migrations),
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "apply_artifact_migrations": apply_artifact_migrations,
        "sqlite_result": sqlite_result,
        "storage_total_mib": report.get("total_mib").cloned().unwrap_or(Value::Null),
        "top_storage_entries": report.get("top_entries").cloned().unwrap_or_else(|| json!([])),
        "retrieval": retrieval,
    });
    write_json_if_changed(&resolved_output_dir.join("run_summary.json"), &run_summary)?;

    Ok(json!({
        "workspace": workspace,
        "memory_root": resolved_memory_root.to_string_lossy(),
        "changed_files": changed_files,
        "archive": archive,
        "planned_current_artifact_migrations": migration_plan_values(&planned_current_artifact_migrations),
        "planned_legacy_root_migrations": migration_plan_values(&planned_legacy_root_migrations),
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "apply_artifact_migrations": apply_artifact_migrations,
        "report": report,
        "sqlite_result": sqlite_result,
        "retrieval": retrieval,
        "bootstrap": bootstrap,
        "output_dir": resolved_output_dir.to_string_lossy(),
    }))
}

#[derive(Clone)]
struct MigrationPlan {
    source: String,
    destination: String,
}

fn migration_plan_values(plans: &[MigrationPlan]) -> Value {
    Value::Array(
        plans
            .iter()
            .map(|plan| {
                json!({
                    "source": plan.source,
                    "destination": plan.destination,
                })
            })
            .collect(),
    )
}

fn current_local_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn default_codex_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
}

fn ops_memory_automation_root(repo_root: &Path) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("ops")
        .join("memory_automation")
}

fn evidence_artifact_root(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("evidence");
    task_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

fn scratch_artifact_root(repo_root: &Path, run_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("scratch");
    run_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

fn render_memory_automation_snapshot(
    workspace: &str,
    generated_at: &str,
    memory_root: &Path,
    storage_root: &Path,
    report: &Map<String, Value>,
    sqlite_result: &Map<String, Value>,
    changed_files: &[String],
    archive_result: &Map<String, Value>,
    planned_current_artifact_migrations: &[MigrationPlan],
    planned_legacy_root_migrations: &[MigrationPlan],
    apply_artifact_migrations: bool,
) -> String {
    let mut lines = vec![
        "# CLI-common memory automation pipeline".to_string(),
        "".to_string(),
        format!("- workspace: {workspace}"),
        format!("- generated_at: {generated_at}"),
        format!("- memory_root: {}", memory_root.display()),
        format!("- storage_root: {}", storage_root.display()),
        format!(
            "- total_mib: {}",
            report
                .get("total_mib")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        ),
        format!("- memory_changed: {}", !changed_files.is_empty()),
        format!(
            "- sqlite_path: {}",
            sqlite_result
                .get("db_path")
                .and_then(Value::as_str)
                .unwrap_or("")
        ),
        format!(
            "- sqlite_memory_items: {}",
            sqlite_result
                .get("memory_items")
                .and_then(Value::as_i64)
                .unwrap_or(0)
        ),
        format!(
            "- legacy_rows_archived: {}",
            archive_result
                .get("legacy_row_count")
                .and_then(Value::as_i64)
                .unwrap_or(0)
        ),
        format!(
            "- legacy_memory_items_archived: {}",
            archive_result
                .get("legacy_memory_item_count")
                .and_then(Value::as_i64)
                .unwrap_or(0)
        ),
        format!("- apply_artifact_migrations: {apply_artifact_migrations}"),
        format!(
            "- planned_current_artifact_migrations: {}",
            planned_current_artifact_migrations.len()
        ),
        format!(
            "- planned_legacy_root_migrations: {}",
            planned_legacy_root_migrations.len()
        ),
    ];
    if changed_files.is_empty() {
        lines.push("- changed_files: none".to_string());
    } else {
        lines.push("- changed_files:".to_string());
        lines.extend(changed_files.iter().map(|path| format!("  - {path}")));
    }
    lines.push("".to_string());
    lines.push("## recommendations".to_string());
    lines.push("".to_string());
    let recommendations = top_storage_recommendations(report);
    if recommendations.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(recommendations.into_iter().map(|line| format!("- {line}")));
    }
    lines.join("\n") + "\n"
}

fn top_storage_recommendations(report: &Map<String, Value>) -> Vec<String> {
    report
        .get("top_entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(5)
        .filter_map(|entry| entry.get("path").and_then(Value::as_str))
        .filter_map(|path| {
            if path.contains("__pycache__") {
                Some(format!("consider pruning cache: {path}"))
            } else if path.ends_with("logs_1.sqlite") || path.ends_with("logs_2.sqlite") {
                Some(format!("rotate or compact trace database: {path}"))
            } else if path.contains("/sessions/") && path.ends_with(".jsonl") {
                Some(format!("archive or compress old session trace: {path}"))
            } else if path.contains("/tmp/arg0/") {
                Some(format!("clean stale tmp runtime wrappers: {path}"))
            } else if path.ends_with(".sqlite3") {
                Some(format!("monitor sqlite growth: {path}"))
            } else {
                None
            }
        })
        .collect()
}

fn run_framework_memory_recall(
    repo_root: &Path,
    query: &str,
    top: usize,
    mode: &str,
    memory_root: Option<&Path>,
    artifact_source_dir: Option<&Path>,
) -> Result<Value, String> {
    let mut args = vec![
        "--framework-memory-recall-json".to_string(),
        "--framework-memory-mode".to_string(),
        mode.to_string(),
        "--limit".to_string(),
        top.to_string(),
    ];
    if !query.trim().is_empty() {
        args.push("--query".to_string());
        args.push(query.to_string());
    }
    if let Some(path) = memory_root {
        args.push("--framework-memory-root".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    if let Some(path) = artifact_source_dir {
        args.push("--framework-artifact-source-dir".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    let payload = run_router_rs_json(repo_root, &args)?;
    payload
        .get("memory_recall")
        .cloned()
        .ok_or_else(|| "router-rs memory recall payload missing memory_recall object".to_string())
}

fn collect_storage_report(root: &Path, top: usize) -> Result<Value, String> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut total_bytes = 0u64;
    while let Some(path) = stack.pop() {
        if !path.exists() {
            continue;
        }
        for entry in fs::read_dir(&path).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let candidate = entry.path();
            let metadata = entry.metadata().map_err(|err| err.to_string())?;
            if metadata.is_dir() {
                stack.push(candidate);
            } else if metadata.is_file() {
                total_bytes += metadata.len();
                entries.push((metadata.len(), candidate));
            }
        }
    }
    entries.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    Ok(json!({
        "root": root.to_string_lossy(),
        "total_mib": round_mib(total_bytes),
        "top_entries": entries
            .into_iter()
            .take(top)
            .map(|(bytes, path)| {
                json!({
                    "path": path.to_string_lossy(),
                    "bytes": bytes,
                    "mib": round_mib(bytes),
                })
            })
            .collect::<Vec<_>>(),
    }))
}

fn round_mib(bytes: u64) -> f64 {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    (mib * 1000.0).round() / 1000.0
}

fn move_path(source: &Path, destination: &Path) -> Result<String, String> {
    let mut resolved_destination = destination.to_path_buf();
    if resolved_destination.exists() {
        let suffix = current_local_timestamp().replace(':', "").replace('+', "_");
        let stem = resolved_destination
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("moved");
        let extension = resolved_destination
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        resolved_destination =
            resolved_destination.with_file_name(format!("{stem}-{suffix}{extension}"));
    }
    if let Some(parent) = resolved_destination.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::rename(source, &resolved_destination).map_err(|err| err.to_string())?;
    Ok(resolved_destination.to_string_lossy().into_owned())
}

fn destination_for_current_artifact(
    repo_root: &Path,
    path: &Path,
    active_task_id: &str,
) -> Option<PathBuf> {
    let current_root = repo_root.join("artifacts").join("current");
    let task_root = current_root.join(active_task_id);
    if !path.exists()
        || (path.parent() != Some(current_root.as_path())
            && path.parent() != Some(task_root.as_path()))
    {
        return None;
    }
    if CURRENT_ALLOWED_ARTIFACT_NAMES.contains(&path.file_name()?.to_str()?)
        || path.file_name()?.to_str()? == active_task_id
    {
        return None;
    }
    if path.parent() == Some(task_root.as_path())
        && TASK_ALLOWED_ARTIFACT_NAMES.contains(&path.file_name()?.to_str()?)
    {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    if name == "framework_default_bootstrap.json" || name == "hermes_default_bootstrap.json" {
        let suffix = if path.parent() == Some(current_root.as_path()) {
            PathBuf::from(name)
        } else {
            PathBuf::from(active_task_id).join(name)
        };
        return Some(
            repo_root
                .join("artifacts")
                .join("bootstrap")
                .join("legacy-current")
                .join(suffix),
        );
    }
    if name == "run_summary.json"
        || name == "storage_audit.json"
        || name == "snapshot.json"
        || name == "snapshot.md"
    {
        let suffix = if path.parent() == Some(current_root.as_path()) {
            PathBuf::from(name)
        } else {
            PathBuf::from(active_task_id).join(name)
        };
        return Some(
            ops_memory_automation_root(repo_root)
                .join("legacy-current")
                .join(suffix),
        );
    }
    if name.starts_with("tmp-") {
        return Some(if path.parent() == Some(current_root.as_path()) {
            scratch_artifact_root(repo_root, None).join(name)
        } else {
            scratch_artifact_root(repo_root, Some("legacy-current"))
                .join(active_task_id)
                .join(name)
        });
    }
    let suffix = if path.parent() == Some(current_root.as_path()) {
        PathBuf::from(name)
    } else {
        PathBuf::from(active_task_id).join(name)
    };
    Some(evidence_artifact_root(repo_root, Some("legacy-current")).join(suffix))
}

fn plan_current_artifact_clutter_migrations(
    repo_root: &Path,
    active_task_id: &str,
) -> Result<Vec<MigrationPlan>, String> {
    let current_root = repo_root.join("artifacts").join("current");
    if !current_root.exists() {
        return Ok(Vec::new());
    }
    let mut plans = Vec::new();
    for entry in fs::read_dir(&current_root).map_err(|err| err.to_string())? {
        let path = entry.map_err(|err| err.to_string())?.path();
        if let Some(destination) =
            destination_for_current_artifact(repo_root, &path, active_task_id)
        {
            plans.push(MigrationPlan {
                source: path.to_string_lossy().into_owned(),
                destination: destination.to_string_lossy().into_owned(),
            });
        }
    }
    let task_root = current_root.join(active_task_id);
    if task_root.is_dir() {
        for entry in fs::read_dir(&task_root).map_err(|err| err.to_string())? {
            let path = entry.map_err(|err| err.to_string())?.path();
            if let Some(destination) =
                destination_for_current_artifact(repo_root, &path, active_task_id)
            {
                plans.push(MigrationPlan {
                    source: path.to_string_lossy().into_owned(),
                    destination: destination.to_string_lossy().into_owned(),
                });
            }
        }
    }
    plans.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(plans)
}

fn migrate_current_artifact_clutter(
    repo_root: &Path,
    active_task_id: &str,
) -> Result<Vec<String>, String> {
    let plans = plan_current_artifact_clutter_migrations(repo_root, active_task_id)?;
    let mut moved = Vec::new();
    for plan in plans {
        moved.push(move_path(
            Path::new(&plan.source),
            Path::new(&plan.destination),
        )?);
    }
    Ok(moved)
}

fn plan_legacy_artifact_root_migrations(repo_root: &Path) -> Result<Vec<MigrationPlan>, String> {
    let artifacts_root = repo_root.join("artifacts");
    if !artifacts_root.exists() {
        return Ok(Vec::new());
    }
    let mut plans = Vec::new();
    let legacy_memory_root = artifacts_root.join("memory_automation");
    if legacy_memory_root.exists() {
        plans.push(MigrationPlan {
            source: legacy_memory_root.to_string_lossy().into_owned(),
            destination: ops_memory_automation_root(repo_root)
                .join("legacy-root")
                .to_string_lossy()
                .into_owned(),
        });
    }
    for entry in fs::read_dir(&artifacts_root).map_err(|err| err.to_string())? {
        let path = entry.map_err(|err| err.to_string())?.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("tmp-") {
            plans.push(MigrationPlan {
                source: path.to_string_lossy().into_owned(),
                destination: scratch_artifact_root(repo_root, None)
                    .join(name)
                    .to_string_lossy()
                    .into_owned(),
            });
        }
    }
    plans.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(plans)
}

fn migrate_legacy_artifact_roots(repo_root: &Path) -> Result<Vec<String>, String> {
    let plans = plan_legacy_artifact_root_migrations(repo_root)?;
    let mut moved = Vec::new();
    for plan in plans {
        moved.push(move_path(
            Path::new(&plan.source),
            Path::new(&plan.destination),
        )?);
    }
    Ok(moved)
}

fn bootstrap_payload_matches_contract(payload: &Value, repo_root: &Path) -> bool {
    payload
        .get("bootstrap")
        .and_then(Value::as_object)
        .zip(payload.get("memory-bootstrap").and_then(Value::as_object))
        .zip(payload.get("skills-export").and_then(Value::as_object))
        .zip(
            payload
                .get("evolution-proposals")
                .and_then(Value::as_object),
        )
        .map(|(((bootstrap, _memory), skills), _proposals)| {
            bootstrap
                .get("repo_root")
                .and_then(Value::as_str)
                .map(|value| value == repo_root.to_string_lossy())
                .unwrap_or(false)
                && skills.get("source").and_then(Value::as_str)
                    == Some("skills/SKILL_ROUTING_RUNTIME.json")
        })
        .unwrap_or(false)
}

fn ensure_default_bootstrap(repo_root: &Path, output_dir: Option<&Path>) -> Result<Value, String> {
    let resolved_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_bootstrap_output_dir(repo_root));
    fs::create_dir_all(&resolved_output_dir).map_err(|err| err.to_string())?;
    let mirror_bootstrap_path = default_bootstrap_mirror_path(&resolved_output_dir);
    let had_existing_file = mirror_bootstrap_path.exists();
    let existing_payload = read_text_if_exists(&mirror_bootstrap_path)?
        .and_then(|content| serde_json::from_str::<Value>(&content).ok());
    if mirror_bootstrap_path.is_file()
        && existing_payload
            .as_ref()
            .is_some_and(|payload| bootstrap_payload_matches_contract(payload, repo_root))
    {
        return Ok(json!({
            "success": true,
            "changed": false,
            "status": "already-present",
            "output_dir": resolved_output_dir.to_string_lossy(),
            "bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        }));
    }

    let parsed = build_default_bootstrap_payload(
        repo_root,
        Some(&resolved_output_dir),
        "",
        None,
        None,
        None,
        8,
    )?;
    let output_dir_value = parsed
        .get("paths")
        .and_then(|value| value.get("output_dir"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| resolved_output_dir.to_string_lossy().into_owned());
    let mirror_bootstrap_value = parsed
        .get("paths")
        .and_then(|value| value.get("mirror_bootstrap_path"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| mirror_bootstrap_path.to_string_lossy().into_owned());
    let task_output_dir_value = parsed
        .get("paths")
        .and_then(|value| value.get("task_output_dir"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let bootstrap_path_value = parsed
        .get("bootstrap_path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let task_id_value = parsed
        .get("payload")
        .and_then(|value| value.get("bootstrap"))
        .and_then(|value| value.get("task_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(json!({
        "success": true,
        "changed": true,
        "status": if had_existing_file { "repaired-stale" } else { "materialized" },
        "output_dir": output_dir_value,
        "task_output_dir": task_output_dir_value,
        "bootstrap_path": bootstrap_path_value,
        "mirror_bootstrap_path": mirror_bootstrap_value,
        "task_id": task_id_value,
        "memory_items": parsed.get("memory_items").and_then(Value::as_u64),
        "proposal_count": parsed.get("proposal_count").and_then(Value::as_u64),
    }))
}

fn validate_default_bootstrap(bootstrap_path: &Path, repo_root: &Path) -> Result<bool, String> {
    let path = normalize_path(bootstrap_path)?;
    let repo_root = normalize_path(repo_root)?;
    let Some(content) = read_text_if_exists(&path)? else {
        return Ok(false);
    };
    let payload = serde_json::from_str::<Value>(&content).map_err(|err| err.to_string())?;
    Ok(bootstrap_payload_matches_contract(&payload, &repo_root))
}

fn ensure_config_file(config_path: &Path) -> Result<bool, String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if config_path.exists() {
        return Ok(false);
    }
    fs::write(config_path, CONFIG_SCHEMA_HEADER).map_err(|err| err.to_string())?;
    Ok(true)
}

fn build_browser_server_block(repo_root: &Path) -> String {
    format!(
        "[mcp_servers.browser-mcp]\ncommand = \"node\"\nargs = [\"{}\"]\ncwd = \"{}\"",
        repo_root
            .join("tools/browser-mcp/dist/index.js")
            .to_string_lossy(),
        repo_root.join("tools/browser-mcp").to_string_lossy(),
    )
}

fn build_framework_server_block(repo_root: &Path) -> String {
    let binary_path = repo_root
        .join("scripts")
        .join("router-rs")
        .join("target")
        .join("release")
        .join("router-rs");
    format!(
        "[mcp_servers.framework-mcp]\ncommand = \"{}\"\nargs = [\"--framework-mcp-stdio\", \"--repo-root\", \"{}\"]\ncwd = \"{}\"",
        binary_path.to_string_lossy(),
        repo_root.to_string_lossy(),
        repo_root.to_string_lossy(),
    )
}

fn build_openai_developer_docs_server_block() -> String {
    format!(
        "[mcp_servers.openaiDeveloperDocs]\nurl = \"{}\"",
        OPENAI_DEVELOPER_DOCS_MCP_URL
    )
}

fn ensure_codex_hooks_feature(config_path: &Path) -> Result<bool, String> {
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    let feature_line = "codex_hooks = true";
    if let Some((start, end)) = find_named_block_bounds(&content, "[features]") {
        let block = content[start..end].trim_end_matches('\n');
        let mut codex_hooks_found = false;
        let mut codex_hooks_needs_change = false;
        for line in block.lines() {
            if !is_named_setting(line, "codex_hooks") {
                continue;
            }
            codex_hooks_found = true;
            if line.trim() != feature_line {
                codex_hooks_needs_change = true;
            }
        }
        if !codex_hooks_found {
            codex_hooks_needs_change = true;
        }
        if !codex_hooks_needs_change {
            return Ok(false);
        }
        let mut replaced = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_named_setting(line, "codex_hooks") {
                updated_lines.push(feature_line.to_string());
                replaced = true;
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if !replaced {
            updated_lines.push(feature_line.to_string());
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        return write_text_if_changed(config_path, &updated);
    }

    let updated = if content.trim().is_empty() {
        "[features]\ncodex_hooks = true\n".to_string()
    } else {
        format!("{}\n\n[features]\ncodex_hooks = true\n", content.trim_end())
    };
    write_text_if_changed(config_path, &updated)
}

fn find_named_block_bounds(content: &str, marker: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let normalized = line.trim_end_matches('\n');
        if start.is_none() {
            if normalized == marker {
                start = Some(offset);
            }
        } else if normalized.starts_with('[') {
            return Some((start.unwrap_or(0), offset));
        }
        offset += line.len();
    }
    start.map(|value| (value, content.len()))
}

fn install_mcp_block(config_path: &Path, marker: &str, block: &str) -> Result<bool, String> {
    let content = read_text_if_exists(config_path)?;
    let existing = content.unwrap_or_default();
    if let Some((start, end)) = find_named_block_bounds(&existing, marker) {
        let current_block = existing[start..end].trim_end_matches('\n');
        if current_block == block {
            return Ok(false);
        }
        let updated = format!("{}{}\n{}", &existing[..start], block, &existing[end..]);
        return write_text_if_changed(config_path, &updated);
    }
    let updated = if existing.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", existing.trim_end())
    };
    write_text_if_changed(config_path, &updated)
}

fn ensure_tui_status_line(config_path: &Path) -> Result<bool, String> {
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    let status_line = format_status_line();
    if let Some((start, end)) = find_tui_block_bounds(&content) {
        let block = content[start..end].trim_end_matches('\n');
        let mut replaced = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_status_line(line) {
                updated_lines.push(status_line.clone());
                replaced = true;
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if !replaced {
            updated_lines.push(status_line);
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        return write_text_if_changed(config_path, &updated);
    }

    let mut updated = content.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str("[tui]\n");
    updated.push_str(&format_status_line());
    updated.push('\n');
    write_text_if_changed(config_path, &updated)
}

fn find_tui_block_bounds(content: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    for line in content.split_inclusive('\n') {
        let normalized = line.trim_end_matches('\n');
        if start.is_none() {
            if normalized == "[tui]" {
                start = Some(offset);
            }
        } else if normalized.starts_with('[') {
            return Some((start.unwrap_or(0), offset));
        }
        offset += line.len();
    }
    start.map(|value| (value, content.len()))
}

fn is_status_line(line: &str) -> bool {
    is_named_setting(line, "status_line")
}

fn is_named_setting(line: &str, key: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(key) && trimmed.contains('=')
}

fn format_status_line() -> String {
    let items = DEFAULT_TUI_STATUS_ITEMS
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("status_line = [{items}]")
}

fn sync_directory(source: &Path, destination: &Path, skip_names: &[&str]) -> Result<bool, String> {
    if !source.is_dir() {
        return Err(format!(
            "Plugin source directory not found: {}",
            source.to_string_lossy()
        ));
    }
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    let mut changed = false;

    let source_children = read_dir_map(source)?;
    let destination_children = read_dir_map(destination)?;

    for (name, stale_path) in destination_children {
        if skip_names.contains(&name.as_str()) {
            continue;
        }
        if source_children.contains_key(&name) {
            continue;
        }
        remove_path(&stale_path).map_err(|err| err.to_string())?;
        changed = true;
    }

    for (name, source_path) in source_children {
        if skip_names.contains(&name.as_str()) {
            continue;
        }
        let destination_path = destination.join(name);
        if source_path.is_dir() {
            if destination_path.exists() && !destination_path.is_dir() {
                remove_path(&destination_path).map_err(|err| err.to_string())?;
                changed = true;
            }
            changed = sync_directory(&source_path, &destination_path, skip_names)? || changed;
            continue;
        }
        let source_bytes = fs::read(&source_path).map_err(|err| err.to_string())?;
        let destination_bytes = fs::read(&destination_path).ok();
        if destination_bytes.as_ref() == Some(&source_bytes) {
            continue;
        }
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::copy(&source_path, &destination_path).map_err(|err| err.to_string())?;
        changed = true;
    }

    Ok(changed)
}

fn read_dir_map(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        entries.insert(
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        );
    }
    Ok(entries)
}

fn ensure_directory_symlink(source: &Path, target_path: &Path) -> Result<bool, String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    if symlink_exists(target_path) {
        let current_target = fs::read_link(target_path).map_err(|err| err.to_string())?;
        let resolved_target = if current_target.is_absolute() {
            current_target
        } else {
            target_path
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(current_target)
        };
        if resolved_target == source {
            return Ok(false);
        }
        remove_path(target_path).map_err(|err| err.to_string())?;
    } else if target_path.exists() {
        let backup_path = target_path.with_file_name(format!(
            "{}.bak",
            target_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("skills")
        ));
        if backup_path.exists() || symlink_exists(&backup_path) {
            remove_path(&backup_path).map_err(|err| err.to_string())?;
        }
        fs::rename(target_path, &backup_path).map_err(|err| err.to_string())?;
    }

    #[cfg(unix)]
    {
        unix_fs::symlink(source, target_path).map_err(|err| err.to_string())?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = source;
        let _ = target_path;
        Err("home codex skills link requires unix symlink support".to_string())
    }
}

fn ensure_home_skills_link(repo_root: &Path, target_path: &Path) -> Result<bool, String> {
    let source = repo_root.join(skill_bridge_source_rel(repo_root)?);
    ensure_directory_symlink(&source, target_path)
}

fn retire_home_skills_link(repo_root: &Path, target_path: &Path) -> Result<bool, String> {
    let source = repo_root
        .join(skill_bridge_source_rel(repo_root)?)
        .canonicalize()
        .map_err(|err| err.to_string())?;
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let resolved = target_path.canonicalize().map_err(|err| err.to_string())?;
    if resolved != source {
        return Ok(false);
    }
    remove_path(target_path).map_err(|err| err.to_string())?;
    Ok(true)
}

fn ensure_home_claude_refresh_command(content: &str, command_path: &Path) -> Result<bool, String> {
    write_text_if_changed(command_path, content)
}

fn retire_home_claude_refresh_command(content: &str, command_path: &Path) -> Result<bool, String> {
    let Some(existing) = read_text_if_exists(command_path)? else {
        return Ok(false);
    };
    if existing != content {
        return Ok(false);
    }
    remove_path(command_path).map_err(|err| err.to_string())?;
    Ok(true)
}

fn ensure_home_claude_mcp_servers(repo_root: &Path, config_path: &Path) -> Result<bool, String> {
    let mut payload = read_json_map_if_exists(config_path)?.unwrap_or_default();
    let mcp_value = payload
        .remove("mcpServers")
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut mcp_servers = match mcp_value {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    for server_name in shared_project_mcp_servers(repo_root)? {
        mcp_servers.insert(
            server_name.clone(),
            managed_home_claude_mcp_server(repo_root, &server_name)?,
        );
    }

    payload.insert("mcpServers".to_string(), Value::Object(mcp_servers));
    write_json_if_changed(config_path, &Value::Object(payload))
}

fn validate_marketplace_plugin(marketplace_path: &Path, plugin_name: &str) -> Result<bool, String> {
    let path = normalize_path(marketplace_path)?;
    let Some(Value::Object(payload)) = read_json_value_if_exists(&path)? else {
        return Ok(false);
    };
    let Some(Value::Array(plugins)) = payload.get("plugins") else {
        return Ok(false);
    };
    Ok(plugins.iter().any(|plugin| {
        plugin
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == plugin_name)
    }))
}

fn validate_home_claude_mcp(config_path: &Path, repo_root: &Path) -> Result<bool, String> {
    let path = normalize_path(config_path)?;
    let repo_root = normalize_path(repo_root)?;
    let Some(Value::Object(payload)) = read_json_value_if_exists(&path)? else {
        return Ok(false);
    };
    let Some(Value::Object(servers)) = payload.get("mcpServers") else {
        return Ok(false);
    };

    let browser = servers.get("browser-mcp").and_then(Value::as_object);
    let framework = servers.get("framework-mcp").and_then(Value::as_object);
    let openai_docs = servers
        .get("openaiDeveloperDocs")
        .and_then(Value::as_object);
    let Some(browser) = browser else {
        return Ok(false);
    };
    let Some(framework) = framework else {
        return Ok(false);
    };
    let Some(openai_docs) = openai_docs else {
        return Ok(false);
    };

    let expected_browser_command = "node";
    let expected_browser_cwd = repo_root.join("tools/browser-mcp").to_string_lossy().into_owned();
    let expected_browser_entrypoint = repo_root
        .join("tools/browser-mcp/dist/index.js")
        .to_string_lossy()
        .into_owned();
    let browser_ok = browser.get("command").and_then(Value::as_str)
        == Some(expected_browser_command)
        && browser.get("cwd").and_then(Value::as_str) == Some(expected_browser_cwd.as_str())
        && browser
            .get("args")
            .and_then(Value::as_array)
            .is_some_and(|args| args == &vec![Value::String(expected_browser_entrypoint.clone())])
        && browser
            .get("env")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty);
    let expected_framework_command = repo_root
        .join("scripts/router-rs/target/release/router-rs")
        .to_string_lossy()
        .into_owned();
    let framework_ok = framework.get("command").and_then(Value::as_str)
        == Some(expected_framework_command.as_str())
        && framework.get("cwd").and_then(Value::as_str)
            == Some(repo_root.to_string_lossy().as_ref())
        && framework
            .get("args")
            .and_then(Value::as_array)
            .is_some_and(|args| {
                args == &vec![
                    Value::String("--framework-mcp-stdio".to_string()),
                    Value::String("--repo-root".to_string()),
                    Value::String(repo_root.to_string_lossy().into_owned()),
                ]
            })
        && framework
            .get("env")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty);
    let openai_docs_ok = openai_docs.get("type").and_then(Value::as_str) == Some("http")
        && openai_docs.get("url").and_then(Value::as_str) == Some(OPENAI_DEVELOPER_DOCS_MCP_URL);

    Ok(browser_ok && framework_ok && openai_docs_ok)
}

fn validate_personal_plugin_mcp(config_path: &Path, repo_root: &Path) -> Result<bool, String> {
    let path = normalize_path(config_path)?;
    let repo_root = normalize_path(repo_root)?;
    let Some(payload) = read_json_value_if_exists(&path)? else {
        return Ok(false);
    };
    Ok(payload == build_personal_plugin_mcp_payload(&repo_root))
}

fn managed_home_claude_mcp_server(repo_root: &Path, server_name: &str) -> Result<Value, String> {
    let repo_root_value = repo_root.to_string_lossy().into_owned();
    match server_name {
        "browser-mcp" => Ok(json!({
            "type": "stdio",
            "command": "node",
            "args": [repo_root.join("tools").join("browser-mcp").join("dist").join("index.js").to_string_lossy()],
            "cwd": repo_root.join("tools").join("browser-mcp").to_string_lossy(),
            "env": {},
        })),
        "framework-mcp" => Ok(json!({
            "type": "stdio",
            "command": repo_root.join("scripts").join("router-rs").join("target").join("release").join("router-rs").to_string_lossy(),
            "args": ["--framework-mcp-stdio", "--repo-root", repo_root_value],
            "cwd": repo_root_value,
            "env": {},
        })),
        "openaiDeveloperDocs" => Ok(json!({
            "type": "http",
            "url": OPENAI_DEVELOPER_DOCS_MCP_URL,
        })),
        other => Err(format!(
            "Unsupported shared project MCP server for Claude global sync: {other}"
        )),
    }
}

fn build_personal_plugin_mcp_payload(repo_root: &Path) -> Value {
    let repo_root_value = repo_root.to_string_lossy().into_owned();
    let browser_entrypoint = repo_root
        .join("tools/browser-mcp/dist/index.js")
        .to_string_lossy()
        .into_owned();
    json!({
        "mcpServers": {
            "framework-mcp": {
                "command": repo_root.join("scripts").join("router-rs").join("target").join("release").join("router-rs").to_string_lossy(),
                "args": ["--framework-mcp-stdio", "--repo-root", repo_root_value],
                "cwd": repo_root_value,
            },
            "browser-mcp": {
                "command": "node",
                "args": [browser_entrypoint],
                "cwd": repo_root.join("tools").join("browser-mcp").to_string_lossy(),
            },
            "openaiDeveloperDocs": {
                "type": "http",
                "url": OPENAI_DEVELOPER_DOCS_MCP_URL,
            },
        }
    })
}

fn ensure_personal_plugin_live_projection(
    repo_root: &Path,
    plugin_source: &Path,
    plugin_root: &Path,
) -> Result<bool, String> {
    let mut changed = sync_directory(
        plugin_source,
        plugin_root,
        &PERSONAL_PLUGIN_LIVE_PROJECTION_EXCLUDES,
    )?;
    changed = ensure_directory_symlink(
        &repo_root.join(skill_bridge_source_rel(repo_root)?),
        &plugin_root.join("skills"),
    )? || changed;
    changed = write_json_if_changed(
        &plugin_root.join(".mcp.json"),
        &build_personal_plugin_mcp_payload(repo_root),
    )? || changed;
    Ok(changed)
}

fn install_personal_marketplace(
    marketplace_path: &Path,
    plugin_root: &Path,
    plugin_name: &str,
    plugin_category: &str,
) -> Result<bool, String> {
    let existing = read_json_map_if_exists(marketplace_path)?;
    let relative_base = marketplace_root(marketplace_path)?;
    let payload = build_personal_marketplace_payload(
        plugin_root,
        &relative_base,
        existing,
        plugin_name,
        plugin_category,
    )?;
    write_json_if_changed(marketplace_path, &Value::Object(payload))
}

fn marketplace_root(path: &Path) -> Result<PathBuf, String> {
    let absolute = normalize_path(path)?;
    absolute
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "Could not derive marketplace root from {}",
                absolute.to_string_lossy()
            )
        })
}

fn build_personal_marketplace_payload(
    plugin_root: &Path,
    marketplace_root: &Path,
    existing: Option<Map<String, Value>>,
    plugin_name: &str,
    plugin_category: &str,
) -> Result<Map<String, Value>, String> {
    let mut payload = existing.unwrap_or_default();
    let plugin_relative = plugin_root
        .strip_prefix(marketplace_root)
        .map_err(|_| {
            format!(
                "Plugin root {} is not under marketplace root {}",
                plugin_root.to_string_lossy(),
                marketplace_root.to_string_lossy()
            )
        })?
        .to_string_lossy()
        .into_owned();
    let plugin_path = format!("./{plugin_relative}");

    payload
        .entry("name".to_string())
        .or_insert_with(|| Value::String("skill-personal-marketplace".to_string()));

    let interface_value = payload
        .remove("interface")
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut interface = match interface_value {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    if !interface.contains_key("displayName") {
        interface.insert(
            "displayName".to_string(),
            Value::String("Skill Personal Marketplace".to_string()),
        );
    }
    payload.insert("interface".to_string(), Value::Object(interface));

    let plugins_value = payload
        .remove("plugins")
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let existing_plugins = match plugins_value {
        Value::Array(items) => items,
        _ => Vec::new(),
    };

    let mut updated_plugins = Vec::new();
    let mut replaced = false;
    for row in existing_plugins {
        let Value::Object(mut row_map) = row else {
            continue;
        };
        if row_map.get("name").and_then(Value::as_str) != Some(plugin_name) {
            updated_plugins.push(Value::Object(row_map));
            continue;
        }
        replaced = true;
        let category = row_map
            .remove("category")
            .unwrap_or_else(|| Value::String(plugin_category.to_string()));
        updated_plugins.push(plugin_marketplace_row(plugin_name, &plugin_path, category));
    }
    if !replaced {
        updated_plugins.push(plugin_marketplace_row(
            plugin_name,
            &plugin_path,
            Value::String(plugin_category.to_string()),
        ));
    }
    payload.insert("plugins".to_string(), Value::Array(updated_plugins));
    Ok(payload)
}

fn plugin_marketplace_row(plugin_name: &str, plugin_path: &str, category: Value) -> Value {
    json!({
        "name": plugin_name,
        "source": {"source": "local", "path": plugin_path},
        "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
        "category": category,
    })
}

fn retire_overlay(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({
            "success": true,
            "path": path.to_string_lossy(),
            "changed": false,
            "status": "already-retired",
            "retirement_mode": "missing",
        }));
    }
    let original = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let stripped = strip_managed_block(&original).trim().to_string();
    if !stripped.is_empty() {
        let updated = format!("{stripped}\n");
        let changed = updated != original;
        if changed {
            fs::write(path, updated).map_err(|err| err.to_string())?;
        }
        return Ok(json!({
            "success": true,
            "path": path.to_string_lossy(),
            "changed": changed,
            "status": "retired-managed-block",
            "retirement_mode": "preserved-user-content",
        }));
    }
    remove_path(path).map_err(|err| err.to_string())?;
    Ok(json!({
        "success": true,
        "path": path.to_string_lossy(),
        "changed": true,
        "status": "retired-file",
        "retirement_mode": "deleted-empty-overlay",
    }))
}

fn strip_managed_block(text: &str) -> String {
    let start = text.find(FRAMEWORK_START_MARKER);
    let end = text.find(FRAMEWORK_END_MARKER);
    match (start, end) {
        (Some(start_index), Some(end_index)) => {
            let after = &text[end_index + FRAMEWORK_END_MARKER.len()..];
            let merged = format!("{}{}", &text[..start_index], after);
            let trimmed = merged.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{trimmed}\n")
            }
        }
        _ => text.to_string(),
    }
}

fn read_text_if_exists(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path)?;
    if existing.as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, content).map_err(|err| err.to_string())?;
    Ok(true)
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let formatted = format!(
        "{}\n",
        serde_json::to_string_pretty(payload).map_err(|err| err.to_string())?
    );
    write_text_if_changed(path, &formatted)
}

fn read_json_map_if_exists(path: &Path) -> Result<Option<Map<String, Value>>, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    let parsed: Value = serde_json::from_str(&content).map_err(|err| err.to_string())?;
    match parsed {
        Value::Object(map) => Ok(Some(map)),
        _ => Ok(None),
    }
}

fn read_json_value_if_exists(path: &Path) -> Result<Option<Value>, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn remove_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn symlink_exists(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}
