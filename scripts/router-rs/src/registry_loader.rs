//! Disk-primary [`RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json) loader for hook hot paths.
//! Replaces compile-time `include_str!` for `review_gate` lane sets (ADR-005).

use crate::lane_normalize::normalize_subagent_lane;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct ReviewGateSnapshot {
    deep_gate_lanes: HashSet<String>,
    claude_reviewer_lanes: HashSet<String>,
}

static CACHE: OnceLock<Mutex<HashMap<PathBuf, ReviewGateSnapshot>>> = OnceLock::new();

thread_local! {
    static HOOK_REGISTRY_REPO_ROOT: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Set the repo root for the current hook dispatch (Cursor/Codex/Claude stdin handlers).
pub fn set_hook_registry_repo_root(repo_root: &Path) {
    HOOK_REGISTRY_REPO_ROOT.with(|c| *c.borrow_mut() = Some(repo_root.to_path_buf()));
}

/// Clear after hook dispatch completes (best-effort; avoids leaking across tests on same thread).
pub fn clear_hook_registry_repo_root() {
    HOOK_REGISTRY_REPO_ROOT.with(|c| *c.borrow_mut() = None);
}

/// RAII: binds disk registry lookup to this repo for the current thread.
pub struct HookRegistryRepoGuard;

impl HookRegistryRepoGuard {
    pub fn new(repo_root: &Path) -> Self {
        set_hook_registry_repo_root(repo_root);
        Self
    }
}

impl Drop for HookRegistryRepoGuard {
    fn drop(&mut self) {
        clear_hook_registry_repo_root();
    }
}

fn cache() -> &'static Mutex<HashMap<PathBuf, ReviewGateSnapshot>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn default_registry_json_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/framework/RUNTIME_REGISTRY.json")
}

fn registry_json_path(repo_root: Option<&Path>) -> PathBuf {
    if let Some(root) = repo_root {
        return root.join("configs/framework/RUNTIME_REGISTRY.json");
    }
    if let Some(root) = HOOK_REGISTRY_REPO_ROOT.with(|c| c.borrow().clone()) {
        return root.join("configs/framework/RUNTIME_REGISTRY.json");
    }
    default_registry_json_path()
}

fn repo_cache_key(registry_path: &Path) -> PathBuf {
    registry_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| registry_path.to_path_buf())
}

fn lane_set(root: &Value, field: &str) -> Result<HashSet<String>, String> {
    let lanes = root
        .get("review_gate")
        .and_then(|v| v.get(field))
        .and_then(Value::as_array)
        .ok_or_else(|| format!("RUNTIME_REGISTRY.review_gate.{field} must be a non-empty array"))?;
    let mut out = HashSet::new();
    for item in lanes {
        let s = item
            .as_str()
            .ok_or_else(|| format!("review_gate.{field} entry must be string"))?;
        out.insert(normalize_subagent_lane(s));
    }
    if out.is_empty() {
        return Err(format!(
            "RUNTIME_REGISTRY.review_gate.{field} must not be empty"
        ));
    }
    Ok(out)
}

fn load_snapshot_from_disk(registry_path: &Path) -> Result<ReviewGateSnapshot, String> {
    let raw =
        std::fs::read_to_string(registry_path).map_err(|e| format!("read registry: {e}"))?;
    let root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse registry: {e}"))?;
    Ok(ReviewGateSnapshot {
        deep_gate_lanes: lane_set(&root, "deep_gate_lanes")?,
        claude_reviewer_lanes: lane_set(&root, "claude_reviewer_lanes")?,
    })
}

fn snapshot(repo_root: Option<&Path>) -> Result<ReviewGateSnapshot, String> {
    let path = registry_json_path(repo_root);
    let key = repo_cache_key(&path);
    let mut guard = cache()
        .lock()
        .map_err(|e| format!("registry cache lock poisoned: {e}"))?;
    if let Some(hit) = guard.get(&key) {
        return Ok(hit.clone());
    }
    let loaded = load_snapshot_from_disk(&path)?;
    guard.insert(key, loaded.clone());
    Ok(loaded)
}

/// Fail-closed: registry unreadable → lane is **not** treated as a deep gate lane.
pub(crate) fn is_deep_review_gate_lane_from_registry(lane: &str, repo_root: Option<&Path>) -> bool {
    let key = normalize_subagent_lane(lane);
    snapshot(repo_root)
        .map(|s| s.deep_gate_lanes.contains(&key))
        .unwrap_or(false)
}

pub(crate) fn is_claude_reviewer_lane_from_registry(lane: &str, repo_root: Option<&Path>) -> bool {
    let key = normalize_subagent_lane(lane);
    snapshot(repo_root)
        .map(|s| s.claude_reviewer_lanes.contains(&key))
        .unwrap_or(false)
}

/// Operator/doctor probe: returns `Err` when disk registry cannot be loaded for hook lane sets.
pub fn check_review_gate_registry_snapshot(repo_root: &Path) -> Result<(), String> {
    let path = registry_json_path(Some(repo_root));
    load_snapshot_from_disk(&path).map(|_| ())
}

/// Smoke matrix for tests (uses disk registry at `repo_root` or crate-relative default).
pub(crate) fn assert_deep_review_gate_lane_matrix(repo_root: Option<&Path>) {
    assert!(is_deep_review_gate_lane_from_registry("general-purpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("generalpurpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("best-of-n-runner", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("bestofnrunner", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("explore", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("ci-investigator", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("review", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("reviewer", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("critic", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("code-review", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("General_Purpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("Best_Of_N_Runner", repo_root));
}

pub(crate) fn assert_claude_reviewer_lane_matrix(repo_root: Option<&Path>) {
    assert_deep_review_gate_lane_matrix(repo_root);
    assert!(is_claude_reviewer_lane_from_registry("review", repo_root));
    assert!(is_claude_reviewer_lane_from_registry("reviewer", repo_root));
    assert!(is_claude_reviewer_lane_from_registry("critic", repo_root));
    assert!(is_claude_reviewer_lane_from_registry("code-review", repo_root));
    assert!(!is_claude_reviewer_lane_from_registry("explore", repo_root));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn deep_review_gate_lane_matrix_disk_default() {
        assert_deep_review_gate_lane_matrix(None);
    }

    #[test]
    fn claude_reviewer_lane_matrix_disk_default() {
        assert_claude_reviewer_lane_matrix(None);
    }

    #[test]
    fn loader_picks_up_runtime_edit_without_rebuild() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-reg-loader-{suffix}"));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("configs/framework")).unwrap();
        let registry = repo.join("configs/framework/RUNTIME_REGISTRY.json");
        std::fs::write(
            &registry,
            r#"{"review_gate":{"deep_gate_lanes":["probe-lane-only"],"claude_reviewer_lanes":["probe-lane-only","review"]}}"#,
        )
        .unwrap();
        let key = repo_cache_key(&registry);
        cache().lock().unwrap().remove(&key);
        assert!(is_deep_review_gate_lane_from_registry("probe-lane-only", Some(&repo)));
        assert!(!is_deep_review_gate_lane_from_registry("general-purpose", Some(&repo)));
        let _ = std::fs::remove_dir_all(&repo);
    }
}
