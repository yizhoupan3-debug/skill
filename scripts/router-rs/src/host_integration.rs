use chrono::Local;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG_SCHEMA_HEADER: &str =
    "#:schema https://developers.openai.com/codex/config-schema.json\n";
const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v1";
const DEFAULT_TUI_STATUS_ITEMS: [&str; 4] = [
    "model-with-reasoning",
    "fast-mode",
    "context-remaining",
    "git-branch",
];
const INSTALL_SKILLS_TOOLS: [&str; 1] = ["codex"];
const CODEX_STUB_SKILLS: [&str; 3] = ["autopilot", "deepinterview", "gitx"];
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
        #[arg(long)]
        repo_root: PathBuf,
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
        #[arg(long)]
        repo_root: PathBuf,
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        bootstrap_output_dir: Option<PathBuf>,
        #[arg(long)]
        skip_default_bootstrap: bool,
        #[arg(default_value = "status")]
        command: String,
        #[arg()]
        tools: Vec<String>,
    },
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
            serde_json::to_value(load_runtime_registry_payload(&repo_root)?)
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
            home,
            bootstrap_output_dir,
            skip_default_bootstrap,
            command,
            tools,
        } => install_skills_command(
            &repo_root,
            home.as_deref(),
            bootstrap_output_dir.as_deref(),
            skip_default_bootstrap,
            &command,
            &tools,
        )?,
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

fn skills_source_rel(repo_root: &Path) -> Result<String, String> {
    let registry = load_runtime_registry(repo_root)?;
    Ok(registry
        .workspace_bootstrap_defaults
        .skills
        .source_rel
        .unwrap_or_else(|| ".codex/skills".to_string()))
}

fn router_rs_crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../router-rs")
}

fn router_rs_self_launcher_candidates(repo_root: &Path) -> Vec<PathBuf> {
    let repo_launcher = router_rs_launcher_command(repo_root);
    let crate_launcher = router_rs_crate_root().join("run_router_rs.sh");
    if repo_launcher == crate_launcher {
        vec![repo_launcher]
    } else {
        vec![repo_launcher, crate_launcher]
    }
}

fn run_router_rs_json(repo_root: &Path, args: &[String]) -> Result<Value, String> {
    let mut last_error = None;
    for candidate in router_rs_self_launcher_candidates(repo_root) {
        if !candidate.is_file() {
            continue;
        }
        let manifest_path = candidate
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("Cargo.toml");
        let output = Command::new(&candidate)
            .arg(&manifest_path)
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
        if !stderr.is_empty() {
            last_error = Some(stderr);
        }
    }

    Err(last_error.unwrap_or_else(|| {
        format!(
            "missing required router-rs launcher: {}",
            router_rs_launcher_command(repo_root).to_string_lossy()
        )
    }))
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
    let codex_hooks_feature_changed = ensure_codex_hooks_feature(&home_config_path)?;
    let tui_changed = ensure_tui_status_line(&home_config_path)?;
    let home_codex_skills_link_changed = if install_home_codex_skills_link {
        ensure_home_skills_link(&repo_root, &home_codex_skills_path)?
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
        "created_config": created_config,
        "codex_hooks_feature_changed": codex_hooks_feature_changed,
        "tui_status_line_changed": tui_changed,
        "home_codex_skills_link_changed": home_codex_skills_link_changed,
        "default_bootstrap": default_bootstrap,
    }))
}

