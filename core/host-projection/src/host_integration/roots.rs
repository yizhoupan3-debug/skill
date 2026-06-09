use super::*;
use router_rs::framework_error::{FrameworkError, FrameworkResult};

pub fn run_host_integration_payload(cli: Cli) -> FrameworkResult<Value> {
    let payload = match cli.command {
        Commands::ExportRuntimeRegistry { repo_root } => {
            let framework_root = resolve_framework_root(repo_root.as_deref())?;
            serde_json::to_value(load_runtime_registry_payload(&framework_root)?)
                .map_err(|err| FrameworkError::other(err.to_string()))?
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
            claude_home,
            antigravity_home,
                    opencode_home,
            to,
            scope,
            bootstrap_output_dir,
            skip_default_bootstrap,
            command,
            tools,
            ..
        } => {
            let _ = (bootstrap_output_dir, skip_default_bootstrap);
            let selected = install_skills_projection_tools(&command, &tools, &to);
            let projection_command = ProjectionCommand {
                framework_root: repo_root,
                project_root,
                artifact_root,
                codex_home,
                cursor_home,
                claude_home,
                antigravity_home: antigravity_home.clone(),
                            opencode_home,
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
                        claude_home: projection_command.claude_home.clone(),
                        antigravity_home: projection_command.antigravity_home.clone(),
                        opencode_home: projection_command.opencode_home.clone(),
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
            skip_generator_run,
        } => generated_artifacts_status(
            framework_root.as_deref(),
            artifact_root.as_deref(),
            skip_generator_run
                || std::env::var("ROUTER_RS_GENERATED_ARTIFACTS_SKIP_GENERATORS")
                    .ok()
                    .is_some_and(|value| {
                        matches!(
                            value.trim().to_ascii_lowercase().as_str(),
                            "1" | "true" | "yes" | "on"
                        )
                    }),
        )?,
    };
    Ok(payload)
}

pub fn normalize_path(path: &Path) -> FrameworkResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            ?
            .join(path))
    }
}

pub fn try_framework_root_from_workspace_env() -> Option<PathBuf> {
    for name in ["ROUTER_RS_CURSOR_WORKSPACE_ROOT", "CURSOR_WORKSPACE_ROOT"] {
        let Some(raw) = std::env::var_os(name) else {
            continue;
        };
        let candidate = PathBuf::from(raw);
        if let Ok(root) = normalize_path(&candidate) {
            if is_framework_root(&root) {
                return Some(root);
            }
        }
    }
    None
}

pub fn try_framework_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    framework_root_from_executable_path(&exe)
}

pub fn resolve_framework_root(explicit: Option<&Path>) -> FrameworkResult<PathBuf> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_FRAMEWORK_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    let cwd = std::env::current_dir()?;
    if is_framework_root(&cwd) {
        return normalize_path(&cwd);
    }
    for ancestor in cwd.ancestors() {
        if is_framework_root(ancestor) {
            return normalize_path(ancestor);
        }
    }
    if let Some(root) = try_framework_root_from_workspace_env() {
        return Ok(root);
    }
    if let Some(root) = try_framework_root_from_current_exe() {
        return Ok(root);
    }
    Err(
        FrameworkError::other(
            "missing framework_root; pass --framework-root, set SKILL_FRAMEWORK_ROOT, \
             or set ROUTER_RS_CURSOR_WORKSPACE_ROOT / CURSOR_WORKSPACE_ROOT to the framework checkout when cwd is outside the repo"
        ),
    )
}

pub fn resolve_projection_framework_root(explicit: Option<&Path>) -> FrameworkResult<PathBuf> {
    let root = resolve_framework_root(explicit)?;
    if !is_framework_root(&root) {
        return Err(FrameworkError::other(format!(
            "stale or missing framework_root: {}. Repair by passing --framework-root pointing at the framework checkout containing configs/framework/RUNTIME_REGISTRY.json and core/router-rs/Cargo.toml",
            root.display()
        )));
    }
    Ok(root)
}

pub fn resolve_project_root(explicit: Option<&Path>, framework_root: &Path) -> FrameworkResult<PathBuf> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_PROJECT_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    let cwd = std::env::current_dir()?;
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
    Err(FrameworkError::other("missing project_root; pass --project-root or set SKILL_PROJECT_ROOT"))
}

