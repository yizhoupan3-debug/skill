//! Projection drift detection: warns when the installed Codex hook manifest
//! is older than the compiled-in projection version.

use serde_json::Value;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Router-rs hook projection version (shared across all hosts).
pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";

static DRIFT_CACHE: LazyLock<std::sync::Mutex<(std::time::Instant, Option<String>)>> =
    LazyLock::new(|| {
        std::sync::Mutex::new((
            std::time::Instant::now() - std::time::Duration::from_secs(600),
            None,
        ))
    });

pub fn projection_version_older(manifest_version: &str, current: &str) -> bool {
    fn parse(value: &str) -> Option<(u64, u64, u64)> {
        let cleaned = value.trim().trim_start_matches('v');
        let mut parts = cleaned.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }
    match (parse(manifest_version), parse(current)) {
        (Some(found), Some(expected)) => found < expected,
        _ => true,
    }
}

pub fn check_hook_projection_drift(repo_root: &Path) -> Option<String> {
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);
    {
        let guard = DRIFT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if guard.0.elapsed() < CACHE_TTL {
            return guard.1.clone();
        }
    }
    let warning = "[router-rs] hook projection drift detected; consider re-running `router-rs framework maint install-codex-user-hooks`.".to_string();
    let local_codex_home = repo_root.join("codex-home");
    let manifest_path = if local_codex_home.is_dir() {
        local_codex_home.join(".router-rs-install.manifest.json")
    } else {
        let codex_home = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
        if !codex_home.is_dir() {
            return None;
        }
        codex_home.join(".router-rs-install.manifest.json")
    };
    let text = match fs::read_to_string(manifest_path) {
        Ok(v) => v,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(warning),
    };
    let manifest: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Some(warning),
    };
    let projection = manifest
        .get("projection_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = if projection_version_older(projection, ROUTER_RS_HOOK_PROJECTION_VERSION) {
        Some(warning)
    } else {
        None
    };
    if let Ok(mut guard) = DRIFT_CACHE.lock() {
        *guard = (std::time::Instant::now(), result.clone());
    }
    result
}
