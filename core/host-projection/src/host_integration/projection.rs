use super::*;

#[allow(clippy::too_many_arguments)]
pub fn install_native_integration(
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
    let (hooks_disabled_changed, deprecated_codex_hooks_removed) =
        ensure_codex_hooks_feature_disabled(&home_config_path)?;
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
        "hooks_enabled": false,
        "hooks_disabled_changed": hooks_disabled_changed,
        "deprecated_codex_hooks_removed": deprecated_codex_hooks_removed,
        "tui_status_line_changed": tui_changed,
        "home_codex_skills_changed": home_codex_skills_changed,
        "default_bootstrap": default_bootstrap,
    }))
}

pub fn projection_install_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
) -> Result<Value, String> {
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        command.codex_home.as_deref(),
        command.cursor_home.as_deref(),
        command.claude_home.as_deref(),
        command.antigravity_home.as_deref(),
        command.antigravity_cli_home.as_deref(),
        command.opencode_home.as_deref(),
        command.home.as_deref(),
    )?;
    let scope = canonical_scope(&command.scope)?;
    let selected_tools =
        selected_projection_tools(&roots.framework_root, &command.to, true, scope)?;
    let mut results = Map::new();
    for tool in selected_tools {
        results.insert(
            tool.to_string(),
            install_projection_tool(&roots, &tool, scope)?,
        );
    }
    projection_envelope("install", compatibility_alias, &roots, Some(scope), results)
}

pub fn projection_status_command(command: ProjectionStatusCommand) -> Result<Value, String> {
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        command.codex_home.as_deref(),
        command.cursor_home.as_deref(),
        command.claude_home.as_deref(),
        command.antigravity_home.as_deref(),
        command.antigravity_cli_home.as_deref(),
        command.opencode_home.as_deref(),
        command.home.as_deref(),
    )?;
    let mut results = Map::new();
    for tool in framework_kernel::framework_host_targets::skills_install_tools_ordered(&roots.framework_root)?
    {
        results.insert(tool.to_string(), projection_tool_status(&roots, &tool)?);
    }
    projection_envelope("status", false, &roots, None, results)
}

pub fn projection_remove_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
) -> Result<Value, String> {
    projection_remove_or_cleanup_command(command, compatibility_alias, false)
}

pub fn projection_cleanup_command(command: ProjectionCommand) -> Result<Value, String> {
    projection_remove_or_cleanup_command(command, false, true)
}

pub fn projection_remove_or_cleanup_command(
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
        command.claude_home.as_deref(),
        command.antigravity_home.as_deref(),
        command.antigravity_cli_home.as_deref(),
        command.opencode_home.as_deref(),
        command.home.as_deref(),
    )?;
    let scope = canonical_scope(&command.scope)?;
    let selected_tools =
        selected_projection_tools(&roots.framework_root, &command.to, false, scope)?;
    validate_cleanup_scope(&command, scope, &selected_tools, cleanup_mode)?;
    let mut results = Map::new();
    for tool in selected_tools {
        results.insert(
            tool.to_string(),
            remove_projection_tool(&roots, &tool, scope, command.dry_run)?,
        );
    }
    projection_envelope(
        if cleanup_mode { "cleanup" } else { "remove" },
        compatibility_alias,
        &roots,
        Some(scope),
        results,
    )
}

pub fn validate_cleanup_scope(
    command: &ProjectionCommand,
    scope: &str,
    tools: &[String],
    cleanup_mode: bool,
) -> Result<(), String> {
    if !cleanup_mode || scope != "user" {
        return Ok(());
    }
    for tool in tools {
        let explicit_home = projection_adapter(tool)
            .map(|adapter| (adapter.explicit_home)(command))
            .unwrap_or(true);
        if !explicit_home && command.home.is_none() {
            return Err(format!(
                "user-scope cleanup for {tool} requires explicit host-home resolution; pass --codex-home/--cursor-home/--claude-home, --home, or the matching host HOME environment variable"
            ));
        }
    }
    Ok(())
}

pub fn projection_envelope(
    command: &str,
    compatibility_alias: bool,
    roots: &ResolvedProjectionRoots,
    scope: Option<&str>,
    results: Map<String, Value>,
) -> Result<Value, String> {
    let pairs = framework_kernel::framework_host_targets::host_id_and_skills_install_tool_pairs(
        &roots.framework_root,
    )?;
    let registry =
        framework_kernel::runtime_registry::load_runtime_registry_json(&roots.framework_root)?;
    let mut host_targets_map = serde_json::Map::new();
    for (host_id, tool) in &pairs {
        let value = if framework_kernel::framework_host_targets::host_is_installable(&registry, host_id)? {
            results.get(tool.as_str()).cloned().unwrap_or(Value::Null)
        } else {
            results.get(host_id.as_str()).cloned().unwrap_or_else(|| {
                non_installable_projection_result(host_id, scope.unwrap_or("status"))
            })
        };
        host_targets_map.insert(host_id.clone(), value);
    }
    let host_targets = Value::Object(host_targets_map);
    Ok(json!({
        "success": true,
        "command": command,
        "invocation": {
            "primary_command": "framework host-integration",
            "alias_used": if compatibility_alias { Value::String("install-skills".to_string()) } else { Value::Null },
            "deprecated_alias": compatibility_alias,
        },
        "resolved_roots": resolved_roots_payload(roots, &pairs)?,
        "scope": scope.unwrap_or("all-scopes-status"),
        "results": results,
        "host_targets": host_targets,
    }))
}

pub fn resolved_roots_payload(
    roots: &ResolvedProjectionRoots,
    pairs: &[(String, String)],
) -> Result<Value, String> {
    let mut host_home_roots = serde_json::Map::new();
    let registry =
        framework_kernel::runtime_registry::load_runtime_registry_json(&roots.framework_root)?;
    for (host_id, tool) in pairs {
        if !framework_kernel::framework_host_targets::host_is_installable(&registry, host_id)? {
            host_home_roots.insert(host_id.clone(), Value::Null);
            continue;
        }
        let adapter = projection_adapter(tool).ok_or_else(|| {
            format!(
                "resolved_roots_payload: unsupported skills-install tool `{tool}` \
                 (extend host projection adapters when adding hosts)"
            )
        })?;
        host_home_roots.insert(host_id.clone(), json!((adapter.home_root)(roots)));
    }
    Ok(json!({
        "framework_root": roots.framework_root.to_string_lossy(),
        "project_root": roots.project_root.to_string_lossy(),
        "artifact_root": roots.artifact_root.to_string_lossy(),
        "host_home_roots": Value::Object(host_home_roots),
    }))
}

pub fn selected_projection_tools(
    framework_root: &Path,
    raw_tools: &[String],
    default_all: bool,
    scope: &str,
) -> Result<Vec<String>, String> {
    if raw_tools.is_empty() && default_all {
        return default_projection_tools_for_scope(framework_root, scope);
    }
    let mut selected = Vec::new();
    for raw in raw_tools {
        if raw.trim().eq_ignore_ascii_case("all") {
            for tool in default_projection_tools_for_scope(framework_root, scope)? {
                if !selected.contains(&tool) {
                    selected.push(tool);
                }
            }
            continue;
        }
        let tool = canonical_tool_name(raw, framework_root)?;
        if !selected.contains(&tool) {
            selected.push(tool);
        }
    }
    if selected.is_empty() {
        let known = projection_supported_tools_for_message(framework_root);
        return Err(format!(
            "projection command requires --to <tool> or --to all. Supported tools: {}",
            known.join(", ")
        ));
    }
    Ok(selected)
}

pub fn default_projection_tools_for_scope(
    framework_root: &Path,
    scope: &str,
) -> Result<Vec<String>, String> {
    let mut tools = registry_projection_tools(framework_root)?;
    if canonical_scope(scope)? == "project" {
        tools.retain(|tool| tool != "claude");
    }
    Ok(tools)
}

pub struct HostProjectionAdapter {
    tool: &'static str,
    host_id: &'static str,
    aliases: &'static [&'static str],
    install: fn(&ResolvedProjectionRoots, &str) -> Result<Value, String>,
    status: fn(&ResolvedProjectionRoots) -> Result<Value, String>,
    remove: fn(&ResolvedProjectionRoots, &str, bool) -> Result<Value, String>,
    home_root: fn(&ResolvedProjectionRoots) -> String,
    explicit_home: fn(&ProjectionCommand) -> bool,
}

const HOST_PROJECTION_ADAPTERS: &[HostProjectionAdapter] = &[
    HostProjectionAdapter {
        tool: "cursor",
        host_id: "cursor",
        aliases: &[],
        install: install_cursor_projection,
        status: cursor_projection_status,
        remove: remove_cursor_projection,
        home_root: cursor_home_root_string,
        explicit_home: cursor_home_explicit,
    },
    HostProjectionAdapter {
        tool: "claude",
        host_id: "claude-code",
        aliases: &["claude-code"],
        install: install_claude_projection,
        status: claude_projection_status,
        remove: remove_claude_projection,
        home_root: claude_home_root_string,
        explicit_home: claude_home_explicit,
    },
    HostProjectionAdapter {
        tool: "antigravity",
        host_id: "antigravity",
        aliases: &["antigravity-app"],
        install: install_antigravity_projection,
        status: antigravity_projection_status,
        remove: remove_antigravity_projection,
        home_root: antigravity_home_root_string,
        explicit_home: antigravity_home_explicit,
    },
    HostProjectionAdapter {
        tool: "opencode",
        host_id: "opencode",
        aliases: &[],
        install: install_opencode_projection,
        status: opencode_projection_status,
        remove: remove_opencode_projection,
        home_root: opencode_home_root_string,
        explicit_home: opencode_home_explicit,
    },
    HostProjectionAdapter {
        tool: "codex",
        host_id: "codex",
        aliases: &["codex-cli", "codex-app"],
        install: install_codex_projection,
        status: codex_projection_status,
        remove: remove_codex_projection,
        home_root: codex_home_root_string,
        explicit_home: codex_home_explicit,
    },
];

pub fn opencode_config_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.account_home_root.join(".config/opencode/opencode.json")
    } else {
        roots.project_root.join(".opencode/opencode.json")
    }
}

pub fn opencode_projection_config_dir(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.account_home_root.join(".config/opencode")
    } else {
        roots.project_root.join(".opencode")
    }
}

pub fn opencode_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload_with_env(
        roots,
        &["opencode", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
        "Framework snapshot, skill routing, goal/closeout gating (MCP advisory for my-light)",
        None,
    )
}

/// Shared browser-mcp stdio payload (all hosts). Uses framework_root as repo-root.
pub fn browser_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    let args = vec![
        "browser".to_string(),
        "mcp-stdio".to_string(),
        "--repo-root".to_string(),
        roots.framework_root.to_string_lossy().into_owned(),
    ];
    match resolve_mcp_router_rs_command(&roots.framework_root) {
        McpRouterRsCommand::CargoBootstrap => json!({
            "command": "cargo",
            "args": router_rs_cargo_bootstrap_args(&roots.framework_root, &[
                "browser", "mcp-stdio", "--repo-root",
                &roots.framework_root.to_string_lossy(),
            ]),
            "type": "stdio",
            "description": "Browser automation, session worker, background tasks (via Cargo bootstrap)",
        }),
        command => json!({
            "command": mcp_router_rs_command_value(&command),
            "args": args,
            "type": "stdio",
            "description": "Browser automation, session worker, background tasks",
        }),
    }
}

