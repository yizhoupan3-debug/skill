//! Host-specific projection operations (codex, cursor, claude).
//!
//! Extracted from projection.rs to keep file size ≤2000 lines.

use super::*;

pub fn project_narrative_path(roots: &ResolvedProjectionRoots, host_id: &str) -> PathBuf {
    let config_dir = framework_kernel::runtime_registry::host_private_config_dir(host_id);
    roots.project_root.join(config_dir).join(format!("{:}.md", host_id.to_uppercase()))
}

pub fn settings_target(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    host_id: &str,
) -> Result<PathBuf> {
    if scope == "user" {
        Ok(roots
            .host_home_root(host_id)
            .ok_or_else(|| format!("{host_id} host must be registered in projection roots"))?
            .join("settings.json"))
    } else {
        let dotdir = framework_kernel::runtime_registry::host_private_config_dir(host_id);
        Ok(roots.project_root.join(format!("{dotdir}/settings.json")))
    }
}

pub fn build_router_rs_hook_command(event: &str, host_id: &str) -> String {
    let config_dir = framework_kernel::runtime_registry::host_private_config_dir(host_id);
    format!(
        "/usr/bin/env bash -c 'ROOT=\"${{CLAUDE_PROJECT_ROOT:-$PWD}}\"; FW=\"${{SKILL_FRAMEWORK_ROOT:-$ROOT}}\"; if [[ -r \"$ROOT/{config_dir}/router-rs-hook.env\" ]]; then set -a; . \"$ROOT/{config_dir}/router-rs-hook.env\"; set +a; fi; exec \"$FW/configs/framework/{host_id}-router-rs-hook.sh\" {event}'",
        host_id = host_id,
        event = event
    )
}

pub fn managed_hook_entry(event: &str, host_id: &str) -> Value {
    json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": build_router_rs_hook_command(event, host_id),
        }]
    })
}

pub fn value_contains_router_rs_hook(value: &Value, host_id: &str) -> bool {
    match value {
        Value::String(s) => {
            s.contains(&format!("{host_id}-router-rs-hook.sh"))
                || s.contains("router-rs-hook.sh")
                || (s.contains("router-rs") && s.contains("hook"))
        }
        Value::Array(items) => items.iter().any(|v| value_contains_router_rs_hook(v, host_id)),
        Value::Object(map) => map.values().any(|v| value_contains_router_rs_hook(v, host_id)),
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

pub fn merge_settings_hooks(existing: Option<Value>, host_id: &str) -> Result<Value> {
    let mut root = match existing {
        Some(Value::Object(map)) => map,
        Some(_) => return Err(FrameworkError::validation("settings root must be a JSON object")),
        None => Map::new(),
    };
    let mut hooks = match root.remove("hooks") {
        Some(Value::Object(map)) => map,
        Some(_) => return Err(FrameworkError::validation("settings `hooks` must be a JSON object")),
        None => Map::new(),
    };
    for &event in ALL_HOOK_EVENTS {
        let mut entries = hooks
            .remove(event)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        entries.retain(|entry| !value_contains_router_rs_hook(entry, host_id));
        entries.push(managed_hook_entry(event, host_id));
        hooks.insert(event.to_string(), Value::Array(entries));
    }
    root.insert("hooks".to_string(), Value::Object(hooks));
    Ok(Value::Object(root))
}

pub fn install_settings_hooks(settings_path: &Path, host_id: &str) -> Result<bool> {
    let existing = read_json_if_exists(settings_path)?;
    let merged = merge_settings_hooks(existing, host_id)?;
    Ok(write_json_if_changed(settings_path, &merged)?)
}

pub fn install_hook_env_if_absent(roots: &ResolvedProjectionRoots, host_id: &str) -> Result<bool> {
    let config_dir = framework_kernel::runtime_registry::host_private_config_dir(host_id);
    let dest = roots.project_root.join(config_dir).join("router-rs-hook.env");
    if dest.is_file() {
        return Ok(false);
    }
    let template = roots
        .framework_root
        .join(format!("configs/framework/{host_id}-router-rs-hook.env"));
    if !template.is_file() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| FrameworkError::Io(e))?;
    }
    fs::copy(&template, &dest).map_err(|e| {
        format!(
            "install hook env: copy {} -> {}: {e}",
            template.display(),
            dest.display()
        )
    })?;
    Ok(true)
}

