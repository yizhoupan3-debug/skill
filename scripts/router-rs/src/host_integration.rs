use crate::framework_runtime::is_framework_root;
use chrono::Local;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CONFIG_SCHEMA_HEADER: &str =
    "#:schema https://developers.openai.com/codex/config-schema.json\n";
const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v1";
const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "fast-mode",
    "context-remaining",
    "git-branch",
];
const INSTALL_SKILLS_TOOLS: [&str; 2] = ["codex", "cursor"];
const CODEX_SKILL_SURFACE_REL: &str = "artifacts/codex-skill-surface/skills";
const CODEX_SKILL_SURFACE_MANIFEST_NAME: &str = ".codex-skill-surface.json";
const FRAMEWORK_PROJECTION_SCHEMA_VERSION: &str = "framework-host-projection-v1";
const GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION: &str =
    "framework-generated-artifacts-manifest-v1";
const GENERATED_ARTIFACT_GENERATOR_TIMEOUT: Duration = Duration::from_secs(120);
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
const HOST_SKILL_SURFACE_PINNED_SKILLS: [&str; 4] = ["autopilot", "deepinterview", "gitx", "team"];
const REQUIRED_GENERATED_ARTIFACTS: [&str; 15] = [
    "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
    "skills/SKILL_ROUTING_REGISTRY.md",
    "skills/SKILL_ROUTING_INDEX.md",
    "skills/SKILL_MANIFEST.json",
    "skills/SKILL_ROUTING_RUNTIME.json",
    "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
    "skills/SKILL_PLUGIN_CATALOG.json",
    "skills/SKILL_ROUTING_METADATA.json",
    "skills/SKILL_HEALTH_MANIFEST.json",
    "skills/SKILL_SHADOW_MAP.json",
    "skills/SKILL_APPROVAL_POLICY.json",
    "skills/SKILL_LOADOUTS.json",
    "skills/SKILL_TIERS.json",
    "AGENTS.md",
    ".codex/host_entrypoints_sync_manifest.json",
];
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
struct RuntimeRegistry {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    #[serde(default)]
    workspace_bootstrap_defaults: RuntimeWorkspaceBootstrapDefaults,
}

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

#[derive(Debug, Clone, Deserialize, Default)]
struct RuntimeWorkspaceBootstrapDefaults {
    #[serde(default)]
    skills: RuntimeSkillsDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RuntimeSkillsDefaults {
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
    },
}

#[derive(clap::Args, Debug, Clone)]
struct ProjectionCommand {
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
    home: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_PROJECT_SCOPE)]
    scope: String,
    #[arg(long)]
    to: Vec<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(clap::Args, Debug, Clone)]
struct ProjectionStatusCommand {
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
    home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ResolvedProjectionRoots {
    framework_root: PathBuf,
    project_root: PathBuf,
    artifact_root: PathBuf,
    codex_home_root: PathBuf,
    cursor_home_root: PathBuf,
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
        Commands::ExportRuntimeRegistry { repo_root } => {
            let framework_root = resolve_framework_root(repo_root.as_deref())?;
            serde_json::to_value(load_runtime_registry_payload(&framework_root)?)
                .map_err(|err| err.to_string())?
        }
        Commands::ResolveSkillsSource { repo_root } => json!({
            "path": normalize_path(&repo_root)?
                .join(skills_source_rel(&repo_root)?)
                .to_string_lossy(),
        }),
        Commands::ValidateDefaultBootstrap {
            bootstrap_path,
            repo_root,
        } => json!({
            "ok": validate_default_bootstrap(&bootstrap_path, &repo_root)?,
        }),
        Commands::BuildDefaultBootstrap {
            repo_root,
            output_dir,
            query,
            artifact_source_dir,
            workspace,
            top,
        } => build_default_bootstrap_payload(
            &repo_root,
            output_dir.as_deref(),
            &query,
            artifact_source_dir.as_deref(),
            workspace.as_deref(),
            top,
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
        Commands::EnsureDefaultBootstrap {
            repo_root,
            output_dir,
        } => ensure_default_bootstrap(&repo_root, output_dir.as_deref())?,
        Commands::InstallNativeIntegration {
            repo_root,
            home_config_path,
            home_codex_skills_path,
            bootstrap_output_dir,
            skip_home_codex_skills_link,
            skip_default_bootstrap,
        } => install_native_integration(
            &repo_root,
            &home_config_path,
            &home_codex_skills_path,
            bootstrap_output_dir.as_deref(),
            !skip_home_codex_skills_link,
            !skip_default_bootstrap,
        )?,
        Commands::InstallSkills {
            repo_root,
            project_root,
            artifact_root,
            home,
            codex_home,
            cursor_home,
            to,
            scope,
            bootstrap_output_dir,
            skip_default_bootstrap,
            command,
            tools,
        } => {
            let _ = (bootstrap_output_dir, skip_default_bootstrap);
            let selected = install_skills_projection_tools(&command, &tools, &to);
            let projection_command = ProjectionCommand {
                framework_root: repo_root,
                project_root,
                artifact_root,
                codex_home,
                cursor_home,
                home,
                scope,
                to: selected,
                dry_run: false,
            };
            let normalized_command = canonical_install_skills_command(&command);
            let has_selected_targets = !projection_command.to.is_empty();
            match normalized_command.as_str() {
                "status" | "ls" if !has_selected_targets => {
                    projection_status_command(ProjectionStatusCommand {
                        framework_root: projection_command.framework_root.clone(),
                        project_root: projection_command.project_root.clone(),
                        artifact_root: projection_command.artifact_root.clone(),
                        codex_home: projection_command.codex_home.clone(),
                        cursor_home: projection_command.cursor_home.clone(),
                        home: projection_command.home.clone(),
                    })?
                }
                "remove" | "rm" => projection_remove_command(projection_command, true)?,
                _ => projection_install_command(projection_command, true)?,
            }
        }
        Commands::Install(command) => projection_install_command(command, false)?,
        Commands::Status(command) => projection_status_command(command)?,
        Commands::Remove(command) => projection_remove_command(command, false)?,
        Commands::Cleanup(command) => projection_cleanup_command(command)?,
        Commands::CompatibilityAliases => compatibility_alias_inventory(),
        Commands::GeneratedArtifactsStatus {
            framework_root,
            artifact_root,
        } => generated_artifacts_status(framework_root.as_deref(), artifact_root.as_deref())?,
    };
    Ok(payload)
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

fn resolve_framework_root(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_FRAMEWORK_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
    if is_framework_root(&cwd) {
        return normalize_path(&cwd);
    }
    for ancestor in cwd.ancestors() {
        if is_framework_root(ancestor) {
            return normalize_path(ancestor);
        }
    }
    Err("missing framework_root; pass --framework-root or set SKILL_FRAMEWORK_ROOT".to_string())
}

fn resolve_projection_framework_root(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let root = resolve_framework_root(explicit)?;
    if !is_framework_root(&root) {
        return Err(format!(
            "stale or missing framework_root: {}. Repair by passing --framework-root pointing at the framework checkout containing configs/framework/RUNTIME_REGISTRY.json and scripts/router-rs/Cargo.toml",
            root.display()
        ));
    }
    Ok(root)
}

fn resolve_project_root(explicit: Option<&Path>, framework_root: &Path) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_PROJECT_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
    if let Some(git_root) = nearest_marker_root(&cwd, ".git") {
        return normalize_discovered_project_root(&git_root, framework_root);
    }
    for marker in ["AGENTS.md"] {
        if let Some(root) = nearest_marker_root(&cwd, marker) {
            return normalize_discovered_project_root(&root, framework_root);
        }
    }
    if is_framework_root(framework_root) && cwd.starts_with(framework_root) {
        return normalize_path(framework_root);
    }
    Err("missing project_root; pass --project-root or set SKILL_PROJECT_ROOT".to_string())
}

fn normalize_discovered_project_root(
    candidate: &Path,
    framework_root: &Path,
) -> Result<PathBuf, String> {
    let candidate = normalize_path(candidate)?;
    let framework_root = normalize_path(framework_root)?;
    if is_framework_root(&candidate) && candidate != framework_root {
        return Err(format!(
            "ambiguous project_root discovery: {} looks like a framework checkout but does not match framework_root {}. Pass both --framework-root and --project-root explicitly",
            candidate.display(),
            framework_root.display()
        ));
    }
    Ok(candidate)
}

fn resolve_artifact_root(
    explicit: Option<&Path>,
    framework_root: &Path,
) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_ARTIFACT_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    Ok(framework_root.join("artifacts"))
}

fn resolve_host_home(
    explicit: Option<&Path>,
    shared_home: Option<&Path>,
    env_var: &str,
    default_leaf: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os(env_var) {
        return normalize_path(&PathBuf::from(path));
    }
    if let Some(home) = shared_home {
        return Ok(normalize_path(home)?.join(default_leaf));
    }
    Ok(default_home_dir().join(default_leaf))
}

