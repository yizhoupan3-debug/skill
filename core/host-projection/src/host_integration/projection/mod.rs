use super::*;

// ── Sub-modules (split for file size ≤2000 lines) ──
mod projection_bootstrap;
mod projection_host_ops;
mod projection_manifest;
pub mod projection_ops_trait;
pub use projection_bootstrap::*;
pub use projection_host_ops::*;
pub use projection_manifest::*;

/// Load managed MCP server IDs from RUNTIME_REGISTRY.json.
pub fn registry_managed_mcp_server_ids(framework_root: &Path) -> Result<Vec<String>, String> {
    let registry = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root)?;
    Ok(registry
        .get("managed_mcp_servers")
        .and_then(Value::as_object)
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default())
}

/// Generic router-rs-framework MCP payload, parameterized by host label.
/// Host label is resolved from RUNTIME_REGISTRY.host_targets.metadata.<host>.install_tool.
pub fn host_router_rs_framework_payload(
    roots: &ResolvedProjectionRoots,
    host_label: &str,
    description: &str,
) -> Value {
    make_mcp_server_payload(
        roots,
        &[
            "host",
            host_label,
            "agent",
            "--repo-root",
            roots.project_root.to_string_lossy().as_ref(),
        ],
        description,
    )
}

/// Resolve host install_tool label from RUNTIME_REGISTRY for MCP payload construction.
pub fn registry_host_install_tool(framework_root: &Path, host_id: &str) -> Result<String, String> {
    let registry = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root)?;
    registry
        .get("host_targets")
        .and_then(|t| t.get("metadata"))
        .and_then(|m| m.get(host_id))
        .and_then(|h| h.get("install_tool"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            format!("host {host_id} not found in RUNTIME_REGISTRY.host_targets.metadata")
        })
}

/// Shared MCP server binary validation loop.
/// Reads managed server IDs from registry, validates each server's binary from the given JSON config.
/// `mcp_servers_key` is the JSON key containing server entries (e.g. "mcp_servers" for Cursor, "mcpServers" for OpenCode).
pub fn validate_mcp_servers_from_json(
    roots: &ResolvedProjectionRoots,
    config_payload: Option<&Value>,
    mcp_servers_key: &str,
) -> (serde_json::Map<String, Value>, bool, Option<String>) {
    let mut server_status: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut all_valid = true;
    let mut first_error = None;

    let managed_servers = match registry_managed_mcp_server_ids(&roots.framework_root) {
        Ok(ids) => ids,
        Err(err) => {
            return (server_status, false, Some(err));
        }
    };

    let Some(payload) = config_payload else {
        return (
            server_status,
            false,
            Some("No MCP config found".to_string()),
        );
    };
    let servers = payload.get(mcp_servers_key).and_then(Value::as_object);
    for server_id in &managed_servers {
        let entry = servers.and_then(|s| s.get(server_id.as_str()));
        if let Some(cmd) = entry.and_then(|v| v.get("command")).and_then(Value::as_str) {
            match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                Ok(()) => {
                    // Deep validation for router-rs-based servers
                    if server_id.as_str() == "router-rs-framework" && cmd != "cargo" {
                        let resolved = if cmd == "router-rs" {
                            resolve_stable_router_rs_executable(&roots.framework_root)
                        } else {
                            Some(PathBuf::from(cmd))
                        };
                        match resolved.and_then(|path| {
                            framework_kernel::router_self::validate_router_rs_binary_runnable(&path)
                                .ok()
                        }) {
                            Some(()) => {
                                server_status
                                    .insert(server_id.to_string(), json!({"binary_valid": true}));
                            }
                            None => {
                                all_valid = false;
                                let msg = "router-rs not found or not runnable; run `router-rs self install`".to_string();
                                if first_error.is_none() {
                                    first_error = Some(msg.clone());
                                }
                                server_status.insert(
                                    server_id.to_string(),
                                    json!({"binary_valid": false, "error": msg}),
                                );
                            }
                        }
                    } else {
                        server_status.insert(server_id.to_string(), json!({"binary_valid": true}));
                    }
                }
                Err(err) => {
                    all_valid = false;
                    if first_error.is_none() {
                        first_error = Some(err.clone());
                    }
                    server_status.insert(
                        server_id.to_string(),
                        json!({"binary_valid": false, "error": err}),
                    );
                }
            }
        } else {
            all_valid = false;
            let msg = format!("missing or incomplete {server_id} payload");
            if first_error.is_none() {
                first_error = Some(msg.clone());
            }
            server_status.insert(
                server_id.to_string(),
                json!({"binary_valid": false, "error": msg}),
            );
        }
    }

    (server_status, all_valid, first_error)
}