pub fn install_opencode_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    let config_path = opencode_config_path(roots, scope);
    let config_dir = config_path.parent().ok_or_else(|| {
        format!("cannot determine parent directory of {}", config_path.display())
    })?;
    std::fs::create_dir_all(config_dir)
        .map_err(|err| format!("failed to create {}: {err}", config_dir.display()))?;

    let mut payload = read_json_if_exists(&config_path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let servers = payload.as_object_mut()
        .ok_or_else(|| "opencode.json root must be an object".to_string())?;
    let mcp_servers = servers
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !mcp_servers.is_object() {
        *mcp_servers = json!({});
    }
    let entries = mcp_servers.as_object_mut()
        .ok_or_else(|| "mcpServers must be an object".to_string())?;
    let framework_payload = opencode_mcp_server_payload(roots);
    let framework_changed = entries.get("router-rs-framework") != Some(&framework_payload);
    entries.insert("router-rs-framework".to_string(), framework_payload);
    let browser_payload = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser_payload);
    entries.insert("browser-mcp".to_string(), browser_payload);
    let paperplain_changed = merge_paperplain_into_mcp_servers_map(entries, "paperplain");
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    write_json_if_changed(&config_path, &payload)?;
    let changed = framework_changed || browser_changed || paperplain_changed || codegraph_changed;

    let manifest_dir = opencode_projection_config_dir(roots, scope);
    std::fs::create_dir_all(&manifest_dir)
        .map_err(|err| format!("failed to create {}: {err}", manifest_dir.display()))?;
    let manifest_path = manifest_dir.join(FRAMEWORK_PROJECTION_MANIFEST_NAME);
    let manifest_changed = write_json_if_changed(
        &manifest_path,
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "opencode",
            "scope": scope,
            "files": [projection_manifest_file_ref(roots, &config_path)],
            "settings": {
                "managed_key_paths": [
                    "mcpServers.router-rs-framework",
                    "mcpServers.browser-mcp",
                    "mcpServers.paperplain",
                    "mcpServers.mcp-codegraph",
                ],
            }
        }),
    )?;

    Ok(json!({
        "status": "installed",
        "changed": changed || manifest_changed,
        "scope": scope,
        "mcp_config": {
            "scope": scope,
            "path": config_path.to_string_lossy(),
            "changed": changed,
        },
        "projection_manifest": {
            "path": manifest_path.to_string_lossy(),
            "changed": manifest_changed,
        },
    }))
}

pub fn opencode_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_path = opencode_config_path(roots, "project");
    let user_path = opencode_config_path(roots, "user");
    let project_exists = project_path.is_file();
    let user_exists = user_path.is_file();

    // Pick the best available config (project first, then user)
    let config_payload = read_json_if_exists(&project_path).ok().flatten()
        .or_else(|| read_json_if_exists(&user_path).ok().flatten());

    let managed_servers = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
    let mut server_status: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut all_valid = true;
    let mut first_error = None;

    if let Some(ref payload) = config_payload {
        let servers = payload.get("mcpServers").and_then(Value::as_object);
        for server_id in &managed_servers {
            let entry = servers.and_then(|s| s.get(*server_id));
            if let Some(cmd) = entry.and_then(|v| v.get("command")).and_then(Value::as_str) {
                match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                    Ok(()) => {
                        // Deep validation for router-rs-based servers
                        if *server_id == "router-rs-framework" && cmd != "cargo" {
                            let resolved = if cmd == "router-rs" {
                                resolve_stable_router_rs_executable(&roots.framework_root)
                            } else {
                                Some(PathBuf::from(cmd))
                            };
                            match resolved {
                                Some(path) => match framework_kernel::router_self::validate_router_rs_binary_runnable(&path) {
                                    Ok(()) => { server_status.insert(server_id.to_string(), json!({"binary_valid": true})); }
                                    Err(err) => {
                                        all_valid = false;
                                        if first_error.is_none() { first_error = Some(err.clone()); }
                                        server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": err}));
                                    }
                                },
                                None => {
                                    all_valid = false;
                                    let msg = "router-rs not found on PATH; run `router-rs self install`".to_string();
                                    if first_error.is_none() { first_error = Some(msg.clone()); }
                                    server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": msg}));
                                }
                            }
                        } else {
                            server_status.insert(server_id.to_string(), json!({"binary_valid": true}));
                        }
                    }
                    Err(err) => {
                        all_valid = false;
                        if first_error.is_none() { first_error = Some(err.clone()); }
                        server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": err}));
                    }
                }
            } else {
                all_valid = false;
                let msg = format!("missing or incomplete {server_id} payload");
                if first_error.is_none() { first_error = Some(msg.clone()); }
                server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": msg}));
            }
        }
    } else {
        all_valid = false;
        first_error = Some("No opencode.json found in project or user scope".to_string());
    }

    Ok(json!({
        "ready": (project_exists || user_exists) && all_valid,
        "status": "projection-status",
        "error": first_error,
        "mcp_config": {
            "project_scope": project_exists,
            "user_scope": user_exists,
            "all_binaries_valid": all_valid,
            "servers": server_status,
        },
        "projection_manifest": {
            "project_scope": opencode_projection_config_dir(roots, "project").join(FRAMEWORK_PROJECTION_MANIFEST_NAME).exists(),
            "user_scope": opencode_projection_config_dir(roots, "user").join(FRAMEWORK_PROJECTION_MANIFEST_NAME).exists(),
        },
    }))
}

pub fn remove_opencode_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let config_path = opencode_config_path(roots, scope);
    let config_dir = opencode_projection_config_dir(roots, scope);

    let mut config_removed = false;
    if config_path.is_file() && !dry_run {
        let mut payload = read_json_if_exists(&config_path)?
            .unwrap_or_else(|| json!({}));
        let mut changed = false;
        let managed_keys = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
        if let Some(servers) = payload.get_mut("mcpServers").and_then(Value::as_object_mut) {
            for key in &managed_keys {
                changed |= servers.remove(*key).is_some();
            }
        }
        if changed {
            write_json_if_changed(&config_path, &payload)?;
        }
        config_removed = changed;
    }

    let manifest_path = config_dir.join(FRAMEWORK_PROJECTION_MANIFEST_NAME);
    let manifest_removed = if manifest_path.is_file() && !dry_run {
        std::fs::remove_file(&manifest_path).is_ok()
    } else {
        false
    };

    Ok(json!({
        "status": if config_removed || manifest_removed { "removed" } else { "not-found" },
        "changed": config_removed || manifest_removed,
        "scope": scope,
        "dry_run": dry_run,
        "mcp_framework_entry_removed": config_removed,
        "projection_manifest_removed": manifest_removed,
    }))
}

pub fn opencode_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.opencode_home_root.to_string_lossy().into_owned()
}

pub fn opencode_home_explicit(command: &ProjectionCommand) -> bool {
    command.opencode_home.is_some() || std::env::var_os("OPENCODE_HOME").is_some()
}

pub fn projection_adapter(tool: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = tool.trim().to_lowercase();
    HOST_PROJECTION_ADAPTERS
        .iter()
        .find(|adapter| adapter.tool == normalized)
}

pub fn projection_adapter_for_raw(raw: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = raw.trim().to_lowercase();
    HOST_PROJECTION_ADAPTERS.iter().find(|adapter| {
        adapter.tool == normalized || adapter.aliases.iter().any(|alias| *alias == normalized)
    })
}

pub fn registry_projection_tools(framework_root: &Path) -> Result<Vec<String>, String> {
    let pairs = framework_kernel::framework_host_targets::installable_host_id_and_skills_install_tool_pairs(
        framework_root,
    )?;
    let mut tools = Vec::new();
    for (host_id, tool) in pairs {
        let adapter = projection_adapter(&tool).ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY host {host_id:?} declares unsupported install_tool {tool:?}; extend host projection adapters"
            )
        })?;
        if !tools.contains(&adapter.tool.to_string()) {
            tools.push(adapter.tool.to_string());
        }
    }
    validate_projection_adapters_against_registry(framework_root)?;
    let registry = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root)?;
    framework_kernel::framework_host_targets::validate_host_providers_against_registry(&registry)?;
    Ok(tools)
}

pub fn validate_projection_adapters_against_registry(framework_root: &Path) -> Result<(), String> {
    let registry = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root)?;
    let supported = framework_kernel::framework_host_targets::host_targets_supported_host_ids(&registry)?;
    for adapter in HOST_PROJECTION_ADAPTERS {
        if !supported.iter().any(|host_id| host_id == adapter.host_id) {
            return Err(format!(
                "host projection adapter `{}` declares host_id `{}` outside RUNTIME_REGISTRY.host_targets.supported",
                adapter.tool, adapter.host_id
            ));
        }
    }
    Ok(())
}

pub fn projection_alias_summary() -> String {
    HOST_PROJECTION_ADAPTERS
        .iter()
        .flat_map(|adapter| {
            adapter
                .aliases
                .iter()
                .map(move |alias| format!("{alias} → {}", adapter.tool))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn canonical_scope(scope: &str) -> Result<&'static str, String> {
    match scope.trim().to_lowercase().as_str() {
        "" | "project" | "project-local" => Ok("project"),
        "user" => Ok("user"),
        other => Err(format!(
            "Unsupported scope: {other}. Supported scopes: project user"
        )),
    }
}

/// Cursor framework rules (`framework.mdc`) and browser MCP projection are **user-scope only**.
/// Project repos keep `.cursor/hooks.json` and harness gate rules locally.
pub fn projection_scope_for_tool(tool: &str, scope: &str) -> Result<&'static str, String> {
    if projection_adapter(tool).is_some_and(|adapter| adapter.tool == "cursor") {
        let _ = canonical_scope(scope)?;
        return Ok("user");
    }
    canonical_scope(scope)
}

pub fn install_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
) -> Result<Value, String> {
    if tool.contains("..") || tool.contains('/') || tool.contains('\\') {
        return Err(format!("Invalid tool name: {}", tool));
    }
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    let out = (adapter.install)(roots, effective_scope)?;
    // MCP 配置已统一由各宿主的 user-level install 路径写入，
    // 不再在 project scope 写入 .mcp.json / .codex/config.toml。
    Ok(out)
}

pub fn projection_tool_status(roots: &ResolvedProjectionRoots, tool: &str) -> Result<Value, String> {
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    (adapter.status)(roots)
}

pub fn remove_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    (adapter.remove)(roots, effective_scope, dry_run)
}

pub fn non_installable_projection_result(host_id: &str, scope: &str) -> Value {
    json!({
        "status": "unsupported",
        "supported": false,
        "installable": false,
        "host_id": host_id,
        "scope": scope,
        "reason": "runtime-supported host does not install file projections",
    })
}

pub fn codex_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.codex_home_root.to_string_lossy().into_owned()
}

pub fn cursor_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.cursor_home_root.to_string_lossy().into_owned()
}

pub fn claude_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.claude_home_root.to_string_lossy().into_owned()
}

pub fn codex_home_explicit(command: &ProjectionCommand) -> bool {
    command.codex_home.is_some() || std::env::var_os("CODEX_HOME").is_some()
}

pub fn cursor_home_explicit(command: &ProjectionCommand) -> bool {
    command.cursor_home.is_some() || std::env::var_os("CURSOR_HOME").is_some()
}

pub fn claude_home_explicit(command: &ProjectionCommand) -> bool {
    command.claude_home.is_some() || std::env::var_os("CLAUDE_HOME").is_some()
}

pub fn antigravity_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.antigravity_home_root.to_string_lossy().into_owned()
}

pub fn antigravity_home_explicit(command: &ProjectionCommand) -> bool {
    command.antigravity_home.is_some() || std::env::var_os("ANTIGRAVITY_HOME").is_some()
}