fn resolve_projection_roots(
    framework_root: Option<&Path>,
    project_root: Option<&Path>,
    artifact_root: Option<&Path>,
    codex_home: Option<&Path>,
    cursor_home: Option<&Path>,
    shared_home: Option<&Path>,
) -> Result<ResolvedProjectionRoots, String> {
    let framework_root = resolve_projection_framework_root(framework_root)?;
    let project_root = resolve_project_root(project_root, &framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    let codex_home_root = resolve_host_home(codex_home, shared_home, "CODEX_HOME", ".codex")?;
    let cursor_home_root = resolve_host_home(cursor_home, shared_home, "CURSOR_HOME", ".cursor")?;
    Ok(ResolvedProjectionRoots {
        framework_root,
        project_root,
        artifact_root,
        codex_home_root,
        cursor_home_root,
    })
}

fn nearest_marker_root(start: &Path, marker: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(marker).exists())
        .map(Path::to_path_buf)
}

fn runtime_registry_path(repo_root: &Path) -> Result<PathBuf, String> {
    let repo_candidate = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if repo_candidate.is_file() {
        return Ok(repo_candidate);
    }
    Err(format!(
        "Runtime registry not found at active workspace root: {}. Expected {}. Fix by opening the framework repo root as the active workspace or passing --framework-root <framework-repo-root>.",
        repo_root.to_string_lossy(),
        repo_candidate.to_string_lossy()
    ))
}

fn load_runtime_registry_payload(repo_root: &Path) -> Result<Value, String> {
    let path = runtime_registry_path(repo_root)?;
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

fn load_runtime_registry_payload_if_repo_local(repo_root: &Path) -> Result<Option<Value>, String> {
    let path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
    if !path.is_file() {
        return Ok(None);
    }
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
    Ok(Some(parsed))
}

fn load_runtime_registry(repo_root: &Path) -> Result<RuntimeRegistry, String> {
    let payload = load_runtime_registry_payload(repo_root)?;
    serde_json::from_value::<RuntimeRegistry>(payload).map_err(|err| err.to_string())
}

fn compatibility_alias_inventory() -> Value {
    json!({
        "schema_version": "framework-compatibility-alias-inventory-v1",
        "aliases": [
            {
                "alias": "codex host-integration ...",
                "primary_command": "framework host-integration ...",
                "owner": "host-integration",
                "reason": "backward-compatible parser path for existing Codex automation that has not moved to the host-neutral namespace",
                "independent_behavior": false,
                "kept_policy": "thin parser alias only; dispatches to the same host-integration implementation as the primary command",
                "removal_condition": "remove after all checked-in docs, tests, bootstrap snippets, and generated host entrypoints use framework host-integration",
            },
            {
                "alias": "framework host-integration install-skills",
                "primary_command": "framework host-integration install",
                "owner": "host-integration",
                "reason": "backward-compatible install command spelling for existing project-local projection setup calls",
                "independent_behavior": false,
                "kept_policy": "thin subcommand alias only; maps to install with compatibility_alias=true metadata",
                "removal_condition": "remove after project-local projection installs and docs no longer call install-skills",
            },
            {
                "alias": "--repo-root",
                "primary_command": "--framework-root",
                "owner": "root-resolution",
                "reason": "backward-compatible flag for selecting the shared framework root in old automation",
                "independent_behavior": false,
                "kept_policy": "framework-root alias only; never resolves or fills project_root",
                "removal_condition": "kept indefinitely unless all old automation migrates to --framework-root and no docs/tests/generated entrypoints reference --repo-root",
            }
        ]
    })
}

fn generated_artifacts_status(
    framework_root: Option<&Path>,
    artifact_root: Option<&Path>,
) -> Result<Value, String> {
    let framework_root = resolve_framework_root(framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    let manifest_path = framework_root.join("configs/framework/GENERATED_ARTIFACTS.json");
    let manifest = read_json_if_exists(&manifest_path)?.ok_or_else(|| {
        format!(
            "missing generated artifact manifest: {}",
            manifest_path.display()
        )
    })?;
    let manifest: GeneratedArtifactsManifest = serde_json::from_value(manifest)
        .map_err(|err| format!("invalid generated artifact manifest: {err}"))?;
    if manifest.schema_version != GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported generated artifact manifest schema_version {:?} at {}; expected {}",
            manifest.schema_version,
            manifest_path.display(),
            GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION
        ));
    }
    let temp_root_guard = prepare_generated_artifact_temp_root(&framework_root, &artifact_root)?;
    let temp_root = temp_root_guard.path();
    let mut results = Vec::new();
    let mut ok = true;
    let mut declared_paths = BTreeSet::new();
    let mut executed_generators = BTreeSet::new();

    for artifact in &manifest.generated_artifacts {
        validate_generated_artifact_entry(artifact)?;
        declared_paths.insert(artifact.path.clone());
        if executed_generators.insert(artifact.generator.clone()) {
            run_generated_artifact_generator(&artifact.generator, &framework_root, temp_root)?;
        }
        let checked_in_path = framework_root.join(&artifact.path);
        let regenerated_path = temp_root.join(&artifact.path);
        let exists = checked_in_path.is_file();
        let regenerated_exists = regenerated_path.is_file();
        let checked_in = if exists {
            Some(fs::read(&checked_in_path).map_err(|err| err.to_string())?)
        } else {
            None
        };
        let regenerated = if regenerated_exists {
            Some(fs::read(&regenerated_path).map_err(|err| err.to_string())?)
        } else {
            None
        };
        let forbidden = checked_in
            .as_ref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(|content| generated_artifact_forbidden_markers(&artifact.path, content))
            .unwrap_or_default();
        let drifted = checked_in.as_ref() != regenerated.as_ref();
        let clean = exists && regenerated_exists && !drifted && forbidden.is_empty();
        ok &= clean;
        results.push(json!({
            "path": artifact.path,
            "exists": exists,
            "regenerated_exists": regenerated_exists,
            "clean": clean,
            "drifted": drifted,
            "forbidden_markers": forbidden,
            "compare": artifact.compare,
            "generator": artifact.generator,
        }));
    }

    let undeclared = undeclared_generated_framework_artifacts(&framework_root, &declared_paths)?;
    let missing_required = missing_required_generated_artifacts(&declared_paths);
    ok &= undeclared.is_empty() && missing_required.is_empty();
    let drifted_artifacts: Vec<Value> = results
        .iter()
        .filter(|artifact| artifact.get("drifted").and_then(Value::as_bool) == Some(true))
        .map(|artifact| {
            json!({
                "path": artifact["path"].clone(),
                "generator": artifact["generator"].clone(),
                "compare": artifact["compare"].clone(),
            })
        })
        .collect();

    Ok(json!({
        "schema_version": "framework-generated-artifacts-status-v1",
        "ok": ok,
        "manifest_status": {
            "mode": "manifest-backed-byte-for-byte-drift-gate",
            "artifact_root": artifact_root.to_string_lossy(),
            "temp_root": temp_root.to_string_lossy(),
            "undeclared_generated_artifacts": undeclared,
            "missing_required_generated_artifacts": missing_required,
            "required_generated_artifacts": REQUIRED_GENERATED_ARTIFACTS,
            "drifted_artifacts": drifted_artifacts,
        },
        "drift_gate": {
            "enabled": true,
            "compare": "byte-for-byte",
            "manifest": manifest_path.to_string_lossy(),
        },
        "framework_root": framework_root.to_string_lossy(),
        "artifact_root": artifact_root.to_string_lossy(),
        "manifest": manifest_path.to_string_lossy(),
        "generated_artifacts": results,
    }))
}

fn validate_generated_artifact_entry(
    artifact: &GeneratedArtifactManifestEntry,
) -> Result<(), String> {
    if Path::new(&artifact.path).is_absolute()
        || artifact.path.contains("..")
        || (artifact.path.starts_with('.') && !allowed_dot_generated_artifact(&artifact.path))
    {
        return Err(format!(
            "generated artifact path must be repo-relative and non-traversing: {}",
            artifact.path
        ));
    }
    if artifact.compare != "byte-for-byte" {
        return Err(format!(
            "unsupported generated artifact compare mode for {}: {}",
            artifact.path, artifact.compare
        ));
    }
    Ok(())
}

fn allowed_dot_generated_artifact(path: &str) -> bool {
    path == ".codex/host_entrypoints_sync_manifest.json"
}

struct GeneratedArtifactTempRoot {
    path: PathBuf,
}

