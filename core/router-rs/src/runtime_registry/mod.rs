//! Disk-primary [`RUNTIME_REGISTRY.json`](../../../configs/framework/RUNTIME_REGISTRY.json) loader.
//! Unified entry for hook hot paths, host targets, and host integration (ADR-005).

use crate::lane_normalize::normalize_subagent_lane;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub(crate) const RUNTIME_REGISTRY_SCHEMA_VERSION: &str = "framework-runtime-registry-v1";
pub(crate) const RUNTIME_REGISTRY_PATH: &str = "configs/framework/RUNTIME_REGISTRY.json";
pub(crate) const HOST_ADAPTER_CONTRACT_PATH: &str = "docs/host_adapter_contract.md";

const DEFAULT_SPAWN_FIRST_NUDGE: &str = "配对审稿：首轮工具前先 spawn 只读 reviewer（general-purpose/best-of-n-runner，fork_context=false）；主线程做调研须另开独立 reviewer，explore 不计入证据。细则 skills/code-review-deep/SKILL.md";
const DEFAULT_SUBAGENT_MODEL_INHERIT_NUDGE: &str =
    "子代理模型：继承主会话；Task 省略 model；禁止默认 claude/sonnet，除非主会话已选 Anthropic。地区不可用见 cursor.com/docs/account/regions";

// ---------------------------------------------------------------------------
// Typed registry subset (host integration)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeRegistry {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    #[serde(default)]
    pub(crate) workspace_bootstrap_defaults: RuntimeWorkspaceBootstrapDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct RuntimeWorkspaceBootstrapDefaults {
    #[serde(default)]
    pub(crate) skills: RuntimeSkillsDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct RuntimeSkillsDefaults {
    #[serde(default)]
    pub(crate) source_rel: Option<String>,
}

pub(crate) fn runtime_registry_path(repo_root: &Path) -> Result<PathBuf, String> {
    let repo_candidate = repo_root.join(RUNTIME_REGISTRY_PATH);
    if repo_candidate.is_file() {
        return Ok(repo_candidate);
    }
    Err(format!(
        "Runtime registry not found at active workspace root: {}. Expected {}. Fix by opening the framework repo root as the active workspace or passing --framework-root <framework-repo-root>.",
        repo_root.to_string_lossy(),
        repo_candidate.to_string_lossy()
    ))
}

pub(crate) fn load_runtime_registry_json(framework_root: &Path) -> Result<Value, String> {
    let path = framework_root.join(RUNTIME_REGISTRY_PATH);
    if !path.is_file() {
        return Err(format!(
            "runtime registry not found under framework root {} (expected {})",
            framework_root.display(),
            path.display()
        ));
    }
    let payload = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_str(&payload).map_err(|e| {
        format!(
            "invalid JSON in {}: {e}; see {HOST_ADAPTER_CONTRACT_PATH}",
            path.display()
        )
    })?;
    let sv = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "RUNTIME_REGISTRY.json missing schema_version at {}",
                path.display()
            )
        })?;
    if sv != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported RUNTIME_REGISTRY schema_version {:?} at {}",
            sv,
            path.display()
        ));
    }
    Ok(parsed)
}

pub(crate) fn load_runtime_registry_payload(repo_root: &Path) -> Result<Value, String> {
    match runtime_registry_path(repo_root) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    load_runtime_registry_json(repo_root)
}

pub(crate) fn load_runtime_registry_payload_if_repo_local(
    repo_root: &Path,
) -> Result<Option<Value>, String> {
    let path = repo_root.join(RUNTIME_REGISTRY_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let payload = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let parsed = serde_json::from_str::<Value>(&payload).map_err(|err| err.to_string())?;
    let schema_version = parsed
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "Runtime registry missing schema_version at {}",
                path.to_string_lossy()
            )
        })?;
    if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported runtime registry schema_version {:?} at {}",
            schema_version,
            path.to_string_lossy()
        ));
    }
    Ok(Some(parsed))
}

pub(crate) fn load_runtime_registry(repo_root: &Path) -> Result<RuntimeRegistry, String> {
    let payload = load_runtime_registry_payload(repo_root)?;
    serde_json::from_value::<RuntimeRegistry>(payload).map_err(|err| err.to_string())
}

