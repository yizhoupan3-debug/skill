//! Disk-primary `RUNTIME_REGISTRY.json` review_gate subset for hook policy (B0).

use crate::lane_normalize::normalize_subagent_lane;
use serde_json::Value;
use core_errors::FrameworkError;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub(crate) const RUNTIME_REGISTRY_PATH: &str = "configs/framework/RUNTIME_REGISTRY.json";

const DEFAULT_SPAWN_FIRST_NUDGE: &str = "配对审稿：首轮工具前先 spawn 只读 reviewer（general-purpose/best-of-n-runner，fork_context=false）；主线程做调研须另开独立 reviewer，explore 不计入证据。细则 skills/code-review-deep/SKILL.md";
const DEFAULT_SUBAGENT_MODEL_INHERIT_NUDGE: &str = "子代理模型：继承主会话；Task 省略 model；禁止默认 claude/sonnet，除非主会话已选 Anthropic。地区不可用见宿主官方文档";

#[derive(Clone)]
struct ReviewGateSnapshot {
    reviewer_lanes: HashSet<String>,
    spawn_first_enabled: bool,
    spawn_first_nudge: String,
    spawn_first_nudge_template: Option<String>,
    spawn_first_nudge_host_labels: HashMap<String, String>,
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

/// Set the repo root for the current hook dispatch (mirrors router-rs `runtime_registry`).
pub fn set_hook_registry_repo_root(repo_root: &Path) {
    HOOK_REGISTRY_REPO_ROOT.with(|c| *c.borrow_mut() = Some(repo_root.to_path_buf()));
}

/// Clear after hook dispatch completes.
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

/// Resolve `RUNTIME_REGISTRY.json` path (honors [`HookRegistryRepoGuard`] when `repo_root` is None).
pub fn runtime_registry_json_path(repo_root: Option<&Path>) -> PathBuf {
    registry_json_path(repo_root)
}

fn repo_cache_key(registry_path: &Path) -> PathBuf {
    registry_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| registry_path.to_path_buf())
}

fn lane_set(root: &Value, field: &str) -> Result<HashSet<String>, FrameworkError> {
    let lanes = root
        .get("review_gate")
        .and_then(|v| v.get(field))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            FrameworkError::validation(format!(
                "RUNTIME_REGISTRY.review_gate.{field} must be a non-empty array"
            ))
        })?;
    let mut out = HashSet::new();
    for item in lanes {
        let s = item.as_str().ok_or_else(|| {
            FrameworkError::validation(format!("review_gate.{field} entry must be string"))
        })?;
        out.insert(normalize_subagent_lane(s));
    }
    if out.is_empty() {
        return Err(FrameworkError::validation(format!(
            "RUNTIME_REGISTRY.review_gate.{field} must not be empty"
        )));
    }
    Ok(out)
}

fn reviewer_lanes_from_root(root: &Value) -> Result<HashSet<String>, FrameworkError> {
    lane_set(root, "reviewer_lanes")
}

fn string_map_by_host(review_gate: &Value, field: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(by_host) = review_gate.get(field).and_then(Value::as_object) {
        for (host_id, line) in by_host {
            if let Some(s) = line.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                out.insert(host_id.clone(), s.to_string());
            }
        }
    }
    out
}

fn bool_map_by_host(review_gate: &Value, field: &str) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    if let Some(by_host) = review_gate.get(field).and_then(Value::as_object) {
        for (host_id, flag) in by_host {
            if let Some(v) = flag.as_bool() {
                out.insert(host_id.clone(), v);
            }
        }
    }
    out
}

fn load_snapshot_from_disk(registry_path: &Path) -> Result<ReviewGateSnapshot, FrameworkError> {
    let raw = fs::read_to_string(registry_path)
        .map_err(|e| FrameworkError::validation(format!("read registry: {e}")))?;
    let root: Value = serde_json::from_str(&raw)
        .map_err(|e| FrameworkError::validation(format!("parse registry: {e}")))?;
    let review_gate = root.get("review_gate").ok_or_else(|| {
        FrameworkError::validation("RUNTIME_REGISTRY.review_gate missing".to_string())
    })?;
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
    let subagent_model_inherit_nudge = review_gate
        .get("subagent_model_inherit_nudge")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SUBAGENT_MODEL_INHERIT_NUDGE)
        .to_string();
    let spawn_first_nudge_template = review_gate
        .get("spawn_first_nudge_template")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(ReviewGateSnapshot {
        reviewer_lanes: reviewer_lanes_from_root(&root)?,
        spawn_first_enabled,
        spawn_first_nudge,
        spawn_first_nudge_template,
        spawn_first_nudge_host_labels: string_map_by_host(
            review_gate,
            "spawn_first_nudge_host_labels",
        ),
        spawn_first_nudge_by_host: string_map_by_host(review_gate, "spawn_first_nudge_by_host"),
        subagent_model_inherit_nudge,
        subagent_model_inherit_nudge_by_host: string_map_by_host(
            review_gate,
            "subagent_model_inherit_nudge_by_host",
        ),
        spawn_first_includes_model_inherit_by_host: bool_map_by_host(
            review_gate,
            "spawn_first_includes_model_inherit_by_host",
        ),
    })
}