impl GeneratedArtifactTempRoot {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for GeneratedArtifactTempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

fn prepare_generated_artifact_temp_root(
    framework_root: &Path,
    artifact_root: &Path,
) -> Result<GeneratedArtifactTempRoot, String> {
    let temp_root = artifact_root
        .join("generated-artifacts-drift-check")
        .join(format!(
            "run-{}",
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
    copy_framework_tree_for_generation(framework_root, &temp_root)?;
    Ok(GeneratedArtifactTempRoot { path: temp_root })
}

fn copy_framework_tree_for_generation(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|err| format!("failed to create {}: {err}", destination.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|err| format!("failed to read directory {}: {err}", source.display()))?
    {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read directory entry under {}: {err}",
                source.display()
            )
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let target = destination.join(&name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if should_skip_generated_artifact_copy_dir(&name_text) {
                continue;
            }
            copy_framework_tree_for_generation(&path, &target)?;
        } else if metadata.is_file() {
            if should_skip_generated_artifact_copy_file(&name_text) {
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
            }
            fs::copy(&path, &target).map_err(|err| {
                format!(
                    "failed to copy {} to {}: {err}",
                    path.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn should_skip_generated_artifact_copy_dir(name: &str) -> bool {
    GENERATED_ARTIFACT_COPY_SKIP_DIR_NAMES.contains(&name)
}

fn should_skip_generated_artifact_copy_file(name: &str) -> bool {
    name == ".DS_Store" || name.ends_with(".marker")
}

fn run_generated_artifact_generator(
    generator: &str,
    framework_root: &Path,
    temp_root: &Path,
) -> Result<(), String> {
    let timeout = generated_artifact_generator_timeout();
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(rewrite_generated_artifact_generator(
            generator,
            framework_root,
            temp_root,
        ))
        .current_dir(temp_root)
        .env("SKILL_FRAMEWORK_ROOT", temp_root)
        .env("SKILL_ARTIFACT_ROOT", temp_root.join("artifacts"))
        .env("ROUTER_RS_NO_REBUILD", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| err.to_string())?;
    let start = Instant::now();
    loop {
        if child.try_wait().map_err(|err| err.to_string())?.is_some() {
            break;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|err| err.to_string())?;
            return Err(format!(
                "generated artifact generator timed out after {}s: {generator}\nstdout:\n{}\nstderr:\n{}",
                timeout.as_secs(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "generated artifact generator failed: {generator}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn generated_artifact_generator_timeout() -> Duration {
    match std::env::var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
    {
        Some(0) | None => GENERATED_ARTIFACT_GENERATOR_TIMEOUT,
        Some(seconds) => Duration::from_secs(seconds),
    }
}

fn rewrite_generated_artifact_generator(
    generator: &str,
    framework_root: &Path,
    temp_root: &Path,
) -> String {
    generator
        .replace(
            &framework_root.to_string_lossy().to_string(),
            &temp_root.to_string_lossy(),
        )
        .replace(
            "./scripts/",
            &format!("{}/scripts/", temp_root.to_string_lossy()),
        )
        .replace(
            " scripts/",
            &format!(" {}/scripts/", temp_root.to_string_lossy()),
        )
}

fn generated_artifact_forbidden_markers(path: &str, content: &str) -> Vec<&'static str> {
    let mut markers = Vec::new();
    for (name, needle) in [
        ("expanded-codex-home", "/Users/joe/.codex"),
        (
            "expanded-consuming-project-root",
            "/Users/joe/Documents/skill",
        ),
        ("copied-skill-body", "# Plan To Code"),
    ] {
        if content.contains(needle) {
            markers.push(name);
        }
    }
    if !path.starts_with("skills/SKILL_") && content.contains("\"skills\":[[") {
        markers.push("copied-runtime-payload");
    }
    markers
}

fn missing_required_generated_artifacts(declared_paths: &BTreeSet<String>) -> Vec<&'static str> {
    REQUIRED_GENERATED_ARTIFACTS
        .iter()
        .copied()
        .filter(|path| !declared_paths.contains(*path))
        .collect()
}

fn undeclared_generated_framework_artifacts(
    framework_root: &Path,
    declared_paths: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let mut undeclared = Vec::new();
    let candidates = generated_artifact_reverse_reference_candidates(framework_root)?;
    for path in candidates {
        let rel = path
            .strip_prefix(framework_root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .into_owned();
        if !declared_paths.contains(&rel) {
            undeclared.push(rel);
        }
    }
    undeclared.sort();
    undeclared.dedup();
    Ok(undeclared)
}

fn generated_artifact_reverse_reference_candidates(
    framework_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    for rel in [
        "configs/framework",
        "docs",
        "tests",
        ".github/workflows",
        ".codex",
    ] {
        collect_generated_artifact_marker_files(
            framework_root,
            &framework_root.join(rel),
            &mut candidates,
        )?;
    }
    let rel = "AGENTS.md";
    collect_generated_artifact_marker_files(
        framework_root,
        &framework_root.join(rel),
        &mut candidates,
    )?;
    collect_root_skill_generated_surfaces(framework_root, &mut candidates)?;
    Ok(candidates)
}

fn collect_root_skill_generated_surfaces(
    framework_root: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let skills_root = framework_root.join("skills");
    if !skills_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&skills_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("SKILL_") {
            collect_generated_artifact_marker_files(framework_root, &path, candidates)?;
        }
    }
    Ok(())
}

fn collect_generated_artifact_marker_files(
    framework_root: &Path,
    path: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        if matches!(name, ".git" | "target" | "artifacts") {
            return Ok(());
        }
        for entry in fs::read_dir(path).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            collect_generated_artifact_marker_files(framework_root, &entry.path(), candidates)?;
        }
        return Ok(());
    }
    if !path.is_file() || !is_generated_artifact_scan_file(path) {
        return Ok(());
    }
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(());
    };
    if !content.contains("generated-by-") {
        return Ok(());
    }
    let rel = path
        .strip_prefix(framework_root)
        .map_err(|err| err.to_string())?;
    if rel.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|item| matches!(item, "target" | "artifacts"))
    }) {
        return Ok(());
    }
    candidates.push(path.to_path_buf());
    Ok(())
}

fn is_generated_artifact_scan_file(path: &Path) -> bool {
    if matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("AGENTS.md")
    ) {
        return true;
    }
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("json" | "md" | "toml" | "yaml" | "yml" | "txt")
    )
}

fn skills_source_rel(repo_root: &Path) -> Result<String, String> {
    let registry = load_runtime_registry(repo_root)?;
    let source_rel = registry
        .workspace_bootstrap_defaults
        .skills
        .source_rel
        .unwrap_or_else(|| "skills".to_string());
    validate_source_rel(&source_rel)?;
    Ok(source_rel)
}

fn validate_source_rel(source_rel: &str) -> Result<(), String> {
    let candidate = Path::new(source_rel);
    if candidate.as_os_str().is_empty() {
        return Err("skills source_rel must not be empty".to_string());
    }
    if candidate.is_absolute() {
        return Err(format!(
            "skills source_rel must be repository-relative, got absolute path: {source_rel}"
        ));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "skills source_rel must not contain '..' segments: {source_rel}"
        ));
    }
    Ok(())
}

fn resolve_router_rs_executable(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(raw) = std::env::var("ROUTER_RS_BIN") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(path);
        }
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        let base = PathBuf::from(td);
        for candidate in [base.join("release/router-rs"), base.join("debug/router-rs")] {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    let cur = std::env::current_exe().map_err(|err| err.to_string())?;
    if cur
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "router-rs" || name.starts_with("router-rs"))
    {
        return Ok(cur);
    }
    let repo_root = normalize_path(repo_root)?;
    for candidate in [
        repo_root.join("target/release/router-rs"),
        repo_root.join("target/debug/router-rs"),
        repo_root.join("scripts/router-rs/target/release/router-rs"),
        repo_root.join("scripts/router-rs/target/debug/router-rs"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not resolve router-rs executable for subprocess (try `cargo build --release --manifest-path scripts/router-rs/Cargo.toml`, `router-rs self install`, or set ROUTER_RS_BIN); repo_root={}",
        repo_root.display()
    ))
}

fn run_router_rs_json(repo_root: &Path, args: &[String]) -> Result<Value, String> {
    let exe = resolve_router_rs_executable(repo_root)?;
    let output = Command::new(&exe)
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
        format!(
            "router-rs subprocess failed (status {:?}); executable {}",
            output.status,
            exe.display()
        )
    } else {
        stderr
    })
}

#[allow(clippy::too_many_arguments)]
fn install_native_integration(
    repo_root: &Path,
    home_config_path: &Path,
    home_codex_skills_path: &Path,
    bootstrap_output_dir: Option<&Path>,
    install_home_codex_skills_link: bool,
    install_default_bootstrap: bool,
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let home_config_path = normalize_path(home_config_path)?;
    let home_codex_skills_path = normalize_path(home_codex_skills_path)?;
    let bootstrap_output_dir = bootstrap_output_dir.map(normalize_path).transpose()?;

    let created_config = ensure_config_file(&home_config_path)?;
    let codex_hooks_disabled_changed = ensure_codex_hooks_disabled(&home_config_path)?;
    let tui_changed = ensure_tui_status_line(&home_config_path)?;
    let home_codex_dir = home_config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_home_dir().join(".codex"));
    let prompt_entrypoints = codex_prompt_entrypoints_disabled(&home_codex_dir);
    let surface = if install_home_codex_skills_link {
        Some(ensure_codex_skill_surface(&repo_root)?)
    } else {
        None
    };
    let home_codex_skills_changed = if install_home_codex_skills_link {
        ensure_codex_skills_symlink(
            &home_codex_skills_path,
            &shared_codex_skill_surface(&repo_root),
        )?
    } else {
        false
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
        "home_codex_skills_path": home_codex_skills_path.to_string_lossy(),
        "codex_skill_surface": surface.unwrap_or(Value::Null),
        "codex_prompt_entrypoints": prompt_entrypoints,
        "created_config": created_config,
        "codex_hooks_disabled_changed": codex_hooks_disabled_changed,
        "tui_status_line_changed": tui_changed,
        "home_codex_skills_changed": home_codex_skills_changed,
        "default_bootstrap": default_bootstrap,
    }))
}