pub fn normalize_discovered_project_root(
    candidate: &Path,
    framework_root: &Path,
) -> FrameworkResult<PathBuf> {
    let candidate = normalize_path(candidate)?;
    let framework_root = normalize_path(framework_root)?;
    if is_framework_root(&candidate) && candidate != framework_root {
        return Err(FrameworkError::other(format!(
            "ambiguous project_root discovery: {} looks like a framework checkout but does not match framework_root {}. Pass both --framework-root and --project-root explicitly",
            candidate.display(),
            framework_root.display()
        )));
    }
    Ok(candidate)
}

pub fn resolve_artifact_root(
    explicit: Option<&Path>,
    framework_root: &Path,
) -> FrameworkResult<PathBuf> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(path) = std::env::var_os("SKILL_ARTIFACT_ROOT") {
        return normalize_path(&PathBuf::from(path));
    }
    Ok(framework_root.join("artifacts"))
}

pub fn resolve_maint_roots(
    framework_root: Option<&Path>,
    artifact_root: Option<&Path>,
) -> FrameworkResult<(PathBuf, PathBuf)> {
    let framework_root = resolve_projection_framework_root(framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    Ok((framework_root, artifact_root))
}

pub fn cargo_router_rs_executable(framework_root: &Path) -> Option<PathBuf> {
    let manifest = framework_root.join("core/router-rs/Cargo.toml");
    if !manifest.is_file() {
        return None;
    }
    let output = std::process::Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let td = meta.get("target_directory")?.as_str()?;
    let base = PathBuf::from(td);
    for tail in ["release/router-rs", "debug/router-rs"] {
        let candidate = base.join(tail);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn is_ephemeral_executable_path(path: &str) -> bool {
    router_rs::router_self::is_ephemeral_router_rs_path(path)
}

pub fn is_repo_build_executable_path(path: &str, framework_root: &Path) -> bool {
    router_rs::router_self::is_repo_build_router_rs_path(path, framework_root)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpRouterRsCommand {
    OnPath,
    Absolute(PathBuf),
    CargoBootstrap,
}

pub fn resolve_mcp_router_rs_command(framework_root: &Path) -> McpRouterRsCommand {
    if let Ok(raw) = std::env::var("ROUTER_RS_BIN") {
        let trimmed = raw.trim();
        if !trimmed.is_empty()
            && Path::new(trimmed).is_file()
            && !is_ephemeral_executable_path(trimmed)
            && validate_mcp_command_binary(trimmed, Some(framework_root)).is_ok()
        {
            return McpRouterRsCommand::Absolute(PathBuf::from(trimmed));
        }
    }
    if let Ok(exe) = which::which("router-rs") {
        let path_text = exe.to_string_lossy();
        if !is_ephemeral_executable_path(&path_text)
            && !is_repo_build_executable_path(&path_text, framework_root)
        {
            return McpRouterRsCommand::OnPath;
        }
    }
    let installed = router_rs::router_self::default_router_rs_install_path();
    if installed.is_file() {
        return McpRouterRsCommand::Absolute(installed);
    }
    McpRouterRsCommand::CargoBootstrap
}

pub fn mcp_router_rs_command_value(command: &McpRouterRsCommand) -> Value {
    match command {
        McpRouterRsCommand::OnPath => json!("router-rs"),
        McpRouterRsCommand::Absolute(path) => json!(path.to_string_lossy()),
        McpRouterRsCommand::CargoBootstrap => json!("cargo"),
    }
}

pub fn ensure_router_rs_installed_for_mcp_with_roots(roots: &ResolvedProjectionRoots) -> FrameworkResult<()> {
    if matches!(
        resolve_mcp_router_rs_command(&roots.framework_root),
        McpRouterRsCommand::CargoBootstrap
    ) {
        router_rs::router_self::ensure_router_rs_installed_for_runtime()?;
    }
    Ok(())
}


pub fn resolve_stable_router_rs_executable(framework_root: &Path) -> Option<PathBuf> {
    match resolve_mcp_router_rs_command(framework_root) {
        McpRouterRsCommand::OnPath => which::which("router-rs").ok(),
        McpRouterRsCommand::Absolute(path) => Some(path),
        McpRouterRsCommand::CargoBootstrap => None,
    }
}

pub fn router_rs_cargo_bootstrap_args(framework_root: &Path, host_args: &[&str]) -> Vec<String> {
    let manifest_path = framework_root.join("core/router-rs/Cargo.toml");
    let mut args = vec![
        "run".to_string(),
        "--release".to_string(),
        "--quiet".to_string(),
        "--manifest-path".to_string(),
        manifest_path.to_string_lossy().into_owned(),
        "--".to_string(),
    ];
    for arg in host_args {
        args.push(arg.to_string());
    }
    args
}

pub fn validate_mcp_command_binary(cmd: &str, framework_root: Option<&Path>) -> FrameworkResult<()> {
    if cmd == "cargo" {
        if which::which("cargo").is_err() {
            return Err(FrameworkError::other("Cargo is not found on system PATH"));
        }
        return Ok(());
    }
    if cmd == "router-rs" {
        if which::which("router-rs").is_err() {
            return Err(
                FrameworkError::other("router-rs is not found on system PATH; run `router-rs self install`"),
            );
        }
        return Ok(());
    }
    let path = Path::new(cmd);
    if !path.is_file() {
        return Err(FrameworkError::other(format!("MCP executable binary '{cmd}' is missing on disk")));
    }
    if is_ephemeral_executable_path(cmd) {
        return Err(FrameworkError::other(format!(
            "MCP executable '{cmd}' points at an ephemeral build path; run `router-rs self install` then `framework host-integration install --to <host>`"
        )));
    }
    if framework_root.is_some_and(|root| is_repo_build_executable_path(cmd, root)) {
        return Err(FrameworkError::other(format!(
            "MCP executable '{cmd}' points at a repo build artifact; run `router-rs self install` then `framework host-integration install --to <host>`"
        )));
    }
    Ok(())
}

pub fn resolve_host_home(
    explicit: Option<&Path>,
    shared_home: Option<&Path>,
    env_var: &str,
    default_leaf: &str,
) -> FrameworkResult<PathBuf> {
    if let Some(path) = explicit {
        return normalize_path(path);
    }
    if let Some(home) = shared_home {
        return Ok(normalize_path(home)?.join(default_leaf));
    }
    if let Some(path) = std::env::var_os(env_var) {
        return normalize_path(&PathBuf::from(path));
    }
    Ok(default_home_dir().join(default_leaf))
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_projection_roots(
    framework_root: Option<&Path>,
    project_root: Option<&Path>,
    artifact_root: Option<&Path>,
    codex_home: Option<&Path>,
    cursor_home: Option<&Path>,
    claude_home: Option<&Path>,
    antigravity_home: Option<&Path>,
    opencode_home: Option<&Path>,
    shared_home: Option<&Path>,
) -> FrameworkResult<ResolvedProjectionRoots> {
    let framework_root = resolve_projection_framework_root(framework_root)?;
    let project_root = resolve_project_root(project_root, &framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    let codex_home_root = resolve_host_home(codex_home, shared_home, "CODEX_HOME", ".codex")?;
    let cursor_home_root = resolve_host_home(cursor_home, shared_home, "CURSOR_HOME", ".cursor")?;
    let claude_home_root = resolve_host_home(claude_home, shared_home, "CLAUDE_HOME", ".claude")?;
    let antigravity_home_root = resolve_host_home(antigravity_home, shared_home, "ANTIGRAVITY_HOME", ".gemini")?;
        let opencode_home_root = resolve_host_home(
        opencode_home,
        shared_home,
        "OPENCODE_HOME",
        ".opencode",
    )?;
    let account_home_root = match shared_home {
        Some(home) => normalize_path(home)?,
        None => default_home_dir(),
    };
    Ok(ResolvedProjectionRoots {
        framework_root,
        project_root,
        artifact_root,
        account_home_root,
        codex_home_root,
        cursor_home_root,
        claude_home_root,
        antigravity_home_root,
        opencode_home_root,
    })
}

pub fn nearest_marker_root(start: &Path, marker: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(marker).exists())
        .map(Path::to_path_buf)
}

