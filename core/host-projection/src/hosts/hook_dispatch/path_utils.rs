use std::path::{Path, PathBuf};

/// Lexically normalize `.` / `..` segments (no filesystem access).
/// Delegates to `pretool::normalize_repo_relative_path` for canonical path normalization.
pub fn normalize_path_lexical(path: &Path) -> PathBuf {
    PathBuf::from(
        crate::hosts::host_extensions::pretool::normalize_repo_relative_path(
            &path.to_string_lossy(),
        ),
    )
}

/// Collapse `.` / `..` in a relative path string. Extra `..` at virtual root are ignored.
/// (i.e. `a/../../b` normalizes to `b`, matching the original manual implementation.)
pub fn compact_repo_relative_segments(rel_raw: &str) -> Option<PathBuf> {
    let normalized = crate::hosts::host_extensions::pretool::normalize_repo_relative_path(rel_raw);
    // Strip leading ".." components: they represent paths that went above
    // the virtual root, which should be treated as at-root for relative purposes.
    let mut s = normalized.as_str();
    while let Some(rest) = s.strip_prefix("../") {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix("..") {
        s = rest;
    }
    if s.is_empty() || s == "." {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Check if a JSON key name suggests it contains a file path.
pub fn is_path_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("path")
        || lower.contains("file")
        || lower.contains("dir")
        || lower.contains("directory")
        || lower == "cwd"
        || lower == "command"
}

/// Check if a path looks like framework source code.
pub fn is_framework_source_path(path: &str) -> bool {
    path.contains("/core/") || path.contains("/src/") || path.ends_with(".rs")
}

/// Check if a path is a generated entrypoint file.
pub fn is_generated_entrypoint(path: &str) -> bool {
    path.contains("hook") && (path.ends_with(".sh") || path.ends_with(".json"))
}

/// Check if a path is host-private state (data-driven via generated `host_home_dirs`).
pub fn is_host_private_path(path: &str) -> bool {
    if path.contains("/hook-state/") {
        return true;
    }
    for dir in framework_core::runtime_registry::host_home_dirs() {
        if path.contains(&format!("/{dir}/")) {
            return true;
        }
    }
    false
}

/// Check if a path is a settings/config file.
pub fn is_settings_path(path: &str) -> bool {
    path.ends_with("settings.json")
        || path.ends_with("settings.local.json")
        || path.ends_with("hooks.json")
        || path.ends_with("config.json")
}