fn projection_install_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
) -> Result<Value, String> {
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        command.codex_home.as_deref(),
        command.cursor_home.as_deref(),
        command.home.as_deref(),
    )?;
    let selected_tools = selected_projection_tools(&command.to, true)?;
    let scope = canonical_scope(&command.scope)?;
    let mut results = Map::new();
    for tool in selected_tools {
        results.insert(
            tool.to_string(),
            install_projection_tool(&roots, tool, scope)?,
        );
    }
    Ok(projection_envelope(
        "install",
        compatibility_alias,
        &roots,
        Some(scope),
        results,
    ))
}

fn projection_status_command(command: ProjectionStatusCommand) -> Result<Value, String> {
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        command.codex_home.as_deref(),
        command.cursor_home.as_deref(),
        command.home.as_deref(),
    )?;
    let mut results = Map::new();
    for tool in INSTALL_SKILLS_TOOLS {
        results.insert(tool.to_string(), projection_tool_status(&roots, tool)?);
    }
    Ok(projection_envelope("status", false, &roots, None, results))
}

fn projection_remove_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
) -> Result<Value, String> {
    projection_remove_or_cleanup_command(command, compatibility_alias, false)
}

fn projection_cleanup_command(command: ProjectionCommand) -> Result<Value, String> {
    projection_remove_or_cleanup_command(command, false, true)
}

fn projection_remove_or_cleanup_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
    cleanup_mode: bool,
) -> Result<Value, String> {
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        command.codex_home.as_deref(),
        command.cursor_home.as_deref(),
        command.home.as_deref(),
    )?;
    let selected_tools = selected_projection_tools(&command.to, false)?;
    let scope = canonical_scope(&command.scope)?;
    validate_cleanup_scope(&command, scope, &selected_tools, cleanup_mode)?;
    let mut results = Map::new();
    for tool in selected_tools {
        results.insert(
            tool.to_string(),
            remove_projection_tool(&roots, tool, scope, command.dry_run)?,
        );
    }
    Ok(projection_envelope(
        if cleanup_mode { "cleanup" } else { "remove" },
        compatibility_alias,
        &roots,
        Some(scope),
        results,
    ))
}

fn validate_cleanup_scope(
    command: &ProjectionCommand,
    scope: &str,
    tools: &[&'static str],
    cleanup_mode: bool,
) -> Result<(), String> {
    if !cleanup_mode || scope != "user" {
        return Ok(());
    }
    for tool in tools {
        let explicit_home = match *tool {
            "codex" => command.codex_home.is_some() || std::env::var_os("CODEX_HOME").is_some(),
            "cursor" => command.cursor_home.is_some() || std::env::var_os("CURSOR_HOME").is_some(),
            _ => true,
        };
        if !explicit_home && command.home.is_none() {
            return Err(format!(
                "user-scope cleanup for {tool} requires explicit host-home resolution; pass --codex-home/--cursor-home, --home, or the matching host HOME environment variable"
            ));
        }
    }
    Ok(())
}

fn projection_envelope(
    command: &str,
    compatibility_alias: bool,
    roots: &ResolvedProjectionRoots,
    scope: Option<&str>,
    results: Map<String, Value>,
) -> Value {
    let host_targets = json!({
        "codex-cli": results.get("codex").cloned().unwrap_or(Value::Null),
        "cursor": results.get("cursor").cloned().unwrap_or(Value::Null),
    });
    json!({
        "success": true,
        "command": command,
        "invocation": {
            "primary_command": "framework host-integration",
            "alias_used": if compatibility_alias { Value::String("install-skills".to_string()) } else { Value::Null },
            "deprecated_alias": compatibility_alias,
        },
        "resolved_roots": resolved_roots_payload(roots),
        "scope": scope.unwrap_or("all-scopes-status"),
        "results": results,
        "host_targets": host_targets,
    })
}

fn resolved_roots_payload(roots: &ResolvedProjectionRoots) -> Value {
    json!({
        "framework_root": roots.framework_root.to_string_lossy(),
        "project_root": roots.project_root.to_string_lossy(),
        "artifact_root": roots.artifact_root.to_string_lossy(),
        "host_home_roots": {
            "codex-cli": roots.codex_home_root.to_string_lossy(),
            "cursor": roots.cursor_home_root.to_string_lossy(),
        },
    })
}

fn selected_projection_tools(
    raw_tools: &[String],
    default_all: bool,
) -> Result<Vec<&'static str>, String> {
    if raw_tools.is_empty() && default_all {
        return Ok(INSTALL_SKILLS_TOOLS.to_vec());
    }
    let mut selected = Vec::new();
    for raw in raw_tools {
        if raw.trim().eq_ignore_ascii_case("all") {
            for tool in INSTALL_SKILLS_TOOLS {
                if !selected.contains(&tool) {
                    selected.push(tool);
                }
            }
            continue;
        }
        let tool = canonical_tool_name(raw)?;
        if !selected.contains(&tool) {
            selected.push(tool);
        }
    }
    if selected.is_empty() {
        return Err("projection command requires --to codex/--to cursor or --to all".to_string());
    }
    Ok(selected)
}

fn canonical_scope(scope: &str) -> Result<&'static str, String> {
    match scope.trim().to_lowercase().as_str() {
        "" | "project" | "project-local" => Ok("project"),
        "user" => Ok("user"),
        other => Err(format!(
            "Unsupported scope: {other}. Supported scopes: project user"
        )),
    }
}

fn install_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
) -> Result<Value, String> {
    match tool {
        "codex" => install_codex_projection(roots, scope),
        "cursor" => install_cursor_projection(roots, scope),
        _ => Err(format!("Unsupported tool: {tool}")),
    }
}

fn projection_tool_status(roots: &ResolvedProjectionRoots, tool: &str) -> Result<Value, String> {
    match tool {
        "codex" => codex_projection_status(roots),
        "cursor" => cursor_projection_status(roots),
        _ => Err(format!("Unsupported tool: {tool}")),
    }
}

fn remove_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    match tool {
        "codex" => remove_codex_projection(roots, scope, dry_run),
        "cursor" => remove_cursor_projection(roots, scope, dry_run),
        _ => Err(format!("Unsupported tool: {tool}")),
    }
}

fn install_codex_projection(roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
    let target = codex_entrypoint_target(roots, scope);
    let changed = write_text_if_changed(&target, &render_codex_framework_entrypoint(roots, scope))?;
    let prompt_entrypoints =
        codex_prompt_entrypoints_disabled(&codex_prompt_entrypoints_root(roots, scope));
    let manifest_changed = write_codex_projection_manifest(roots, scope, &target)?;
    let prompt_entrypoints_changed = prompt_entrypoints
        .get("changed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(json!({
        "status": "installed",
        "changed": changed || manifest_changed || prompt_entrypoints_changed,
        "scope": scope,
        "prompts": {
            "framework": {
                "scope": scope,
                "path": target.to_string_lossy(),
                "logical_entrypoint": "$framework",
                "native_representation": "prompt-file",
            }
        },
        "prompt_entrypoints": prompt_entrypoints,
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
        "aliases": {"managed": false, "reason": "compatibility-aliases-not-managed-by-default-projection"},
    }))
}

fn codex_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_target = codex_entrypoint_target(roots, "project");
    let user_target = codex_entrypoint_target(roots, "user");
    Ok(json!({
        "ready": managed_projection_file_exists(&project_target)? || managed_projection_file_exists(&user_target)?,
        "status": "projection-status",
        "prompts": {
            "framework": {
                "project": codex_projection_file_status(&project_target)?,
                "user": codex_projection_file_status(&user_target)?,
            }
        },
        "manifest": {
            "project": projection_manifest_status(&projection_manifest_path(roots, "codex-cli", "project"))?,
            "user": projection_manifest_status(&projection_manifest_path(roots, "codex-cli", "user"))?,
        },
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
    }))
}

