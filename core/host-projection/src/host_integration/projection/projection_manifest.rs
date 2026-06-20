//! Projection manifest management.
//!
//! Extracted from projection.rs to keep file size ≤2000 lines.

use super::*;
pub fn make_mcp_server_payload(
    roots: &ResolvedProjectionRoots,
    host_args: &[&str],
    description: &str,
) -> Value {
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
    if let Some(env) = env
        && let Some(obj) = payload.as_object_mut() {
            obj.insert("env".to_string(), env);
        }
    payload
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
    if let Some(expected) = host_projection
        && manifest.get("host_projection").and_then(Value::as_str) != Some(expected) {
            return false;
        }
    if let Some(expected) = scope
        && manifest.get("scope").and_then(Value::as_str) != Some(expected) {
            return false;
        }
    true
}

pub fn projection_manifest_files_include(
    manifest_path: &Path,
    projection_path: &Path,
) -> Result<bool, String> {
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
        roots
            .host_home_root("codex")
            .expect("codex host must be registered in projection roots")
            .join("prompts")
            .join("framework.md")
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
        roots
            .host_home_root("cursor")
            .expect("cursor host must be registered in projection roots")
            .join("rules")
            .join("framework.mdc")
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
        roots
            .host_home_root("codex")
            .expect("codex host must be registered in projection roots")
            .clone()
    } else {
        roots.project_root.join(".codex")
    }
}

pub fn projection_manifest_path(
    roots: &ResolvedProjectionRoots,
    host_projection: &str,
    scope: &str,
) -> PathBuf {
    let manifest_name = FRAMEWORK_PROJECTION_MANIFEST_NAME;
    if scope == "user" {
        return roots
            .host_home_root(host_projection)
            .unwrap_or_else(|| {
                panic!(
                    "host '{}' must be registered in projection roots",
                    host_projection
                )
            })
            .join(manifest_name);
    }
    // Project scope: use .<host_dir> under project_root.
    // Host dir mapping: host_id -> dotfile name (e.g. "claude" -> ".claude")
    let host_dir = match host_projection {
        "claude" => ".claude".to_string(),
        other => format!(".{other}"),
    };
    roots.project_root.join(&host_dir).join(manifest_name)
}

