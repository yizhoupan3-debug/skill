//! Tool safety rules: path protection constants and functions.
//! Extracted from claude_hooks.rs (Phase 4 S1).
//! Shared across all 4 hosts for PreToolUse path guarding.

// ── Framework guarded prefixes ──
pub const FRAMEWORK_GUARDED_PREFIXES: &[&str] = &[
    "configs/framework/",
    "skills/SKILL_PLUGIN_CATALOG.json",
    "skills/SKILL_ROUTING_METADATA.json",
    "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
    "skills/SKILL_HEALTH_MANIFEST.json",
    "skills/SKILL_APPROVAL_POLICY.json",
    "skills/SKILL_ROUTING_INDEX.md",
    "skills/SKILL_TIERS.json",
];

// ── Framework source prefixes ──
pub const FRAMEWORK_SOURCE_PREFIXES: &[&str] = &["core/router-rs/", "configs/framework/"];

// ── Cross-host surfaces ──
pub const CROSS_HOST_SURFACES: &[&str] = &[".codex/hooks.json"];

/// Check if a path is a framework-guarded path.
pub fn is_framework_guarded_path(path: &str) -> bool {
    FRAMEWORK_GUARDED_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

/// Check if a path is a cross-host or retired surface.
pub fn is_cross_host_or_retired_surface(path: &str) -> bool {
    CROSS_HOST_SURFACES
        .iter()
        .any(|surface| path == *surface || path.starts_with(&format!("{surface}/")))
}

// ── Host-specific path lists (Phase 4 S2) ──

/// Settings-guarded paths per host.
///
/// Data sourced from `framework_kernel::runtime_registry` (single source of truth).
pub fn settings_guarded_paths(host_id: &str) -> &'static [&'static str] {
    framework_kernel::runtime_registry::HOST_SETTINGS_PATHS
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, paths)| *paths)
        .unwrap_or(&[])
}

/// Generated entrypoint paths per host.
///
/// Data sourced from `framework_kernel::runtime_registry`.
pub fn generated_entrypoint_paths(host_id: &str) -> &'static [&'static str] {
    framework_kernel::runtime_registry::HOST_ENTRYPOINT_PATHS
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, paths)| *paths)
        .unwrap_or(&[])
}

/// Host private config directory leaf name per host.
///
/// Data sourced from `framework_kernel::runtime_registry`.
pub fn host_private_config_dir(host_id: &str) -> &'static str {
    framework_kernel::runtime_registry::HOST_CONFIG_DIRS
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, dir)| *dir)
        .unwrap_or("")
}

/// Check if a path is a host-specific generated entrypoint (parameterized).
pub fn is_generated_entrypoint(host_id: &str, path: &str) -> bool {
    generated_entrypoint_paths(host_id)
        .iter()
        .any(|p| path == *p || path.starts_with(p))
}

/// Check if a path is host-private (hook state, home config dir, etc).
pub fn is_host_private_path(host_id: &str, path: &str) -> bool {
    let dir_str = host_private_config_dir(host_id);
    if dir_str.is_empty() {
        return false;
    }
    let normalized = path.replace('\\', "/");

    // Hook state files under the host config dir are private
    let hook_state_prefix = format!("{dir_str}/hook-state/");
    if normalized.starts_with(&hook_state_prefix) {
        return true;
    }
    // Scratch directory exemption (e.g. .claude/plans/)
    let scratch_prefix = format!("{dir_str}/plans/");
    if normalized.starts_with(&scratch_prefix)
        || normalized.contains(&format!("/{scratch_prefix}"))
    {
        return false;
    }
    // Home directory paths (~/<dir>/... or $HOME/<dir>/...)
    let tilde_prefix = format!("~/{dir_str}/");
    if normalized.starts_with(&tilde_prefix) {
        return true;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home_prefix = std::path::PathBuf::from(home)
            .join(dir_str)
            .to_string_lossy()
            .replace('\\', "/")
            + "/";
        if normalized.starts_with(&home_prefix) {
            return true;
        }
    }
    false
}

/// Check if a path is a host-specific settings path (parameterized).
pub fn is_settings_path(host_id: &str, path: &str) -> bool {
    settings_guarded_paths(host_id)
        .iter()
        .any(|p| path == *p)
}
