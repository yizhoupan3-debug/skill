use super::super::*;

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
        command.opencode_home.as_deref(),
        command.home.as_deref(),
    )?;
    let mut results = Map::new();
    for tool in crate::framework_host_targets::skills_install_tools_ordered(&roots.framework_root)?
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
    let pairs = crate::framework_host_targets::host_id_and_skills_install_tool_pairs(
        &roots.framework_root,
    )?;
    let registry =
        router_rs::runtime_registry::load_runtime_registry_json(&roots.framework_root)?;
    let mut host_targets_map = serde_json::Map::new();
    for (host_id, tool) in &pairs {
        let value = if crate::framework_host_targets::host_is_installable(&registry, host_id)? {
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
        router_rs::runtime_registry::load_runtime_registry_json(&roots.framework_root)?;
    for (host_id, tool) in pairs {
        if !crate::framework_host_targets::host_is_installable(&registry, host_id)? {
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