pub fn install_codex_projection(roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
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

pub fn codex_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
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
            "project": projection_manifest_status(&projection_manifest_path(roots, "codex", "project"))?,
            "user": projection_manifest_status(&projection_manifest_path(roots, "codex", "user"))?,
        },
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
    }))
}

pub fn install_cursor_projection(
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
        ensure_router_rs_installed_for_mcp_with_roots(roots)?;
        let mcp_path = cursor_mcp_config_path(roots);
        let mcp_install = install_cursor_mcp_server(roots, &mcp_path)?;
        changed |= mcp_install.changed;
        if mcp_install.managed {
            managed_files.push(mcp_path.to_string_lossy().to_string());
            managed_key_paths.push(cursor_mcp_server_key_path().to_string());
            managed_key_paths.push(cursor_router_rs_framework_key_path().to_string());
            managed_key_paths.push(cursor_codegraph_mcp_server_key_path().to_string());
            managed_key_paths.push(cursor_paperplain_mcp_server_key_path().to_string());
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

pub fn cursor_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let user_target = cursor_entrypoint_target(roots, "user");
    let mcp_path = cursor_mcp_config_path(roots);
    let rules_ready = managed_projection_file_exists(&user_target)?;
    let mcp_exists = mcp_path.is_file();
    let managed_servers = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
    let mut server_status: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut all_valid = true;
    let mut first_error = None;

    if mcp_exists {
        if let Ok(Some(mcp_json)) = read_json_if_exists(&mcp_path) {
            let servers = mcp_json.get("mcp_servers").and_then(Value::as_object);
            for server_id in &managed_servers {
                let entry = servers.and_then(|s| s.get(*server_id));
                if let Some(cmd) = entry.and_then(|v| v.get("command")).and_then(Value::as_str) {
                    match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                        Ok(()) => { server_status.insert(server_id.to_string(), json!({"binary_valid": true})); }
                        Err(err) => {
                            all_valid = false;
                            if first_error.is_none() { first_error = Some(err.clone()); }
                            server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": err}));
                        }
                    }
                } else {
                    all_valid = false;
                    let msg = format!("missing or incomplete {server_id} payload");
                    if first_error.is_none() { first_error = Some(msg.clone()); }
                    server_status.insert(server_id.to_string(), json!({"binary_valid": false, "error": msg}));
                }
            }
        } else {
            all_valid = false;
            first_error = Some("Failed to read or parse ~/.cursor/mcp.json".to_string());
        }
    } else {
        all_valid = false;
        first_error = Some("~/.cursor/mcp.json does not exist".to_string());
    }

    let ready = rules_ready && mcp_exists && all_valid;
    Ok(json!({
        "ready": ready,
        "status": "projection-status",
        "error": first_error,
        "rules": {
            "framework": {
                "user": cursor_projection_file_status(&user_target)?,
            }
        },
        "mcp_config": {
            "user_scope": mcp_exists,
            "project_scope": false,
            "all_binaries_valid": all_valid,
            "path": mcp_path.to_string_lossy(),
            "servers": server_status,
        },
        "manifest": {
            "user": projection_manifest_status(&projection_manifest_path(roots, "cursor", "user"))?,
        },
        "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
        "policy": "user-scope-only",
    }))
}

pub fn remove_codex_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let target = codex_entrypoint_target(roots, scope);
    let manifest_path = projection_manifest_path(roots, "codex", scope);
    let manifest_ownership =
        projection_manifest_ownership(&manifest_path, "codex", scope, &target)?;
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
    let toml_removed = if !dry_run && any_changed {
        remove_codex_mcp_toml_entries(roots).unwrap_or(false)
    } else {
        false
    };
    Ok(json!({
        "status": if dry_run && (would_remove_projection || would_remove_manifest) { "would-remove" } else if any_changed || toml_removed { "removed" } else { "not-installed-or-user-owned" },
        "changed": any_changed || toml_removed,
        "dry_run": dry_run,
        "scope": scope,
        "removed_paths": removed_projection_paths(changed, &target, manifest_removed, &manifest_path),
        "would_remove_paths": removed_projection_paths(would_remove_projection, &target, would_remove_manifest, &manifest_path),
        "skipped_user_owned_paths": if would_remove_projection || !target.exists() { json!([]) } else { json!([target.to_string_lossy()]) },
    }))
}

pub fn remove_cursor_projection(
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

pub fn claude_entrypoint_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.claude_home_root.join("rules").join("framework.md")
    } else {
        roots
            .project_root
            .join(".claude")
            .join("rules")
            .join("framework.md")
    }
}

pub fn claude_project_narrative_path(roots: &ResolvedProjectionRoots) -> PathBuf {
    roots.project_root.join(".claude").join("CLAUDE.md")
}

pub fn claude_settings_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.claude_home_root.join("settings.json")
    } else {
        roots.project_root.join(".claude").join("settings.json")
    }
}

pub fn build_router_rs_claude_hook_command(event: &str) -> String {
    format!(
        "/usr/bin/env bash -c 'ROOT=\"${{CLAUDE_PROJECT_ROOT:-$PWD}}\"; FW=\"${{SKILL_FRAMEWORK_ROOT:-$ROOT}}\"; if [[ -r \"$ROOT/.claude/router-rs-hook.env\" ]]; then set -a; . \"$ROOT/.claude/router-rs-hook.env\"; set +a; fi; exec \"$FW/configs/framework/claude-router-rs-hook.sh\" {event}'",
        event = event
    )
}

pub fn managed_claude_hook_entry(event: &str) -> Value {
    json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": build_router_rs_claude_hook_command(event),
        }]
    })
}

pub fn value_contains_router_rs_claude_hook(value: &Value) -> bool {
    match value {
        Value::String(s) => {
            s.contains("claude-router-rs-hook.sh")
                || s.contains("router-rs-hook.sh")
                || (s.contains("router-rs") && s.contains("claude hook"))
        }
        Value::Array(items) => items.iter().any(value_contains_router_rs_claude_hook),
        Value::Object(map) => map.values().any(value_contains_router_rs_claude_hook),
        _ => false,
    }
}

/// Core hook events required for all Claude Code projections.
/// These events are always installed regardless of host version.
const CORE_HOOK_EVENTS: &[&str] = &[
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
];

/// Optional hook events that may not be supported by all Claude Desktop versions.
/// Installation continues gracefully if these are absent from the host.
const OPTIONAL_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SubagentStart",
    "SubagentStop",
];

/// All hook events (core + optional), in canonical order.
const ALL_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];

pub fn merge_claude_settings_hooks(existing: Option<Value>) -> Result<Value, String> {
    let mut root = match existing {
        Some(Value::Object(map)) => map,
        Some(_) => return Err("Claude settings root must be a JSON object".to_string()),
        None => Map::new(),
    };
    let mut hooks = match root.remove("hooks") {
        Some(Value::Object(map)) => map,
        Some(_) => return Err("Claude settings `hooks` must be a JSON object".to_string()),
        None => Map::new(),
    };
    for &event in ALL_HOOK_EVENTS {
        let mut entries = hooks
            .remove(event)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        entries.retain(|entry| !value_contains_router_rs_claude_hook(entry));
        entries.push(managed_claude_hook_entry(event));
        hooks.insert(event.to_string(), Value::Array(entries));
    }
    root.insert("hooks".to_string(), Value::Object(hooks));
    Ok(Value::Object(root))
}

pub fn install_claude_settings_hooks(settings_path: &Path) -> Result<bool, String> {
    let existing = read_json_if_exists(settings_path)?;
    let merged = merge_claude_settings_hooks(existing)?;
    write_json_if_changed(settings_path, &merged)
}

pub fn install_claude_hook_env_if_absent(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let dest = roots.project_root.join(".claude/router-rs-hook.env");
    if dest.is_file() {
        return Ok(false);
    }
    let template = roots
        .framework_root
        .join("configs/framework/claude-router-rs-hook.env");
    if !template.is_file() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&template, &dest).map_err(|e| {
        format!(
            "install claude hook env: copy {} -> {}: {e}",
            template.display(),
            dest.display()
        )
    })?;
    Ok(true)
}

pub fn install_claude_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    let target = claude_entrypoint_target(roots, scope);
    let settings_path = claude_settings_target(roots, scope);
    let changed =
        write_text_if_changed(&target, &render_claude_framework_entrypoint(roots, scope))?;
    let narrative_changed = if scope == "project" {
        write_text_if_changed(
            &claude_project_narrative_path(roots),
            &render_claude_project_narrative(roots),
        )?
    } else {
        false
    };
    let hooks_changed = install_claude_settings_hooks(&settings_path)?;
    let env_changed = install_claude_hook_env_if_absent(roots)?;
    let manifest_changed = write_claude_projection_manifest(roots, scope, &target, &settings_path)?;
    Ok(json!({
        "status": "installed",
        "changed": changed || narrative_changed || hooks_changed || env_changed || manifest_changed,
        "scope": scope,
        "prompts": {
            "framework": {
                "scope": scope,
                "path": target.to_string_lossy(),
                "logical_entrypoint": "$framework",
                "native_representation": "markdown-rule",
            }
        },
        "hooks": {
            "managed": true,
            "path": settings_path.to_string_lossy(),
            "changed": hooks_changed,
            "events": ALL_HOOK_EVENTS.to_vec(),
        },
        "aliases": {"managed": false, "reason": "compatibility-aliases-not-managed-by-default-projection"},
    }))
}

pub fn claude_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_target = claude_entrypoint_target(roots, "project");
    let user_target = claude_entrypoint_target(roots, "user");
    let project_settings = claude_settings_target(roots, "project");
    let user_settings = claude_settings_target(roots, "user");
    Ok(json!({
        "ready": managed_projection_file_exists(&project_target)? || managed_projection_file_exists(&user_target)?,
        "status": "projection-status",
        "prompts": {
            "framework": {
                "project": claude_projection_file_status(&project_target)?,
                "user": claude_projection_file_status(&user_target)?,
            }
        },
        "manifest": {
            "project": projection_manifest_status(&projection_manifest_path(roots, "claude-code", "project"))?,
            "user": projection_manifest_status(&projection_manifest_path(roots, "claude-code", "user"))?,
        },
        "hooks": {
            "project": claude_settings_hook_status(&project_settings)?,
            "user": claude_settings_hook_status(&user_settings)?,
        },
    }))
}

pub fn remove_claude_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let target = claude_entrypoint_target(roots, scope);
    let settings_path = claude_settings_target(roots, scope);
    let manifest_path = projection_manifest_path(roots, "claude-code", scope);
    let manifest_ownership =
        projection_manifest_ownership(&manifest_path, "claude-code", scope, &target)?;
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
    let settings_removal = remove_claude_settings_hooks(&settings_path, dry_run)?;
    // Clean up .mcp.json MCP entries (shared file, gitignored runtime artifact)
    let mcp_cleaned = if !dry_run {
        remove_project_mcp_json_entries(roots).unwrap_or(false)
    } else {
        roots.project_root.join(".mcp.json").is_file()
    };
    let any_changed = changed || manifest_removed || settings_removal.changed || mcp_cleaned;
    let would_remove_any =
        would_remove_projection || would_remove_manifest || settings_removal.would_change;
    let mut removed_paths =
        removed_projection_paths(changed, &target, manifest_removed, &manifest_path);
    if settings_removal.removed_file {
        if let Some(paths) = removed_paths.as_array_mut() {
            paths.push(Value::String(settings_path.to_string_lossy().into_owned()));
        }
    }
    let mut would_remove_paths = removed_projection_paths(
        would_remove_projection,
        &target,
        would_remove_manifest,
        &manifest_path,
    );
    if settings_removal.would_remove_file {
        if let Some(paths) = would_remove_paths.as_array_mut() {
            paths.push(Value::String(settings_path.to_string_lossy().into_owned()));
        }
    }
    Ok(json!({
        "status": if dry_run && would_remove_any { "would-remove" } else if any_changed { "removed" } else { "not-installed-or-user-owned" },
        "changed": any_changed,
        "dry_run": dry_run,
        "scope": scope,
        "removed_paths": removed_paths,
        "would_remove_paths": would_remove_paths,
        "settings": {
            "path": settings_path.to_string_lossy(),
            "changed": settings_removal.changed,
            "would_change": dry_run && settings_removal.would_change,
            "removed_file": settings_removal.removed_file,
            "would_remove_file": dry_run && settings_removal.would_remove_file,
            "removed_events": settings_removal.removed_events,
        },
        "skipped_user_owned_paths": if would_remove_projection || !target.exists() { json!([]) } else { json!([target.to_string_lossy()]) },
    }))
}

