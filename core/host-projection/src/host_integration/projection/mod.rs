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
pub fn registry_managed_mcp_server_ids(framework_root: &Path) -> Result<Vec<String>> {
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
            return (server_status, false, Some(err.to_string()));
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
                        let resolved = if !cmd.contains('/') && !cmd.contains('\\') {
                            which::which(cmd).ok()
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
                                let msg = "router-rs not found or not runnable; run `router-rs-cli self install`".to_string();
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
                        first_error = Some(err.to_string());
                    }
                    server_status.insert(
                        server_id.to_string(),
                        json!({"binary_valid": false, "error": err.to_string()}),
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
}

impl McpConfigFormat {
    /// JSON config with camelCase `mcpServers` key (Claude, OpenCode, `.mcp.json`).
    pub const JSON_CAMEL_CASE: Self = Self::Json {
        top_level_key: "mcpServers",
    };
    /// JSON config with snake_case `mcp_servers` key (Cursor).
    pub const JSON_SNAKE_CASE: Self = Self::Json {
        top_level_key: "mcp_servers",
    };
}

/// Remove managed MCP servers from a JSON config file.
/// Returns whether the file was changed.
pub fn mcp_json_remove_servers(
    path: &Path,
    framework_root: &Path,
    format: McpConfigFormat,
) -> Result<bool> {
    let McpConfigFormat::Json { top_level_key } = format;
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
) -> Result<Vec<String>> {
    let McpConfigFormat::Json { top_level_key } = format;
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
    host_id: &str,
) -> Result<Value> {
    let repo_root = normalize_path(repo_root)?;
    let home_config_path = normalize_path(home_config_path)?;
    let bootstrap_output_dir = bootstrap_output_dir.map(normalize_path).transpose()?;

    let created_config = ensure_config_file(&home_config_path)?;
    let hooks_disabled_changed = ensure_hooks_feature_disabled(&home_config_path)?;
    let tui_changed = ensure_tui_status_line(&home_config_path)?;
    let home_config_dir = home_config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            default_home_dir().join(framework_kernel::runtime_registry::host_private_config_dir(
                host_id,
            ))
        });
    let prompt_entrypoints = prompt_entrypoints_disabled(&home_config_dir);
    let default_bootstrap = if install_default_bootstrap {
        ensure_default_bootstrap(&repo_root, bootstrap_output_dir.as_deref())?
    } else {
        Value::Null
    };

    Ok(json!({
        "success": true,
        "repo_root": repo_root.to_string_lossy(),
        "home_config_path": home_config_path.to_string_lossy(),
        "prompt_entrypoints": prompt_entrypoints,
        "created_config": created_config,
        "hooks_enabled": false,
        "hooks_disabled_changed": hooks_disabled_changed,
        "tui_status_line_changed": tui_changed,
        "default_bootstrap": default_bootstrap,
    }))
}

