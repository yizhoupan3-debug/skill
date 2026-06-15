//! Developer Exemption: explicit env gate + canonical path prefix allowlist.
//!
//! Enable via `core-policy` feature `dev-exempt` and `ROUTER_RS_DEV_EXEMPT=1`.

/// Prefixes compared after `fs::canonicalize` (symlink-safe).
#[cfg(feature = "dev-exempt")]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &["artifacts", "target", ".cursor", ".claude"];

#[cfg(not(feature = "dev-exempt"))]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &[];

#[cfg(feature = "dev-exempt")]
const DEV_EXEMPT_ENV: &str = "ROUTER_RS_DEV_EXEMPT";

#[cfg(feature = "dev-exempt")]
use framework_kernel::{TelemetryEvent, emit_telemetry};
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
        Err(_) => repo_root.to_path_buf(),
    };
    // Paths outside the repo are never exempt.
    let Some(rel) = canonical.strip_prefix(&repo).ok() else {
        return false;
    };
    let rel_norm = rel.to_string_lossy().replace('\\', "/");
    EXEMPT_PATH_PREFIXES
        .iter()
        .any(|prefix| rel_norm == *prefix || rel_norm.starts_with(&format!("{prefix}/")))
}

/// Returns true when dev exempt is active and `path` resolves under an exempt prefix.
pub fn should_dev_exempt(path: &Path, repo_root: &Path) -> bool {
    #[cfg(not(feature = "dev-exempt"))]
    {
        let _ = (path, repo_root);
        return false;
    }
    #[cfg(feature = "dev-exempt")]
    {
        if !dev_exempt_enabled() {
            return false;
        }
        let Some(canonical) = canonicalize_best_effort(path) else {
            return false;
        };
        let exempt = path_matches_exempt_prefix(&canonical, repo_root);
        if exempt {
            emit_telemetry(&TelemetryEvent::DevExempt {
                path: canonical.display().to_string(),
                action: "fast_tunnel".into(),
            });
        }
        exempt
    }
}

#[cfg(all(test, feature = "dev-exempt"))]
mod tests {
    use super::*;
    use crate::test_env_sync::{process_env_lock, with_env_var, with_env_var_removed};
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
        // Safety: if repo_root path contains "artifacts" (e.g. /home/user/artifacts-workspace/),
        // a file like src/main.rs should NOT be exempted. This was a bug before the fix
        // that removed the absolute-path `contains` fallback.
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
}