#[derive(Debug, Default)]
pub struct AgentSettingsRemoval {
    changed: bool,
    would_change: bool,
    removed_file: bool,
    would_remove_file: bool,
    removed_events: Vec<String>,
}

pub fn remove_claude_settings_hooks(
    settings_path: &Path,
    dry_run: bool,
) -> Result<AgentSettingsRemoval, String> {
    let Some(Value::Object(mut root)) = read_json_if_exists(settings_path)? else {
        return Ok(AgentSettingsRemoval::default());
    };
    let Some(hooks_value) = root.remove("hooks") else {
        return Ok(AgentSettingsRemoval::default());
    };
    let Value::Object(mut hooks) = hooks_value else {
        root.insert("hooks".to_string(), hooks_value);
        return Ok(AgentSettingsRemoval::default());
    };

    let mut removed_events = Vec::new();
    for &event in ALL_HOOK_EVENTS {
        let Some(value) = hooks.remove(event) else {
            continue;
        };
        let Value::Array(entries) = value else {
            hooks.insert(event.to_string(), value);
            continue;
        };
        let original_len = entries.len();
        let retained = entries
            .into_iter()
            .filter(|entry| !value_contains_router_rs_claude_hook(entry))
            .collect::<Vec<_>>();
        if retained.len() != original_len {
            removed_events.push(event.to_string());
        }
        if !retained.is_empty() {
            hooks.insert(event.to_string(), Value::Array(retained));
        }
    }

    if removed_events.is_empty() {
        root.insert("hooks".to_string(), Value::Object(hooks));
        return Ok(AgentSettingsRemoval::default());
    }

    if !hooks.is_empty() {
        root.insert("hooks".to_string(), Value::Object(hooks));
    }
    let remove_file = root.is_empty();
    if !dry_run {
        if remove_file {
            fs::remove_file(settings_path).map_err(|err| err.to_string())?;
        } else {
            write_json_if_changed(settings_path, &Value::Object(root))?;
        }
    }
    Ok(AgentSettingsRemoval {
        changed: !dry_run,
        would_change: true,
        removed_file: !dry_run && remove_file,
        would_remove_file: remove_file,
        removed_events,
    })
}

#[derive(Debug, Deserialize)]
pub struct HostProjectionNarrative {
    schema_version: String,
    #[serde(default, alias = "gsd_default_lifecycle_paragraph")]
    default_lifecycle_paragraph: String,
    #[serde(default, alias = "gsd_lifecycle_by_host")]
    lifecycle_by_host: BTreeMap<String, String>,
    review_findings_only_paragraph: String,
}

pub fn lifecycle_paragraph_for_host(narrative: &HostProjectionNarrative, host_projection: &str) -> String {
    narrative
        .lifecycle_by_host
        .get(host_projection)
        .cloned()
        .or_else(|| {
            match host_projection {
                "codex-cli" | "codex-app" => narrative.lifecycle_by_host.get("codex").cloned(),
                "claude-desktop" => narrative.lifecycle_by_host.get("claude-code").cloned(),
                "antigravity-app" | "antigravity-cli" => {
                    narrative.lifecycle_by_host.get("antigravity").cloned()
                }
                _ => None,
            }
        })
        .unwrap_or_else(|| narrative.default_lifecycle_paragraph.clone())
}

pub fn load_host_projection_narrative(framework_root: &Path) -> Result<HostProjectionNarrative, String> {
    let path = framework_root.join("configs/framework/host_projection_narrative.json");
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("read host projection narrative {}: {err}", path.display()))?;
    let narrative: HostProjectionNarrative = serde_json::from_str(&raw)
        .map_err(|err| format!("invalid host projection narrative {}: {err}", path.display()))?;
    if narrative.schema_version != HOST_PROJECTION_NARRATIVE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported host projection narrative schema_version {:?} at {}; expected {}",
            narrative.schema_version,
            path.display(),
            HOST_PROJECTION_NARRATIVE_SCHEMA_VERSION
        ));
    }
    Ok(narrative)
}

fn skills_runtime_rel_path(framework_root: &Path) -> String {
    skills_source_rel(framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string())
}

fn framework_entrypoint_render_context(
    roots: &ResolvedProjectionRoots,
    host_label: &str,
) -> (HostProjectionNarrative, String) {
    let narrative = load_host_projection_narrative(&roots.framework_root).unwrap_or_else(|err| {
        panic!("host projection narrative must load before rendering {host_label} entrypoint: {err}")
    });
    let runtime_rel = skills_runtime_rel_path(&roots.framework_root);
    (narrative, runtime_rel)
}

fn framework_entrypoint_common_footer(runtime_rel: &str, agents_delta_file: &str) -> String {
    format!(
        "1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `{agents_delta_file}`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n"
    )
}

pub fn render_claude_project_narrative(roots: &ResolvedProjectionRoots) -> String {
    let narrative = load_host_projection_narrative(&roots.framework_root)
        .expect("host projection narrative must load before rendering claude project narrative");
    format!(
        r#"<!-- managed_by: skill-framework · claude-code · keep ≤48 lines -->
<!-- projection_id: claude-code-project-narrative -->
<!-- host_projection: claude-code -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**；宿主差异 **`AGENTS_CLAUDE.md`**；手册 **`docs/hosts/claude.md`**。

## 语言（硬约束）

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
- 仅当用户**当轮明确要求英文**时可切换。
- **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」。

{gsd}

## Hook 集成（非 MCP）

- 四事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/RFV：`framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/`。
- 默认 **`lifecycle_profile: my-light`**：closeout/complete 为 advisory，suppress review Stop nudge；非 my-light 时 closeout 可 fail-closed（与 REVIEW_GATE advisory 分层，见 `docs/host_adapter_contract.md` §0.1）。
- 检查点：`session_checkpoint`（非自动）。

## MCP（可选）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等；历史 Desktop 配置见 **`mcp.README.md`**（`claude-desktop` 已退役，勿作真源）。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
"#,
        gsd = lifecycle_paragraph_for_host(&narrative, "claude-code"),
    )
}

pub fn render_claude_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let (narrative, runtime_rel) = framework_entrypoint_render_context(roots, "claude");
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: claude-code -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n{gsd}\n\n{review}\n\n{footer}",
        gsd = lifecycle_paragraph_for_host(&narrative, "claude-code"),
        review = narrative.review_findings_only_paragraph,
        footer = framework_entrypoint_common_footer(&runtime_rel, "AGENTS_CLAUDE.md"),
    )
}

pub fn projection_manifest_file_ref(roots: &ResolvedProjectionRoots, path: &Path) -> String {
    path.strip_prefix(&roots.project_root)
        .map(|rel| rel.to_string_lossy().trim_start_matches('/').to_string())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub fn write_claude_projection_manifest(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    command_path: &Path,
    settings_path: &Path,
) -> Result<bool, String> {
    let mut files = vec![
        projection_manifest_file_ref(roots, command_path),
        projection_manifest_file_ref(roots, settings_path),
    ];
    if scope == "project" {
        files.push(projection_manifest_file_ref(
            roots,
            &claude_project_narrative_path(roots),
        ));
    }
    write_json_if_changed(
        &projection_manifest_path(roots, "claude-code", scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "claude-code",
            "scope": scope,
            "files": files,
            "settings": {
                "managed_key_paths": ALL_HOOK_EVENTS.iter()
                    .map(|e| format!("hooks.{}", e))
                    .collect::<Vec<_>>(),
            }
        }),
    )
}

pub fn claude_settings_hook_status(path: &Path) -> Result<Value, String> {
    let payload = read_json_if_exists(path)?;
    let mut managed_events = Vec::new();
    if let Some(Value::Object(root)) = payload.as_ref() {
        if let Some(Value::Object(hooks)) = root.get("hooks") {
            for event in ALL_HOOK_EVENTS {
                if hooks
                    .get(*event)
                    .map(value_contains_router_rs_claude_hook)
                    .unwrap_or(false)
                {
                    managed_events.push(*event);
                }
            }
        }
    }
    // Core events are mandatory; optional events are advisory.
    // At minimum all core events must be present for "managed" status.
    let managed_set: std::collections::HashSet<&str> = managed_events.iter().copied().collect();
    let all_core_present = CORE_HOOK_EVENTS.iter().all(|e| managed_set.contains(e));
    let all_optional_present = OPTIONAL_HOOK_EVENTS.iter().all(|e| managed_set.contains(e));
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": all_core_present,
        "managed_events": managed_events,
        "core_complete": all_core_present,
        "optional_complete": all_optional_present,
    }))
}

pub fn claude_projection_file_status(path: &Path) -> Result<Value, String> {
    projection_file_status(path, "claude-code")
}


/// Shared paperplain MCP entry (five-host research harness).
pub fn paperplain_mcp_server_payload() -> Value {
    json!({
        "command": "npx",
        "args": ["-y", "paperplain-mcp"],
        "type": "stdio",
        "description": "Academic paper metadata fetch/search (paperplain-mcp)"
    })
}

/// Independent `mcp-codegraph` stdio server (Roadmap v5 §2.8 W3 / CG-3).
pub fn codegraph_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    let repo_root = roots.project_root.to_string_lossy();
    match crate::host_integration::roots::resolve_mcp_codegraph_command(&roots.framework_root) {
        crate::host_integration::roots::McpCodegraphCommand::CargoBootstrap => json!({
            "command": "cargo",
            "args": crate::host_integration::roots::codegraph_mcp_cargo_bootstrap_args(
                &roots.framework_root,
                &repo_root,
            ),
            "type": "stdio",
            "description": "Code knowledge graph (search, callers, callees, impact) via mcp-codegraph",
        }),
        command => json!({
            "command": crate::host_integration::roots::mcp_codegraph_command_value(&command),
            "args": ["--repo-root", repo_root.as_ref()],
            "type": "stdio",
            "description": "Code knowledge graph (search, callers, callees, impact) via mcp-codegraph",
        }),
    }
}

/// Project `.mcp.json` + Codex `.codex/config.toml` research MCP (paperplain + mcp-codegraph).
pub fn ensure_research_mcp_five_host_surfaces(
    roots: &ResolvedProjectionRoots,
) -> Result<bool, String> {
    let mut changed = ensure_project_research_mcp_json(roots)?;
    changed |= ensure_codex_research_mcp_toml(roots)?;
    Ok(changed)
}

/// router-rs-framework payload for project `.mcp.json` (Claude Code).
pub fn claude_code_router_rs_framework_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload(
        roots,
        &["claude-code", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
        "Framework snapshot, skill routing, goal/closeout gating",
    )
}