pub fn projection_install_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
) -> Result<Value> {
    let host_homes = command.parsed_host_homes();
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        &host_homes,
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

pub fn projection_status_command(command: ProjectionStatusCommand) -> Result<Value> {
    let host_homes = command.parsed_host_homes();
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        &host_homes,
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
) -> Result<Value> {
    projection_remove_or_cleanup_command(command, compatibility_alias, false)
}

pub fn projection_cleanup_command(command: ProjectionCommand) -> Result<Value> {
    projection_remove_or_cleanup_command(command, false, true)
}

pub fn projection_remove_or_cleanup_command(
    command: ProjectionCommand,
    compatibility_alias: bool,
    cleanup_mode: bool,
) -> Result<Value> {
    let host_homes = command.parsed_host_homes();
    let roots = resolve_projection_roots(
        command.framework_root.as_deref(),
        command.project_root.as_deref(),
        command.artifact_root.as_deref(),
        &host_homes,
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
) -> Result<()> {
    if !cleanup_mode || scope != "user" {
        return Ok(());
    }
    for tool in tools {
        let host_id = projection_ops_trait::projection_ops_for_tool(tool)
            .map(|a| a.host_id())
            .unwrap_or(tool);
        let env_var = format!("{}_HOME", host_id.to_uppercase().replace('-', "_"));
        // Check: --home flag, --host-home override, or $HOST_HOME env var.
        let host_cli_home_set = command.host_home_is_set(host_id);
        let explicit_home =
            command.home.is_some() || host_cli_home_set || std::env::var_os(&env_var).is_some();
        if !explicit_home {
            return Err(format!(
                "user-scope cleanup for {tool} requires explicit host-home resolution; pass --host-home <host_id>=<path>, --home, or the matching host HOME environment variable"
            ).into());
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
) -> Result<Value> {
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
) -> Result<Value> {
    let mut host_home_roots = serde_json::Map::new();
    let registry =
        framework_kernel::runtime_registry::load_runtime_registry_json(&roots.framework_root)?;
    for (host_id, tool) in pairs {
        if !framework_kernel::framework_host_targets::host_is_installable(&registry, host_id)? {
            host_home_roots.insert(host_id.clone(), Value::Null);
            continue;
        }
        let _ = tool; // tool not needed for home_root; host_id suffices
        let home = roots
            .host_home_root(host_id)
            .ok_or_else(|| format!("host_home_root: host_id {host_id:?} not found"))?
            .to_string_lossy()
            .into_owned();
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
) -> Result<Vec<String>> {
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
        )
        .into());
    }
    Ok(selected)
}

pub fn default_projection_tools_for_scope(
    framework_root: &Path,
    scope: &str,
) -> Result<Vec<String>> {
    let mut tools = registry_projection_tools(framework_root)?;
    if canonical_scope(scope)? == "project" {
        // Exclude hosts whose projection is user-scope only (registry: `install_scopes: ["user"]`).
        tools.retain(|tool| !tool_force_user_scope(tool));
    }
    Ok(tools)
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

// ── Projection ops — generated HostProjectionOps impls call unified functions ──
// `install_projection` / `projection_status` / `remove_projection` dispatch on host_id internally.
// The code is generated by build.rs from RUNTIME_REGISTRY.json host_targets.supported.

pub fn registry_projection_tools(framework_root: &Path) -> Result<Vec<String>> {
    let pairs = framework_kernel::framework_host_targets::installable_host_id_and_skills_install_tool_pairs(
        framework_root,
    )?;
    let mut tools = Vec::new();
    for (_host_id, tool) in &pairs {
        let _ = projection_ops_trait::projection_ops_for_tool(tool).ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY host {_host_id:?} declares unsupported install_tool {tool:?}; extend host projection ops"
            )
        })?;
        if !tools.contains(tool) {
            tools.push(tool.clone());
        }
    }
    let registry = framework_kernel::runtime_registry::load_runtime_registry_json(framework_root)?;
    framework_kernel::framework_host_targets::validate_host_providers_against_registry(&registry)?;
    Ok(tools)
}

pub fn canonical_scope(scope: &str) -> Result<&'static str> {
    match scope.trim().to_lowercase().as_str() {
        "" | "project" | "project-local" => Ok("project"),
        "user" => Ok("user"),
        other => Err(format!("Unsupported scope: {other}. Supported scopes: project user").into()),
    }
}

/// Returns the effective scope for a tool: user-scope-only hosts always return "user",
/// others defer to the requested scope. Reads `install_scopes` from RUNTIME_REGISTRY.
pub fn projection_scope_for_tool(tool: &str, scope: &str) -> Result<&'static str> {
    if tool_force_user_scope(tool) {
        let _ = canonical_scope(scope)?;
        return Ok("user");
    }
    canonical_scope(scope)
}

/// Returns true if this tool's projection is user-scope only (not available at project scope).
/// Reads `install_scopes` from RUNTIME_REGISTRY: hosts with `["user"]` only are excluded from project scope.
fn tool_force_user_scope(tool: &str) -> bool {
    let scopes = framework_kernel::runtime_registry::install_scopes(tool);
    scopes.len() == 1 && scopes[0] == "user"
}

