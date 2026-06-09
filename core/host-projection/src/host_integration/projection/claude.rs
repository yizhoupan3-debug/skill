use super::super::*;

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

/// Optional hook events that may not be supported by all Claude Code versions.
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
    let hooks_changed = install_claude_settings_hooks(&settings_path)?;
    let env_changed = install_claude_hook_env_if_absent(roots)?;
    let manifest_changed = write_claude_projection_manifest(roots, scope, &target, &settings_path)?;
    Ok(json!({
        "status": "installed",
        "changed": changed || hooks_changed || env_changed || manifest_changed,
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
    let any_changed = changed || manifest_removed || settings_removal.changed;
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
    pub review_findings_only_paragraph: String,
}

pub fn lifecycle_paragraph_for_host(narrative: &HostProjectionNarrative, host_projection: &str) -> String {
    narrative
        .lifecycle_by_host
        .get(host_projection)
        .cloned()

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

pub fn render_claude_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let narrative = load_host_projection_narrative(&roots.framework_root)
        .expect("host projection narrative must load before rendering claude entrypoint");
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: claude-code -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n{gsd}\n\n{review}\n\n1) Start from `AGENTS.md`（跨宿主内核）；宿主差异见 `AGENTS_CLAUDE.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n",
        gsd = lifecycle_paragraph_for_host(&narrative, "claude-code"),
        review = narrative.review_findings_only_paragraph,
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
    write_json_if_changed(
        &projection_manifest_path(roots, "claude-code", scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": "claude-code",
            "scope": scope,
            "files": [
                projection_manifest_file_ref(roots, command_path),
                projection_manifest_file_ref(roots, settings_path),
            ],
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
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains("host_projection: claude-code"))
            .unwrap_or(false);
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": verified,
        "verification": if verified { "verified" } else if marker_managed { "unknown" } else { "unmanaged" },
        "marker_managed": marker_managed,
    }))
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

