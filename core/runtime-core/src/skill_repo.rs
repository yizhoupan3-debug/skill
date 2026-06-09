//! Locate the skill policy repository root (the checkout that contains routing runtime).

use std::path::{Path, PathBuf};

fn is_skill_policy_repo_root(path: &Path) -> bool {
    path.join("skills/SKILL_ROUTING_RUNTIME.json").is_file() && path.join("AGENTS.md").is_file()
}

/// Resolve the skill repo using env overrides first, then walking parents from [`std::env::current_dir`].
pub fn discover_skill_policy_repo_root() -> Option<PathBuf> {
    for key in [
        "CODEX_PROJECT_ROOT",
        "CURSOR_PROJECT_ROOT",
        "SKILL_REPO_ROOT",
    ] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if is_skill_policy_repo_root(&p) {
                return Some(p.canonicalize().unwrap_or(p));
            }
        }
    }
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if is_skill_policy_repo_root(&cur) {
            return Some(cur.canonicalize().unwrap_or(cur));
        }
        cur = cur.parent()?.to_path_buf();
    }
}

pub fn skill_routing_runtime_json(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_ROUTING_RUNTIME.json")
}
