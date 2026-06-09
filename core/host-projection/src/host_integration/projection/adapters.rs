use super::super::*;

use super::antigravity::{
    antigravity_projection_status, install_antigravity_projection, install_opencode_projection,
    opencode_home_explicit, opencode_home_root_string, opencode_projection_status,
    remove_antigravity_projection, remove_opencode_projection,
};
use super::claude::{
    claude_projection_status, install_claude_projection, remove_claude_projection,
};
use super::codex_cursor::{
    codex_projection_status, cursor_projection_status, install_codex_projection,
    install_cursor_projection, remove_codex_projection, remove_cursor_projection,
};
pub struct HostProjectionAdapter {
    pub tool: &'static str,
    pub host_id: &'static str,
    pub aliases: &'static [&'static str],
    pub install: fn(&ResolvedProjectionRoots, &str) -> Result<Value, String>,
    pub status: fn(&ResolvedProjectionRoots) -> Result<Value, String>,
    pub remove: fn(&ResolvedProjectionRoots, &str, bool) -> Result<Value, String>,
    pub home_root: fn(&ResolvedProjectionRoots) -> String,
    pub explicit_home: fn(&ProjectionCommand) -> bool,
}

const HOST_PROJECTION_ADAPTERS: &[HostProjectionAdapter] = &[
    HostProjectionAdapter {
        tool: "codex",
        host_id: "codex",
        aliases: &["codex"],
        install: install_codex_projection,
        status: codex_projection_status,
        remove: remove_codex_projection,
        home_root: codex_home_root_string,
        explicit_home: codex_home_explicit,
    },
    HostProjectionAdapter {
        tool: "cursor",
        host_id: "cursor",
        aliases: &[],
        install: install_cursor_projection,
        status: cursor_projection_status,
        remove: remove_cursor_projection,
        home_root: cursor_home_root_string,
        explicit_home: cursor_home_explicit,
    },
    HostProjectionAdapter {
        tool: "claude",
        host_id: "claude-code",
        aliases: &["claude-code"],
        install: install_claude_projection,
        status: claude_projection_status,
        remove: remove_claude_projection,
        home_root: claude_home_root_string,
        explicit_home: claude_home_explicit,
    },
    HostProjectionAdapter {
        tool: "antigravity",
        host_id: "antigravity",
        aliases: &[],
        install: install_antigravity_projection,
        status: antigravity_projection_status,
        remove: remove_antigravity_projection,
        home_root: antigravity_home_root_string,
        explicit_home: antigravity_home_explicit,
    },
    HostProjectionAdapter {
        tool: "opencode",
        host_id: "opencode",
        aliases: &[],
        install: install_opencode_projection,
        status: opencode_projection_status,
        remove: remove_opencode_projection,
        home_root: opencode_home_root_string,
        explicit_home: opencode_home_explicit,
    },
];
pub fn projection_adapter(tool: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = tool.trim().to_lowercase();
    HOST_PROJECTION_ADAPTERS
        .iter()
        .find(|adapter| adapter.tool == normalized)
}

pub fn projection_adapter_for_raw(raw: &str) -> Option<&'static HostProjectionAdapter> {
    let normalized = raw.trim().to_lowercase();
    HOST_PROJECTION_ADAPTERS.iter().find(|adapter| {
        adapter.tool == normalized || adapter.aliases.iter().any(|alias| *alias == normalized)
    })
}

pub fn registry_projection_tools(framework_root: &Path) -> Result<Vec<String>, String> {
    let pairs = crate::framework_host_targets::installable_host_id_and_skills_install_tool_pairs(
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
    Ok(tools)
}

pub fn validate_projection_adapters_against_registry(framework_root: &Path) -> Result<(), String> {
    let registry = router_rs::runtime_registry::load_runtime_registry_json(framework_root)?;
    let supported = crate::framework_host_targets::host_targets_supported_host_ids(&registry)?;
    for adapter in HOST_PROJECTION_ADAPTERS {
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
    HOST_PROJECTION_ADAPTERS
        .iter()
        .flat_map(|adapter| {
            adapter
                .aliases
                .iter()
                .map(move |alias| format!("{alias} → {}", adapter.tool))
        })
        .collect::<Vec<_>>()
        .join(", ")
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
    if projection_adapter(tool).is_some_and(|adapter| adapter.tool == "cursor") {
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
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    (adapter.install)(roots, effective_scope)
}

pub fn projection_tool_status(roots: &ResolvedProjectionRoots, tool: &str) -> Result<Value, String> {
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    (adapter.status)(roots)
}

pub fn remove_projection_tool(
    roots: &ResolvedProjectionRoots,
    tool: &str,
    scope: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let adapter = projection_adapter(tool).ok_or_else(|| format!("Unsupported tool: {tool}"))?;
    let effective_scope = projection_scope_for_tool(tool, scope)?;
    (adapter.remove)(roots, effective_scope, dry_run)
}


pub fn codex_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.codex_home_root.to_string_lossy().into_owned()
}

pub fn cursor_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.cursor_home_root.to_string_lossy().into_owned()
}

pub fn claude_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.claude_home_root.to_string_lossy().into_owned()
}

pub fn codex_home_explicit(command: &ProjectionCommand) -> bool {
    command.codex_home.is_some() || std::env::var_os("CODEX_HOME").is_some()
}

pub fn cursor_home_explicit(command: &ProjectionCommand) -> bool {
    command.cursor_home.is_some() || std::env::var_os("CURSOR_HOME").is_some()
}

pub fn claude_home_explicit(command: &ProjectionCommand) -> bool {
    command.claude_home.is_some() || std::env::var_os("CLAUDE_HOME").is_some()
}

pub fn antigravity_home_root_string(roots: &ResolvedProjectionRoots) -> String {
    roots.antigravity_home_root.to_string_lossy().into_owned()
}

pub fn antigravity_home_explicit(command: &ProjectionCommand) -> bool {
    command.antigravity_home.is_some() || std::env::var_os("ANTIGRAVITY_HOME").is_some()
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