fn install_cursor_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    let target = cursor_entrypoint_target(roots, scope);
    let mut managed_files = vec![target.to_string_lossy().to_string()];
    let mut managed_key_paths: Vec<String> = Vec::new();
    let mut changed =
        write_text_if_changed(&target, &render_cursor_framework_entrypoint(roots, scope))?;
    let mut mcp = json!({
        "managed": false,
        "path": cursor_mcp_config_path(roots).to_string_lossy(),
        "server": "browser-mcp",
        "changed": false,
        "reason": "user-scope-only",
    });
    if scope == "user" {
        let mcp_path = cursor_mcp_config_path(roots);
        let mcp_install = install_cursor_mcp_server(roots, &mcp_path)?;
        changed |= mcp_install.changed;
        if mcp_install.managed {
            managed_files.push(mcp_path.to_string_lossy().to_string());
            managed_key_paths.push(cursor_mcp_server_key_path().to_string());
        }
        mcp = json!({
            "managed": mcp_install.managed,
            "path": mcp_path.to_string_lossy(),
            "server": "browser-mcp",
            "changed": mcp_install.changed,
            "reason": mcp_install.reason,
            "skipped_user_owned": mcp_install.skipped_user_owned,
        });
    }
    let manifest_changed =
        write_cursor_projection_manifest(roots, scope, &managed_files, &managed_key_paths)?;
    Ok(json!({
        "status": "installed",
        "changed": changed || manifest_changed,
        "scope": scope,
        "rules": {
            "framework": {
                "scope": scope,
                "path": target.to_string_lossy(),
                "logical_entrypoint": "/framework",
                "native_representation": "cursor-rule-mdc",
            }
        },
        "mcp": mcp,
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
        "aliases": {"managed": false, "reason": "compatibility-aliases-not-managed-by-default-projection"},
    }))
}

fn cursor_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_target = cursor_entrypoint_target(roots, "project");
    let user_target = cursor_entrypoint_target(roots, "user");
    Ok(json!({
        "ready": managed_projection_file_exists(&project_target)? || managed_projection_file_exists(&user_target)?,
        "status": "projection-status",
        "rules": {
            "framework": {
                "project": cursor_projection_file_status(&project_target)?,
                "user": cursor_projection_file_status(&user_target)?,
            }
        },
        "manifest": {
            "project": projection_manifest_status(&projection_manifest_path(roots, "cursor", "project"))?,
            "user": projection_manifest_status(&projection_manifest_path(roots, "cursor", "user"))?,
        },
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
    }))
}

fn remove_codex_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let target = codex_entrypoint_target(roots, scope);
    let manifest_path = projection_manifest_path(roots, "codex-cli", scope);
    let manifest_ownership =
        projection_manifest_ownership(&manifest_path, "codex-cli", scope, &target)?;
    let would_remove_projection = target.is_file() && manifest_ownership.owns_projection_file;
    let changed = if !dry_run && would_remove_projection {
        fs::remove_file(&target).map_err(|err| err.to_string())?;
        true
    } else {
        false
    };
    let would_remove_manifest = manifest_ownership.managed;
    let manifest_removed = if !dry_run && would_remove_manifest {
        fs::remove_file(&manifest_path).map_err(|err| err.to_string())?;
        true
    } else {
        false
    };
    let any_changed = changed || manifest_removed;
    Ok(json!({
        "status": if dry_run && (would_remove_projection || would_remove_manifest) { "would-remove" } else if any_changed { "removed" } else { "not-installed-or-user-owned" },
        "changed": any_changed,
        "dry_run": dry_run,
        "scope": scope,
        "removed_paths": removed_projection_paths(changed, &target, manifest_removed, &manifest_path),
        "would_remove_paths": removed_projection_paths(would_remove_projection, &target, would_remove_manifest, &manifest_path),
        "skipped_user_owned_paths": if would_remove_projection || !target.exists() { json!([]) } else { json!([target.to_string_lossy()]) },
    }))
}

fn remove_cursor_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let target = cursor_entrypoint_target(roots, scope);
    let manifest_path = projection_manifest_path(roots, "cursor", scope);
    let manifest_ownership =
        projection_manifest_ownership(&manifest_path, "cursor", scope, &target)?;
    let would_remove_projection = target.is_file() && manifest_ownership.owns_projection_file;
    let changed = if !dry_run && would_remove_projection {
        fs::remove_file(&target).map_err(|err| err.to_string())?;
        true
    } else {
        false
    };
    let would_remove_manifest = manifest_ownership.managed;
    let manifest_removed = if !dry_run && would_remove_manifest {
        fs::remove_file(&manifest_path).map_err(|err| err.to_string())?;
        true
    } else {
        false
    };
    let mcp_path = cursor_mcp_config_path(roots);
    let mcp_matches_framework =
        cursor_mcp_server_matches_framework(roots, &mcp_path)?.unwrap_or(false);
    let mcp_managed = scope == "user"
        && (projection_manifest_manages_key_path(&manifest_path, cursor_mcp_server_key_path())?
            || mcp_matches_framework);
    let mcp_would_remove = mcp_managed && mcp_matches_framework;
    let mcp_skipped_user_owned =
        scope == "user" && !mcp_would_remove && cursor_mcp_server_exists(&mcp_path)?;
    let mcp_changed = if !dry_run && mcp_would_remove {
        remove_cursor_mcp_server(&mcp_path)?
    } else {
        false
    };
    let any_changed = changed || manifest_removed || mcp_changed;
    let would_remove_any = would_remove_projection || would_remove_manifest || mcp_would_remove;
    let mut skipped_user_owned_paths = Vec::new();
    if !would_remove_projection && target.exists() {
        skipped_user_owned_paths.push(Value::String(target.to_string_lossy().into_owned()));
    }
    if mcp_skipped_user_owned {
        skipped_user_owned_paths.push(Value::String(mcp_path.to_string_lossy().into_owned()));
    }
    let mut removed_paths =
        removed_projection_paths(changed, &target, manifest_removed, &manifest_path);
    append_mcp_path(&mut removed_paths, mcp_changed, &mcp_path);
    let mut would_remove_paths = removed_projection_paths(
        would_remove_projection,
        &target,
        would_remove_manifest,
        &manifest_path,
    );
    append_mcp_path(&mut would_remove_paths, mcp_would_remove, &mcp_path);
    Ok(json!({
        "status": if dry_run && would_remove_any { "would-remove" } else if any_changed { "removed" } else { "not-installed-or-user-owned" },
        "changed": any_changed,
        "dry_run": dry_run,
        "scope": scope,
        "removed_paths": removed_paths,
        "would_remove_paths": would_remove_paths,
        "mcp": {
            "managed": mcp_managed,
            "path": mcp_path.to_string_lossy(),
            "server": "browser-mcp",
            "changed": mcp_changed,
            "would_remove": dry_run && mcp_would_remove,
            "skipped_user_owned": mcp_skipped_user_owned,
        },
        "skipped_user_owned_paths": Value::Array(skipped_user_owned_paths),
    }))
}

#[derive(Debug, Clone, Copy)]
struct ProjectionManifestOwnership {
    managed: bool,
    owns_projection_file: bool,
}

fn projection_manifest_status(path: &Path) -> Result<Value, String> {
    let manifest = read_json_if_exists(path)?;
    Ok(projection_manifest_status_from_payload(
        path,
        manifest.as_ref(),
    ))
}

fn projection_manifest_status_from_payload(path: &Path, manifest: Option<&Value>) -> Value {
    json!({
        "path": path.to_string_lossy(),
        "exists": path.is_file(),
        "managed": projection_manifest_payload_is_managed(manifest, None, None),
    })
}

fn projection_manifest_ownership(
    path: &Path,
    host_projection: &str,
    scope: &str,
    projection_path: &Path,
) -> Result<ProjectionManifestOwnership, String> {
    let managed = projection_manifest_is_managed(path, Some(host_projection), Some(scope))?;
    let owns_projection_file = managed && projection_manifest_files_include(path, projection_path)?;
    Ok(ProjectionManifestOwnership {
        managed,
        owns_projection_file,
    })
}

fn projection_manifest_is_managed(
    path: &Path,
    host_projection: Option<&str>,
    scope: Option<&str>,
) -> Result<bool, String> {
    let Some(manifest) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    Ok(projection_manifest_payload_is_managed(
        Some(&manifest),
        host_projection,
        scope,
    ))
}

fn projection_manifest_payload_is_managed(
    manifest: Option<&Value>,
    host_projection: Option<&str>,
    scope: Option<&str>,
) -> bool {
    let Some(manifest) = manifest else {
        return false;
    };
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some(FRAMEWORK_PROJECTION_SCHEMA_VERSION)
        || manifest.get("managed_by").and_then(Value::as_str) != Some("skill-framework")
    {
        return false;
    }
    if let Some(expected) = host_projection {
        if manifest.get("host_projection").and_then(Value::as_str) != Some(expected) {
            return false;
        }
    }
    if let Some(expected) = scope {
        if manifest.get("scope").and_then(Value::as_str) != Some(expected) {
            return false;
        }
    }
    true
}

