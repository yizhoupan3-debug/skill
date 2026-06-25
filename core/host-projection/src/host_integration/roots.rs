use super::*;

pub fn run_host_integration_payload(cli: Cli) -> Result<Value> {
    let payload = match cli.command {
        Commands::ExportRuntimeRegistry { repo_root } => {
            let framework_root = resolve_framework_root(repo_root.as_deref())?;
            serde_json::to_value(load_runtime_registry_payload(&framework_root)?)?
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
            bootstrap_output_dir,
            skip_default_bootstrap,
            host_id,
        } => install_native_integration(
            &repo_root,
            &home_config_path,
            bootstrap_output_dir.as_deref(),
            !skip_default_bootstrap,
            &host_id,
        )?,
        Commands::InstallSkills {
            repo_root,
            project_root,
            artifact_root,
            host_homes,
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
                host_homes,
                home: None,
                scope,
                to: selected,
                dry_run: false,
            };
            let normalized_command = canonical_install_skills_command(&command);
            let has_selected_targets = !projection_command.to.is_empty();
            match normalized_command.as_str() {
                "status" | "ls" if !has_selected_targets => {
                    let host_homes = projection_command.host_homes.clone();
                    projection_status_command(ProjectionStatusCommand {
                        framework_root: projection_command.framework_root.clone(),
                        project_root: projection_command.project_root.clone(),
                        artifact_root: projection_command.artifact_root.clone(),
                        host_homes,
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

pub fn normalize_path(path: &Path) -> Result<PathBuf> {
    let combined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?
            .join(path)
    };
    // Resolve . and .. components without requiring path to exist on disk
    let mut normalized = PathBuf::new();
    for component in combined.components() {
        match component {
            std::path::Component::ParentDir => { normalized.pop(); }
            std::path::Component::CurDir => {} // skip
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

pub fn try_framework_root_from_workspace_env() -> Option<PathBuf> {
    for name in framework_kernel::runtime_registry::ALL_HOST_WORKSPACE_ROOT_ENV_VARS {
        let Some(raw) = std::env::var_os(name) else {
            continue;
        };
        let candidate = PathBuf::from(raw);
        if let Ok(root) = normalize_path(&candidate)
            && is_framework_root(&root) {
                return Some(root);
            }
    }
    None
}

pub fn try_framework_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    framework_root_from_executable_path(&exe)
}

pub fn resolve_framework_root(explicit: Option<&Path>) -> Result<PathBuf> {
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
    if let Some(root) = try_framework_root_from_workspace_env() {
        return Ok(root);
    }
    if let Some(root) = try_framework_root_from_current_exe() {
        return Ok(root);
    }
    Err(FrameworkError::validation(
        "missing framework_root; pass --framework-root, set SKILL_FRAMEWORK_ROOT, \
         or set a registered host workspace root env var \
         to the framework checkout when cwd is outside the repo"
    ))
}

pub fn resolve_projection_framework_root(explicit: Option<&Path>) -> Result<PathBuf> {
    let root = resolve_framework_root(explicit)?;
    if !is_framework_root(&root) {
        return Err(FrameworkError::config(format!(
            "stale or missing framework_root: {}. Repair by passing --framework-root pointing at the framework checkout containing configs/framework/RUNTIME_REGISTRY.json and core/router-rs/Cargo.toml",
            root.display()
        )));
    }
    Ok(root)
}

pub fn resolve_project_root(
    explicit: Option<&Path>,
    framework_root: &Path,
) -> Result<PathBuf> {
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
    Err(FrameworkError::config(
        "missing project_root; pass --project-root or set SKILL_PROJECT_ROOT",
    ))
}

pub fn normalize_discovered_project_root(
    candidate: &Path,
    framework_root: &Path,
) -> Result<PathBuf> {
    let candidate = normalize_path(candidate)?;
    let framework_root = normalize_path(framework_root)?;
    if is_framework_root(&candidate) && candidate != framework_root {
        return Err(FrameworkError::config(format!(
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
) -> Result<PathBuf> {
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
) -> Result<(PathBuf, PathBuf)> {
    let framework_root = resolve_projection_framework_root(framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    Ok((framework_root, artifact_root))
}

/// Run `cargo metadata` on a manifest and return its target_directory.
fn cargo_metadata_target_dir(manifest: &Path) -> Option<PathBuf> {
    if !manifest.is_file() {
        return None;
    }
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path"])
        .arg(manifest)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let td = meta.get("target_directory")?.as_str()?;
    Some(PathBuf::from(td))
}

pub fn cargo_router_rs_executable(framework_root: &Path) -> Option<PathBuf> {
    let manifest = framework_root.join("core/router-rs/Cargo.toml");
    let td = cargo_metadata_target_dir(&manifest)?;
    for tail in ["release/router-rs-cli", "debug/router-rs-cli"] {
        let candidate = td.join(tail);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn is_ephemeral_executable_path(path: &str) -> bool {
    framework_kernel::router_self::is_ephemeral_router_rs_path(path)
}

pub fn is_repo_build_executable_path(path: &str, framework_root: &Path) -> bool {
    framework_kernel::router_self::is_repo_build_router_rs_path(path, framework_root)
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
    if let Ok(exe) = which::which("router-rs-cli") {
        let path_text = exe.to_string_lossy();
        if !is_ephemeral_executable_path(&path_text)
            && !is_repo_build_executable_path(&path_text, framework_root)
        {
            return McpRouterRsCommand::OnPath;
        }
    }
    let installed = framework_kernel::router_self::default_router_rs_install_path();
    if installed.is_file() {
        return McpRouterRsCommand::Absolute(installed);
    }
    McpRouterRsCommand::CargoBootstrap
}

pub fn mcp_router_rs_command_value(command: &McpRouterRsCommand) -> Value {
    match command {
        McpRouterRsCommand::OnPath => json!("router-rs-cli"),
        McpRouterRsCommand::Absolute(path) => json!(path.to_string_lossy()),
        McpRouterRsCommand::CargoBootstrap => json!("cargo"),
    }
}

pub fn ensure_router_rs_installed_for_mcp_with_roots(
    roots: &ResolvedProjectionRoots,
) -> Result<()> {
    if matches!(
        resolve_mcp_router_rs_command(&roots.framework_root),
        McpRouterRsCommand::CargoBootstrap
    ) {
        framework_kernel::router_self::ensure_router_rs_installed_for_runtime()?;
    }
    Ok(())
}

pub fn resolve_stable_router_rs_executable(framework_root: &Path) -> Option<PathBuf> {
    match resolve_mcp_router_rs_command(framework_root) {
        McpRouterRsCommand::OnPath => which::which("router-rs-cli").ok(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpCodegraphCommand {
    OnPath,
    Absolute(PathBuf),
    CargoBootstrap,
}

pub fn workspace_mcp_codegraph_release_binary(framework_root: &Path) -> Option<PathBuf> {
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(td).join("release/mcp-codegraph");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let manifest = framework_root.join("Cargo.toml");
    let td = cargo_metadata_target_dir(&manifest)?;
    let candidate = td.join("release/mcp-codegraph");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

pub fn resolve_mcp_codegraph_command(framework_root: &Path) -> McpCodegraphCommand {
    if let Ok(raw) = std::env::var("CODEGRAPH_MCP_BIN") {
        let trimmed = raw.trim();
        if !trimmed.is_empty()
            && Path::new(trimmed).is_file()
            && !is_ephemeral_executable_path(trimmed)
        {
            return McpCodegraphCommand::Absolute(PathBuf::from(trimmed));
        }
    }
    if let Ok(exe) = which::which("mcp-codegraph") {
        let path_text = exe.to_string_lossy();
        if !is_ephemeral_executable_path(&path_text) {
            return McpCodegraphCommand::OnPath;
        }
    }
    if let Some(path) = workspace_mcp_codegraph_release_binary(framework_root) {
        return McpCodegraphCommand::Absolute(path);
    }
    McpCodegraphCommand::CargoBootstrap
}

pub fn mcp_codegraph_command_value(command: &McpCodegraphCommand) -> Value {
    match command {
        McpCodegraphCommand::OnPath => json!("mcp-codegraph"),
        McpCodegraphCommand::Absolute(path) => json!(path.to_string_lossy()),
        McpCodegraphCommand::CargoBootstrap => json!("cargo"),
    }
}

pub fn codegraph_mcp_cargo_bootstrap_args(framework_root: &Path, repo_root: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "--release".to_string(),
        "--quiet".to_string(),
        "-p".to_string(),
        "codegraph-rs".to_string(),
        "--bin".to_string(),
        "mcp-codegraph".to_string(),
        "--manifest-path".to_string(),
        framework_root
            .join("Cargo.toml")
            .to_string_lossy()
            .into_owned(),
        "--".to_string(),
        "--repo-root".to_string(),
        repo_root.to_string(),
    ]
}

pub fn validate_mcp_command_binary(cmd: &str, framework_root: Option<&Path>) -> Result<()> {
    if cmd == "cargo" {
        if which::which("cargo").is_err() {
            return Err(FrameworkError::config(
                "Cargo is not found on system PATH",
            ));
        }
        return Ok(());
    }
    if cmd == "router-rs-cli" {
        if which::which("router-rs-cli").is_err() {
            return Err(FrameworkError::validation(
                "router-rs-cli is not found on system PATH; run `router-rs-cli self install`",
            ));
        }
        return Ok(());
    }
    let path = Path::new(cmd);
    if !path.is_file() {
        return Err(FrameworkError::config(format!(
            "MCP executable binary '{cmd}' is missing on disk"
        )));
    }
    if is_ephemeral_executable_path(cmd) {
        return Err(FrameworkError::config(format!(
            "MCP executable '{cmd}' points at an ephemeral build path; run `router-rs self install` then `framework host-integration install --to <host>`"
        )));
    }
    if framework_root.is_some_and(|root| is_repo_build_executable_path(cmd, root)) {
        return Err(FrameworkError::config(format!(
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
) -> Result<PathBuf> {
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

pub fn resolve_projection_roots(
    framework_root: Option<&Path>,
    project_root: Option<&Path>,
    artifact_root: Option<&Path>,
    host_homes: &[(String, PathBuf)],
    shared_home: Option<&Path>,
) -> Result<ResolvedProjectionRoots> {
    let framework_root = resolve_projection_framework_root(framework_root)?;
    let project_root = resolve_project_root(project_root, &framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    let account_home_root = match shared_home {
        Some(home) => normalize_path(home)?,
        None => default_home_dir(),
    };

    // Build host home roots map from registry-driven host list + explicit overrides.
    // Host ids, env vars, and default leafs are all derived from RUNTIME_REGISTRY.json.
    let host_home_roots = {
        let mut m = BTreeMap::new();
        // Map explicit CLI overrides by host_id for lookup
        // Hosts not in `host_homes` have no entry — same as None in the old per-host scheme.
        let explicit_overrides: std::collections::HashMap<&str, Option<&Path>> =
            host_homes.iter().map(|(id, path)| (id.as_str(), Some(path.as_path()))).collect();
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let env_var = framework_kernel::runtime_registry::home_env_var(host_id);
            let default_leaf = framework_kernel::runtime_registry::host_private_config_dir(host_id);
            if default_leaf.is_empty() {
                continue;
            }
            let explicit = explicit_overrides.get(host_id).and_then(|o| *o);
            m.insert(
                host_id.to_string(),
                resolve_host_home(explicit, shared_home, env_var, default_leaf)?,
            );
        }
        m
    };

    Ok(ResolvedProjectionRoots {
        framework_root,
        project_root,
        artifact_root,
        account_home_root,
        host_home_roots,
    })
}

pub fn nearest_marker_root(start: &Path, marker: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(marker).exists())
        .map(Path::to_path_buf)
}
