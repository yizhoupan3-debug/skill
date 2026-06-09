use super::super::*;
/// Resolve the artifact base directory.
fn resolve_artifact_base(repo_root: &Path) -> PathBuf {
    std::env::var_os("SKILL_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("artifacts"))
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
    let server = cursor_mcp_server_payload(roots);
    if let Some(payload) = read_json_if_exists(path)? {
        if let Some(existing) = payload
            .get("mcp_servers")
            .and_then(Value::as_object)
            .and_then(|servers| servers.get("browser-mcp"))
        {
            if cursor_mcp_server_semantically_matches_framework(existing, roots) {
                return Ok(CursorMcpInstallOutcome {
                    managed: true,
                    changed: false,
                    reason: "already-managed-equivalent",
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
    let changed = servers.get("browser-mcp") != Some(&server);
    if changed {
        servers.insert("browser-mcp".to_string(), server);
    }
    let file_changed = write_json_if_changed(path, &payload)?;
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
    let mut changed = false;
    if let Some(mcp_servers) = root.get_mut("mcp_servers") {
        if let Some(servers) = mcp_servers.as_object_mut() {
            changed |= servers.remove("browser-mcp").is_some();
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
    str_args.contains(&"mcp-stdio")
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
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains("host_projection: codex"))
            .unwrap_or(false);
    Ok(json!({
        "path": path.to_string_lossy(),
        "exists": path.exists(),
        "managed": verified,
        "verification": if verified { "verified" } else if marker_managed { "unknown" } else { "unmanaged" },
        "marker_managed": marker_managed,
    }))
}

pub fn cursor_projection_file_status(path: &Path) -> Result<Value, String> {
    let content = read_text_if_exists(path)?;
    let marker_managed = content
        .as_deref()
        .map(is_managed_projection_content)
        .unwrap_or(false);
    let verified = marker_managed
        && content
            .as_deref()
            .map(|content| content.contains("host_projection: cursor"))
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
    let normalized = raw.trim().to_lowercase();
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
    let mut tools = registry_projection_tools(framework_root).unwrap_or_else(|_| {
        vec![
            "codex".to_string(),
            "cursor".to_string(),
            "claude".to_string(),
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
    normalize_path(&resolved).map(|resolved| resolved == source_path).map_err(|e| e.to_string())
}

pub fn default_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_bootstrap_output_dir(repo_root: &Path) -> PathBuf {
    resolve_artifact_base(repo_root).join("bootstrap")
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

#[derive(Clone, Debug)]
pub struct MigrationPlan {
    pub source: String,
    pub destination: String,
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
    let root = resolve_artifact_base(repo_root).join("evidence");
    task_id
        .map(|value| root.join(safe_slug(value)))
        .unwrap_or(root)
}

pub fn scratch_artifact_root(repo_root: &Path, run_id: Option<&str>) -> PathBuf {
    let root = resolve_artifact_base(repo_root).join("scratch");
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
    let current_root = resolve_artifact_base(repo_root).join("current");
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
            resolve_artifact_base(repo_root)
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
            resolve_artifact_base(repo_root)
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
    router_rs::path_guard::reject_unsafe_path(path)?;
    let existing = read_text_if_exists(path)?;
    if existing.as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    router_rs::atomic_write::write_atomic_text(path, content)?;
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