fn snapshot(repo_root: Option<&Path>) -> Result<ReviewGateSnapshot, FrameworkError> {
    let path = registry_json_path(repo_root);
    let key = repo_cache_key(&path);
    let hit = {
        let guard = cache()
            .lock()
            .map_err(|e| FrameworkError::lock(format!("registry cache lock poisoned: {e}")))?;
        guard.get(&key).cloned()
    };
    if let Some(snapshot) = hit {
        return Ok(snapshot);
    }
    let loaded = load_snapshot_from_disk(&path)?;
    let mut guard = cache()
        .lock()
        .map_err(|e| FrameworkError::lock(format!("registry cache lock poisoned: {e}")))?;
    if guard.len() >= 64 {
        guard.clear();
    }
    guard.insert(key, loaded.clone());
    Ok(loaded)
}

/// Fail-closed: registry unreadable → lane is **not** treated as a reviewer lane.
pub fn is_reviewer_lane_from_registry(lane: &str, repo_root: Option<&Path>) -> bool {
    let key = normalize_subagent_lane(lane);
    snapshot(repo_root)
        .map(|s| s.reviewer_lanes.contains(&key))
        .unwrap_or(false)
}

/// Spawn-first pairing reviewer nudge enabled (registry default false for fail-closed).
pub fn review_spawn_first_enabled(repo_root: Option<&Path>) -> bool {
    snapshot(repo_root)
        .map(|s| s.spawn_first_enabled)
        .unwrap_or(false)
}

/// Sorted lane spellings from `review_gate.reviewer_lanes` (MCP prompts / stop nudges).
pub fn reviewer_lanes_sorted(repo_root: Option<&Path>) -> Vec<String> {
    snapshot(repo_root)
        .map(|s| {
            let mut lanes: Vec<String> = s.reviewer_lanes.iter().cloned().collect();
            lanes.sort();
            lanes
        })
        .unwrap_or_default()
}