// ---------------------------------------------------------------------------
// Review gate snapshot (hook hot paths)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ReviewGateSnapshot {
    deep_gate_lanes: HashSet<String>,
    claude_reviewer_lanes: HashSet<String>,
    spawn_first_enabled: bool,
    spawn_first_nudge: String,
    spawn_first_nudge_by_host: HashMap<String, String>,
    subagent_model_inherit_nudge: String,
    subagent_model_inherit_nudge_by_host: HashMap<String, String>,
    spawn_first_includes_model_inherit_by_host: HashMap<String, bool>,
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
        return root.join(RUNTIME_REGISTRY_PATH);
    }
    if let Some(root) = HOOK_REGISTRY_REPO_ROOT.with(|c| c.borrow().clone()) {
        return root.join(RUNTIME_REGISTRY_PATH);
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
        fs::read_to_string(registry_path).map_err(|e| format!("read registry: {e}"))?;
    let root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse registry: {e}"))?;
    let review_gate = root
        .get("review_gate")
        .ok_or_else(|| "RUNTIME_REGISTRY.review_gate missing".to_string())?;
    let spawn_first_enabled = review_gate
        .get("spawn_first_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let spawn_first_nudge = review_gate
        .get("spawn_first_nudge")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SPAWN_FIRST_NUDGE)
        .to_string();
    let mut spawn_first_nudge_by_host = HashMap::new();
    if let Some(by_host) = review_gate.get("spawn_first_nudge_by_host").and_then(Value::as_object)
    {
        for (host_id, line) in by_host {
            if let Some(s) = line.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                spawn_first_nudge_by_host.insert(host_id.clone(), s.to_string());
            }
        }
    }
    let subagent_model_inherit_nudge = review_gate
        .get("subagent_model_inherit_nudge")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SUBAGENT_MODEL_INHERIT_NUDGE)
        .to_string();
    let mut subagent_model_inherit_nudge_by_host = HashMap::new();
    if let Some(by_host) = review_gate
        .get("subagent_model_inherit_nudge_by_host")
        .and_then(Value::as_object)
    {
        for (host_id, line) in by_host {
            if let Some(s) = line.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                subagent_model_inherit_nudge_by_host.insert(host_id.clone(), s.to_string());
            }
        }
    }
    let mut spawn_first_includes_model_inherit_by_host = HashMap::new();
    if let Some(by_host) = review_gate
        .get("spawn_first_includes_model_inherit_by_host")
        .and_then(Value::as_object)
    {
        for (host_id, flag) in by_host {
            if let Some(v) = flag.as_bool() {
                spawn_first_includes_model_inherit_by_host.insert(host_id.clone(), v);
            }
        }
    }
    Ok(ReviewGateSnapshot {
        deep_gate_lanes: lane_set(&root, "deep_gate_lanes")?,
        claude_reviewer_lanes: lane_set(&root, "claude_reviewer_lanes")?,
        spawn_first_enabled,
        spawn_first_nudge,
        spawn_first_nudge_by_host,
        subagent_model_inherit_nudge,
        subagent_model_inherit_nudge_by_host,
        spawn_first_includes_model_inherit_by_host,
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

/// Sorted lane spellings from `review_gate.claude_reviewer_lanes` for MCP prompts/docs.
pub(crate) fn claude_reviewer_lanes_sorted(repo_root: Option<&Path>) -> Vec<String> {
    snapshot(repo_root)
        .map(|s| {
            let mut lanes: Vec<String> = s.claude_reviewer_lanes.iter().cloned().collect();
            lanes.sort();
            lanes
        })
        .unwrap_or_default()
}

/// Operator/doctor probe: returns `Err` when disk registry cannot be loaded for hook lane sets.
pub fn check_review_gate_registry_snapshot(repo_root: &Path) -> Result<(), String> {
    let path = registry_json_path(Some(repo_root));
    load_snapshot_from_disk(&path).map(|_| ())
}

/// Spawn-first pairing reviewer nudge enabled (registry default true).
pub fn review_spawn_first_enabled(repo_root: Option<&Path>) -> bool {
    snapshot(repo_root)
        .map(|s| s.spawn_first_enabled)
        .unwrap_or(true)
}

/// One-line spawn-first nudge for hook `additional_context` (registry-backed).
pub fn review_spawn_first_nudge_line(repo_root: Option<&Path>, host_id: &str) -> String {
    snapshot(repo_root)
        .ok()
        .and_then(|s| {
            s.spawn_first_nudge_by_host
                .get(host_id)
                .cloned()
                .or(Some(s.spawn_first_nudge.clone()))
        })
        .unwrap_or_else(|| DEFAULT_SPAWN_FIRST_NUDGE.to_string())
}

/// One-line subagent model inherit nudge for hook `additional_context` (registry-backed).
pub fn review_subagent_model_inherit_nudge_line(repo_root: Option<&Path>, host_id: &str) -> String {
    snapshot(repo_root)
        .ok()
        .and_then(|s| {
            s.subagent_model_inherit_nudge_by_host
                .get(host_id)
                .cloned()
                .or(Some(s.subagent_model_inherit_nudge.clone()))
        })
        .unwrap_or_else(|| DEFAULT_SUBAGENT_MODEL_INHERIT_NUDGE.to_string())
}

/// Registry-backed: spawn-first line for this host already carries model-inherit guidance.
pub fn spawn_first_includes_model_inherit_for_host(repo_root: Option<&Path>, host_id: &str) -> bool {
    snapshot(repo_root)
        .ok()
        .and_then(|s| {
            s.spawn_first_includes_model_inherit_by_host
                .get(host_id)
                .copied()
        })
        .unwrap_or(false)
}

fn load_registry_root(repo_root: Option<&Path>) -> Result<Value, String> {
    let path = registry_json_path(repo_root);
    let raw = fs::read_to_string(&path).map_err(|e| format!("read registry: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse registry: {e}"))
}

/// `lifecycle_profiles.<name>.disable_review_gate_hard_block` (default false if missing).
pub fn lifecycle_profile_disables_review_gate_hard_block(
    repo_root: Option<&Path>,
    profile: &str,
) -> Result<bool, String> {
    let root = load_registry_root(repo_root)?;
    Ok(root
        .get("lifecycle_profiles")
        .and_then(|p| p.get(profile))
        .and_then(|p| p.get("disable_review_gate_hard_block"))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

/// `lifecycle_profiles.<name>.disable_spawn_first_nudge` (default false if missing).
pub fn lifecycle_profile_disables_spawn_first_nudge(
    repo_root: Option<&Path>,
    profile: &str,
) -> Result<bool, String> {
    let root = load_registry_root(repo_root)?;
    Ok(root
        .get("lifecycle_profiles")
        .and_then(|p| p.get(profile))
        .and_then(|p| p.get("disable_spawn_first_nudge"))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

#[cfg(test)]
pub(crate) fn assert_spawn_first_registry_fields(repo_root: Option<&Path>) {
    assert!(review_spawn_first_enabled(repo_root));
    let line = review_spawn_first_nudge_line(repo_root, "cursor");
    assert!(line.contains("fork_context"));
    assert!(line.contains("code-review-deep") || line.contains("配对审稿"));
    assert!(
        spawn_first_includes_model_inherit_for_host(repo_root, "cursor"),
        "cursor spawn_first_includes_model_inherit_by_host must be true"
    );
}

#[cfg(test)]
pub(crate) fn assert_subagent_model_inherit_registry_fields(repo_root: Option<&Path>) {
    let line = review_subagent_model_inherit_nudge_line(repo_root, "cursor");
    assert!(
        line.contains("子代理模型（Cursor）") || line.contains("继承主会话"),
        "cursor by_host line: {line}"
    );
    assert!(line.contains("sonnet") || line.contains("claude"));
}

/// Smoke matrix for tests (uses disk registry at `repo_root` or crate-relative default).
#[cfg(test)]
pub(crate) fn assert_deep_review_gate_lane_matrix(repo_root: Option<&Path>) {
    assert!(is_deep_review_gate_lane_from_registry("general-purpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("generalpurpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("best-of-n-runner", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("bestofnrunner", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("deep-reviewer", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("deepreviewer", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("explore", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("ci-investigator", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("review", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("reviewer", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("critic", repo_root));
    assert!(!is_deep_review_gate_lane_from_registry("code-review", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("General_Purpose", repo_root));
    assert!(is_deep_review_gate_lane_from_registry("Best_Of_N_Runner", repo_root));
}

#[cfg(test)]
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

    #[test]
    fn deep_review_gate_lane_matrix_disk_default() {
        assert_deep_review_gate_lane_matrix(None);
    }

    #[test]
    fn claude_reviewer_lane_matrix_disk_default() {
        assert_claude_reviewer_lane_matrix(None);
    }

    #[test]
    fn spawn_first_registry_fields_disk_default() {
        assert_spawn_first_registry_fields(None);
    }

    #[test]
    fn subagent_model_inherit_registry_fields_disk_default() {
        assert_subagent_model_inherit_registry_fields(None);
    }

    #[test]
    fn loader_picks_up_runtime_edit_without_rebuild() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-reg-loader-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("configs/framework")).unwrap();
        let registry = repo.join(RUNTIME_REGISTRY_PATH);
        fs::write(
            &registry,
            r#"{"review_gate":{"deep_gate_lanes":["probe-lane-only"],"claude_reviewer_lanes":["probe-lane-only","review"]}}"#,
        )
        .unwrap();
        let key = repo_cache_key(&registry);
        cache().lock().unwrap().remove(&key);
        assert!(is_deep_review_gate_lane_from_registry("probe-lane-only", Some(&repo)));
        assert!(!is_deep_review_gate_lane_from_registry("general-purpose", Some(&repo)));
        let _ = fs::remove_dir_all(&repo);
    }
}
