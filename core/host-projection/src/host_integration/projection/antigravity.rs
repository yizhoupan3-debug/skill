use super::super::*;

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

pub fn antigravity_projection_status(roots: &ResolvedProjectionRoots) -> Result<Value, String> {
    let project_mcp_path = roots.project_root.join(".gemini/mcp.json");
    let project_settings_path = roots.project_root.join(".gemini/settings.json");
    let project_framework_md_path = roots.project_root.join(".gemini/antigravity/rules/framework.md");
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
                    Err(err) => status_error = Some(err.to_string()),
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
            projection_manifest_ownership(&manifest_path, "antigravity-app", scope, target)
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
        projection_manifest_ownership(&manifest_path, "antigravity-app", scope, &framework_md_target)
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
        roots.project_root.join(".gemini/antigravity/rules/framework.md")
    }
}

pub fn antigravity_mcp_server_payload(roots: &ResolvedProjectionRoots) -> Value {
    make_mcp_server_payload(
        roots,
        &["antigravity-app", "agent", "--repo-root", roots.project_root.to_string_lossy().as_ref()],
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
    entries.insert(
        "router-rs-framework".to_string(),
        antigravity_mcp_server_payload(roots),
    );
    write_json_if_changed(path, &payload)
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
         <!-- host_projection: antigravity-app -->\n\
         <!-- install_scope: {scope} -->\n\n\
         # Antigravity Framework\n\n\
         Antigravity **App**（Desktop / Planning Mode）**`router-rs-framework`** MCP。协议：**`docs/hosts/antigravity-app.md`**；跨宿主 **`AGENTS.md`**；**`AGENTS_ANTIGRAVITY.md`**。\n\n\
         ## 会话操作（按序）\n\n\
         1. `framework_snapshot` — 开头一次\n\
         2. `skill_route` → 只读 `skill_path`\n\
         3. `goal_state_manage operation=start`（宏任务）\n\
         4. 验证后 `record_evidence`\n\
         5. `closeout_gate` → `goal_state_manage operation=complete`\n\n\
         ## 门控说明（App / MCP）\n\n\
         - **App 无 shell hook 表**；`goal_state_manage complete` 与 `closeout_gate` 在 MCP 工具层报告 findings（advisory，不阻断）。终端 **Antigravity CLI** 使用 `.antigravitycli/hooks.json`（见 CLI 手册）。\n\n\
         ## 共享资源\n\n\
         与其它宿主共用 `artifacts/current/` 工作区。路由：`{runtime_rel}`。\n"
    );
    write_text_if_changed(path, &content)
}

pub fn antigravity_projection_manifest_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    let app = projection_manifest_path(roots, "antigravity-app", scope);
    if app.exists() {
        return app;
    }
    projection_manifest_path(roots, "antigravity", scope)
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
            "host_projection": "antigravity-app",
            "scope": scope,
            "files": [
                mcp_path.to_string_lossy(),
                settings_path.to_string_lossy(),
                framework_md_path.to_string_lossy()
            ],
        }),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectionManifestOwnership {
    pub managed: bool,
    pub owns_projection_file: bool,
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
    let changed = entries.get("router-rs-framework") != Some(&framework_payload);
    entries.insert("router-rs-framework".to_string(), framework_payload);
    write_json_if_changed(&config_path, &payload)?;

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
                "managed_key_paths": ["mcpServers.router-rs-framework"],
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

    let mcp_command = read_json_if_exists(&project_path)
        .ok()
        .flatten()
        .and_then(|payload| {
            payload.get("mcpServers")
                .and_then(|s| s.get("router-rs-framework"))
                .cloned()
        })
        .or_else(|| {
            read_json_if_exists(&user_path)
                .ok()
                .flatten()
                .and_then(|payload| {
                    payload.get("mcpServers")
                        .and_then(|s| s.get("router-rs-framework"))
                        .cloned()
                })
        });

    let mut binary_valid = false;
    let mut status_error = None;
    if let Some(payload) = mcp_command.as_ref() {
        if let Some(cmd) = payload.get("command").and_then(Value::as_str) {
            match validate_mcp_command_binary(cmd, Some(&roots.framework_root)) {
                Ok(()) => match router_rs::router_self::validate_router_rs_binary_runnable(Path::new(cmd)) {
                    Ok(()) => binary_valid = true,
                    Err(err) => status_error = Some(err.to_string()),
                },
                Err(err) => status_error = Some(err.to_string()),
            }
        }
    }

    Ok(json!({
        "ready": (project_exists || user_exists) && binary_valid,
        "status": "projection-status",
        "error": status_error,
        "mcp_config": {
            "project_scope": project_exists,
            "user_scope": user_exists,
            "binary_valid": binary_valid,
            "router_rs_framework": mcp_command,
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
        if let Some(servers) = payload.get_mut("mcpServers").and_then(Value::as_object_mut) {
            if servers.remove("router-rs-framework").is_some() {
                changed = true;
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

