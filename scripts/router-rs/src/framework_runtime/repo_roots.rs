//! Repository root discovery for framework continuity paths.
use std::path::{Path, PathBuf};

/// Skill framework 仓库根：`RUNTIME_REGISTRY` + `router-rs` 清单同时存在。
/// 与 host 投影解析共用此判定，避免双份漂移。
pub fn is_framework_root(path: &Path) -> bool {
    path.join("configs/framework/RUNTIME_REGISTRY.json")
        .is_file()
        && path.join("scripts/router-rs/Cargo.toml").is_file()
}

/// CLI / 若干调用方常在 `scripts/router-rs/` 等子目录执行；continuity、`RUNTIME_REGISTRY`
/// 与 `artifacts/current` 均以仓库根为真源，因此从 cwd 或传入路径向上探测 framework root。
pub fn resolve_repo_root_arg(repo_root: Option<&Path>) -> Result<PathBuf, String> {
    let base = if let Some(path) = repo_root {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|err| format!("resolve current directory failed: {err}"))?
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
