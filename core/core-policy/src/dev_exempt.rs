//! Developer Exemption: explicit env gate + canonical path prefix allowlist.
//!
//! Enable via `core-policy` feature `dev-exempt` and `ROUTER_RS_DEV_EXEMPT=1`.
//!
//! ## Exemption modes
//!
//! | mode | env | effect |
//! |------|-----|--------|
//! | prefix | `ROUTER_RS_DEV_EXEMPT=1` | Exempt known build/host dirs (`artifacts`, `target`, host config dirs) |
//! | all | `ROUTER_RS_DEV_EXEMPT=1` + `ROUTER_RS_DEV_EXEMPT_ALL=1` | Exempt **every** path under the repo root — for framework developers editing source code |

/// Static prefixes (non-host: build artifacts).
/// Host private config dirs come from `framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS`.
#[cfg(feature = "dev-exempt")]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &["artifacts", "target"];

#[cfg(not(feature = "dev-exempt"))]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &[];

#[cfg(feature = "dev-exempt")]
const DEV_EXEMPT_ENV: &str = "ROUTER_RS_DEV_EXEMPT";
/// When `ROUTER_RS_DEV_EXEMPT_ALL=1` alongside `ROUTER_RS_DEV_EXEMPT=1`,
/// every path under the repo root is exempt (not just build/host dirs).
/// Use when the repo IS the dev workspace (editing framework source).
#[cfg(feature = "dev-exempt")]
const DEV_EXEMPT_ALL_ENV: &str = "ROUTER_RS_DEV_EXEMPT_ALL";

#[cfg(feature = "dev-exempt")]
use std::env;
#[cfg(feature = "dev-exempt")]
use std::fs;
use std::path::Path;
#[cfg(feature = "dev-exempt")]
use std::path::PathBuf;

#[cfg(feature = "dev-exempt")]
fn dev_exempt_enabled() -> bool {
    env::var(DEV_EXEMPT_ENV).as_deref() == Ok("1")
}

/// True when all-repo exemption is requested (not just prefix-matching).
#[cfg(feature = "dev-exempt")]
fn dev_exempt_all_enabled() -> bool {
    env::var(DEV_EXEMPT_ALL_ENV).as_deref() == Ok("1")
}

#[cfg(feature = "dev-exempt")]
fn canonicalize_best_effort(path: &Path) -> Option<PathBuf> {
    if let Ok(c) = fs::canonicalize(path) {
        return Some(c);
    }
    let parent = path.parent()?;
    let name = path.file_name()?;
    fs::canonicalize(parent).ok().map(|p| p.join(name))
}

#[cfg(feature = "dev-exempt")]
fn path_matches_exempt_prefix(canonical: &Path, repo_root: &Path) -> bool {
    let repo = match fs::canonicalize(repo_root) {
        Ok(p) => p,
        Err(_) => {
            tracing::warn!(path = %repo_root.display(), "failed to canonicalize repo_root, using as-is");
            repo_root.to_path_buf()
        }
    };
    // Paths outside the repo are never exempt.
    let Some(rel) = canonical.strip_prefix(&repo).ok() else {
        return false;
    };
    let rel_norm = rel.to_string_lossy().replace('\\', "/");

    // Check static prefixes ("artifacts", "target") then all known host config dirs
    // from RUNTIME_REGISTRY.json (single source of truth via framework-kernel build.rs).
    use framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS;
    EXEMPT_PATH_PREFIXES
        .iter()
        .chain(ALL_KNOWN_HOST_DIRS.iter())
        .any(|prefix| rel_norm == *prefix || rel_norm.starts_with(&format!("{prefix}/")))
}