/// Unified projection manifest writer for all hosts.
/// Replaces per-host `write_*_projection_manifest` functions.
pub fn write_projection_manifest(
    roots: &ResolvedProjectionRoots,
    host_projection: &str,
    scope: &str,
    files: &[String],
    managed_key_paths: &[String],
) -> Result<bool, String> {
    write_json_if_changed(
        &projection_manifest_path(roots, host_projection, scope),
        &json!({
            "schema_version": FRAMEWORK_PROJECTION_SCHEMA_VERSION,
            "managed_by": "skill-framework",
            "host_projection": host_projection,
            "scope": scope,
            "files": files,
            "settings": {
                "managed_key_paths": managed_key_paths,
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
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nargument-hint: \"[framework task...]\"\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: codex -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse `$framework` semantics via the Rust-owned shared core.\n\n{gsd}\n\n{review}\n\n1) Start from `AGENTS.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n\n$ARGUMENTS\n",
        gsd = lifecycle_paragraph_for_host(&narrative, "codex"),
        review = narrative.review_findings_only_paragraph,
    )
}

pub fn cursor_mcp_config_path(roots: &ResolvedProjectionRoots) -> PathBuf {
    roots
        .host_home_root("cursor")
        .expect("cursor host must be registered in projection roots")
        .join("mcp.json")
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
    let framework_server = host_router_rs_framework_payload(
        roots,
        "cursor",
        "Framework snapshot, skill routing, goal/closeout gating (Cursor)",
    );
    // codegraph 注入走 merge_codegraph_into_mcp_servers_map，无需提前构造
    if let Some(payload) = read_json_if_exists(path)?
        && let Some(existing) = payload
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
                let framework_changed =
                    servers.get("router-rs-framework") != Some(&framework_server);
                if framework_changed {
                    servers.insert("router-rs-framework".to_string(), framework_server);
                }
                let paperplain_changed =
                    merge_paperplain_into_mcp_servers_map(servers, "paperplain");
                let codegraph_changed =
                    merge_codegraph_into_mcp_servers_map(servers, roots, "mcp-codegraph");
                let file_changed = write_json_if_changed(path, &payload)?;
                let changed =
                    framework_changed || paperplain_changed || codegraph_changed || file_changed;
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

pub fn remove_cursor_mcp_server(path: &Path, framework_root: &Path) -> Result<bool, String> {
    mcp_json_remove_servers(path, framework_root, McpConfigFormat::CURSOR)
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
    Ok(Some(cursor_mcp_server_semantically_matches_framework(
        server, roots,
    )))
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

// ── Claude Code MCP Server ──────────────────────────────────────────────────

pub fn claude_mcp_config_path(roots: &ResolvedProjectionRoots, scope: &str) -> PathBuf {
    if scope == "user" {
        roots
            .account_home_root
            .join(".claude/mcp.json")
    } else {
        roots.project_root.join(".mcp.json")
    }
}

pub fn install_claude_mcp_server(
    roots: &ResolvedProjectionRoots,
    path: &Path,
    _scope: &str,
) -> Result<bool, String> {
    let mut payload = read_json_if_exists(path)?.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        payload = json!({});
    }
    let root = payload
        .as_object_mut()
        .ok_or_else(|| "claude mcp.json root must be an object".to_string())?;
    let mcp_servers = root
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
        "claude",
        "Framework snapshot, skill routing, goal/closeout gating",
    );
    let framework_changed = entries.get("router-rs-framework") != Some(&framework_payload);
    if framework_changed {
        entries.insert("router-rs-framework".to_string(), framework_payload);
    }

    let browser_payload = browser_mcp_server_payload(roots);
    let browser_changed = entries.get("browser-mcp") != Some(&browser_payload);
    if browser_changed {
        entries.insert("browser-mcp".to_string(), browser_payload);
    }

    let paperplain_changed = merge_paperplain_into_mcp_servers_map(entries, "paperplain");
    let codegraph_changed = merge_codegraph_into_mcp_servers_map(entries, roots, "mcp-codegraph");

    let file_changed = write_json_if_changed(path, &payload)?;
    Ok(framework_changed || browser_changed || paperplain_changed || codegraph_changed || file_changed)
}

pub fn remove_claude_mcp_server(path: &Path, framework_root: &Path) -> Result<bool, String> {
    mcp_json_remove_servers(path, framework_root, McpConfigFormat::CLAUDE)
}

pub fn render_cursor_framework_entrypoint(roots: &ResolvedProjectionRoots, scope: &str) -> String {
    let narrative = load_host_projection_narrative(&roots.framework_root)
        .expect("host projection narrative must load before rendering cursor entrypoint");
    let runtime_rel = skills_source_rel(&roots.framework_root)
        .map(|source_rel| format!("{source_rel}/SKILL_ROUTING_RUNTIME.json"))
        .unwrap_or_else(|_| "skills/SKILL_ROUTING_RUNTIME.json".to_string());
    format!(
        "---\ndescription: Route framework tasks through the Rust-owned shared core.\nglobs: [\"**/*\"]\nalwaysApply: true\n---\n\n<!-- managed_by: skill-framework -->\n<!-- projection_id: framework-root-entrypoint -->\n<!-- host_projection: cursor -->\n<!-- logical_entrypoint: framework -->\n<!-- framework_schema_version: {FRAMEWORK_PROJECTION_SCHEMA_VERSION} -->\n<!-- install_scope: {scope} -->\n\nUse this repository's shared framework runtime.\n\n{gsd}\n\n{review}\n\n1) Start from `AGENTS.md`。\n2) Route via `{runtime_rel}`.\n3) Read only the matched `skill_path`.\n\nFramework root: `${{FRAMEWORK_ROOT}}`.\nProject root: `${{PROJECT_ROOT}}`.\n",
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

pub(super) fn projection_file_status(path: &Path, host_projection: &str) -> Result<Value, String> {
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

pub fn install_skills_projection_tools(
    command: &str,
    tools: &[String],
    to: &[String],
) -> Vec<String> {
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
    if let Some(adapter) = projection_adapter_for_raw(raw) {
        return Ok(adapter.tool.to_string());
    }
    let known = projection_supported_tools_for_message(framework_root);
    let _aliases = projection_alias_summary();
    Err(format!(
        "Unknown tool: {}. Supported tools: {}",
        raw.trim().to_lowercase(),
        known.join(", "),
    ))
}

pub fn projection_supported_tools_for_message(framework_root: &Path) -> Vec<String> {
    
    registry_projection_tools(framework_root).unwrap_or_else(|_| {
        vec![
            "cursor".to_string(),
            "claude".to_string(),
            "opencode".to_string(),
            "codex".to_string(),
        ]
    })
}