pub fn install_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
) -> Result<Value> {
    if tool.contains("..") || tool.contains('/') || tool.contains('\\') {
        return Err(format!("Invalid tool name: {}", tool).into());
    }
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    Ok(projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .install(roots, effective_scope)?)
}

pub fn projection_tool_status(roots: &ResolvedProjectionRoots, tool: &str) -> Result<Value> {
    Ok(projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .status(roots)?)
}

pub fn remove_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
    dry_run: bool,
) -> Result<Value> {
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    Ok(projection_ops_trait::projection_ops_for_tool(tool)
        .ok_or_else(|| format!("No projection ops registered for tool: {tool}"))?
        .remove(roots, effective_scope, dry_run)?)
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

pub fn load_host_projection_narrative(framework_root: &Path) -> Result<HostProjectionNarrative> {
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
        )
        .into());
    }
    Ok(narrative)
}

pub fn render_project_narrative(roots: &ResolvedProjectionRoots, host_id: &str) -> Result<String> {
    let narrative = load_host_projection_narrative(&roots.framework_root).map_err(|err| {
        format!(
            "host projection narrative must load before rendering project narrative for {host_id}: {err}"
        )
    })?;
    Ok(format!(
        r#"<!-- managed_by: skill-framework · {host_id} · keep ≤48 lines -->
<!-- projection_id: {host_id}-project-narrative -->
<!-- host_projection: {host_id} -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**（宿主差异见该文件内「宿主行为差异」节）。

## 语言（硬约束）

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
- 仅当用户**当轮明确要求英文**时可切换。
- **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」。

{gsd}

## Hook 集成（非 MCP）

- 四事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/Quality Gate：`framework_goal_drive` / `framework_quality_gate` stdio + `artifacts/current/<task_id>/`。
- 默认 **交互式模式**：closeout/complete 为 advisory，suppress review Stop nudge；非交互式时 closeout 可 fail-closed（与 REVIEW_GATE advisory 分层，见 `docs/architecture.md` §6）。
- 检查点：`session_checkpoint`（非自动）。
- Goal 自动触发：`UserPromptSubmit` 检测复杂任务（自然语言+启发式）→ 注入 goal 建议上下文；`has_structured_goal_contract` 已扩展为在 regex 失败时回退到复杂度分析。
- Goal amend：`goal_state_manage(operation="amend")` 更新 goal 字段，保留 checkpoints；scope change 检测自动触发 `[Goal Amendment]` 上下文注入。
- Goal 完成自动归档：`complete` 操作标记 `archived: true`，不再物理删除 GOAL_STATE.json。
- 严格退出验证：Stop 管线读取磁盘 `done_when` 与响应内容比对，列出未完成项。

## MCP（可选）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
"#,
        gsd = lifecycle_paragraph_for_host(&narrative, host_id),
    ))
}

