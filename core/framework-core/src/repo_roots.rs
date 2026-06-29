//! Repository root discovery for framework continuity paths.
use core_errors::FrameworkError;
use std::path::{Path, PathBuf};

/// Skill framework 仓库根：`RUNTIME_REGISTRY` + `router-rs` 清单同时存在。
/// 与 host 投影解析共用此判定，避免双份漂移。
pub fn is_framework_root(path: &Path) -> bool {
    path.join("configs/framework/RUNTIME_REGISTRY.json")
        .is_file()
        && path.join("core/router-rs/Cargo.toml").is_file()
}

/// 从 `router-rs`（或其它位于框架仓库树下）的可执行路径向上探测 skill 框架仓库根，判定同 [`is_framework_root`]。
///
/// 先尝试 `canonicalize`，失败则用原始路径再向上 walk，减轻部分平台上 `current_exe()` 不便解析带来的影响。
pub fn framework_root_from_executable_path(exe: &Path) -> Option<PathBuf> {
    let normalized = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    if is_framework_root(&normalized) {
        return Some(normalized);
    }
    for ancestor in normalized.ancestors() {
        if is_framework_root(ancestor) {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

/// Returns true when the raw string is a recognized MCP placeholder that should be
/// resolved to an actual directory path.
fn is_mcp_placeholder(raw: &str) -> bool {
    let trimmed = raw.trim();
    matches!(
        trimmed,
        "${workspaceRoot}" | "${CLAUDE_PROJECT_DIR}" | "${CLAUDE_PROJECT_DIR:-.}" | "."
    )
}

/// CLI / 若干调用方常在 `core/router-rs/` 等子目录执行；continuity、`RUNTIME_REGISTRY`
/// 与 `artifacts/current` 均以仓库根为真源，因此从 cwd 或传入路径向上探测 framework root。
fn repo_root_from_mcp_placeholder(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed == "${workspaceRoot}"
        || trimmed == "${CLAUDE_PROJECT_DIR}"
        || trimmed == "${CLAUDE_PROJECT_DIR:-.}"
        || trimmed == "."
    {
        if let Ok(value) = std::env::var("CLAUDE_PROJECT_DIR") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(PathBuf::from(value));
            }
        }
        if trimmed == "." || trimmed == "${CLAUDE_PROJECT_DIR:-.}" {
            return None;
        }
        if trimmed == "${workspaceRoot}" {
            return std::env::current_dir().ok();
        }
    }
    None
}

pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, FrameworkError> {
    let base = if let Some(path) = repo_root {
        if let Some(resolved) = path.to_str().and_then(repo_root_from_mcp_placeholder) {
            resolved
        } else if path.to_str().map(is_mcp_placeholder).unwrap_or(false) {
            // MCP placeholder not resolved (e.g. CLAUDE_PROJECT_DIR not set in Claude Desktop).
            // Fall back to cwd instead of using the literal placeholder string as a path.
            std::env::current_dir().map_err(|err| {
                FrameworkError::validation(format!("resolve current directory failed: {err}"))
            })?
        } else {
            path.to_path_buf()
        }
    } else if let Ok(value) = std::env::var("CLAUDE_PROJECT_DIR") {
        let value = value.trim();
        if value.is_empty() {
            std::env::current_dir().map_err(|err| {
                FrameworkError::validation(format!("resolve current directory failed: {err}"))
            })?
        } else {
            PathBuf::from(value)
        }
    } else {
        std::env::current_dir().map_err(|err| {
            FrameworkError::validation(format!("resolve current directory failed: {err}"))
        })?
    };
    let normalized = base.canonicalize().unwrap_or(base);
    if is_framework_root(&normalized) {
        return Ok(normalized);
    }
    for ancestor in normalized.ancestors() {
        if is_framework_root(ancestor) {
            return Ok(ancestor.to_path_buf());
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod repo_root_placeholder_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::resolve_repo_root_arg;
    use std::env;

    #[test]
    fn workspace_root_placeholder_falls_back_to_cwd_without_claude_project_dir() {
        let prior = env::var_os("CLAUDE_PROJECT_DIR");
        // SAFETY: Rust 2024 edition requires `unsafe` for `env::remove_var` /
        // `env::set_var` because they are not thread-safe on all platforms.
        // This is safe in the test context: the test runner does not read or
        // write `CLAUDE_PROJECT_DIR` concurrently, and the original value is
        // restored before the function returns.
        unsafe { env::remove_var("CLAUDE_PROJECT_DIR") };
        let cwd = env::current_dir().expect("cwd");
        let resolved =
            resolve_repo_root_arg(Some(std::path::Path::new("${workspaceRoot}"))).expect("resolve");
        // resolve_repo_root_arg walks up ancestors to find framework root;
        // the resolved path must be cwd or one of its ancestors.
        assert!(
            cwd.starts_with(&resolved),
            "expected resolved={:?} to be an ancestor of cwd={:?}",
            resolved,
            cwd
        );
        if let Some(value) = prior {
            // SAFETY: see above — test-only, restoring original env value.
            unsafe { env::set_var("CLAUDE_PROJECT_DIR", value) };
        }
    }
}

#[cfg(test)]
mod framework_root_from_exe_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::{framework_root_from_executable_path, is_framework_root};
    use std::fs;

    fn touch_framework_root_skeleton(root: &std::path::Path) {
        fs::create_dir_all(root.join("configs/framework")).unwrap();
        fs::write(
            root.join("configs/framework/RUNTIME_REGISTRY.json"),
            r#"{"schema_version":"framework-runtime-registry-v2","framework_commands":{}}"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("core/router-rs")).unwrap();
        fs::write(
            root.join("core/router-rs/Cargo.toml"),
            "[package]\nname = \"router-rs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
    }

    #[test]
    fn resolves_from_nested_fake_release_binary() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-exe-resolve-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        touch_framework_root_skeleton(&tmp);
        let fake_exe = tmp.join("core/router-rs/target/release/router-rs");
        fs::create_dir_all(fake_exe.parent().unwrap()).unwrap();
        fs::write(&fake_exe, b"").unwrap();

        let got = framework_root_from_executable_path(&fake_exe).expect("should find root");
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(got.canonicalize().unwrap_or_else(|_| got.clone()), expect);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn returns_none_when_no_markers_along_path() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-exe-nomarker-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("a/b")).unwrap();
        let fake_exe = tmp.join("a/b/tool");
        fs::write(&fake_exe, b"").unwrap();
        assert!(framework_root_from_executable_path(&fake_exe).is_none());
        assert!(!is_framework_root(&tmp));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolves_when_exe_is_at_framework_root_router_rs_binary_path_shape() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-fw-exe-at-rootshape-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        touch_framework_root_skeleton(&tmp);
        // Simulate a copied binary sitting directly under core/router-rs/ (still below root).
        let fake_exe = tmp.join("core/router-rs/router-rs");
        fs::write(&fake_exe, b"").unwrap();

        let got = framework_root_from_executable_path(&fake_exe).expect("ancestor walk");
        let expect = tmp.canonicalize().unwrap_or_else(|_| tmp.clone());
        assert_eq!(got.canonicalize().unwrap_or_else(|_| got.clone()), expect);
        let _ = fs::remove_dir_all(&tmp);
    }
}