/// MCP prompt bullet lines for registry `reviewer_lanes` (OpenCode shared).
pub fn reviewer_lanes_prompt_lines(repo_root: Option<&Path>) -> String {
    let lanes = reviewer_lanes_sorted(repo_root);
    if lanes.is_empty() {
        "- (registry reviewer_lanes unavailable)\n".to_string()
    } else {
        lanes
            .iter()
            .map(|lane| format!("- {lane}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}

fn spawn_first_nudge_from_template(template: &str, host_label: &str) -> String {
    template.replace("{host_label}", host_label)
}

fn default_host_label(host_id: &str) -> String {
    if host_id.is_empty() {
        return String::new();
    }
    let mut chars = host_id.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().to_string();
    out.push_str(chars.as_str());
    out
}

/// One-line spawn-first nudge for hook `additional_context` (registry-backed).
///
/// Resolution order: per-host override → `spawn_first_nudge_template` + host label → global `spawn_first_nudge`.
pub fn review_spawn_first_nudge_line(repo_root: Option<&Path>, host_id: &str) -> String {
    snapshot(repo_root)
        .ok()
        .map(|s| {
            if let Some(line) = s.spawn_first_nudge_by_host.get(host_id) {
                return line.clone();
            }
            if let Some(template) = s.spawn_first_nudge_template.as_deref() {
                let label = s
                    .spawn_first_nudge_host_labels
                    .get(host_id)
                    .cloned()
                    .unwrap_or_else(|| default_host_label(host_id));
                if !label.is_empty() {
                    return spawn_first_nudge_from_template(template, &label);
                }
            }
            s.spawn_first_nudge.clone()
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
pub fn spawn_first_includes_model_inherit_for_host(
    repo_root: Option<&Path>,
    host_id: &str,
) -> bool {
    snapshot(repo_root)
        .ok()
        .and_then(|s| {
            s.spawn_first_includes_model_inherit_by_host
                .get(host_id)
                .copied()
        })
        .unwrap_or(false)
}

/// Operator/doctor probe: returns `Err` when disk registry lane snapshot cannot load.
pub fn check_review_gate_registry_snapshot(repo_root: &Path) -> Result<(), FrameworkError> {
    let path = registry_json_path(Some(repo_root));
    load_snapshot_from_disk(&path).map(|_| ())
}

fn load_registry_root(repo_root: Option<&Path>) -> Result<Value, FrameworkError> {
    let path = registry_json_path(repo_root);
    let raw = fs::read_to_string(&path)
        .map_err(|e| FrameworkError::validation(format!("read registry: {e}")))?;
    serde_json::from_str(&raw)
        .map_err(|e| FrameworkError::validation(format!("parse registry: {e}")))
}

/// `lifecycle_profiles.<name>.disable_spawn_first_nudge` (default false if missing).
pub fn lifecycle_profile_disables_spawn_first_nudge(
    repo_root: Option<&Path>,
    profile: &str,
) -> Result<bool, FrameworkError> {
    let root = load_registry_root(repo_root)?;
    Ok(root
        .get("lifecycle_profiles")
        .and_then(|p| p.get(profile))
        .and_then(|p| p.get("disable_spawn_first_nudge"))
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

/// Test helper shared with router-rs disk registry parity checks.
#[cfg(any(test, feature = "test-sync"))]
pub fn assert_reviewer_lane_matrix(repo_root: Option<&Path>) {
    assert!(is_reviewer_lane_from_registry("general-purpose", repo_root));
    assert!(is_reviewer_lane_from_registry("generalpurpose", repo_root));
    assert!(is_reviewer_lane_from_registry(
        "best-of-n-runner",
        repo_root
    ));
    assert!(is_reviewer_lane_from_registry("bestofnrunner", repo_root));
    assert!(is_reviewer_lane_from_registry("deep-reviewer", repo_root));
    assert!(is_reviewer_lane_from_registry("deepreviewer", repo_root));
    assert!(is_reviewer_lane_from_registry("review", repo_root));
    assert!(is_reviewer_lane_from_registry("reviewer", repo_root));
    assert!(is_reviewer_lane_from_registry("critic", repo_root));
    assert!(is_reviewer_lane_from_registry("code-review", repo_root));
    assert!(!is_reviewer_lane_from_registry("explore", repo_root));
    assert!(!is_reviewer_lane_from_registry(
        "ci-investigator",
        repo_root
    ));
    assert!(is_reviewer_lane_from_registry("General_Purpose", repo_root));
    assert!(is_reviewer_lane_from_registry(
        "Best_Of_N_Runner",
        repo_root
    ));
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_lane_matrix_disk_default() {
        assert_reviewer_lane_matrix(None);
    }

    #[test]
    fn registry_reviewer_lanes_snapshot() {
        // Snapshot sorted reviewer lanes for regression detection.
        insta::assert_debug_snapshot!(reviewer_lanes_sorted(None));
    }

    #[test]
    fn spawn_first_registry_fields_disk_default() {
        assert!(review_spawn_first_enabled(None));
        let line = review_spawn_first_nudge_line(None, "cursor");
        assert!(line.contains("fork_context"));
        assert!(line.contains("code-review-deep") || line.contains("配对审稿"));
        assert!(
            spawn_first_includes_model_inherit_for_host(None, "cursor"),
            "cursor spawn_first_includes_model_inherit_by_host must be true"
        );
        let codex_line = review_spawn_first_nudge_line(None, "codex");
        assert!(
            codex_line.contains("Codex"),
            "codex line from template: {codex_line}"
        );
        let claude_line = review_spawn_first_nudge_line(None, "claude");
        assert!(
            claude_line.contains("Claude"),
            "claude line from template: {claude_line}"
        );
    }

    #[test]
    fn subagent_model_inherit_registry_fields_disk_default() {
        let line = review_subagent_model_inherit_nudge_line(None, "cursor");
        assert!(
            line.contains("子代理模型（Cursor）") || line.contains("继承主会话"),
            "cursor by_host line: {line}"
        );
        assert!(line.contains("sonnet") || line.contains("claude"));
    }

    #[test]
    fn reviewer_lanes_prompt_lines_disk_default() {
        let lines = reviewer_lanes_prompt_lines(None);
        assert!(lines.contains("- general-purpose") || lines.contains("- review"));
        assert!(lines.ends_with('\n'));
    }

    #[test]
    fn loader_picks_up_runtime_edit_without_rebuild() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("core-policy-reg-loader-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("configs/framework")).unwrap();
        let registry = repo.join(RUNTIME_REGISTRY_PATH);
        fs::write(
            &registry,
            r#"{"review_gate":{"reviewer_lanes":["probe-lane-only","review"]}}"#,
        )
        .unwrap();
        assert!(is_reviewer_lane_from_registry(
            "probe-lane-only",
            Some(&repo)
        ));
        assert!(!is_reviewer_lane_from_registry(
            "general-purpose",
            Some(&repo)
        ));
        let _ = fs::remove_dir_all(&repo);
    }
}