/// Project-root `.mcp.json` with all four shared MCP servers (gitignored; materialized on host install).
pub fn ensure_project_research_mcp_json(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".mcp.json");
    let mut payload = read_json_if_exists(&path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let root = payload
        .as_object_mut()
        .ok_or_else(|| "project .mcp.json root must be an object".to_string())?;
    let servers = root
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    let entries = servers
        .as_object_mut()
        .ok_or_else(|| "project .mcp.json mcpServers must be an object".to_string())?;
    let framework = claude_code_router_rs_framework_payload(roots);
    let framework_changed = entries.get("router-rs-framework") != Some(&framework);
    entries.insert("router-rs-framework".to_string(), framework);
    let browser = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser);
    entries.insert("browser-mcp".to_string(), browser);
    let plain = paperplain_mcp_server_payload();
    let paperplain_changed = entries.get("paperplain") != Some(&plain);
    entries.insert("paperplain".to_string(), plain);
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    write_json_if_changed(&path, &payload)
        .map(|file_changed| framework_changed || browser_changed || paperplain_changed || codegraph_changed || file_changed)
}

/// Remove all managed MCP entries from project-root `.mcp.json`.
pub fn remove_project_mcp_json_entries(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".mcp.json");
    let managed_keys = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
    let Some(mut payload) = read_json_if_exists(&path)? else {
        return Ok(false);
    };
    let Some(root) = payload.as_object_mut() else {
        return Ok(false);
    };
    let mut changed = false;
    if let Some(servers) = root.get_mut("mcpServers").and_then(Value::as_object_mut) {
        for key in &managed_keys {
            changed |= servers.remove(*key).is_some();
        }
        if servers.is_empty() {
            root.remove("mcpServers");
        }
    }
    if changed {
        write_json_if_changed(&path, &payload)?;
    }
    Ok(changed)
}

fn merge_codegraph_into_mcp_servers_map(
    servers: &mut Map<String, Value>,
    roots: &ResolvedProjectionRoots,
    existing_key: &str,
) -> bool {
    let payload = codegraph_mcp_server_payload(roots);
    let changed = servers.get(existing_key) != Some(&payload);
    servers.insert(existing_key.to_string(), payload);
    changed
}

fn merge_paperplain_into_mcp_servers_map(
    servers: &mut Map<String, Value>,
    existing_key: &str,
) -> bool {
    let plain = paperplain_mcp_server_payload();
    let changed = servers.get(existing_key) != Some(&plain);
    servers.insert(existing_key.to_string(), plain);
    changed
}

fn codex_mcp_managed_marker(server_id: &str) -> String {
    format!("# managed_by: skill-framework · mcp_servers.{server_id}")
}

fn render_codex_mcp_toml_section(server_id: &str, command: &str, args: &[&str]) -> String {
    let escaped_cmd = command.replace('\\', "\\\\").replace('"', "\\\"");
    let args_toml = args
        .iter()
        .map(|arg| format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[mcp_servers.{server_id}]\ncommand = \"{escaped_cmd}\"\nargs = [{args_toml}]\nenabled = true\n"
    )
}

fn upsert_codex_mcp_toml_section(
    path: &Path,
    server_id: &str,
    command: &str,
    args: &[&str],
) -> Result<bool, String> {
    let marker = codex_mcp_managed_marker(server_id);
    let block = format!("{marker}\n{}", render_codex_mcp_toml_section(server_id, command, args));
    let existing = read_text_if_exists(path)?.unwrap_or_default();
    let new_text = if let Some(start) = existing.find(&marker) {
        let after_marker = start + marker.len();
        let end = existing[after_marker..]
            .find("\n# managed_by: skill-framework · mcp_servers.")
            .map(|idx| after_marker + idx)
            .unwrap_or(existing.len());
        let mut merged = String::new();
        merged.push_str(&existing[..start]);
        merged.push_str(&block);
        if end < existing.len() {
            if !merged.ends_with('\n') {
                merged.push('\n');
            }
            merged.push_str(&existing[end..]);
        }
        merged
    } else {
        let mut merged = existing;
        if !merged.is_empty() && !merged.ends_with('\n') {
            merged.push('\n');
        }
        if !merged.is_empty() {
            merged.push('\n');
        }
        merged.push_str(&block);
        merged
    };
    let normalized = format!("{}\n", new_text.trim_end());
    write_text_if_changed(path, &normalized)
}

/// router-rs-framework payload for Codex (TOML `mcp_servers`).
pub fn codex_router_rs_framework_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload(
        roots,
        &["codex", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
        "Framework snapshot, skill routing, goal/closeout gating (Codex)",
    )
}

/// Codex reads MCP from project `.codex/config.toml` (`mcp_servers.*` sections).
pub fn ensure_codex_research_mcp_toml(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".codex/config.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut changed = false;
    // -- router-rs-framework --
    let framework = codex_router_rs_framework_payload(roots);
    let fw_cmd = framework.get("command").and_then(Value::as_str).unwrap_or("router-rs");
    let fw_args: Vec<String> = framework.get("args").and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let fw_arg_refs: Vec<&str> = fw_args.iter().map(String::as_str).collect();
    changed |= upsert_codex_mcp_toml_section(&path, "router-rs-framework", fw_cmd, &fw_arg_refs)?;
    // -- browser-mcp --
    let browser = browser_mcp_server_payload(roots);
    let br_cmd = browser.get("command").and_then(Value::as_str).unwrap_or("router-rs");
    let br_args: Vec<String> = browser.get("args").and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let br_arg_refs: Vec<&str> = br_args.iter().map(String::as_str).collect();
    changed |= upsert_codex_mcp_toml_section(&path, "browser-mcp", br_cmd, &br_arg_refs)?;
    // -- paperplain --
    changed |= upsert_codex_mcp_toml_section(&path, "paperplain", "npx", &["-y", "paperplain-mcp"])?;
    // -- mcp-codegraph --
    let codegraph = codegraph_mcp_server_payload(roots);
    let command = codegraph
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("mcp-codegraph");
    let args: Vec<String> = codegraph
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    changed |= upsert_codex_mcp_toml_section(&path, "mcp-codegraph", command, &arg_refs)?;
    Ok(changed)
}

/// Remove all managed MCP TOML sections from `.codex/config.toml`.
pub fn remove_codex_mcp_toml_entries(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".codex/config.toml");
    let managed_server_ids = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
    let existing = read_text_if_exists(&path)?.unwrap_or_default();
    let mut result = existing.clone();
    let mut changed = false;
    for server_id in &managed_server_ids {
        let marker = codex_mcp_managed_marker(server_id);
        if let Some(start) = result.find(&marker) {
            let after_marker = start + marker.len();
            let end = result[after_marker..]
                .find("\n# managed_by: skill-framework · mcp_servers.")
                .map(|idx| after_marker + idx)
                .unwrap_or(result.len());
            let before = &result[..start];
            let after = if end < result.len() { &result[end..] } else { "" };
            result = format!("{}{}", before, after);
            changed = true;
        }
    }
    if changed {
        let normalized = format!("{}\n", result.trim_end());
        write_text_if_changed(&path, &normalized)?;
    }
    Ok(changed)
}

pub fn make_mcp_server_payload(roots: &ResolvedProjectionRoots, host_args: &[&str], description: &str) -> Value {
    make_mcp_server_payload_with_env(roots, host_args, description, None)
}

pub fn make_mcp_server_payload_with_env(
    roots: &ResolvedProjectionRoots,
    host_args: &[&str],
    description: &str,
    env: Option<Value>,
) -> Value {
    let mut args = Vec::new();
    for arg in host_args {
        args.push(arg.to_string());
    }
    let mut payload = match resolve_mcp_router_rs_command(&roots.framework_root) {
        McpRouterRsCommand::CargoBootstrap => json!({
            "command": "cargo",
            "args": router_rs_cargo_bootstrap_args(&roots.framework_root, host_args),
            "type": "stdio",
            "description": format!("{} (via Cargo bootstrap)", description),
        }),
        command => json!({
            "command": mcp_router_rs_command_value(&command),
            "args": args,
            "type": "stdio",
            "description": description.to_string(),
        }),
    };
    if let Some(env) = env {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("env".to_string(), env);
        }
    }
    payload
}


pub fn install_antigravity_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
    let mcp_target = antigravity_mcp_target(roots, scope);
    let settings_target = antigravity_settings_target(roots, scope);
    let framework_md_target = antigravity_framework_md_target(roots, scope);

    let mcp_changed = write_antigravity_mcp_json(&mcp_target, roots)?;
    let settings_changed = write_antigravity_settings_json(&settings_target, roots, scope)?;
    let md_changed = write_antigravity_framework_md(&framework_md_target, roots, scope)?;
    let manifest_changed = write_antigravity_projection_manifest(
        roots,
        scope,
        &mcp_target,
        &settings_target,
        &framework_md_target,
    )?;

    Ok(json!({
        "status": "installed",
        "changed": mcp_changed || settings_changed || md_changed || manifest_changed,
        "scope": scope,
        "mcp_config": {
            "scope": scope,
            "path": mcp_target.to_string_lossy(),
            "changed": mcp_changed,
        },
        "settings": {
            "scope": scope,
            "path": settings_target.to_string_lossy(),
            "changed": settings_changed,
        },
        "framework_md": {
            "scope": scope,
            "path": framework_md_target.to_string_lossy(),
            "changed": md_changed,
        },
    }))
}

/// Extract the first file-like entrypoint path from the registry for a given host.
fn registry_framework_md_path(
    framework_root: &Path,
    host_id: &str,
) -> Option<String> {
    let reg = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root).ok()?;
    let ep = framework_kernel::framework_host_targets::host_entrypoints_value_for_id(&reg, host_id).ok()?;
    let paths: Vec<String> = match ep {
        Value::Array(arr) => arr.into_iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        Value::String(s) => vec![s],
        _ => vec![],
    };
    paths.into_iter().find(|p| (p.contains('/') || p.contains('.')) && p.ends_with(".md"))
}

pub fn antigravity_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let framework_md_rel = registry_framework_md_path(&roots.framework_root, "antigravity")
        .unwrap_or_else(|| ".gemini/antigravity/rules/framework.md".to_string());
    let project_mcp_path = roots.project_root.join(".gemini/mcp.json");
    let project_settings_path = roots.project_root.join(".gemini/settings.json");
    let project_framework_md_path = roots.project_root.join(&framework_md_rel);
    let user_mcp_path = roots.antigravity_home_root.join("mcp.json");
    let user_settings_path = roots.antigravity_home_root.join("settings.json");
    let user_framework_md_path = roots.antigravity_home_root.join("antigravity/rules/framework.md");

    let mcp_exists = project_mcp_path.exists() || user_mcp_path.exists();
    let md_exists = project_framework_md_path.exists() || user_framework_md_path.exists();

    let mut binary_valid = false;
    let mut status_error = None;

    if mcp_exists {
        let mcp_to_check = if project_mcp_path.exists() {
            &project_mcp_path
        } else {
            &user_mcp_path
        };
        if let Ok(Some(mcp_json)) = read_json_if_exists(mcp_to_check) {
            if let Some(cmd) = mcp_json
                .get("mcpServers")
                .and_then(|v| v.get("router-rs-framework"))
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
            {
                match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                    Ok(()) => binary_valid = true,
                    Err(err) => status_error = Some(err),
                }
            } else {
                status_error =
                    Some("Invalid or incomplete mcpServers payload structure".to_string());
            }
        } else {
            status_error = Some("Failed to read or parse mcp.json config".to_string());
        }
    } else {
        status_error = Some("mcp.json does not exist in project or user scope".to_string());
    }

    let ready = md_exists && mcp_exists && binary_valid;

    Ok(json!({
        "ready": ready,
        "status": "projection-status",
        "error": status_error,
        "mcp_config": {
            "project_scope": project_mcp_path.exists(),
            "user_scope": user_mcp_path.exists(),
            "binary_valid": binary_valid,
        },
        "settings": {
            "project_scope": project_settings_path.exists(),
            "user_scope": user_settings_path.exists(),
        },
        "framework_md": {
            "project_scope": project_framework_md_path.exists(),
            "user_scope": user_framework_md_path.exists(),
        },
    }))
}

