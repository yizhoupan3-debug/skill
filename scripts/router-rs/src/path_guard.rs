//! Path safety helpers: single-component `task_id` and repo-relative joins.

use std::fs;
use std::path::{Component, Path, PathBuf};

pub const TASK_ID_COMPONENT_ERR: &str =
    "task_id must be a single safe path component (no /, \\, .., or NUL)";

/// Returns `Some(trimmed)` when `task_id` is safe to use as **one** `Path` component under
/// `artifacts/current/`.
pub fn safe_task_id_component(task_id: &str) -> Option<&str> {
    let tid = task_id.trim();
    if tid.is_empty()
        || tid == "."
        || tid == ".."
        || tid.contains("..")
        || tid.contains('/')
        || tid.contains('\\')
        || tid.contains('\0')
    {
        return None;
    }
    Some(tid)
}

pub fn validate_task_id_component(task_id: &str) -> Result<&str, String> {
    safe_task_id_component(task_id).ok_or_else(|| TASK_ID_COMPONENT_ERR.to_string())
}

/// Join `relative` (POSIX-ish, no `..` / absolute) under `root`. Used for host entrypoint sync
/// and runtime `skill_path` validation.
pub fn join_repo_relative_under_root(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = relative.trim().replace('\\', "/");
    if rel.is_empty() {
        return Err("relative path is empty".to_string());
    }
    let parsed = Path::new(rel.as_str());
    if parsed.is_absolute() {
        return Err(format!(
            "path must be relative to repo root, got {relative:?}"
        ));
    }
    let mut tail = PathBuf::new();
    for c in parsed.components() {
        match c {
            Component::CurDir => {}
            Component::Normal(part) => tail.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("invalid relative path segment in {relative:?}"));
            }
        }
    }
    Ok(root.join(tail))
}

/// True when `path` resolves under `repo_root` (canonical when possible, else lexical `strip_prefix`).
pub fn path_is_within_repo_root(repo_root: &Path, path: &Path) -> bool {
    match (fs::canonicalize(repo_root), fs::canonicalize(path)) {
        (Ok(root), Ok(file)) => file.starts_with(&root),
        _ => path.strip_prefix(repo_root).is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_rejects_parent_dir() {
        let root = std::env::temp_dir().join("path-guard-root-traversal");
        assert!(join_repo_relative_under_root(&root, "a/../../etc/passwd").is_err());
    }

    #[test]
    fn path_within_repo_canonical() {
        let tmp = std::env::temp_dir().join("path-guard-within");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).expect("mkdir");
        let inner = tmp.join("src").join("lib.rs");
        fs::write(&inner, b"fn x() {}\n").expect("write");
        assert!(path_is_within_repo_root(&tmp, &inner));
        assert!(!path_is_within_repo_root(
            &tmp,
            std::path::Path::new("/nonexistent-router-rs-guard-path-xyz"),
        ));
    }
}