fn install_skills_command(
    repo_root: &Path,
    home: Option<&Path>,
    bootstrap_output_dir: Option<&Path>,
    skip_default_bootstrap: bool,
    command: &str,
    tools: &[String],
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let home = home
        .map(normalize_path)
        .transpose()?
        .unwrap_or_else(default_home_dir);
    let bootstrap_output_dir = bootstrap_output_dir.map(normalize_path).transpose()?;
    let command = canonical_install_skills_command(command);

    match command.as_str() {
        "init" | "all" | "install" => {
            let selected_tools = selected_install_tools(tools, true)?;
            let mut results = Map::new();
            for tool in selected_tools {
                results.insert(
                    tool.to_string(),
                    install_skill_tool(
                        &repo_root,
                        &home,
                        tool,
                        bootstrap_output_dir.as_deref(),
                        skip_default_bootstrap,
                    )?,
                );
            }
            Ok(json!({
                "success": true,
                "command": command,
                "repo_root": repo_root.to_string_lossy(),
                "home": home.to_string_lossy(),
                "results": results,
            }))
        }
        "status" | "ls" => {
            let mut results = Map::new();
            for tool in INSTALL_SKILLS_TOOLS {
                results.insert(
                    tool.to_string(),
                    skill_tool_status_with_bootstrap(
                        &repo_root,
                        &home,
                        tool,
                        bootstrap_output_dir.as_deref(),
                    )?,
                );
            }
            Ok(json!({
                "success": true,
                "command": "status",
                "repo_root": repo_root.to_string_lossy(),
                "home": home.to_string_lossy(),
                "skills_source": shared_skills_source(&repo_root)?.to_string_lossy(),
                "total_skills": count_top_level_skills(&shared_skills_source(&repo_root)?)?,
                "results": results,
            }))
        }
        "remove" | "rm" => {
            if tools.is_empty() {
                return Err("install-skills remove requires at least one tool".to_string());
            }
            let selected_tools = selected_install_tools(tools, false)?;
            let mut results = Map::new();
            for tool in selected_tools {
                results.insert(
                    tool.to_string(),
                    remove_skill_tool(&repo_root, &home, tool)?,
                );
            }
            Ok(json!({
                "success": true,
                "command": "remove",
                "repo_root": repo_root.to_string_lossy(),
                "home": home.to_string_lossy(),
                "results": results,
            }))
        }
        other => {
            let selected_tools = selected_install_tools(&[other.to_string()], false)?;
            let mut results = Map::new();
            for tool in selected_tools {
                results.insert(
                    tool.to_string(),
                    install_skill_tool(
                        &repo_root,
                        &home,
                        tool,
                        bootstrap_output_dir.as_deref(),
                        skip_default_bootstrap,
                    )?,
                );
            }
            Ok(json!({
                "success": true,
                "command": "install",
                "repo_root": repo_root.to_string_lossy(),
                "home": home.to_string_lossy(),
                "results": results,
            }))
        }
    }
}

fn canonical_install_skills_command(command: &str) -> String {
    match command.trim() {
        "" => "status".to_string(),
        raw => raw.to_lowercase(),
    }
}

fn selected_install_tools(
    raw_tools: &[String],
    default_all: bool,
) -> Result<Vec<&'static str>, String> {
    if raw_tools.is_empty() && default_all {
        return Ok(INSTALL_SKILLS_TOOLS.to_vec());
    }
    let mut selected = Vec::new();
    for raw in raw_tools {
        let tool = canonical_tool_name(raw)?;
        if !selected.contains(&tool) {
            selected.push(tool);
        }
    }
    Ok(selected)
}

fn canonical_tool_name(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_lowercase().as_str() {
        "codex" => Ok("codex"),
        other => Err(format!(
            "Unknown tool: {other}. Supported tools: {}",
            INSTALL_SKILLS_TOOLS.join(" ")
        )),
    }
}

fn install_skill_tool(
    repo_root: &Path,
    home: &Path,
    tool: &str,
    bootstrap_output_dir: Option<&Path>,
    skip_default_bootstrap: bool,
) -> Result<Value, String> {
    if tool == "codex" {
        let payload = install_native_integration(
            repo_root,
            &home.join(".codex").join("config.toml"),
            &home.join(".codex").join("skills"),
            bootstrap_output_dir,
            true,
            !skip_default_bootstrap,
        )?;
        return Ok(json!({
            "status": "installed",
            "changed": install_native_integration_changed(&payload),
            "native_integration": payload,
        }));
    }

    Err(format!("Unsupported tool: {tool}"))
}

fn install_native_integration_changed(payload: &Value) -> bool {
    [
        "created_config",
        "codex_hooks_feature_changed",
        "tui_status_line_changed",
        "home_codex_skills_link_changed",
    ]
    .iter()
    .any(|key| payload.get(*key).and_then(Value::as_bool) == Some(true))
        || payload
            .get("default_bootstrap")
            .and_then(|value| value.get("changed"))
            .and_then(Value::as_bool)
            == Some(true)
}

fn remove_skill_tool(repo_root: &Path, home: &Path, tool: &str) -> Result<Value, String> {
    if tool == "codex" {
        let target = home.join(".codex").join("skills");
        let changed = retire_home_skills_link(repo_root, &target)?;
        return Ok(json!({
            "status": if changed { "removed-link" } else { "native-surfaces-left-in-place" },
            "changed": changed,
            "target": target.to_string_lossy(),
        }));
    }

    Err(format!("Unsupported tool: {tool}"))
}

fn skill_tool_status_with_bootstrap(
    repo_root: &Path,
    home: &Path,
    tool: &str,
    bootstrap_output_dir: Option<&Path>,
) -> Result<Value, String> {
    if tool == "codex" {
        return codex_install_status_with_bootstrap(repo_root, home, bootstrap_output_dir);
    }

    Err(format!("Unsupported tool: {tool}"))
}

