//! Developer Exemption: explicit env gate + canonical path prefix allowlist (Roadmap v5 §4.4).
//!
//! Enable via `core-policy` feature `dev-exempt` and `ROUTER_RS_DEV_EXEMPT=1`.

/// Prefixes compared after `fs::canonicalize` (symlink-safe).
#[cfg(feature = "dev-exempt")]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &[
    "artifacts",
    "target",
    ".cursor",
    ".claude",
];

#[cfg(not(feature = "dev-exempt"))]
pub const EXEMPT_PATH_PREFIXES: &[&str] = &[];

#[cfg(feature = "dev-exempt")]
const DEV_EXEMPT_ENV: &str = "ROUTER_RS_DEV_EXEMPT";

#[cfg(feature = "dev-exempt")]
use framework_kernel::{emit_telemetry, TelemetryEvent};
#[cfg(feature = "dev-exempt")]
use std::env;
#[cfg(feature = "dev-exempt")]
use std::fs;
use std::path::{Path, PathBuf};

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
    let rel = canonical.strip_prefix(&repo).ok();
    let rel_str = rel
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| canonical.to_string_lossy().to_string());
    let rel_norm = rel_str.replace('\\', "/");
    EXEMPT_PATH_PREFIXES.iter().any(|prefix| {
        rel_norm == *prefix
            || rel_norm.starts_with(&format!("{prefix}/"))
            || canonical.to_string_lossy().contains(&format!("/{prefix}/"))
    })
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
    use crate::test_env_sync::process_env_lock;
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
        let _lock = process_env_lock();
        let repo = temp_repo();
        env::remove_var(DEV_EXEMPT_ENV);
        let path = repo.join("artifacts/current/x.json");
        assert!(!should_dev_exempt(&path, &repo));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn exempt_hits_artifacts_when_enabled() {
        let _lock = process_env_lock();
        let repo = temp_repo();
        env::set_var(DEV_EXEMPT_ENV, "1");
        let path = repo.join("artifacts/current/x.json");
        fs::write(&path, "{}").unwrap();
        assert!(should_dev_exempt(&path, &repo));
        env::remove_var(DEV_EXEMPT_ENV);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn exempt_rejects_outside_prefix_even_when_enabled() {
        let _lock = process_env_lock();
        let repo = temp_repo();
        env::set_var(DEV_EXEMPT_ENV, "1");
        let path = repo.join("src/main.rs");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(&path, "fn main() {}").unwrap();
        assert!(!should_dev_exempt(&path, &repo));
        env::remove_var(DEV_EXEMPT_ENV);
        let _ = fs::remove_dir_all(&repo);
    }
}
