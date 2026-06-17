//! Host-specific projection operations (codex, cursor, claude).
//!
//! Extracted from projection.rs to keep file size ≤2000 lines.

use super::*;

pub fn install_codex_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
    let target = codex_entrypoint_target(roots, scope);
    let changed = write_text_if_changed(&target, &render_codex_framework_entrypoint(roots, scope))?;
    let mcp_changed = ensure_codex_research_mcp_toml(roots)?;
    let prompt_entrypoints =
        codex_prompt_entrypoints_disabled(&codex_prompt_entrypoints_root(roots, scope));
    let manifest_changed = write_projection_manifest(
        roots,
        "codex",
        scope,
        &[target.to_string_lossy().into_owned()],
        &[],
    )?;
    let prompt_entrypoints_changed = prompt_entrypoints
        .get("changed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(json!({
        "status": "installed",
        "changed": changed || mcp_changed || manifest_changed || prompt_entrypoints_changed,
        "scope": scope,
        "mcp": {
            "changed": mcp_changed,
            "config": ".codex/config.toml",
        },
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
            managed_key_paths.extend(mcp_json_managed_key_paths(
                &roots.framework_root,
                McpConfigFormat::CURSOR,
            )?);
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
        write_projection_manifest(roots, "cursor", scope, &managed_files, &managed_key_paths)?;
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
    let config_payload = if mcp_exists {
        read_json_if_exists(&mcp_path).ok().flatten()
    } else {
        None
    };
    let (server_status, mcp_valid, mcp_error) =
        validate_mcp_servers_from_json(roots, config_payload.as_ref(), "mcp_servers");
    let all_valid = mcp_valid;
    let first_error = if !mcp_exists {
        Some("~/.cursor/mcp.json does not exist".to_string())
    } else {
        mcp_error
    };

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

// ── Mimo Projection Ops ────────────────────────────────────────────────────

pub fn mimo_config_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots
            .host_home_root("mimo")
            .expect("mimo host must be registered in projection roots")
            .join("settings.json")
    } else {
        roots.project_root.join(".mimo/settings.json")
    }
}

pub fn install_mimo_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
) -> Result<Value, String> {
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
    let config_path = mimo_config_path(roots, scope);
    let config_dir = config_path.parent().ok_or_else(|| {
        format!("cannot determine parent directory of {}", config_path.display())
    })?;
    std::fs::create_dir_all(config_dir)
        .map_err(|err| format!("failed to create {}: {err}", config_dir.display()))?;

    let mut payload = read_json_if_exists(&config_path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let servers = payload
        .as_object_mut()
        .ok_or_else(|| "settings.json root must be an object".to_string())?;
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
        "mimo",
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

    let manifest_changed = write_projection_manifest(
        roots,
        "mimo",
        scope,
        &[projection_manifest_file_ref(roots, &config_path)],
        &mcp_json_managed_key_paths(&roots.framework_root, McpConfigFormat::OPENCODE)?,
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
            "path": projection_manifest_path(roots, "mimo", scope).to_string_lossy(),
            "changed": manifest_changed,
        },
    }))
}

pub fn mimo_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_path = mimo_config_path(roots, "project");
    let user_path = mimo_config_path(roots, "user");
    let project_exists = project_path.is_file();
    let user_exists = user_path.is_file();

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
            "project_scope": projection_manifest_path(roots, "mimo", "project").exists(),
            "user_scope": projection_manifest_path(roots, "mimo", "user").exists(),
        },
    }))
}

pub fn remove_mimo_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let config_path = mimo_config_path(roots, scope);
    let manifest_path = projection_manifest_path(roots, "mimo", scope);

    let mut config_removed = false;
    if config_path.is_file() && !dry_run {
        config_removed = mcp_json_remove_servers(
            &config_path,
            &roots.framework_root,
            McpConfigFormat::OPENCODE,
        )?;
    }

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

pub struct MimoProjectionOps;

impl HostProjectionOps for MimoProjectionOps {
    fn host_id(&self) -> &'static str { "mimo" }

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
        install_mimo_projection(roots, scope)
    }

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String> {
        mimo_projection_status(roots)
    }

    fn remove(&self, roots: &ResolvedProjectionRoots, scope: &str, dry_run: bool) -> Result<Value, String> {
        remove_mimo_projection(roots, scope, dry_run)
    }
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
        remove_cursor_mcp_server(&mcp_path, &roots.framework_root)?
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
        roots
            .host_home_root("claude-code")
            .expect("claude-code host must be registered in projection roots")
            .join("rules")
            .join("framework.md")
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
        roots
            .host_home_root("claude-code")
            .expect("claude-code host must be registered in projection roots")
            .join("settings.json")
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
pub(super) const CORE_HOOK_EVENTS: &[&str] =
    &["PreToolUse", "UserPromptSubmit", "PostToolUse", "Stop"];