pub fn remove_antigravity_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let mcp_target = antigravity_mcp_target(roots, scope);
    let settings_target = antigravity_settings_target(roots, scope);
    let framework_md_target = antigravity_framework_md_target(roots, scope);
    let manifest_path = antigravity_projection_manifest_path(roots, scope);

    let mut changed = false;
    let mut would_change = false;
    let mut removed_paths = Vec::new();
    let mut would_remove_paths = Vec::new();

    for (target, label) in [
        (&mcp_target, "mcp_config"),
        (&settings_target, "settings"),
        (&framework_md_target, "framework_md"),
    ] {
        let managed =
            projection_manifest_ownership(&manifest_path, "antigravity", scope, target)
                .map(|o| o.owns_projection_file)
                .unwrap_or(false);
        if target.exists() && managed {
            if !dry_run {
                let _ = std::fs::remove_file(target);
                removed_paths.push(json!({"path": target.to_string_lossy(), "type": label}));
            }
            changed = true;
            would_change = true;
            if dry_run {
                would_remove_paths.push(json!({"path": target.to_string_lossy(), "type": label}));
            }
        }
    }

    let manifest_managed = manifest_path.exists() &&
        projection_manifest_ownership(&manifest_path, "antigravity", scope, &framework_md_target)
            .map(|o| o.managed)
            .unwrap_or(false);
    if manifest_managed {
        if !dry_run {
            let _ = std::fs::remove_file(&manifest_path);
            removed_paths.push(json!({"path": manifest_path.to_string_lossy(), "type": "manifest"}));
        } else {
            would_remove_paths.push(json!({"path": manifest_path.to_string_lossy(), "type": "manifest"}));
            would_change = true;
        }
        changed = true;
    }

    Ok(json!({
        "status": if dry_run && would_change { "would-remove" } else if changed { "removed" } else { "not-installed-or-user-owned" },
        "changed": changed,
        "dry_run": dry_run,
        "scope": scope,
        "removed_paths": removed_paths,
        "would_remove_paths": would_remove_paths,
    }))
}

pub fn antigravity_mcp_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.antigravity_home_root.join("mcp.json")
    } else {
        roots.project_root.join(".gemini/mcp.json")
    }
}

pub fn antigravity_settings_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.antigravity_home_root.join("settings.json")
    } else {
        roots.project_root.join(".gemini/settings.json")
    }
}

pub fn antigravity_framework_md_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.antigravity_home_root.join("antigravity/rules/framework.md")
    } else {
        let rel = registry_framework_md_path(&roots.framework_root, "antigravity")
            .unwrap_or_else(|| ".gemini/antigravity/rules/framework.md".to_string());
        roots.project_root.join(rel)
    }
}

pub fn antigravity_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload(
        roots,
        &["antigravity", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
        "Framework runtime snapshot, continuity, skill routing, closeout gating",
    )
}

pub fn write_antigravity_mcp_json(
    path: &Path,
    roots: &ResolvedProjectionRoots,
) -> Result<bool, String> {
    let mut payload = read_json_if_exists(path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let servers = payload
        .as_object_mut()
        .ok_or_else(|| "mcp.json root must be an object".to_string())?;
    let mcp_servers = servers
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !mcp_servers.is_object() {
        *mcp_servers = json!({});
    }
    let entries = mcp_servers
        .as_object_mut()
        .ok_or_else(|| "mcpServers must be an object".to_string())?;
    let framework = antigravity_mcp_server_payload(roots);
    let framework_changed = entries.get("router-rs-framework") != Some(&framework);
    entries.insert("router-rs-framework".to_string(), framework);
    let browser_payload = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser_payload);
    entries.insert("browser-mcp".to_string(), browser_payload);
    let paperplain_changed = merge_paperplain_into_mcp_servers_map(entries, "paperplain");
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    write_json_if_changed(path, &payload)
        .map(|file_changed| file_changed || framework_changed || browser_changed || paperplain_changed || codegraph_changed)
}

pub fn write_antigravity_settings_json(
    path: &Path,
    _roots: &ResolvedProjectionRoots,
    _scope: &str,
) -> Result<bool, String> {
    let mut payload = read_json_if_exists(path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    write_json_if_changed(path, &payload)
}

pub fn write_antigravity_framework_md(
    path: &Path,
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<bool, String> {
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    let content = format!(
        "<!-- managed_by: skill-framework · antigravity · keep ≤40 lines -->\n\
         <!-- projection_id: antigravity-self-discipline -->\n\
         <!-- host_projection: antigravity -->\n\
         <!-- install_scope: {scope} -->\n\n\
         # Antigravity Framework\n\n\
         Antigravity（Desktop / Planning Mode）**`router-rs-framework`** MCP。协议：**`docs/hosts/antigravity.md`**；跨宿主 **`AGENTS.md`**；**`AGENTS_ANTIGRAVITY.md`**。\n\n\
         ## 会话操作（按序）\n\n\
         1. `framework_snapshot` — 开头一次\n\
         2. `skill_route` → 只读 `skill_path`\n\
         3. `goal_state_manage operation=start`（宏任务）\n\
         4. 验证后 `record_evidence`\n\
         5. `closeout_gate` → `goal_state_manage operation=complete`\n\n\
         ## 门控说明（MCP）\n\n\
         - **无 shell hook 表**；`goal_state_manage complete` 与 `closeout_gate` 在 MCP 工具层报告 findings（advisory，不阻断）。\n\n\
         ## 共享资源\n\n\
         与其它宿主共用 `artifacts/current/` 工作区。路由：`{runtime_rel}`。\n"
    );
    write_text_if_changed(path, &content)
}

pub fn antigravity_projection_manifest_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    let canonical = projection_manifest_path(roots, "antigravity", scope);
    if canonical.exists() {
        return canonical;
    }
    projection_manifest_path(roots, "antigravity-app", scope)
}

pub fn write_antigravity_projection_manifest(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    mcp_path: &Path,
    settings_path: &Path,
    framework_md_path: &Path,
) -> Result<bool, String> {
    write_json_if_changed(
        &antigravity_projection_manifest_path(roots, scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "antigravity",
            "scope": scope,
            "files": [
                mcp_path.to_string_lossy(),
                settings_path.to_string_lossy(),
                framework_md_path.to_string_lossy()
            ],
            "settings": {
                "managed_key_paths": [
                    "mcpServers.router-rs-framework",
                    "mcpServers.browser-mcp",
                    "mcpServers.paperplain",
                    "mcpServers.mcp-codegraph",
                ],
            },
        }),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectionManifestOwnership {
    managed: bool,
    owns_projection_file: bool,
}

pub fn projection_manifest_status(path: &Path) -> Result<Value, String> {
    let manifest = read_json_if_exists(path)?;
    Ok(projection_manifest_status_from_payload(
        path,
        manifest.as_ref(),
    ))
}

pub fn projection_manifest_status_from_payload(path: &Path, manifest: Option<&Value>) -> Value {
    json!({
        "path": path.to_string_lossy(),
        "exists": path.is_file(),
        "managed": projection_manifest_payload_is_managed(manifest, None, None),
    })
}

pub fn projection_manifest_ownership(
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

pub fn projection_manifest_is_managed(
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

pub fn projection_manifest_payload_is_managed(
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

pub fn projection_manifest_files_include(manifest_path: &Path, projection_path: &Path) -> Result<bool, String> {
    let Some(manifest) = read_json_if_exists(manifest_path)? else {
        return Ok(false);
    };
    let expected = normalize_path(projection_path)?;
    let manifest_base = manifest_path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(manifest
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files.iter().filter_map(Value::as_str).any(|raw| {
                let candidate = PathBuf::from(raw);
                let resolved = if candidate.is_absolute() {
                    normalize_path(&candidate).ok()
                } else {
                    normalize_path(&manifest_base.join(candidate)).ok()
                };
                resolved == Some(expected.clone())
            })
        })
        .unwrap_or(false))
}

pub fn removed_projection_paths(
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

pub fn append_mcp_path(paths: &mut Value, include: bool, mcp_path: &Path) {
    if !include {
        return;
    }
    if let Some(array) = paths.as_array_mut() {
        array.push(Value::String(mcp_path.to_string_lossy().into_owned()));
    }
}

pub fn codex_entrypoint_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
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

pub fn cursor_entrypoint_target(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
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

pub fn codex_prompt_entrypoints_root(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots.codex_home_root.clone()
    } else {
        roots.project_root.join(".codex")
    }
}

pub fn projection_manifest_path(
    roots: &ResolvedProjectionRoots,
    host_projection: &str,
    scope: &str,
) -> PathBuf {
    match (host_projection, scope) {
        ("codex", "user") => roots
            .codex_home_root
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("codex", _) => roots
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
        ("claude-code", "user") => roots
            .claude_home_root
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("claude-code", _) => roots
            .project_root
            .join(".claude")
            .join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
        ("antigravity-app" | "antigravity", "user") => roots
            .antigravity_home_root
            .join(FRAMEWORK_PROJECTION_ANTIGRAVITY_MANIFEST_NAME),
        ("antigravity-app" | "antigravity", _) => roots
            .project_root
            .join(".gemini")
            .join(FRAMEWORK_PROJECTION_ANTIGRAVITY_MANIFEST_NAME),
        _ => roots.project_root.join(FRAMEWORK_PROJECTION_MANIFEST_NAME),
    }
}

pub fn write_codex_projection_manifest(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    command_path: &Path,
) -> Result<bool, String> {
    write_json_if_changed(
        &projection_manifest_path(roots, "codex", scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "codex",
            "scope": scope,
            "files": [command_path.to_string_lossy()],
            "settings": {
                "managed_key_paths": [],
            }
        }),
    )
}

pub fn render_codex_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let narrative = load_host_projection_narrative(&roots.framework_root)
        .expect("host projection narrative must load before rendering codex entrypoint");
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nargument-hint: \"[framework task...]\"\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: codex -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse `$framework` semantics via the Rust-owned shared core.\n\n{gsd}\n\n{review}\n\n1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS_CODEX.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n\n$ARGUMENTS\n",
        gsd = lifecycle_paragraph_for_host(&narrative, "codex"),
        review = narrative.review_findings_only_paragraph,
    )
}

pub fn write_cursor_projection_manifest(
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

pub fn cursor_mcp_config_path(roots: &ResolvedProjectionRoots) -> PathBuf {
    roots.cursor_home_root.join("mcp.json")
}

pub fn cursor_mcp_server_key_path() -> &'static str {
    "mcp_servers.browser-mcp"
}

pub fn cursor_codegraph_mcp_server_key_path() -> &'static str {
    "mcp_servers.mcp-codegraph"
}

#[derive(Debug, Clone)]
pub struct CursorMcpInstallOutcome {
    pub managed: bool,
    pub changed: bool,
    pub reason: &'static str,
    pub skipped_user_owned: bool,
}