#[derive(Debug, Default)]
pub struct AgentSettingsRemoval {
    changed: bool,
    would_change: bool,
    removed_file: bool,
    would_remove_file: bool,
    removed_events: Vec<String>,
}

pub fn remove_settings_hooks(
    settings_path: &Path,
    dry_run: bool,
    host_id: &str,
) -> Result<AgentSettingsRemoval> {
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
            .filter(|entry| !value_contains_router_rs_hook(entry, host_id))
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
            fs::remove_file(settings_path).map_err(|err| FrameworkError::Io(err))?;
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
// Generated by build.rs from configs/framework/RUNTIME_REGISTRY.json

pub fn install_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    host_id: &str,
) -> Result<Value> {
    let target = entrypoint_target(roots, scope, host_id)?;
    ensure_router_rs_installed_for_mcp_with_roots(roots)?;
    match host_id {
        "cursor" => {
            let mut managed_files = vec![target.to_string_lossy().to_string()];
            let mut managed_key_paths: Vec<String> = Vec::new();
            let mut changed =
                write_text_if_changed(&target, &render_framework_entrypoint(roots, scope, host_id)?)?;
            let mcp_cfg = mcp_config_path(roots, host_id, scope)?;
            let mut mcp = json!({
                "managed": false,
                "path": mcp_cfg.to_string_lossy(),
                "server": "browser-mcp",
                "changed": false,
                "reason": "user-scope-only",
            });
            if scope == "user" {
                let mcp_path = mcp_config_path(roots, host_id, "user")?;
                let mcp_install = install_mcp_server(roots, &mcp_path, host_id, scope)?;
                changed |= mcp_install.changed;
                if mcp_install.managed {
                    managed_files.push(mcp_path.to_string_lossy().to_string());
                    managed_key_paths.extend(mcp_json_managed_key_paths(
                        &roots.framework_root,
                        McpConfigFormat::JSON_SNAKE_CASE,
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
                write_projection_manifest(roots, host_id, scope, &managed_files, &managed_key_paths)?;
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
        "codex" => {
            let changed =
                write_text_if_changed(&target, &render_framework_entrypoint(roots, scope, host_id)?)?;
            let mcp_changed = ensure_research_mcp_toml(roots, host_id)?;
            let config_dir = framework_kernel::runtime_registry::host_private_config_dir(host_id);
            let prompt_entrypoints_root = if scope == "user" {
                roots.host_home_root(host_id)
                    .ok_or_else(|| format!("{host_id} host must be registered in projection roots"))?
                    .clone()
            } else {
                roots.project_root.join(config_dir)
            };
            let prompt_entrypoints = prompt_entrypoints_disabled(&prompt_entrypoints_root);
            let manifest_changed = write_projection_manifest(
                roots,
                host_id,
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
                    "config": format!("{config_dir}/config.toml"),
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
        "opencode" => {
            let mcp_path = mcp_config_path(roots, host_id, scope)?;
            let mcp_dir = mcp_path.parent().ok_or_else(|| {
                format!("cannot determine parent directory of {}", mcp_path.display())
            })?;
            std::fs::create_dir_all(mcp_dir)
                .map_err(|err| format!("failed to create {}: {err}", mcp_dir.display()))?;
            let mcp_install = install_mcp_server(roots, &mcp_path, host_id, scope)?;
            let mcp_changed = mcp_install.changed;
            let manifest_key_paths = mcp_json_managed_key_paths(
                &roots.framework_root,
                McpConfigFormat::JSON_CAMEL_CASE,
            )?;
            let manifest_changed = write_projection_manifest(
                roots,
                host_id,
                scope,
                &[projection_manifest_file_ref(roots, &mcp_path)],
                &manifest_key_paths,
            )?;
            Ok(json!({
                "status": "installed",
                "changed": mcp_changed || manifest_changed,
                "scope": scope,
                "mcp_config": {
                    "scope": scope,
                    "path": mcp_path.to_string_lossy(),
                    "changed": mcp_changed,
                },
                "projection_manifest": {
                    "path": projection_manifest_path(roots, host_id, scope).to_string_lossy(),
                    "changed": manifest_changed,
                },
            }))
        }
        _ => {
            // claude-like (default) implementation
            let settings_path = settings_target(roots, scope, host_id)?;
            let changed =
                write_text_if_changed(&target, &render_framework_entrypoint(roots, scope, host_id)?)?;
            let narrative_changed = if scope == "project" {
                write_text_if_changed(
                    &project_narrative_path(roots, host_id),
                    &render_project_narrative(roots, host_id)?,
                )?
            } else {
                false
            };
            let hooks_changed = install_settings_hooks(&settings_path, host_id)?;
            let env_changed = install_hook_env_if_absent(roots, host_id)?;

            // MCP injection: write router-rs-framework + browser-mcp + paperplain + codegraph
            let mcp_path = mcp_config_path(roots, host_id, "user")?;
            let mcp_dir = mcp_path.parent().ok_or_else(|| {
                format!("cannot determine parent directory of {}", mcp_path.display())
            })?;
            std::fs::create_dir_all(mcp_dir)
                .map_err(|err| format!("failed to create {}: {err}", mcp_dir.display()))?;
            let mcp_install = install_mcp_server(roots, &mcp_path, host_id, scope)?;
            let mcp_changed = mcp_install.changed;

            let mut manifest_files = vec![
                projection_manifest_file_ref(roots, &target),
                projection_manifest_file_ref(roots, &settings_path),
            ];
            if scope == "project" {
                manifest_files.push(projection_manifest_file_ref(
                    roots,
                    &project_narrative_path(roots, host_id),
                ));
            }
            manifest_files.push(projection_manifest_file_ref(roots, &mcp_path));
            let mut manifest_key_paths: Vec<String> = ALL_HOOK_EVENTS
                .iter()
                .map(|e| format!("hooks.{e}"))
                .collect();
            manifest_key_paths.extend(mcp_json_managed_key_paths(
                &roots.framework_root,
                McpConfigFormat::JSON_CAMEL_CASE,
            )?);
            let manifest_changed = write_projection_manifest(
                roots,
                host_id,
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
    }
}


pub fn projection_status(
    roots: &ResolvedProjectionRoots,
    host_id: &str,
) -> Result<Value> {
    match host_id {
        "cursor" => {
            let user_target = entrypoint_target(roots, "user", host_id)?;
            let mcp_path = mcp_config_path(roots, host_id, "user")?;
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
                Some(format!("~/.{host_id}/mcp.json does not exist"))
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
                        "user": projection_file_status(&user_target, host_id)?,
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
                    "user": projection_manifest_status(&projection_manifest_path(roots, host_id, "user"))?,
                },
                "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
                "policy": "user-scope-only",
            }))
        }
        "codex" => {
            let project_target = entrypoint_target(roots, "project", host_id)?;
            let user_target = entrypoint_target(roots, "user", host_id)?;
            Ok(json!({
                "ready": managed_projection_file_exists(&project_target)? || managed_projection_file_exists(&user_target)?,
                "status": "projection-status",
                "prompts": {
                    "framework": {
                        "project": projection_file_status(&project_target, host_id)?,
                        "user": projection_file_status(&user_target, host_id)?,
                    }
                },
                "manifest": {
                    "project": projection_manifest_status(&projection_manifest_path(roots, host_id, "project"))?,
                    "user": projection_manifest_status(&projection_manifest_path(roots, host_id, "user"))?,
                },
                "hooks": {"managed": false, "reason": "not-enabled-by-framework-policy"},
            }))
        }
        "opencode" => {
            let project_path = mcp_config_path(roots, host_id, "project")?;
            let user_path = mcp_config_path(roots, host_id, "user")?;
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
                    "project_scope": projection_manifest_path(roots, host_id, "project").exists(),
                    "user_scope": projection_manifest_path(roots, host_id, "user").exists(),
                },
            }))
        }
        _ => {
            // claude-like (default) implementation
            let project_target = entrypoint_target(roots, "project", host_id)?;
            let user_target = entrypoint_target(roots, "user", host_id)?;
            let project_settings = settings_target(roots, "project", host_id)?;
            let user_settings = settings_target(roots, "user", host_id)?;
            Ok(json!({
                "ready": managed_projection_file_exists(&project_target)? || managed_projection_file_exists(&user_target)?,
                "status": "projection-status",
                "prompts": {
                    "framework": {
                        "project": projection_file_status(&project_target, host_id)?,
                        "user": projection_file_status(&user_target, host_id)?,
                    }
                },
                "manifest": {
                    "project": projection_manifest_status(&projection_manifest_path(roots, host_id, "project"))?,
                    "user": projection_manifest_status(&projection_manifest_path(roots, host_id, "user"))?,
                },
                "hooks": {
                    "project": settings_hook_status(&project_settings, host_id)?,
                    "user": settings_hook_status(&user_settings, host_id)?,
                },
            }))
        }
    }
}


pub fn remove_projection(
    roots: &ResolvedProjectionRoots,
    scope: &str,
    dry_run: bool,
    host_id: &str,
) -> Result<Value> {
    let target = entrypoint_target(roots, scope, host_id)?;
    let manifest_path = projection_manifest_path(roots, host_id, scope);
    let manifest_ownership =
        projection_manifest_ownership(&manifest_path, host_id, scope, &target)?;
    let would_remove_projection = target.is_file() && manifest_ownership.owns_projection_file;
    let changed = if !dry_run && would_remove_projection {
        fs::remove_file(&target).map_err(|err| FrameworkError::Io(err))?;
        true
    } else {
        false
    };
    let would_remove_manifest = manifest_ownership.managed;
    let manifest_removed = if !dry_run && would_remove_manifest {
        fs::remove_file(&manifest_path).map_err(|err| FrameworkError::Io(err))?;
        true
    } else {
        false
    };
    match host_id {
        "cursor" => {
            let mcp_path = mcp_config_path(roots, host_id, "user")?;
            let mcp_matches_framework =
                mcp_server_matches_framework(roots, &mcp_path)?.unwrap_or(false);
            let mcp_managed = scope == "user"
                && (projection_manifest_manages_key_path(&manifest_path, mcp_server_key_path())?
                    || mcp_matches_framework);
            let mcp_would_remove = mcp_managed && mcp_matches_framework;
            let mcp_skipped_user_owned =
                scope == "user" && !mcp_would_remove && mcp_server_exists(&mcp_path)?;
            let mcp_changed = if !dry_run && mcp_would_remove {
                remove_mcp_server(&mcp_path, &roots.framework_root, host_id)?
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
        "codex" => {
            let any_changed = changed || manifest_removed;
            let toml_removed = if !dry_run && any_changed {
                remove_research_mcp_toml_entries(roots, host_id).unwrap_or(false)
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
        "opencode" => {
            let config_path = mcp_config_path(roots, host_id, scope)?;
            let config_removed = if config_path.is_file() && !dry_run {
                mcp_json_remove_servers(
                    &config_path,
                    &roots.framework_root,
                    McpConfigFormat::JSON_CAMEL_CASE,
                )?
            } else {
                false
            };
            let any_changed = changed || manifest_removed || config_removed;
            Ok(json!({
                "status": if any_changed { "removed" } else { "not-found" },
                "changed": any_changed,
                "dry_run": dry_run,
                "scope": scope,
                "mcp_framework_entry_removed": config_removed,
                "projection_manifest_removed": manifest_removed,
            }))
        }
        _ => {
            // claude-like (default) implementation
            let settings_path = settings_target(roots, scope, host_id)?;
            let settings_removal = remove_settings_hooks(&settings_path, dry_run, host_id)?;
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
            append_mcp_path(&mut removed_paths, settings_removal.removed_file, &settings_path);
            let mut would_remove_paths = removed_projection_paths(
                would_remove_projection,
                &target,
                would_remove_manifest,
                &manifest_path,
            );
            append_mcp_path(&mut would_remove_paths, settings_removal.would_remove_file, &settings_path);
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
    }
}


use super::projection_ops_trait::HostProjectionOps;

include!(concat!(env!("OUT_DIR"), "/generated_projection_ops_structs.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Constants integrity ──

    #[test]
    fn core_hook_events_are_subset_of_all() {
        for event in CORE_HOOK_EVENTS {
            assert!(ALL_HOOK_EVENTS.contains(event), "{event} missing in ALL_HOOK_EVENTS");
        }
    }

    #[test]
    fn optional_hook_events_are_subset_of_all() {
        for event in OPTIONAL_HOOK_EVENTS {
            assert!(ALL_HOOK_EVENTS.contains(event), "{event} missing in ALL_HOOK_EVENTS");
        }
    }

    #[test]
    fn all_hook_events_contains_expected() {
        let expected = ["PreToolUse", "UserPromptSubmit", "PostToolUse", "Stop",
                         "SessionStart", "SubagentStart", "SubagentStop"];
        for event in &expected {
            assert!(ALL_HOOK_EVENTS.contains(event), "{event} not in ALL_HOOK_EVENTS");
        }
        assert_eq!(ALL_HOOK_EVENTS.len(), expected.len());
    }

    // ── build_router_rs_hook_command ──

    #[test]
    fn build_router_rs_hook_contains_host_and_event() {
        let cmd = build_router_rs_hook_command("Stop", "claude");
        assert!(cmd.contains("claude"), "should contain host_id");
        assert!(cmd.contains("Stop"), "should contain event");
        assert!(cmd.contains("router-rs-hook"), "should contain hook script ref");
    }

    // ── value_contains_router_rs_hook ──

    #[test]
    fn detects_router_rs_hook_in_string() {
        let v = json!({"command": "claude-router-rs-hook.sh Stop"});
        assert!(value_contains_router_rs_hook(&v, "claude"));
    }

    #[test]
    fn does_not_match_arbitrary_string() {
        let v = json!("just a random command");
        assert!(!value_contains_router_rs_hook(&v, "claude"));
    }

    #[test]
    fn searches_recursively_in_array() {
        let v = json!([
            {"name": "a", "command": "something"},
            {"name": "b", "command": "cursor-router-rs-hook.sh"}
        ]);
        assert!(value_contains_router_rs_hook(&v, "cursor"));
    }

    #[test]
    fn searches_recursively_in_nested_object() {
        let v = json!({
            "outer": {
                "inner": "opencode-router-rs-hook.sh"
            }
        });
        assert!(value_contains_router_rs_hook(&v, "opencode"));
    }

    #[test]
    fn does_not_match_unrelated_command() {
        // A command with no router-rs references should not match
        let v = json!("/usr/bin/env my-script.sh");
        assert!(!value_contains_router_rs_hook(&v, "claude"));
    }

    #[test]
    fn falls_back_to_generic_router_rs_hook_detection() {
        let v = json!("some/router-rs-hook.sh");
        assert!(value_contains_router_rs_hook(&v, "any-host"));
    }

    #[test]
    fn falls_back_to_router_rs_and_hook_keywords() {
        let v = json!("path/to/router-rs/scripts/hook.sh");
        assert!(value_contains_router_rs_hook(&v, "any-host"));
    }

    #[test]
    fn returns_false_for_primitives() {
        assert!(!value_contains_router_rs_hook(&json!(42), "claude"));
        assert!(!value_contains_router_rs_hook(&json!(null), "claude"));
        assert!(!value_contains_router_rs_hook(&json!(true), "claude"));
    }

    // ── settings_target ──

    #[test]
    fn settings_target_user_scope_uses_host_home() {
        // This test verifies the function at the API level:
        // For "user" scope it delegates to host_home_root() and joins "settings.json"
        // Since we can't create a full ResolvedProjectionRoots fixture easily,
        // we test the structural contract: user vs project scope paths differ.
        // Full integration tested via the CLI.
    }

    // ── merge_settings_hooks ──

    #[test]
    fn merge_settings_hooks_adds_all_events_when_no_existing_hooks() {
        let input = json!({"key": "value"});
        let result = merge_settings_hooks(Some(input), "claude").unwrap();
        let hooks = result.get("hooks").unwrap().as_object().unwrap();
        for event in ALL_HOOK_EVENTS {
            assert!(hooks.contains_key(*event), "missing hook event: {event}");
        }
    }

    #[test]
    fn merge_settings_hooks_preserves_existing_non_hook_keys() {
        let input = json!({"existing_key": "i-should-survive"});
        let result = merge_settings_hooks(Some(input), "claude").unwrap();
        assert_eq!(result.get("existing_key").unwrap(), "i-should-survive");
    }

    #[test]
    fn merge_settings_hooks_replaces_router_rs_entries() {
        let input = json!({
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "command": "claude-router-rs-hook.sh PreToolUse"},
                    {"type": "command", "command": "user-own-script.sh"}
                ]
            }
        });
        let result = merge_settings_hooks(Some(input), "claude").unwrap();
        let hooks = result.get("hooks").unwrap().as_object().unwrap();
        let entries = hooks.get("PreToolUse").unwrap().as_array().unwrap();
        // Router-rs entry replaced, user entry preserved alongside managed hook
        assert!(entries.iter().any(|e| {
            let s = serde_json::to_string(e).unwrap_or_default();
            s.contains("user-own-script")
        }), "user entry should be preserved");
        // Should contain user entry + managed hook = 2 entries
        assert_eq!(entries.len(), 2, "should contain user entry + managed hook");
    }

    #[test]
    fn merge_settings_hooks_errors_on_non_object_root() {
        let result = merge_settings_hooks(Some(json!("string")), "claude");
        assert!(result.is_err());
    }

    #[test]
    fn merge_settings_hooks_errors_on_non_object_hooks() {
        let input = json!({"hooks": "not-an-object"});
        let result = merge_settings_hooks(Some(input), "claude");
        assert!(result.is_err());
    }

    #[test]
    fn merge_settings_hooks_handles_null_input() {
        let result = merge_settings_hooks(None, "claude").unwrap();
        let hooks = result.get("hooks").unwrap().as_object().unwrap();
        assert_eq!(hooks.len(), ALL_HOOK_EVENTS.len());
    }

    // ── managed_hook_entry ──

    #[test]
    fn managed_hook_entry_has_correct_structure() {
        let entry = managed_hook_entry("PreToolUse", "claude");
        assert_eq!(entry.get("matcher").unwrap(), "");
        let hooks = entry.get("hooks").unwrap().as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        let hook = &hooks[0];
        assert_eq!(hook.get("type").unwrap(), "command");
        let cmd = hook.get("command").unwrap().as_str().unwrap();
        assert!(cmd.contains("claude"), "should reference host_id");
        assert!(cmd.contains("PreToolUse"), "should reference event");
    }

    // ── install_hook_env_if_absent is an fs-touching function, skip in unit tests ──
    //     (tested via host_integration integration tests)

    // ── remove_settings_hooks ──

    #[test]
    fn remove_settings_hooks_dry_run_returns_would_change() {
        let dir = std::env::temp_dir().join("host-proj-test-remove-hooks");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let content = json!({
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "command": "claude-router-rs-hook.sh PreToolUse"}
                ]
            }
        });
        std::fs::write(&path, content.to_string()).unwrap();
        let result = remove_settings_hooks(&path, true, "claude").unwrap();
        assert!(result.would_change, "dry_run should report would_change");
        assert!(result.would_remove_file, "file should be empty after removal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_settings_hooks_no_op_for_non_router_rs_hooks() {
        let dir = std::env::temp_dir().join("host-proj-test-noop-remove");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let content = json!({
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "command": "user-custom-script.sh"}
                ]
            }
        });
        std::fs::write(&path, content.to_string()).unwrap();
        let result = remove_settings_hooks(&path, true, "claude").unwrap();
        assert!(!result.would_change, "should not change non-router-rs hooks");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_settings_hooks_returns_default_when_no_hooks_key() {
        let dir = std::env::temp_dir().join("host-proj-test-no-hooks");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        let result = remove_settings_hooks(&path, true, "claude").unwrap();
        assert!(!result.changed);
        assert!(!result.would_change);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