/// Optional hook events that may not be supported by all Claude Code versions.
/// Installation continues gracefully if these are absent from the host.
pub(super) const OPTIONAL_HOOK_EVENTS: &[&str] = &["SessionStart", "SubagentStart", "SubagentStop"];

/// All hook events (core + optional), in canonical order.
pub(super) const ALL_HOOK_EVENTS: &[&str] = &[
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
        write_text_if_changed(&target, &render_claude_framework_entrypoint(roots, scope)?)?;
    let narrative_changed = if scope == "project" {
        write_text_if_changed(
            &claude_project_narrative_path(roots),
            &render_claude_project_narrative(roots)?,
        )?
    } else {
        false
    };
    let hooks_changed = install_claude_settings_hooks(&settings_path)?;
    let env_changed = install_claude_hook_env_if_absent(roots)?;

    // MCP injection: write router-rs-framework + browser-mcp + paperplain + codegraph
    let mcp_path = claude_mcp_config_path(roots, scope);
    let mcp_dir = mcp_path.parent().ok_or_else(|| {
        format!("cannot determine parent directory of {}", mcp_path.display())
    })?;
    std::fs::create_dir_all(mcp_dir)
        .map_err(|err| format!("failed to create {}: {err}", mcp_dir.display()))?;
    let mcp_changed = install_claude_mcp_server(roots, &mcp_path, scope)?;

    let mut manifest_files = vec![
        projection_manifest_file_ref(roots, &target),
        projection_manifest_file_ref(roots, &settings_path),
    ];
    if scope == "project" {
        manifest_files.push(projection_manifest_file_ref(
            roots,
            &claude_project_narrative_path(roots),
        ));
    }
    manifest_files.push(projection_manifest_file_ref(roots, &mcp_path));
    let mut manifest_key_paths: Vec<String> = ALL_HOOK_EVENTS
        .iter()
        .map(|e| format!("hooks.{e}"))
        .collect();
    manifest_key_paths.extend(mcp_json_managed_key_paths(
        &roots.framework_root,
        McpConfigFormat::CLAUDE,
    )?);
    let manifest_changed = write_projection_manifest(
        roots,
        "claude-code",
        scope,
        &manifest_files,
        &manifest_key_paths,
    )?;
    Ok(json!({
        "status": "installed",
        "changed": changed || narrative_changed || hooks_changed || env_changed || mcp_changed || manifest_changed,
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
        "mcp": {
            "managed": true,
            "path": mcp_path.to_string_lossy(),
            "changed": mcp_changed,
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
    if settings_removal.removed_file
        && let Some(paths) = removed_paths.as_array_mut() {
            paths.push(Value::String(settings_path.to_string_lossy().into_owned()));
        }
    let mut would_remove_paths = removed_projection_paths(
        would_remove_projection,
        &target,
        would_remove_manifest,
        &manifest_path,
    );
    if settings_removal.would_remove_file
        && let Some(paths) = would_remove_paths.as_array_mut() {
            paths.push(Value::String(settings_path.to_string_lossy().into_owned()));
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

// ── HostProjectionOps trait implementations ──

use super::projection_ops_trait::HostProjectionOps;

pub struct CursorProjectionOps;

impl HostProjectionOps for CursorProjectionOps {
    fn host_id(&self) -> &'static str { "cursor" }

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
        install_cursor_projection(roots, scope)
    }

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String> {
        cursor_projection_status(roots)
    }

    fn remove(&self, roots: &ResolvedProjectionRoots, scope: &str, dry_run: bool) -> Result<Value, String> {
        remove_cursor_projection(roots, scope, dry_run)
    }
}

pub struct ClaudeProjectionOps;

impl HostProjectionOps for ClaudeProjectionOps {
    fn host_id(&self) -> &'static str { "claude" }

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
        install_claude_projection(roots, scope)
    }

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String> {
        claude_projection_status(roots)
    }

    fn remove(&self, roots: &ResolvedProjectionRoots, scope: &str, dry_run: bool) -> Result<Value, String> {
        remove_claude_projection(roots, scope, dry_run)
    }
}

pub struct OpencodeProjectionOps;

impl HostProjectionOps for OpencodeProjectionOps {
    fn host_id(&self) -> &'static str { "opencode" }

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
        install_opencode_projection(roots, scope)
    }

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String> {
        opencode_projection_status(roots)
    }

    fn remove(&self, roots: &ResolvedProjectionRoots, scope: &str, dry_run: bool) -> Result<Value, String> {
        remove_opencode_projection(roots, scope, dry_run)
    }
}

pub struct CodexProjectionOps;

impl HostProjectionOps for CodexProjectionOps {
    fn host_id(&self) -> &'static str { "codex" }

    fn install(&self, roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
        install_codex_projection(roots, scope)
    }

    fn status(&self, roots: &ResolvedProjectionRoots) -> Result<Value, String> {
        codex_projection_status(roots)
    }

    fn remove(&self, roots: &ResolvedProjectionRoots, scope: &str, dry_run: bool) -> Result<Value, String> {
        remove_codex_projection(roots, scope, dry_run)
    }
}