fn projection_manifest_files_include(path: &Path, projection_path: &Path) -> Result<bool, String> {
    let Some(manifest) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    let expected = normalize_path(projection_path)?;
    Ok(manifest
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .filter_map(|path| normalize_path(&path).ok())
                .any(|path| path == expected)
        })
        .unwrap_or(false))
}

fn removed_projection_paths(
    projection_removed: bool,
    projection_path: &Path,
    manifest_removed: bool,
    manifest_path: &Path,
) -> Value {
    let mut paths = Vec::new();
    if projection_removed {
        paths.push(Value::String(
            projection_path.to_string_lossy().into_owned(),
        ));
    }
    if manifest_removed {
        paths.push(Value::String(manifest_path.to_string_lossy().into_owned()));
    }
    Value::Array(paths)
}

fn append_mcp_path(paths: &mut Value, include: bool, mcp_path: &Path) {
    if !include {
        return;
    }
    if let Some(array) = paths.as_array_mut() {
        array.push(Value::String(mcp_path.to_string_lossy().into_owned()));
    }
}

fn codex_entrypoint_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.codex_home_root.join("prompts").join("framework.md")
    } else {
        roots
            .project_root
            .join(".codex")
            .join("prompts")
            .join("framework.md")
    }
}

fn cursor_entrypoint_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.cursor_home_root.join("rules").join("framework.mdc")
    } else {
        roots
            .project_root
            .join(".cursor")
            .join("rules")
            .join("framework.mdc")
    }
}

fn codex_prompt_entrypoints_root(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.codex_home_root.clone()
    } else {
        roots.project_root.join(".codex")
    }
}

fn projection_manifest_path(
    roots: &ResolvedProjectionRoots,
    host_projection: &str,
    scope: &str,
) -> PathBuf {
    match (host_projection, scope) {
        ("codex-cli", "user") => roots
            .codex_home_root
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("codex-cli", _) => roots
            .project_root
            .join(".codex")
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("cursor", "user") => roots
            .cursor_home_root
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("cursor", _) => roots
            .project_root
            .join(".cursor")
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        _ => roots.project_root.join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
    }
}

fn write_codex_projection_manifest(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    command_path: &Path,
) -> Result<bool, String> {
    write_json_if_changed(
        &projection_manifest_path(roots, "codex-cli", scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "codex-cli",
            "scope": scope,
            "files": [command_path.to_string_lossy()],
            "settings": {
                "managed_key_paths": [],
            }
        }),
    )
}

fn render_codex_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nargument-hint: \"[framework task...]\"\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: codex-cli -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse `$framework` semantics via the Rust-owned shared core.\n\nFramework root: `{}`.\nProject root: `{}`.\n\n$ARGUMENTS\n",
        roots.framework_root.to_string_lossy(),
        roots.project_root.to_string_lossy(),
    )
}

fn write_cursor_projection_manifest(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    managed_files: &[String],
    managed_key_paths: &[String],
) -> Result<bool, String> {
    write_json_if_changed(
        &projection_manifest_path(roots, "cursor", scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "cursor",
            "scope": scope,
            "files": managed_files,
            "settings": {
                "managed_key_paths": managed_key_paths,
            }
        }),
    )
}

fn cursor_mcp_config_path(roots: &ResolvedProjectionRoots) -> PathBuf {
    roots.cursor_home_root.join("mcp.json")
}

fn cursor_mcp_server_key_path() -> &'static str {
    "mcp_servers.browser-mcp"
}

#[derive(Debug, Clone)]
struct CursorMcpInstallOutcome {
    managed: bool,
    changed: bool,
    reason: &'static str,
    skipped_user_owned: bool,
}

fn install_cursor_mcp_server(
    roots: &ResolvedProjectionRoots,
    path: &Path,
) -> Result<CursorMcpInstallOutcome, String> {
    let mut payload = read_json_if_exists(path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let root = payload
        .as_object_mut()
        .ok_or_else(|| "cursor mcp config payload must be an object".to_string())?;
    let mcp_servers = root
        .entry("mcp_servers".to_string())
        .or_insert_with(|| json!({}));
    if !mcp_servers.is_object() {
        *mcp_servers = json!({});
    }
    let servers = mcp_servers
        .as_object_mut()
        .ok_or_else(|| "cursor mcp_servers must be an object".to_string())?;
    let server = cursor_mcp_server_payload(roots);
    if matches!(servers.get("browser-mcp"), Some(existing) if existing != &server) {
        return Ok(CursorMcpInstallOutcome {
            managed: false,
            changed: false,
            reason: "skipped_user_owned",
            skipped_user_owned: true,
        });
    }
    let changed = servers.get("browser-mcp").is_none();
    if changed {
        servers.insert("browser-mcp".to_string(), server);
    }
    let file_changed = write_json_if_changed(path, &payload)?;
    Ok(CursorMcpInstallOutcome {
        managed: true,
        changed: changed || file_changed,
        reason: if changed {
            "installed"
        } else {
            "already-managed-equivalent"
        },
        skipped_user_owned: false,
    })
}

fn remove_cursor_mcp_server(path: &Path) -> Result<bool, String> {
    let Some(mut payload) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    let Some(root) = payload.as_object_mut() else {
        return Ok(false);
    };
    let mut changed = false;
    if let Some(mcp_servers) = root.get_mut("mcp_servers") {
        if let Some(servers) = mcp_servers.as_object_mut() {
            changed |= servers.remove("browser-mcp").is_some();
            if servers.is_empty() {
                root.remove("mcp_servers");
            }
        }
    }
    if changed {
        write_json_if_changed(path, &payload)?;
    }
    Ok(changed)
}

fn cursor_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    json!({
        "command": "bash",
        "args": [
            roots
                .framework_root
                .join("tools/browser-mcp/scripts/start_browser_mcp.sh")
                .to_string_lossy()
                .to_string()
        ]
    })
}

fn projection_manifest_manages_key_path(path: &Path, key_path: &str) -> Result<bool, String> {
    let Some(manifest) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    if !projection_manifest_payload_is_managed(Some(&manifest), None, None) {
        return Ok(false);
    }
    Ok(manifest
        .get("settings")
        .and_then(|settings| settings.get("managed_key_paths"))
        .and_then(Value::as_array)
        .map(|paths| paths.iter().any(|entry| entry.as_str() == Some(key_path)))
        .unwrap_or(false))
}

fn cursor_mcp_server_matches_framework(
    roots: &ResolvedProjectionRoots,
    path: &Path,
) -> Result<Option<bool>, String> {
    let Some(payload) = read_json_if_exists(path)? else {
        return Ok(None);
    };
    let actual = payload
        .get("mcp_servers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.get("browser-mcp"));
    let Some(server) = actual else {
        return Ok(None);
    };
    let expected_script = roots
        .framework_root
        .join("tools/browser-mcp/scripts/start_browser_mcp.sh");
    let is_framework_server = server.get("command").and_then(Value::as_str) == Some("bash")
        && server
            .get("args")
            .and_then(Value::as_array)
            .and_then(|args| args.first())
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .map(|script_path| paths_match_framework_script(&script_path, &expected_script))
            .transpose()?
            .unwrap_or(false);
    Ok(Some(is_framework_server))
}

fn paths_match_framework_script(actual: &Path, expected: &Path) -> Result<bool, String> {
    if let (Ok(actual_canonical), Ok(expected_canonical)) =
        (fs::canonicalize(actual), fs::canonicalize(expected))
    {
        return Ok(actual_canonical == expected_canonical);
    }
    Ok(normalize_path(actual)? == normalize_path(expected)?)
}

fn cursor_mcp_server_exists(path: &Path) -> Result<bool, String> {
    let Some(payload) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    Ok(payload
        .get("mcp_servers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.get("browser-mcp"))
        .is_some())
}

fn render_cursor_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nglobs: [\"**/*\"]\nalwaysApply: true\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: cursor -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n1) Start from `AGENTS.md`.\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `{}`.\nProject root: `{}`.\n",
        roots.framework_root.to_string_lossy(),
        roots.project_root.to_string_lossy(),
    )
}

fn managed_projection_file_exists(path: &Path) -> Result<bool, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(false);
    };
    Ok(is_managed_projection_content(&content))
}

fn codex_projection_file_status(path: &Path) -> Result<Value, String> {
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains("host_projection: codex-cli"))
            .unwrap_or(false);
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": verified,
        "verification": if verified { "verified" } else if marker_managed { "unknown" } else { "unmanaged" },
        "marker_managed": marker_managed,
    }))
}

fn cursor_projection_file_status(path: &Path) -> Result<Value, String> {
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains("host_projection: cursor"))
            .unwrap_or(false);
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": verified,
        "verification": if verified { "verified" } else if marker_managed { "unknown" } else { "unmanaged" },
        "marker_managed": marker_managed,
    }))
}

fn is_managed_projection_content(content: &str) -> bool {
    content.contains("managed_by: skill-framework")
        && content.contains(&format!(
            "framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION}"
        ))
}