pub fn install_cursor_mcp_server(
    roots: &ResolvedProjectionRoots,
    path: &Path,
) -> Result<CursorMcpInstallOutcome, String> {
    let browser_server = cursor_mcp_server_payload(roots);
    let framework_server = cursor_router_rs_framework_payload(roots);
    let _codegraph_server = codegraph_mcp_server_payload(roots);
    if let Some(payload) = read_json_if_exists(path)? {
        if let Some(existing) = payload
            .get("mcp_servers")
            .and_then(Value::as_object)
            .and_then(|servers| servers.get("browser-mcp"))
        {
            if cursor_mcp_server_semantically_matches_framework(existing, roots) {
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
                let framework_changed = servers.get("router-rs-framework") != Some(&framework_server);
                if framework_changed {
                    servers.insert("router-rs-framework".to_string(), framework_server);
                }
                let paperplain_changed = merge_paperplain_into_mcp_servers_map(servers, "paperplain");
                let codegraph_changed =
                    merge_codegraph_into_mcp_servers_map(servers, roots, "mcp-codegraph");
                let file_changed = write_json_if_changed(path, &payload)?;
                let changed = framework_changed || paperplain_changed || codegraph_changed || file_changed;
                return Ok(CursorMcpInstallOutcome {
                    managed: true,
                    changed,
                    reason: if changed {
                        "installed"
                    } else {
                        "already-managed-equivalent"
                    },
                    skipped_user_owned: false,
                });
            }
            if !cursor_mcp_entry_is_framework_owned_stale(existing, &roots.framework_root) {
                return Ok(CursorMcpInstallOutcome {
                    managed: false,
                    changed: false,
                    reason: "skipped_user_owned",
                    skipped_user_owned: true,
                });
            }
        }
    }

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
    let browser_changed = servers.get("browser-mcp") != Some(&browser_server);
    if browser_changed {
        servers.insert("browser-mcp".to_string(), browser_server);
    }
    let framework_changed = servers.get("router-rs-framework") != Some(&framework_server);
    if framework_changed {
        servers.insert("router-rs-framework".to_string(), framework_server);
    }
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(servers, roots, "mcp-codegraph");
    let paperplain_changed = merge_paperplain_into_mcp_servers_map(servers, "paperplain");
    let file_changed = write_json_if_changed(path, &payload)?;
    let changed = browser_changed || framework_changed || codegraph_changed || paperplain_changed;
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

/// Entries that look framework-managed but stale (ephemeral/repo-target/cargo bootstrap) may be rewritten.
pub fn cursor_mcp_entry_is_framework_owned_stale(existing: &Value, framework_root: &Path) -> bool {
    if !cursor_browser_mcp_is_managed_shape(existing) {
        return false;
    }
    let expected_root = framework_root.to_string_lossy();
    let Some(existing_root) = existing
        .get("args")
        .and_then(Value::as_array)
        .and_then(|args| cursor_browser_mcp_repo_root_from_args(args))
    else {
        return false;
    };
    if existing_root != expected_root {
        return false;
    }
    let Some(cmd) = existing.get("command").and_then(Value::as_str) else {
        return false;
    };
    if matches!(cmd, "cargo" | "router-rs") {
        return true;
    }
    if is_ephemeral_executable_path(cmd) {
        return true;
    }
    is_repo_build_executable_path(cmd, framework_root)
}

pub fn remove_cursor_mcp_server(path: &Path) -> Result<bool, String> {
    let Some(mut payload) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    let Some(root) = payload.as_object_mut() else {
        return Ok(false);
    };
    let managed_keys = ["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"];
    let mut changed = false;
    if let Some(mcp_servers) = root.get_mut("mcp_servers") {
        if let Some(servers) = mcp_servers.as_object_mut() {
            for key in &managed_keys {
                changed |= servers.remove(*key).is_some();
            }
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

pub fn cursor_mcp_browser_stdio_args(roots: &ResolvedProjectionRoots) -> Vec<String> {
    vec![
        "browser".into(),
        "mcp-stdio".into(),
        "--repo-root".into(),
        roots.framework_root.to_string_lossy().into_owned(),
    ]
}

pub fn cursor_browser_mcp_repo_root_from_args(args: &[Value]) -> Option<String> {
    let str_args: Vec<&str> = args.iter().filter_map(Value::as_str).collect();
    for window in str_args.windows(2) {
        if window[0] == "--repo-root" {
            return Some(window[1].to_string());
        }
    }
    None
}

pub fn cursor_browser_mcp_is_cargo_bootstrap_shaped(server: &Value) -> bool {
    let Some(cmd) = server.get("command").and_then(Value::as_str) else {
        return false;
    };
    if cmd != "cargo" {
        return false;
    }
    let Some(args) = server.get("args").and_then(Value::as_array) else {
        return false;
    };
    let str_args: Vec<&str> = args.iter().filter_map(Value::as_str).collect();
    str_args.iter().any(|arg| *arg == "mcp-stdio")
        && cursor_browser_mcp_repo_root_from_args(args).is_some()
}

pub fn cursor_browser_mcp_is_managed_shape(server: &Value) -> bool {
    cursor_browser_mcp_is_framework_shaped(server)
        || cursor_browser_mcp_is_cargo_bootstrap_shaped(server)
}

pub fn cursor_browser_mcp_is_framework_shaped(server: &Value) -> bool {
    let Some(args) = server.get("args").and_then(Value::as_array) else {
        return false;
    };
    let str_args: Vec<&str> = args.iter().filter_map(Value::as_str).collect();
    str_args.len() >= 3
        && str_args[0] == "browser"
        && str_args[1] == "mcp-stdio"
        && cursor_browser_mcp_repo_root_from_args(args).is_some()
}

pub fn cursor_browser_mcp_command_is_router_rs(server: &Value, framework_root: &Path) -> bool {
    let Some(cmd) = server.get("command").and_then(Value::as_str) else {
        return false;
    };
    if matches!(cmd, "cargo" | "router-rs") {
        return true;
    }
    if !Path::new(cmd).is_file() {
        return false;
    }
    if is_ephemeral_executable_path(cmd) {
        return cmd.ends_with("/router-rs") || cmd.ends_with("\\router-rs");
    }
    if is_repo_build_executable_path(cmd, framework_root) {
        return true;
    }
    resolve_stable_router_rs_executable(framework_root)
        .is_some_and(|stable| stable.to_string_lossy() == cmd)
        || cmd.ends_with("/router-rs")
        || cmd.ends_with("\\router-rs")
}

pub fn cursor_mcp_server_semantically_matches_framework(
    existing: &Value,
    roots: &ResolvedProjectionRoots,
) -> bool {
    if existing == &cursor_mcp_server_payload(roots) {
        return true;
    }
    if !cursor_browser_mcp_is_framework_shaped(existing) {
        return false;
    }
    let Some(existing_root) = existing
        .get("args")
        .and_then(Value::as_array)
        .and_then(|args| cursor_browser_mcp_repo_root_from_args(args))
    else {
        return false;
    };
    let expected_root = roots.framework_root.to_string_lossy();
    if existing_root != expected_root {
        return false;
    }
    cursor_browser_mcp_command_is_router_rs(existing, &roots.framework_root)
}

pub fn cursor_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    let args = cursor_mcp_browser_stdio_args(roots);
    match resolve_mcp_router_rs_command(&roots.framework_root) {
        McpRouterRsCommand::CargoBootstrap => json!({
            "command": "cargo",
            "args": router_rs_cargo_bootstrap_args(&roots.framework_root, &[
                "browser",
                "mcp-stdio",
                "--repo-root",
                &roots.framework_root.to_string_lossy(),
            ]),
        }),
        command => json!({
            "command": mcp_router_rs_command_value(&command),
            "args": args,
        }),
    }
}

/// router-rs-framework payload for Cursor (uses `mcp_servers` key, snake_case).
pub fn cursor_router_rs_framework_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload(
        roots,
        &["cursor", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
        "Framework snapshot, skill routing, goal/closeout gating (Cursor)",
    )
}

pub fn cursor_router_rs_framework_key_path() -> &'static str {
    "mcp_servers.router-rs-framework"
}

pub fn cursor_paperplain_mcp_server_key_path() -> &'static str {
    "mcp_servers.paperplain"
}

pub fn projection_manifest_manages_key_path(path: &Path, key_path: &str) -> Result<bool, String> {
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

pub fn cursor_mcp_server_matches_framework(
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
    Ok(Some(
        cursor_mcp_server_semantically_matches_framework(server, roots),
    ))
}

pub fn cursor_mcp_server_exists(path: &Path) -> Result<bool, String> {
    let Some(payload) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    Ok(payload
        .get("mcp_servers")
        .and_then(Value::as_object)
        .and_then(|servers| servers.get("browser-mcp"))
        .is_some())
}

pub fn render_cursor_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let narrative = load_host_projection_narrative(&roots.framework_root)
        .expect("host projection narrative must load before rendering cursor entrypoint");
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nglobs: [\"**/*\"]\nalwaysApply: true\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: cursor -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n{gsd}\n\n{review}\n\n1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS_CURSOR.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n",
        gsd = lifecycle_paragraph_for_host(&narrative, "cursor"),
        review = narrative.review_findings_only_paragraph,
    )
}

pub fn managed_projection_file_exists(path: &Path) -> Result<bool, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(false);
    };
    Ok(is_managed_projection_content(&content))
}

pub fn codex_projection_file_status(path: &Path) -> Result<Value, String> {
    projection_file_status(path, "codex")
}

pub fn cursor_projection_file_status(path: &Path) -> Result<Value, String> {
    projection_file_status(path, "cursor")
}

fn projection_file_status(path: &Path, host_projection: &str) -> Result<Value, String> {
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let host_marker = format!("host_projection: {host_projection}");
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains(&host_marker))
            .unwrap_or(false);
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": verified,
        "verification": if verified { "verified" } else if marker_managed { "unknown" } else { "unmanaged" },
        "marker_managed": marker_managed,
    }))
}

pub fn is_managed_projection_content(content: &str) -> bool {
    content.contains("managed_by: skill-framework")
        && content.contains(&format!(
            "framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION}"
        ))
}

pub fn canonical_install_skills_command(command: &str) -> String {
    match command.trim() {
        "" => "status".to_string(),
        raw => raw.to_lowercase(),
    }
}