// ── MCP Config Format Abstraction ──────────────────────────────────────────

/// MCP config format: JSON with a top-level key, or TOML with marker sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigFormat {
    /// JSON config with a named top-level key (e.g. "mcp_servers" or "mcpServers").
    Json { top_level_key: &'static str },
    /// TOML config with `# managed_by: skill-framework · mcp_servers.<id>` marker sections.
    Toml,
}

impl McpConfigFormat {
    /// Cursor uses `mcp_servers` (underscore).
    pub const CURSOR: Self = Self::Json {
        top_level_key: "mcp_servers",
    };
    /// Claude and OpenCode use `mcpServers` (camelCase).
    pub const CLAUDE: Self = Self::Json {
        top_level_key: "mcpServers",
    };
    pub const OPENCODE: Self = Self::Json {
        top_level_key: "mcpServers",
    };
    /// Codex uses TOML sections with managed-by markers.
    pub const CODEX: Self = Self::Toml;
}

/// Insert/update managed MCP servers into a JSON config file.
/// Returns whether the file was changed.
pub fn mcp_json_upsert_servers(
    path: &Path,
    format: McpConfigFormat,
    entries: &[(&str, Value)],
) -> Result<bool, String> {
    let McpConfigFormat::Json { top_level_key } = format else {
        return Err("mcp_json_upsert_servers called with non-JSON format".to_string());
    };
    let mut payload = read_json_if_exists(path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let root = payload.as_object_mut().unwrap();
    let servers = root
        .entry(top_level_key.to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    let map = servers.as_object_mut().unwrap();
    let mut changed = false;
    for (server_id, value) in entries {
        changed |= map.get(*server_id) != Some(value);
        map.insert(server_id.to_string(), value.clone());
    }
    if changed {
        write_json_if_changed(path, &payload)?;
    }
    Ok(changed)
}

/// Remove managed MCP servers from a JSON config file.
/// Returns whether the file was changed.
pub fn mcp_json_remove_servers(
    path: &Path,
    framework_root: &Path,
    format: McpConfigFormat,
) -> Result<bool, String> {
    let McpConfigFormat::Json { top_level_key } = format else {
        return Err("mcp_json_remove_servers called with non-JSON format".to_string());
    };
    let Some(mut payload) = read_json_if_exists(path)? else {
        return Ok(false);
    };
    let Some(root) = payload.as_object_mut() else {
        return Ok(false);
    };
    let managed_keys = registry_managed_mcp_server_ids(framework_root)?;
    let mut changed = false;
    if let Some(servers) = root.get_mut(top_level_key).and_then(Value::as_object_mut) {
        for key in &managed_keys {
            changed |= servers.remove(key.as_str()).is_some();
        }
        if servers.is_empty() {
            root.remove(top_level_key);
        }
    }
    if changed {
        write_json_if_changed(path, &payload)?;
    }
    Ok(changed)
}

/// Build managed_key_paths for a JSON MCP config host from the registry.
pub fn mcp_json_managed_key_paths(
    framework_root: &Path,
    format: McpConfigFormat,
) -> Result<Vec<String>, String> {
    let McpConfigFormat::Json { top_level_key } = format else {
        return Ok(vec![]);
    };
    Ok(registry_managed_mcp_server_ids(framework_root)?
        .iter()
        .map(|id| format!("{top_level_key}.{id}"))
        .collect())
}

pub fn install_native_integration(
    repo_root: &Path,
    home_config_path: &Path,
    bootstrap_output_dir: Option<&Path>,
    install_default_bootstrap: bool,
) -> Result<Value, String> {
    let repo_root = normalize_path(repo_root)?;
    let home_config_path = normalize_path(home_config_path)?;
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
    let default_bootstrap = if install_default_bootstrap {
        ensure_default_bootstrap(&repo_root, bootstrap_output_dir.as_deref())?
    } else {
        Value::Null
    };

    Ok(json!({
        "success": true,
        "repo_root": repo_root.to_string_lossy(),
        "home_config_path": home_config_path.to_string_lossy(),
        "codex_prompt_entrypoints": prompt_entrypoints,
        "created_config": created_config,
        "hooks_enabled": false,
        "hooks_disabled_changed": hooks_disabled_changed,
        "deprecated_codex_hooks_removed": deprecated_codex_hooks_removed,
        "tui_status_line_changed": tui_changed,
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
        command.opencode_home.as_deref(),
        command.home.as_deref(),
    )?;
    let mut results = Map::new();
    for tool in framework_kernel::framework_host_targets::skills_install_tools_ordered(
        &roots.framework_root,
    )? {
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
        let host_id = projection_adapter(tool)
            .map(|a| a.host_id)
            .unwrap_or(tool.as_str());
        let env_var = format!("{}_HOME", host_id.to_uppercase().replace('-', "_"));
        // Check: --home flag, host-specific --<host>-home CLI arg, or $HOST_HOME env var.
        // The CLI arg check mirrors the old per-host *_home_explicit() functions.
        let host_cli_home_set = match host_id {
            "codex" => command.codex_home.is_some(),
            "cursor" => command.cursor_home.is_some(),
            "claude" => command.claude_home.is_some(),
            "opencode" => command.opencode_home.is_some(),
            _ => false,
        };
        let explicit_home =
            command.home.is_some() || host_cli_home_set || std::env::var_os(&env_var).is_some();
        if !explicit_home {
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
        let value =
            if framework_kernel::framework_host_targets::host_is_installable(&registry, host_id)? {
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
        let _ = tool; // tool not needed for home_root; host_id suffices
        let home = roots.host_home_root(host_id)
            .ok_or_else(|| format!("host_home_root: host_id {host_id:?} not found"))?
            .to_string_lossy().into_owned();
        host_home_roots.insert(host_id.clone(), json!(home));
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
        // Claude Code 不支持 project scope 的 host projection（仅 user scope 生效）
        tools.retain(|tool| tool != "claude");
    }
    Ok(tools)
}

/// Lightweight adapter metadata derived from RUNTIME_REGISTRY.
/// Install/status/remove are dispatched by tool name (match), not function pointers.
pub struct HostProjectionAdapter {
    pub tool: &'static str,
    pub host_id: &'static str,
}

/// Known tool→host_id mappings.
/// Source of truth: RUNTIME_REGISTRY.json → host_targets.metadata.*.install_tool
const KNOWN_PROJECTION_TOOLS: &[HostProjectionAdapter] = &[
    HostProjectionAdapter {
        tool: "cursor",
        host_id: "cursor",
    },
    HostProjectionAdapter {
        tool: "claude",
        host_id: "claude",
    },
    HostProjectionAdapter {
        tool: "opencode",
        host_id: "opencode",
    },
    HostProjectionAdapter {
        tool: "codex",
        host_id: "codex",
    },
];

pub fn opencode_config_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots
            .account_home_root
            .join(".config/opencode/opencode.json")
    } else {
        roots.project_root.join(".opencode/opencode.json")
    }
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
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
    let config_path = opencode_config_path(roots, scope);
    let config_dir = config_path.parent().ok_or_else(|| {
        format!(
            "cannot determine parent directory of {}",
            config_path.display()
        )
    })?;
    std::fs::create_dir_all(config_dir)
        .map_err(|err| format!("failed to create {}: {err}", config_dir.display()))?;

    let mut payload = read_json_if_exists(&config_path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let servers = payload
        .as_object_mut()
        .ok_or_else(|| "opencode.json root must be an object".to_string())?;
    let mcp_servers = servers
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !mcp_servers.is_object() {
        *mcp_servers = json!({});
    }
    let entries = mcp_servers
        .as_object_mut()
        .ok_or_else(|| "mcpServers must be an object".to_string())?;
    let framework_payload = host_router_rs_framework_payload(
        roots,
        "opencode",
        "Framework snapshot, skill routing, goal/closeout gating (MCP advisory for my-light)",
    );
    let framework_changed = entries.get("router-rs-framework") != Some(&framework_payload);
    entries.insert("router-rs-framework".to_string(), framework_payload);
    let browser_payload = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser_payload);
    entries.insert("browser-mcp".to_string(), browser_payload);
    let paperplain_changed = merge_paperplain_into_mcp_servers_map(entries, "paperplain");
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    write_json_if_changed(&config_path, &payload)?;
    let changed = framework_changed || browser_changed || paperplain_changed || codegraph_changed;

    let manifest_dir = projection_manifest_path(roots, "opencode", scope);
    if let Some(parent) = manifest_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let manifest_key_paths =
        mcp_json_managed_key_paths(&roots.framework_root, McpConfigFormat::OPENCODE)?;
    let manifest_changed = write_projection_manifest(
        roots,
        "opencode",
        scope,
        &[projection_manifest_file_ref(roots, &config_path)],
        &manifest_key_paths,
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
            "path": projection_manifest_path(roots, "opencode", scope).to_string_lossy(),
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
    let config_payload = read_json_if_exists(&project_path)
        .ok()
        .flatten()
        .or_else(|| read_json_if_exists(&user_path).ok().flatten());

    let (server_status, all_valid, first_error) =
        validate_mcp_servers_from_json(roots, config_payload.as_ref(), "mcpServers");

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
            "project_scope": projection_manifest_path(roots, "opencode", "project").exists(),
            "user_scope": projection_manifest_path(roots, "opencode", "user").exists(),
        },
    }))
}

pub fn remove_opencode_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let config_path = opencode_config_path(roots, scope);

    let mut config_removed = false;
    if config_path.is_file() && !dry_run {
        config_removed = mcp_json_remove_servers(
            &config_path,
            &roots.framework_root,
            McpConfigFormat::OPENCODE,
        )?;
    }

    let manifest_path = projection_manifest_path(roots, "opencode", scope);
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

pub fn projection_adapter(tool: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = tool.trim().to_lowercase();
    KNOWN_PROJECTION_TOOLS
        .iter()
        .find(|adapter| adapter.tool == normalized)
}

pub fn projection_adapter_for_raw(raw: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = raw.trim().to_lowercase();
    KNOWN_PROJECTION_TOOLS.iter().find(|adapter| adapter.tool == normalized)
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
    let supported =
        framework_kernel::framework_host_targets::host_targets_supported_host_ids(&registry)?;
    for adapter in KNOWN_PROJECTION_TOOLS {
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
    String::new()
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
    if tool == "cursor" {
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
    if projection_adapter(tool).is_none() {
        return Err(format!("Unsupported tool: {tool}"));
    }
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .install(roots, effective_scope)
}

pub fn projection_tool_status(
    roots: &ResolvedProjectionRoots,
    tool: &str,
) -> Result<Value, String> {
    if projection_adapter(tool).is_none() {
        return Err(format!("Unsupported tool: {tool}"));
    }
    projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .status(roots)
}

pub fn remove_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    if projection_adapter(tool).is_none() {
        return Err(format!("Unsupported tool: {tool}"));
    }
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .remove(roots, effective_scope, dry_run)
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

#[derive(Debug, Deserialize)]
pub struct HostProjectionNarrative {
    schema_version: String,
    default_lifecycle_paragraph: String,
    lifecycle_by_host: BTreeMap<String, String>,
    review_findings_only_paragraph: String,
}

pub fn lifecycle_paragraph_for_host(
    narrative: &HostProjectionNarrative,
    host_projection: &str,
) -> String {
    narrative
        .lifecycle_by_host
        .get(host_projection)
        .cloned()
        .unwrap_or_else(|| narrative.default_lifecycle_paragraph.clone())
}

pub fn load_host_projection_narrative(
    framework_root: &Path,
) -> Result<HostProjectionNarrative, String> {
    let path = framework_root.join("configs/framework/host_projection_narrative.json");
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("read host projection narrative {}: {err}", path.display()))?;
    let narrative: HostProjectionNarrative = serde_json::from_str(&raw).map_err(|err| {
        format!(
            "invalid host projection narrative {}: {err}",
            path.display()
        )
    })?;
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
) -> Result<(HostProjectionNarrative, String), String> {
    let narrative = load_host_projection_narrative(&roots.framework_root).map_err(|err| {
        format!(
            "host projection narrative must load before rendering {host_label} entrypoint: {err}"
        )
    })?;
    let runtime_rel = skills_runtime_rel_path(&roots.framework_root);
    Ok((narrative, runtime_rel))
}

fn framework_entrypoint_common_footer(runtime_rel: &str) -> String {
    format!(
        "1) Start from `AGENTS.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n"
    )
}

pub fn render_claude_project_narrative(roots: &ResolvedProjectionRoots) -> Result<String, String> {
    let narrative = load_host_projection_narrative(&roots.framework_root).map_err(|err| {
        format!(
            "host projection narrative must load before rendering claude project narrative: {err}"
        )
    })?;
    Ok(format!(
        r#"<!-- managed_by: skill-framework · claude · keep ≤48 lines -->
<!-- projection_id: claude-project-narrative -->
<!-- host_projection: claude -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**；手册 **`docs/hosts/_common.md`** / **`docs/hosts/hook-hosts.md`**。

## 语言（硬约束）

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
- 仅当用户**当轮明确要求英文**时可切换。
- **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」。

{gsd}

## Hook 集成（非 MCP）

- 四事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/Quality Gate：`framework_goal_drive` / `framework_quality_gate` stdio + `artifacts/current/<task_id>/`。
- 默认 **`lifecycle_profile: my-light`**：closeout/complete 为 advisory，suppress review Stop nudge；非 my-light 时 closeout 可 fail-closed（与 REVIEW_GATE advisory 分层，见 `docs/spec.md` §6）。
- 检查点：`session_checkpoint`（非自动）。

## MCP（可选）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
"#,
        gsd = lifecycle_paragraph_for_host(&narrative, "claude"),
    ))
}

pub fn render_claude_framework_entrypoint(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<String, String> {
    let (narrative, runtime_rel) = framework_entrypoint_render_context(roots, "claude")?;
    Ok(format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: claude -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n{gsd}\n\n{review}\n\n{footer}",
        gsd = lifecycle_paragraph_for_host(&narrative, "claude"),
        review = narrative.review_findings_only_paragraph,
        footer = framework_entrypoint_common_footer(&runtime_rel),
    ))
}

pub fn projection_manifest_file_ref(roots: &ResolvedProjectionRoots, path: &Path) -> String {
    path.strip_prefix(&roots.project_root)
        .map(|rel| rel.to_string_lossy().trim_start_matches('/').to_string())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub fn claude_settings_hook_status(path: &Path) -> Result<Value, String> {
    let payload = read_json_if_exists(path)?;
    let mut managed_events = Vec::new();
    if let Some(Value::Object(root)) = payload.as_ref()
        && let Some(Value::Object(hooks)) = root.get("hooks") {
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
    projection_file_status(path, "claude")
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

/// Independent `mcp-codegraph` stdio server.
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
    let framework = host_router_rs_framework_payload(
        roots,
        "claude",
        "Framework snapshot, skill routing, goal/closeout gating",
    );
    let framework_changed = entries.get("router-rs-framework") != Some(&framework);
    entries.insert("router-rs-framework".to_string(), framework);
    let browser = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser);
    entries.insert("browser-mcp".to_string(), browser);
    let plain = paperplain_mcp_server_payload();
    let paperplain_changed = entries.get("paperplain") != Some(&plain);
    entries.insert("paperplain".to_string(), plain);
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    write_json_if_changed(&path, &payload).map(|file_changed| {
        framework_changed
            || browser_changed
            || paperplain_changed
            || codegraph_changed
            || file_changed
    })
}

/// Remove all managed MCP entries from project-root `.mcp.json`.
pub fn remove_project_mcp_json_entries(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".mcp.json");
    mcp_json_remove_servers(&path, &roots.framework_root, McpConfigFormat::CLAUDE)
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
    let block = format!(
        "{marker}\n{}",
        render_codex_mcp_toml_section(server_id, command, args)
    );
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

/// Codex reads MCP from project `.codex/config.toml` (`mcp_servers.*` sections).
pub fn ensure_codex_research_mcp_toml(roots: &ResolvedProjectionRoots) -> Result<bool, String> {
    let path = roots.project_root.join(".codex/config.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut changed = false;
    // -- router-rs-framework --
    let framework = host_router_rs_framework_payload(
        roots,
        "codex",
        "Framework snapshot, skill routing, goal/closeout gating (Codex)",
    );
    let fw_cmd = framework
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("router-rs");
    let fw_args: Vec<String> = framework
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let fw_arg_refs: Vec<&str> = fw_args.iter().map(String::as_str).collect();
    changed |= upsert_codex_mcp_toml_section(&path, "router-rs-framework", fw_cmd, &fw_arg_refs)?;
    // -- browser-mcp --
    let browser = browser_mcp_server_payload(roots);
    let br_cmd = browser
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("router-rs");
    let br_args: Vec<String> = browser
        .get("args")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let br_arg_refs: Vec<&str> = br_args.iter().map(String::as_str).collect();
    changed |= upsert_codex_mcp_toml_section(&path, "browser-mcp", br_cmd, &br_arg_refs)?;
    // -- paperplain --
    changed |=
        upsert_codex_mcp_toml_section(&path, "paperplain", "npx", &["-y", "paperplain-mcp"])?;
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
    let managed_server_ids = registry_managed_mcp_server_ids(&roots.framework_root)?;
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
            let after = if end < result.len() {
                &result[end..]
            } else {
                ""
            };
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

/// 始终返回 disabled 状态。Codex prompt entrypoints 功能已禁用，
/// 保留此函数以维持接口兼容（调用方依赖返回的 JSON 结构）。
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