/// Returns true when dev exempt is active and `path` resolves under an exempt prefix
/// (or when all-repo mode exempts every path under the repo_root).
pub fn should_dev_exempt(path: &Path, repo_root: &Path) -> bool {
    #[cfg(not(feature = "dev-exempt"))]
    {
        let _ = (path, repo_root);
        false
    }
    #[cfg(feature = "dev-exempt")]
    {
        if !dev_exempt_enabled() {
            return false;
        }
        let Some(canonical) = canonicalize_best_effort(path) else {
            return false;
        };
        let Some(canonical_root) = fs::canonicalize(repo_root).ok() else {
            return false;
        };
        // Mode 1: all-repo — every path under repo_root is exempt.
        if dev_exempt_all_enabled() {
            if canonical.starts_with(&canonical_root) {
                return true;
            }
            return false;
        }
        // Mode 2: prefix match — only build/host dirs are exempt.
        let exempt = path_matches_exempt_prefix(&canonical, repo_root);
        exempt
    }
}

#[cfg(all(test, feature = "dev-exempt"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_env_sync::{with_env_var, with_env_var_removed};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("core-policy-exempt-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        repo
    }

    #[test]
    fn exempt_disabled_without_env() {
        let repo = temp_repo();
        with_env_var_removed(DEV_EXEMPT_ENV, || {
            let path = repo.join("artifacts/current/x.json");
            assert!(!should_dev_exempt(&path, &repo));
        });
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn exempt_hits_artifacts_when_enabled() {
        let repo = temp_repo();
        with_env_var(DEV_EXEMPT_ENV, "1", || {
            let path = repo.join("artifacts/current/x.json");
            fs::write(&path, "{}").unwrap();
            assert!(should_dev_exempt(&path, &repo));
        });
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn exempt_rejects_outside_prefix_even_when_enabled() {
        let repo = temp_repo();
        with_env_var(DEV_EXEMPT_ENV, "1", || {
            let path = repo.join("src/main.rs");
            fs::create_dir_all(repo.join("src")).unwrap();
            fs::write(&path, "fn main() {}").unwrap();
            assert!(!should_dev_exempt(&path, &repo));
        });
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn exempt_no_false_positive_when_repo_root_contains_prefix_name() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("artifacts-workspace-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/main.rs"), "fn main() {}").unwrap();
        with_env_var(DEV_EXEMPT_ENV, "1", || {
            let path = repo.join("src/main.rs");
            assert!(
                !should_dev_exempt(&path, &repo),
                "src/main.rs must not be exempt just because repo root contains 'artifacts'"
            );
        });
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn all_repo_exempts_every_path() {
        let repo = temp_repo();
        with_env_var(DEV_EXEMPT_ENV, "1", || {
            with_env_var(DEV_EXEMPT_ALL_ENV, "1", || {
                let src = repo.join("src/main.rs");
                fs::create_dir_all(repo.join("src")).unwrap();
                fs::write(&src, "fn main() {}").unwrap();
                assert!(should_dev_exempt(&src, &repo), "all_repo mode must exempt src/main.rs");
                let deep = repo.join("core/hooks/deep.rs");
                fs::create_dir_all(repo.join("core/hooks")).unwrap();
                fs::write(&deep, "// hook").unwrap();
                assert!(should_dev_exempt(&deep, &repo), "all_repo mode must exempt core/hooks/deep.rs");
                let outside = PathBuf::from("/tmp/outside.txt");
                assert!(!should_dev_exempt(&outside, &repo), "path outside repo must not be exempt");
            });
        });
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn all_repo_requires_dev_exempt_env_too() {
        let repo = temp_repo();
        with_env_var_removed(DEV_EXEMPT_ENV, || {
            with_env_var(DEV_EXEMPT_ALL_ENV, "1", || {
                let path = repo.join("src/main.rs");
                fs::create_dir_all(repo.join("src")).unwrap();
                fs::write(&path, "fn main() {}").unwrap();
                assert!(!should_dev_exempt(&path, &repo), "all_repo without DEV_EXEMPT=1 must not exempt");
            });
        });
        let _ = fs::remove_dir_all(&repo);
    }
}