pub fn install_skills_projection_tools(command: &str, tools: &[String], to: &[String]) -> Vec<String> {
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

pub fn canonical_tool_name(raw: &str, framework_root: &Path) -> Result<String, String> {
    let _normalized = raw.trim().to_lowercase();
    if let Some(adapter) = projection_adapter_for_raw(raw) {
        return Ok(adapter.tool.to_string());
    }
    let known = projection_supported_tools_for_message(framework_root);
    let aliases = projection_alias_summary();
    Err(format!(
        "Unknown tool: {}. Supported tools: {} (aliases: {})",
        raw.trim().to_lowercase(),
        known.join(", "),
        aliases
    ))
}

pub fn projection_supported_tools_for_message(framework_root: &Path) -> Vec<String> {
    let tools = registry_projection_tools(framework_root).unwrap_or_else(|_| {
        vec![
            "cursor".to_string(),
            "claude".to_string(),
            "antigravity".to_string(),
            "opencode".to_string(),
        ]
    });
    tools
}

pub fn codex_prompt_entrypoints_disabled(codex_dir: &Path) -> Value {
    let prompt_dir = codex_dir.join("prompts");
    json!({
        "changed": false,
        "enabled": false,
        "prompt_dir": prompt_dir.to_string_lossy(),
        "written": [],
        "unchanged": [],
    })
}

pub fn shared_skills_source(repo_root: &Path) -> Result<PathBuf, String> {
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

pub fn shared_codex_skill_surface(repo_root: &Path) -> PathBuf {
    repo_root.join(CODEX_SKILL_SURFACE_REL)
}

pub fn ensure_codex_skill_surface(repo_root: &Path) -> Result<Value, String> {
    ensure_host_skill_surface(
        repo_root,
        &shared_codex_skill_surface,
        CODEX_SKILL_SURFACE_MANIFEST_NAME,
        "codex-skill-surface-v1",
        &desired_codex_skill_surface_slugs,
        "runtime-hot-index-plus-pinned-explicit-entrypoints",
    )
}

pub fn ensure_host_skill_surface(
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
        } else if is_framework_command(&repo_root, slug)?
            && framework_command_surface_publish(&repo_root, slug)?
        {
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
    changed |= ensure_codex_skill_surface_runtime(&repo_root, &surface_root, &desired)?;
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

pub fn desired_codex_skill_surface_slugs(repo_root: &Path) -> Result<Vec<String>, String> {
    desired_host_skill_surface_slugs(repo_root, true)
}

pub fn ensure_codex_skill_surface_runtime(
    repo_root: &Path,
    surface_root: &Path,
    desired: &[String],
) -> Result<bool, String> {
    let runtime_path = shared_skills_source(repo_root)?.join("SKILL_ROUTING_RUNTIME.json");
    let Some(content) = read_text_if_exists(&runtime_path)? else {
        return Ok(false);
    };
    let mut runtime = serde_json::from_str::<Value>(&content)
        .map_err(|err| format!("failed parsing {}: {err}", runtime_path.display()))?;
    let desired_set = desired.iter().cloned().collect::<BTreeSet<_>>();
    let mut surfaced_count = 0usize;

    if let Some(skills) = runtime.get_mut("skills").and_then(Value::as_array_mut) {
        let mut filtered = Vec::new();
        for mut row in std::mem::take(skills) {
            let Some(slug) = row
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if desired_set.contains(&slug) {
                if let Some(items) = row.as_array_mut() {
                    let path = Value::String(codex_surface_skill_path(&slug));
                    if items.len() >= 9 {
                        items[8] = path;
                    } else {
                        items.push(path);
                    }
                }
                filtered.push(row);
            }
        }
        surfaced_count = surfaced_count.max(filtered.len());
        *skills = filtered;
    }

    if let Some(scope) = runtime.get_mut("scope").and_then(Value::as_object_mut) {
        scope.insert("kind".to_string(), json!("codex-skill-surface"));
        scope.insert("hot_skill_count".to_string(), json!(surfaced_count));
        scope.insert(
            "source_runtime".to_string(),
            json!("skills/SKILL_ROUTING_RUNTIME.json"),
        );
    }

    write_json_if_changed(&surface_root.join("SKILL_ROUTING_RUNTIME.json"), &runtime)
}

pub fn codex_surface_skill_path(slug: &str) -> String {
    format!("skills/{slug}/SKILL.md")
}

pub fn desired_host_skill_surface_slugs(
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
        if !framework_command_surface_publish(repo_root, slug)? {
            continue;
        }
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

pub fn system_skill_source_exists(source_root: &Path, slug: &str) -> bool {
    source_root
        .join(".system")
        .join(slug)
        .join("SKILL.md")
        .is_file()
}

pub fn is_codex_system_provided_skill(slug: &str) -> bool {
    CODEX_SYSTEM_PROVIDED_SKILLS.contains(&slug)
}

pub fn codex_skill_surface_source_path(
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

pub fn is_framework_command(repo_root: &Path, slug: &str) -> Result<bool, String> {
    Ok(framework_command_names(repo_root)?.contains(slug))
}

/// Whether a `framework_commands` slug may appear on Codex/Cursor skill surface (default true).
pub fn framework_command_surface_publish(repo_root: &Path, slug: &str) -> Result<bool, String> {
    let registry = load_runtime_registry_payload(repo_root)?;
    let Some(command) = registry
        .get("framework_commands")
        .and_then(Value::as_object)
        .and_then(|commands| commands.get(slug))
    else {
        return Ok(true);
    };
    Ok(command
        .get("surface_publish")
        .and_then(Value::as_bool)
        .unwrap_or(true))
}

pub fn ensure_framework_command_skill(
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

pub fn render_framework_command_skill(repo_root: &Path, slug: &str) -> Result<String, String> {
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
    let default_host_entrypoint = format!("/{slug}");
    let host_entrypoint = host_entrypoints
        .and_then(|entrypoints| entrypoints.get("codex"))
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
        .unwrap_or_else(|| format!("codex={0}", default_host_entrypoint));
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

pub fn framework_command_names(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let Some(registry) = load_runtime_registry_payload_if_repo_local(repo_root)? else {
        return Ok(BTreeSet::new());
    };
    Ok(registry
        .get("framework_commands")
        .and_then(Value::as_object)
        .map(|commands| commands.keys().cloned().collect())
        .unwrap_or_default())
}

pub fn runtime_hot_skill_slugs(repo_root: &Path) -> Result<Vec<String>, String> {
    let runtime_path = shared_skills_source(repo_root)?.join("SKILL_ROUTING_RUNTIME.json");
    let Some(content) = read_text_if_exists(&runtime_path)? else {
        return Ok(Vec::new());
    };
    let runtime = serde_json::from_str::<Value>(&content).map_err(|err| err.to_string())?;
    let Some(skills) = runtime.get("skills").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let from_skills: Vec<String> = skills
        .iter()
        .filter_map(Value::as_array)
        .filter_map(|record| record.first())
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok(from_skills)
}

pub fn codex_skills_matches_source(target_path: &Path, source_path: &Path) -> Result<bool, String> {
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

pub fn default_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_bootstrap_output_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("artifacts").join("bootstrap")
}

pub fn default_bootstrap_mirror_path(output_dir: &Path) -> PathBuf {
    output_dir.join("framework_default_bootstrap.json")
}

pub fn workspace_name_from_root(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

pub fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn safe_slug(label: &str) -> String {
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

pub fn build_framework_task_id(label: &str) -> String {
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

pub fn compact_evolution_proposals(payload: &Value) -> Value {
    json!({
        "proposal_count": payload.get("proposal_count").and_then(Value::as_u64).unwrap_or(0),
        "proposals": payload
            .get("proposals")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    })
}

pub fn build_default_bootstrap_payload(
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

pub fn build_default_continuity_bootstrap(
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
pub struct MigrationPlan {
    source: String,
    destination: String,
}

pub fn migration_plan_values(plans: &[MigrationPlan]) -> Value {
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

pub fn evidence_artifact_root(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("evidence");
    task_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

pub fn scratch_artifact_root(repo_root: &Path, run_id: Option<&str>) -> PathBuf {
    let root = repo_root.join("artifacts").join("scratch");
    run_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

pub fn move_path(source: &Path, destination: &Path) -> Result<String, String> {
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

pub fn destination_for_current_artifact(
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

pub fn plan_current_artifact_clutter_migrations(
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

pub fn migrate_current_artifact_clutter(
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

pub fn bootstrap_payload_matches_contract(payload: &Value, repo_root: &Path) -> bool {
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

pub fn ensure_default_bootstrap(repo_root: &Path, output_dir: Option<&Path>) -> Result<Value, String> {
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

pub fn validate_default_bootstrap(bootstrap_path: &Path, repo_root: &Path) -> Result<bool, String> {
    let path = normalize_path(bootstrap_path)?;
    let repo_root = normalize_path(repo_root)?;
    let Some(content) = read_text_if_exists(&path)? else {
        return Ok(false);
    };
    let payload = serde_json::from_str::<Value>(&content).map_err(|err| err.to_string())?;
    Ok(bootstrap_payload_matches_contract(&payload, &repo_root))
}

pub fn ensure_config_file(config_path: &Path) -> Result<bool, String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if config_path.exists() {
        return Ok(false);
    }
    fs::write(config_path, CONFIG_SCHEMA_HEADER).map_err(|err| err.to_string())?;
    Ok(true)
}

pub fn ensure_codex_hooks_feature_disabled(config_path: &Path) -> Result<(bool, bool), String> {
    const HOOKS_DISABLED_LINE: &str = "hooks = false";
    let content = read_text_if_exists(config_path)?.unwrap_or_default();
    if let Some((start, end)) = find_named_block_bounds(&content, "[features]") {
        let block = content[start..end].trim_end_matches('\n');
        let mut has_hooks = false;
        let mut hooks_already_disabled = false;
        let mut hook_setting_count = 0usize;
        let mut deprecated_codex_hooks_removed = false;
        let mut updated_lines = Vec::new();
        for line in block.lines() {
            if is_named_setting(line, "codex_hooks") || is_named_setting(line, "hooks") {
                hook_setting_count += 1;
                deprecated_codex_hooks_removed |= is_named_setting(line, "codex_hooks");
                hooks_already_disabled |=
                    is_named_setting(line, "hooks") && line.trim() == HOOKS_DISABLED_LINE;
                if !has_hooks {
                    updated_lines.push(HOOKS_DISABLED_LINE.to_string());
                    has_hooks = true;
                }
            } else {
                updated_lines.push(line.to_string());
            }
        }
        if has_hooks
            && hooks_already_disabled
            && hook_setting_count == 1
            && !deprecated_codex_hooks_removed
        {
            return Ok((false, false));
        }
        if !has_hooks {
            updated_lines.push(HOOKS_DISABLED_LINE.to_string());
        }
        let new_block = format!("{}\n", updated_lines.join("\n"));
        let updated = format!("{}{}{}", &content[..start], new_block, &content[end..]);
        let changed = write_text_if_changed(config_path, &updated)?;
        return Ok((changed, deprecated_codex_hooks_removed));
    }
    let mut updated = content.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str("[features]\n");
    updated.push_str(HOOKS_DISABLED_LINE);
    updated.push('\n');
    let changed = write_text_if_changed(config_path, &updated)?;
    Ok((changed, false))
}

pub fn find_named_block_bounds(content: &str, marker: &str) -> Option<(usize, usize)> {
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

pub fn ensure_tui_status_line(config_path: &Path) -> Result<bool, String> {
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

pub fn find_tui_block_bounds(content: &str) -> Option<(usize, usize)> {
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

pub fn is_status_line(line: &str) -> bool {
    is_named_setting(line, "status_line")
}

pub fn is_named_setting(line: &str, key: &str) -> bool {
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

pub fn format_status_line() -> String {
    let items = DEFAULT_TUI_STATUS_ITEMS
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("status_line = [{items}]")
}

pub fn ensure_codex_skills_symlink(target_path: &Path, source_path: &Path) -> Result<bool, String> {
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
pub fn create_dir_symlink(source_path: &Path, target_path: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(source_path, target_path).map_err(|err| err.to_string())
}

#[cfg(windows)]
pub fn create_dir_symlink(source_path: &Path, target_path: &Path) -> Result<(), String> {
    std::os::windows::fs::symlink_dir(source_path, target_path).map_err(|err| err.to_string())
}

pub fn read_text_if_exists(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

pub fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    core_state::utils::path_guard::reject_unsafe_path(path)?;
    let existing = read_text_if_exists(path)?;
    if existing.as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    core_state::utils::atomic_write::write_atomic_text(path, content)?;
    Ok(true)
}

pub fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let formatted = format!(
        "{}\n",
        serde_json::to_string_pretty(payload).map_err(|err| err.to_string())?
    );
    write_text_if_changed(path, &formatted)
}

pub fn read_json_if_exists(path: &Path) -> Result<Option<Value>, String> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    serde_json::from_str::<Value>(&content)
        .map(Some)
        .map_err(|err| format!("failed parsing {}: {err}", path.display()))
}

pub fn remove_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn symlink_exists(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