fn codex_install_status_with_bootstrap(
    repo_root: &Path,
    home: &Path,
    bootstrap_output_dir: Option<&Path>,
) -> Result<Value, String> {
    let config_path = home.join(".codex").join("config.toml");
    let codex_skills_path = home.join(".codex").join("skills");
    let bootstrap_path = bootstrap_output_dir
        .map(default_bootstrap_mirror_path)
        .unwrap_or_else(|| {
            default_bootstrap_output_dir(repo_root).join("framework_default_bootstrap.json")
        });
    let source = shared_skills_source(repo_root)?;

    let config_ok = codex_config_matches_contract(&config_path)?;
    let bootstrap_ok = validate_default_bootstrap(&bootstrap_path, repo_root)?;
    let codex_skills_ok = skills_stub_dir_matches_source(&codex_skills_path, &source)?;
    let ready = config_ok && bootstrap_ok && codex_skills_ok;

    Ok(json!({
        "ready": ready,
        "status": if ready { "native-integration-ready" } else { "native-integration-incomplete" },
        "target": codex_skills_path.to_string_lossy(),
        "source": source.to_string_lossy(),
        "checks": {
            "config": config_ok,
            "bootstrap": bootstrap_ok,
            "codex_skills": codex_skills_ok,
        },
    }))
}

fn codex_config_matches_contract(config_path: &Path) -> Result<bool, String> {
    let Some(content) = read_text_if_exists(config_path)? else {
        return Ok(false);
    };
    Ok(content.contains("[tui]") && content.lines().any(is_status_line))
}

fn shared_skills_source(repo_root: &Path) -> Result<PathBuf, String> {
    Ok(repo_root.join(skills_source_rel(repo_root)?))
}

fn skills_stub_dir_matches_source(
    target_path: &Path,
    expected_source: &Path,
) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(target_path).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !CODEX_STUB_SKILLS.contains(&name.as_str()) {
            return Ok(false);
        }
    }
    for skill in CODEX_STUB_SKILLS {
        let source = expected_source.join(skill).join("SKILL.md");
        let target = target_path.join(skill).join("SKILL.md");
        if fs::read(&source).ok() != fs::read(&target).ok() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn count_top_level_skills(skills_root: &Path) -> Result<usize, String> {
    let mut count = 0usize;
    for entry in fs::read_dir(skills_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() && !name.starts_with('.') && name != "dist" {
            count += 1;
        }
    }
    Ok(count)
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

#[allow(clippy::too_many_arguments)]
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
    let moved_current_artifacts =
        if apply_artifact_migrations && resolved_artifact_source_dir.is_none() {
            migrate_current_artifact_clutter(&repo_root, &active_task_id)?
        } else {
            Vec::new()
        };

    let consolidation = run_router_rs_json(
        &repo_root,
        &[
            "--codex-hook-command".to_string(),
            "session-end".to_string(),
            "--framework-max-lines".to_string(),
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
            "moved_current_artifacts": moved_current_artifacts,
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
        "moved_current_artifacts": moved_current_artifacts,
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
        "moved_current_artifacts": moved_current_artifacts,
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

fn router_rs_launcher_command(repo_root: &Path) -> PathBuf {
    repo_root.join("scripts/router-rs/run_router_rs.sh")
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

fn ensure_stub_skill_directory(source: &Path, target_path: &Path) -> Result<bool, String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let mut changed = false;
    if symlink_exists(target_path) {
        remove_path(target_path).map_err(|err| err.to_string())?;
        changed = true;
    } else if target_path.exists() {
        let metadata = fs::symlink_metadata(target_path).map_err(|err| err.to_string())?;
        if !metadata.file_type().is_dir() {
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
            changed = true;
        }
    }

    fs::create_dir_all(target_path).map_err(|err| err.to_string())?;
    for entry in fs::read_dir(target_path).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !CODEX_STUB_SKILLS.contains(&name.as_str()) {
            remove_path(&entry.path()).map_err(|err| err.to_string())?;
            changed = true;
        }
    }

    for skill in CODEX_STUB_SKILLS {
        let source_file = source.join(skill).join("SKILL.md");
        let target_file = target_path.join(skill).join("SKILL.md");
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let desired = fs::read(&source_file).map_err(|err| err.to_string())?;
        if fs::read(&target_file).ok().as_deref() != Some(desired.as_slice()) {
            fs::write(&target_file, desired).map_err(|err| err.to_string())?;
            changed = true;
        }
    }

    Ok(changed)
}

fn ensure_home_skills_link(repo_root: &Path, target_path: &Path) -> Result<bool, String> {
    let source = repo_root.join(skills_source_rel(repo_root)?);
    ensure_stub_skill_directory(&source, target_path)
}

fn retire_home_skills_link(repo_root: &Path, target_path: &Path) -> Result<bool, String> {
    let source = repo_root
        .join(skills_source_rel(repo_root)?)
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