pub fn projection_manifest_file_ref(roots: &ResolvedProjectionRoots, path: &Path) -> String {
    path.strip_prefix(&roots.project_root)
        .map(|rel| rel.to_string_lossy().trim_start_matches('/').to_string())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub fn settings_hook_status(path: &Path, host_id: &str) -> Result<Value> {
    let payload = read_json_if_exists(path)?;
    let mut managed_events = Vec::new();
    if let Some(Value::Object(root)) = payload.as_ref()
        && let Some(Value::Object(hooks)) = root.get("hooks")
    {
        for event in ALL_HOOK_EVENTS {
            if hooks
                .get(*event)
                .map(|v| value_contains_router_rs_hook(v, host_id))
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
pub fn ensure_project_research_mcp_json(
    roots: &ResolvedProjectionRoots,
    host_id: &str,
) -> Result<bool> {
    let path = roots.project_root.join(".mcp.json");
    let mut payload = read_json_if_exists(&path)?.unwrap_or_else(|| json!({}));
    let entries = projection_manifest::mcp_servers_mut(&mut payload, "mcpServers", host_id)?;
    let mut changed = false;
    for (id, val) in [
        (
            "router-rs-framework",
            host_router_rs_framework_payload(
                roots,
                host_id,
                "Framework snapshot, skill routing, goal/closeout gating",
            ),
        ),
        ("browser-mcp", browser_mcp_server_payload(roots)),
        ("paperplain", paperplain_mcp_server_payload()),
    ] {
        if entries.get(id) != Some(&val) {
            changed = true;
        }
        entries.insert(id.to_string(), val);
    }
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");
    changed |= codegraph_changed;
    Ok(write_json_if_changed(&path, &payload).map(|file_changed| changed || file_changed)?)
}

/// Remove all managed MCP entries from project-root `.mcp.json`.
pub fn remove_project_mcp_json_entries(roots: &ResolvedProjectionRoots) -> Result<bool> {
    let path = roots.project_root.join(".mcp.json");
    Ok(mcp_json_remove_servers(
        &path,
        &roots.framework_root,
        McpConfigFormat::JSON_CAMEL_CASE,
    )?)
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

fn mcp_toml_managed_marker(server_id: &str) -> String {
    format!("# managed_by: skill-framework · mcp_servers.{server_id}")
}

fn render_mcp_toml_section(server_id: &str, command: &str, args: &[&str]) -> String {
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

fn upsert_mcp_toml_section(
    path: &Path,
    server_id: &str,
    command: &str,
    args: &[&str],
) -> Result<bool> {
    let marker = mcp_toml_managed_marker(server_id);
    let block = format!(
        "{marker}\n{}",
        render_mcp_toml_section(server_id, command, args)
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
    Ok(write_text_if_changed(path, &normalized)?)
}

/// Ensure MCP tool research sections exist in the project-scope TOML config.
pub fn ensure_research_mcp_toml(roots: &ResolvedProjectionRoots, host_id: &str) -> Result<bool> {
    let rel = framework_kernel::runtime_registry::host_projection_mcp_relative(host_id, "project");
    let path = roots.project_root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let fw_description =
        format!("Framework snapshot, skill routing, goal/closeout gating ({host_id})");
    let mut changed = false;
    for (id, payload) in [
        (
            "router-rs-framework",
            host_router_rs_framework_payload(roots, host_id, &fw_description),
        ),
        ("browser-mcp", browser_mcp_server_payload(roots)),
        ("mcp-codegraph", codegraph_mcp_server_payload(roots)),
    ] {
        let cmd = payload
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("router-rs-cli");
        let args: Vec<String> = payload
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        changed |= upsert_mcp_toml_section(&path, id, cmd, &arg_refs)?;
    }
    changed |= upsert_mcp_toml_section(&path, "paperplain", "npx", &["-y", "paperplain-mcp"])?;
    Ok(changed)
}

/// Remove all managed MCP TOML sections from the project-scope TOML config.
pub fn remove_research_mcp_toml_entries(
    roots: &ResolvedProjectionRoots,
    host_id: &str,
) -> Result<bool> {
    let rel = framework_kernel::runtime_registry::host_projection_mcp_relative(host_id, "project");
    let path = roots.project_root.join(rel);
    let managed_server_ids = registry_managed_mcp_server_ids(&roots.framework_root)?;
    let existing = read_text_if_exists(&path)?.unwrap_or_default();
    let mut result = existing.clone();
    let mut changed = false;
    for server_id in &managed_server_ids {
        let marker = mcp_toml_managed_marker(server_id);
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
pub fn prompt_entrypoints_disabled(config_dir: &Path) -> Value {
    let prompt_dir = config_dir.join("prompts");
    json!({
        "changed": false,
        "enabled": false,
        "prompt_dir": prompt_dir.to_string_lossy(),
        "written": [],
        "unchanged": [],
    })
}