fn canonical_install_skills_command(command: &str) -> String {
    match command.trim() {
        "" => "status".to_string(),
        raw => raw.to_lowercase(),
    }
}

fn install_skills_projection_tools(command: &str, tools: &[String], to: &[String]) -> Vec<String> {
    if !to.is_empty() {
        return to.to_vec();
    }
    if !tools.is_empty() {
        return tools.to_vec();
    }
    match canonical_install_skills_command(command).as_str() {
        "status" | "ls" | "install" | "init" => Vec::new(),
        "all" => vec!["all".to_string()],
        "remove" | "rm" => Vec::new(),
        other => vec![other.to_string()],
    }
}

fn canonical_tool_name(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_lowercase().as_str() {
        "codex" => Ok("codex"),
        "cursor" => Ok("cursor"),
        other => Err(format!(
            "Unknown tool: {other}. Supported tools: {}",
            INSTALL_SKILLS_TOOLS.join(" ")
        )),
    }
}

fn codex_prompt_entrypoints_disabled(codex_dir: &Path) -> Value {
    let prompt_dir = codex_dir.join("prompts");
    json!({
        "changed": false,
        "enabled": false,
        "prompt_dir": prompt_dir.to_string_lossy(),
        "written": [],
        "unchanged": [],
    })
}

fn shared_skills_source(repo_root: &Path) -> Result<PathBuf, String> {
    let repo_root = normalize_path(repo_root)?;
    let source_rel = skills_source_rel(&repo_root)?;
    let candidate = repo_root.join(&source_rel);
    let normalized = normalize_path(&candidate)?;
    if !normalized.starts_with(&repo_root) {
        return Err(format!(
            "resolved skills source escapes repository root: {}",
            normalized.display()
        ));
    }
    Ok(normalized)
}

fn shared_codex_skill_surface(repo_root: &Path) -> PathBuf {
    repo_root.join(CODEX_SKILL_SURFACE_REL)
}

fn ensure_codex_skill_surface(repo_root: &Path) -> Result<Value, String> {
    ensure_host_skill_surface(
        repo_root,
        &shared_codex_skill_surface,
        CODEX_SKILL_SURFACE_MANIFEST_NAME,
        "codex-skill-surface-v1",
        &desired_codex_skill_surface_slugs,
        "runtime-hot-index-plus-pinned-explicit-entrypoints",
    )
}

fn ensure_host_skill_surface(
    repo_root: &Path,
    surface_path: &dyn Fn(&Path) -> PathBuf,
    manifest_name: &str,
    schema_version: &str,
    desired_slugs: &dyn Fn(&Path) -> Result<Vec<String>, String>,
    policy: &str,
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let source_root = shared_skills_source(&repo_root)?;
    let surface_root = surface_path(&repo_root);
    let desired = desired_slugs(&repo_root)?;
    let mut changed = false;

    if let Ok(metadata) = fs::symlink_metadata(&surface_root) {
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            remove_path(&surface_root).map_err(|err| err.to_string())?;
            changed = true;
        }
    }
    fs::create_dir_all(&surface_root).map_err(|err| err.to_string())?;

    let desired_set = desired.iter().cloned().collect::<BTreeSet<_>>();
    let include_system = false;
    for entry in fs::read_dir(&surface_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == manifest_name {
            continue;
        }
        if name == ".system" && include_system {
            continue;
        }
        if !desired_set.contains(&name) {
            remove_path(&entry.path()).map_err(|err| err.to_string())?;
            changed = true;
        }
    }

    for slug in &desired {
        if let Some(source_path) = codex_skill_surface_source_path(&repo_root, slug)? {
            changed |= ensure_codex_skills_symlink(&surface_root.join(slug), &source_path)?;
        } else if is_framework_command(&repo_root, slug)? {
            changed |= ensure_framework_command_skill(&repo_root, &surface_root.join(slug), slug)?;
        }
    }

    let generated_framework_commands = desired
        .iter()
        .filter(|slug| {
            codex_skill_surface_source_path(&repo_root, slug)
                .map(|source| source.is_none())
                .unwrap_or(false)
                && is_framework_command(&repo_root, slug).unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let manifest = json!({
        "schema_version": schema_version,
        "source": source_root.to_string_lossy(),
        "surface": surface_root.to_string_lossy(),
        "policy": policy,
        "skills": desired,
        "count": desired.len(),
        "generated_framework_commands": generated_framework_commands,
        "system_skills_linked": include_system,
        "generated_at": current_local_timestamp(),
    });
    changed |= write_json_if_changed(&surface_root.join(manifest_name), &manifest)?;

    Ok(json!({
        "changed": changed,
        "source": source_root.to_string_lossy(),
        "surface": surface_root.to_string_lossy(),
        "skills": desired,
        "count": desired.len(),
        "system_skills_linked": include_system,
    }))
}

fn desired_codex_skill_surface_slugs(repo_root: &Path) -> Result<Vec<String>, String> {
    desired_host_skill_surface_slugs(repo_root, true)
}

fn desired_host_skill_surface_slugs(
    repo_root: &Path,
    skip_system_provided_codex_skills: bool,
) -> Result<Vec<String>, String> {
    let source_root = shared_skills_source(repo_root)?;
    let mut desired = BTreeSet::new();
    for slug in runtime_hot_skill_slugs(repo_root)? {
        if skip_system_provided_codex_skills && is_codex_system_provided_skill(&slug) {
            continue;
        }
        if system_skill_source_exists(&source_root, &slug) {
            continue;
        }
        if source_root.join(&slug).join("SKILL.md").is_file() {
            desired.insert(slug);
        }
    }
    for slug in HOST_SKILL_SURFACE_PINNED_SKILLS {
        if codex_skill_surface_source_path(repo_root, slug)?.is_some()
            || is_framework_command(repo_root, slug)?
        {
            desired.insert(slug.to_string());
        }
    }
    if desired.is_empty() {
        for entry in fs::read_dir(&source_root).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let slug = entry.file_name().to_string_lossy().to_string();
            if slug.starts_with('.') || slug == "dist" {
                continue;
            }
            if skip_system_provided_codex_skills && is_codex_system_provided_skill(&slug) {
                continue;
            }
            if entry.path().join("SKILL.md").is_file() {
                desired.insert(slug);
            }
        }
    }
    Ok(desired.into_iter().collect())
}

fn system_skill_source_exists(source_root: &Path, slug: &str) -> bool {
    source_root
        .join(".system")
        .join(slug)
        .join("SKILL.md")
        .is_file()
}

fn is_codex_system_provided_skill(slug: &str) -> bool {
    CODEX_SYSTEM_PROVIDED_SKILLS.contains(&slug)
}

fn codex_skill_surface_source_path(
    repo_root: &Path,
    slug: &str,
) -> Result<Option<PathBuf>, String> {
    let source_root = shared_skills_source(repo_root)?;
    let skill_source = source_root.join(slug);
    if skill_source.join("SKILL.md").is_file() {
        return Ok(Some(skill_source));
    }
    Ok(None)
}

fn is_framework_command(repo_root: &Path, slug: &str) -> Result<bool, String> {
    Ok(framework_command_names(repo_root)?.contains(slug))
}

fn ensure_framework_command_skill(
    repo_root: &Path,
    target_path: &Path,
    slug: &str,
) -> Result<bool, String> {
    if target_path.exists() || symlink_exists(target_path) {
        let metadata = fs::symlink_metadata(target_path).map_err(|err| err.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            remove_path(target_path).map_err(|err| err.to_string())?;
        }
    }
    fs::create_dir_all(target_path).map_err(|err| err.to_string())?;
    let content = render_framework_command_skill(repo_root, slug)?;
    write_text_if_changed(&target_path.join("SKILL.md"), &content)
}

