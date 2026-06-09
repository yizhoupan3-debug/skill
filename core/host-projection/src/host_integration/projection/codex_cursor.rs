use super::super::*;

pub fn install_codex_projection(roots: &ResolvedProjectionRoots, scope: &str) -> Result<Value, String> {
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
    let mut binary_valid = false;
    let mut status_error = None;

    if mcp_exists {
        if let Ok(Some(mcp_json)) = read_json_if_exists(&mcp_path) {
            if let Some(cmd) = mcp_json
                .get("mcp_servers")
                .and_then(|v| v.get("browser-mcp"))
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
            {
                match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                    Ok(()) => binary_valid = true,
                    Err(err) => status_error = Some(err.to_string()),
                }
            } else {
                status_error =
                    Some("Invalid or incomplete mcp_servers.browser-mcp payload structure".to_string());
            }
        } else {
            status_error = Some("Failed to read or parse ~/.cursor/mcp.json".to_string());
        }
    } else {
        status_error = Some("~/.cursor/mcp.json does not exist".to_string());
    }

    let ready = rules_ready && mcp_exists && binary_valid;
    Ok(json!({
        "ready": ready,
        "status": "projection-status",
        "error": status_error,
        "rules": {
            "framework": {
                "user": cursor_projection_file_status(&user_target)?,
            }
        },
        "mcp_config": {
            "user_scope": mcp_exists,
            "project_scope": false,
            "binary_valid": binary_valid,
            "path": mcp_path.to_string_lossy(),
            "server": "browser-mcp",
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