fn render_framework_command_skill(repo_root: &Path, slug: &str) -> Result<String, String> {
    let registry = load_runtime_registry_payload(repo_root)?;
    let command = registry
        .get("framework_commands")
        .and_then(Value::as_object)
        .and_then(|commands| commands.get(slug))
        .cloned()
        .unwrap_or(Value::Null);
    let owner = command
        .get("canonical_owner")
        .and_then(Value::as_str)
        .unwrap_or("skill-framework-developer");
    let host_entrypoints = command.get("host_entrypoints").and_then(Value::as_object);
    let default_host_entrypoint = format!("${slug}");
    let host_entrypoint = host_entrypoints
        .and_then(|entrypoints| entrypoints.get("codex-cli"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| default_host_entrypoint.clone());
    let host_entrypoint_summary = host_entrypoints
        .map(|entrypoints| {
            entrypoints
                .iter()
                .filter_map(|(host, entrypoint)| {
                    entrypoint
                        .as_str()
                        .map(|entrypoint| format!("{host}={entrypoint}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|summary| !summary.is_empty())
        .unwrap_or_else(|| format!("codex-cli={0}", default_host_entrypoint));
    let explicit_entrypoints = command
        .get("interaction_invariants")
        .and_then(|invariants| invariants.get("explicit_entrypoints"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![host_entrypoint.clone(), format!("/{slug}")]);
    let mut explicit_entrypoints = explicit_entrypoints;
    explicit_entrypoints.sort();
    explicit_entrypoints.dedup();
    let explicit_entrypoint_summary = explicit_entrypoints
        .iter()
        .map(|entrypoint| format!("`{entrypoint}`"))
        .collect::<Vec<_>>()
        .join(" or ");
    let description = command
        .get("lineage")
        .and_then(|lineage| lineage.get("description"))
        .and_then(Value::as_str)
        .unwrap_or("Generated lightweight framework command alias.");
    Ok(format!(
        "---\nname: {slug}\ndescription: {description} Use when the user invokes {explicit_entrypoint_summary}.\nrouting_layer: L0\nrouting_owner: owner\nrouting_gate: none\nrouting_priority: P1\nsession_start: n/a\nsource: generated-codex-skill-surface\n---\n# {slug}\n\nThis is a generated lightweight Codex CLI alias for `{host_entrypoint}`.\n\nSupported host entrypoints: {host_entrypoint_summary}.\n\nUse it only when the user explicitly invokes {explicit_entrypoint_summary}. Resolve the live workflow through `router-rs framework alias {slug}` and keep the full framework policy in `skills/skill-framework-developer/SKILL.md`.\n\nCanonical owner: `{owner}`.\n"
    ))
}

fn framework_command_names(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let Some(registry) = load_runtime_registry_payload_if_repo_local(repo_root)? else {
        return Ok(BTreeSet::new());
    };
    Ok(registry
        .get("framework_commands")
        .and_then(Value::as_object)
        .map(|commands| commands.keys().cloned().collect())
        .unwrap_or_default())
}

fn runtime_hot_skill_slugs(repo_root: &Path) -> Result<Vec<String>, String> {
    let runtime_path = shared_skills_source(repo_root)?.join("SKILL_ROUTING_RUNTIME.json");
    let Some(content) = read_text_if_exists(&runtime_path)? else {
        return Ok(Vec::new());
    };
    let runtime = serde_json::from_str::<Value>(&content).map_err(|err| err.to_string())?;
    let Some(skills) = runtime.get("skills").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(skills
        .iter()
        .filter_map(Value::as_array)
        .filter_map(|record| record.first())
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

fn codex_skills_matches_source(target_path: &Path, source_path: &Path) -> Result<bool, String> {
    let source_path = normalize_path(source_path)?;
    let Ok(metadata) = fs::symlink_metadata(target_path) else {
        return Ok(false);
    };
    if !metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let link_target = fs::read_link(target_path).map_err(|err| err.to_string())?;
    let resolved = if link_target.is_absolute() {
        link_target
    } else {
        target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(link_target)
    };
    normalize_path(&resolved).map(|resolved| resolved == source_path)
}

fn default_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
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
    artifact_source_dir: Option<&Path>,
    workspace_override: Option<&str>,
    _top: usize,
) -> Result<Value, String> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let resolved_output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_bootstrap_output_dir(&repo_root));
    fs::create_dir_all(&resolved_output_dir).map_err(|err| err.to_string())?;
    let workspace = workspace_override
        .map(str::to_owned)
        .unwrap_or_else(|| workspace_name_from_root(&repo_root));
    let created_at = current_local_timestamp();
    let task_id = build_framework_task_id(if query.trim().is_empty() {
        &workspace
    } else {
        query
    });
    let continuity_bootstrap =
        build_default_continuity_bootstrap(&repo_root, artifact_source_dir, Some(&task_id))?;
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
        "continuity-bootstrap": continuity_bootstrap,
        "evolution-proposals": proposals,
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": repo_root.to_string_lossy(),
            "task_id": task_id,
            "created_at": created_at,
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
            "mirror_bootstrap_path": mirror_bootstrap_path.to_string_lossy(),
        },
        "proposal_count": payload
            .get("evolution-proposals")
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("proposal_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "payload": payload,
    }))
}

fn build_default_continuity_bootstrap(
    repo_root: &Path,
    artifact_source_dir: Option<&Path>,
    task_id: Option<&str>,
) -> Result<Value, String> {
    let mut args = vec!["framework".to_string(), "snapshot".to_string()];
    if let Some(path) = artifact_source_dir {
        args.push("--artifact-source-dir".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    if let Some(task_id) = task_id {
        args.push("--task-id".to_string());
        args.push(task_id.to_string());
    }
    let snapshot = run_router_rs_json(repo_root, &args)?;
    Ok(json!({
        "schema_version": "framework-continuity-bootstrap-v1",
        "source": "framework-runtime-snapshot",
        "snapshot": snapshot.get("runtime_snapshot").cloned().unwrap_or_else(|| json!({})),
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
                .join("archived-current")
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
            repo_root
                .join("artifacts")
                .join("ops")
                .join("archived-current")
                .join(suffix),
        );
    }
    if name.starts_with("tmp-") {
        return Some(if path.parent() == Some(current_root.as_path()) {
            scratch_artifact_root(repo_root, None).join(name)
        } else {
            scratch_artifact_root(repo_root, Some("archived-current"))
                .join(active_task_id)
                .join(name)
        });
    }
    let suffix = if path.parent() == Some(current_root.as_path()) {
        PathBuf::from(name)
    } else {
        PathBuf::from(active_task_id).join(name)
    };
    Some(evidence_artifact_root(repo_root, Some("archived-current")).join(suffix))
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

fn bootstrap_payload_matches_contract(payload: &Value, repo_root: &Path) -> bool {
    let normalized_repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf())
        .to_string_lossy()
        .to_string();
    payload
        .get("bootstrap")
        .and_then(Value::as_object)
        .zip(
            payload
                .get("continuity-bootstrap")
                .and_then(Value::as_object),
        )
        .zip(payload.get("skills-export").and_then(Value::as_object))
        .zip(
            payload
                .get("evolution-proposals")
                .and_then(Value::as_object),
        )
        .map(|(((bootstrap, _continuity), skills), _proposals)| {
            bootstrap
                .get("repo_root")
                .and_then(Value::as_str)
                .map(|value| value == normalized_repo_root)
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

    let parsed =
        build_default_bootstrap_payload(repo_root, Some(&resolved_output_dir), "", None, None, 8)?;
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

fn ensure_codex_hooks_disabled(config_path: &Path) -> Result<bool, String> {
    const HOOKS_DISABLED_LINE: &str = "hooks = false";
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    if let Some((start, end)) = find_named_block_bounds(&content, "[features]") {
        let block = content[start..end].trim_end_matches('\n');
        let mut has_hooks = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_named_setting(line, "codex_hooks") || is_named_setting(line, "hooks") {
                if !has_hooks {
                    updated_lines.push(HOOKS_DISABLED_LINE.to_string());
                    has_hooks = true;
                }
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if !has_hooks {
            updated_lines.push(HOOKS_DISABLED_LINE.to_string());
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        return write_text_if_changed(config_path, &updated);
    }
    let mut updated = content.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str("[features]\n");
    updated.push_str(HOOKS_DISABLED_LINE);
    updated.push('\n');
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
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

fn format_status_line() -> String {
    let items = DEFAULT_TUI_STATUS_ITEMS
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("status_line = [{items}]")
}

fn ensure_codex_skills_symlink(target_path: &Path, source_path: &Path) -> Result<bool, String> {
    let source_path = normalize_path(source_path)?;
    if codex_skills_matches_source(target_path, &source_path)? {
        return Ok(false);
    }
    if target_path.exists() || symlink_exists(target_path) {
        remove_path(target_path).map_err(|err| err.to_string())?;
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    create_dir_symlink(&source_path, target_path)?;
    Ok(true)
}

#[cfg(unix)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(source_path, target_path).map_err(|err| err.to_string())
}

#[cfg(windows)]
fn create_dir_symlink(source_path: &Path, target_path: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_dir(source_path, target_path).map_err(|err| err.to_string())
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

fn read_json_if_exists(path: &Path) -> Result<Option<Value>, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    serde_json::from_str::<Value>(&content)
        .map(Some)
        .map_err(|err| format!("failed parsing {}: {err}", path.display()))
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
            err.contains("--framework-root"),
            "error should suggest --framework-root fix path/flag: {err}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_registry_repo_root_registry_is_used() {
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
            "scripts/router-rs/target/debug/router-rs",
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
            "scripts/router-rs/target",
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
        std::env::set_var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS", "1");

        let timeout = run_generated_artifact_generator("sleep 5", &root, &root);
        assert!(timeout.is_err(), "expected timeout failure");
        let timeout_msg = timeout.err().unwrap();
        assert!(
            timeout_msg.contains("timed out after 1s"),
            "timeout message should include configured timeout: {timeout_msg}"
        );

        std::env::remove_var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS");
        let _ = fs::remove_dir_all(root);
    }
}
