//! Task / harness schema drift: capture baselines and compare on verify.
//!
//! Cursor-specific hooks snapshot logic lives in
//! `host-projection/src/hosts/host_extensions/cursor/schema_drift.rs` (L0).

use chrono::Utc;
use hex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_DRIFT_BASELINE_SCHEMA_VERSION: &str = "schema-drift-baseline-v1";
pub const SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION: &str = "schema-drift-check-response-v1";
pub const ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION: &str = "router-rs-hook-observation-v1";

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaDriftContract {
    pub schema_version: String,
    pub baseline_schema_version: String,
    pub check_response_schema_version: String,
    pub baseline_relative_path: String,
}

pub fn schema_drift_contract() -> SchemaDriftContract {
    SchemaDriftContract {
        schema_version: "schema-drift-contract-v1".to_string(),
        baseline_schema_version: SCHEMA_DRIFT_BASELINE_SCHEMA_VERSION.to_string(),
        check_response_schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
        baseline_relative_path: "artifacts/current/<task_id>/SCHEMA_DRIFT_BASELINE.json"
            .to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskArtifactsDriftSnapshot {
    pub requirements_headings_sha256: Option<String>,
    pub roadmap_headings_sha256: Option<String>,
    pub headings_match: bool,
    pub evidence_index_present: bool,
    pub evidence_index_has_artifacts_array: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractVersionsSnapshot {
    pub closeout_record: String,
    pub hook_observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaDriftBaseline {
    pub schema_version: String,
    pub recorded_at: String,
    pub task_id: String,
    /// Host-specific hooks snapshot (opaque JSON blob from L0 host extension).
    /// No L4 code inspects this struct; comparison is `Value` equality.
    pub cursor_hooks: Value,
    pub task_artifacts: TaskArtifactsDriftSnapshot,
    pub contracts: ContractVersionsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaDriftDriftItem {
    pub field: String,
    pub baseline: Value,
    pub current: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaDriftCheckResponse {
    pub schema_version: String,
    pub task_id: String,
    pub baseline_path: String,
    pub baseline_present: bool,
    pub ok: bool,
    pub drift: Vec<SchemaDriftDriftItem>,
}

// ── Utilities ──

pub fn resolve_task_id_for_schema_drift(
    _repo_root: &Path,
    task_id: Option<&str>,
) -> Result<String, String> {
    if let Some(id) = task_id.map(str::trim).filter(|s| !s.is_empty()) {
        core_state::utils::path_guard::safe_task_id_component(id)
            .ok_or_else(|| format!("schema-drift: invalid task_id {:?}", id))?;
        return Ok(id.to_string());
    }
    Err("schema-drift: provide --task-id (pointer fallback removed)".to_string())
}

pub fn baseline_path(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id)
        .join("SCHEMA_DRIFT_BASELINE.json")
}

// ── Hooks snapshot via L0 cursor extension ──

fn snapshot_cursor_hooks_json(repo_root: &Path) -> Result<Value, String> {
    let snap = host_projection::hosts::host_extensions::cursor_impl::schema_drift::snapshot_cursor_hooks(repo_root)?;
    serde_json::to_value(snap).map_err(|e| format!("serialize cursor_hooks: {e}"))
}

fn cursor_hooks_json_ok(hooks: &Value) -> bool {
    let Some(s) = hooks.as_object() else { return false };
    s.get("forbidden_still_registered")
        .and_then(Value::as_array)
        .map(|a| a.is_empty())
        .unwrap_or(false)
        && s.get("missing_required")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("hook_command_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("gate_timeout_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
        && s.get("hooks_template_parity_issues")
            .and_then(Value::as_array)
            .map(|a| a.is_empty())
            .unwrap_or(false)
}

/// Fallback hooks snapshot used when the real snapshot fails.
fn fallback_cursor_hooks_json() -> Value {
    json!({
        "registered_events": [],
        "forbidden_still_registered": ["snapshot_failed"],
        "missing_required": [],
        "matches_workspace_template": false,
        "hook_command_issues": ["snapshot_failed"],
        "gate_timeout_issues": [],
        "hooks_template_parity_issues": [],
    })
}

// ── Task artifacts snapshot ──

fn md_heading_lines(path: &Path) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| {
            let t = line.trim_start();
            t.starts_with("## ") || t.starts_with("### ")
        })
        .map(str::to_string)
        .collect()
}

fn sha256_hex_lines(lines: &[String]) -> String {
    let joined = lines.join("\n");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn snapshot_task_artifacts(repo_root: &Path, task_id: &str) -> TaskArtifactsDriftSnapshot {
    let task_dir = repo_root.join("artifacts/current").join(task_id);
    let req_path = task_dir.join("REQUIREMENTS.md");
    let road_path = task_dir.join("ROADMAP.md");
    let req_lines = md_heading_lines(&req_path);
    let road_lines = md_heading_lines(&road_path);
    let req_sha = (!req_lines.is_empty()).then(|| sha256_hex_lines(&req_lines));
    let road_sha = (!road_lines.is_empty()).then(|| sha256_hex_lines(&road_lines));
    let headings_match = req_sha.is_some() && road_sha.is_some();

    let evidence_path = task_dir.join("EVIDENCE_INDEX.json");
    let (evidence_index_present, evidence_index_has_artifacts_array) = if evidence_path.is_file() {
        match fs::read_to_string(&evidence_path) {
            Ok(raw) => {
                let has = serde_json::from_str::<Value>(&raw)
                    .ok()
                    .and_then(|doc| doc.get("artifacts").and_then(Value::as_array).cloned())
                    .is_some_and(|a| !a.is_empty());
                (true, has)
            }
            Err(_) => (true, false),
        }
    } else {
        (false, false)
    };

    TaskArtifactsDriftSnapshot {
        requirements_headings_sha256: req_sha,
        roadmap_headings_sha256: road_sha,
        headings_match,
        evidence_index_present,
        evidence_index_has_artifacts_array,
    }
}

// ── Baseline I/O ──

pub fn build_baseline(repo_root: &Path, task_id: &str) -> Result<SchemaDriftBaseline, String> {
    Ok(SchemaDriftBaseline {
        schema_version: SCHEMA_DRIFT_BASELINE_SCHEMA_VERSION.to_string(),
        recorded_at: framework_kernel::time::now_iso(),
        task_id: task_id.to_string(),
        cursor_hooks: snapshot_cursor_hooks_json(repo_root)?,
        task_artifacts: snapshot_task_artifacts(repo_root, task_id),
        contracts: ContractVersionsSnapshot {
            closeout_record: framework_runtime::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
            hook_observation: ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION.to_string(),
        },
    })
}

pub fn write_baseline(
    repo_root: &Path,
    task_id: &str,
) -> Result<(SchemaDriftBaseline, PathBuf), String> {
    let baseline = build_baseline(repo_root, task_id)?;
    let path = baseline_path(repo_root, task_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let text =
        serde_json::to_string_pretty(&baseline).map_err(|e| format!("serialize baseline: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok((baseline, path))
}

fn drift_item(
    field: &str,
    baseline: &impl Serialize,
    current: &impl Serialize,
) -> Option<SchemaDriftDriftItem> {
    let b = serde_json::to_value(baseline).ok()?;
    let c = serde_json::to_value(current).ok()?;
    if b == c {
        None
    } else {
        Some(SchemaDriftDriftItem {
            field: field.to_string(),
            baseline: b,
            current: c,
        })
    }
}

// ── Baseline check ──

pub fn check_against_baseline(repo_root: &Path, task_id: &str) -> SchemaDriftCheckResponse {
    let path = baseline_path(repo_root, task_id);
    let baseline_present = path.is_file();
    let mut drift = Vec::new();

    let current_hooks = snapshot_cursor_hooks_json(repo_root).unwrap_or_else(|_| fallback_cursor_hooks_json());
    let current_artifacts = snapshot_task_artifacts(repo_root, task_id);
    let current_contracts = ContractVersionsSnapshot {
        closeout_record: framework_runtime::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
        hook_observation: ROUTER_RS_HOOK_OBSERVATION_SCHEMA_VERSION.to_string(),
    };

    if !baseline_present {
        return SchemaDriftCheckResponse {
            schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
            task_id: task_id.to_string(),
            baseline_path: path.display().to_string(),
            baseline_present: false,
            ok: false,
            drift: vec![SchemaDriftDriftItem {
                field: "baseline_file".to_string(),
                baseline: json!(null),
                current: json!({"missing": true}),
            }],
        };
    }

    let Ok(raw) = fs::read_to_string(&path) else {
        return SchemaDriftCheckResponse {
            schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
            task_id: task_id.to_string(),
            baseline_path: path.display().to_string(),
            baseline_present: true,
            ok: false,
            drift: vec![SchemaDriftDriftItem {
                field: "baseline_read".to_string(),
                baseline: json!(null),
                current: json!({"error": "read_failed"}),
            }],
        };
    };

    let baseline: SchemaDriftBaseline = match serde_json::from_str(&raw) {
        Ok(b) => b,
        Err(e) => {
            return SchemaDriftCheckResponse {
                schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
                task_id: task_id.to_string(),
                baseline_path: path.display().to_string(),
                baseline_present: true,
                ok: false,
                drift: vec![SchemaDriftDriftItem {
                    field: "baseline_parse".to_string(),
                    baseline: json!(null),
                    current: json!({"error": e.to_string()}),
                }],
            };
        }
    };

    if baseline.task_id != task_id {
        return SchemaDriftCheckResponse {
            schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
            task_id: task_id.to_string(),
            baseline_path: path.display().to_string(),
            baseline_present: true,
            ok: false,
            drift: vec![SchemaDriftDriftItem {
                field: "task_id_mismatch".to_string(),
                baseline: json!(baseline.task_id),
                current: json!(task_id),
            }],
        };
    }

    if let Some(item) = drift_item("cursor_hooks", &baseline.cursor_hooks, &current_hooks) {
        drift.push(item);
    }
    if let Some(item) = drift_item(
        "task_artifacts",
        &baseline.task_artifacts,
        &current_artifacts,
    ) {
        drift.push(item);
    }
    if let Some(item) = drift_item("contracts", &baseline.contracts, &current_contracts) {
        drift.push(item);
    }

    let ok = drift.is_empty()
        && cursor_hooks_json_ok(&baseline.cursor_hooks)
        && cursor_hooks_json_ok(&current_hooks)
        && current_artifacts.headings_match
        && (!current_artifacts.evidence_index_present
            || current_artifacts.evidence_index_has_artifacts_array);

    SchemaDriftCheckResponse {
        schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
        task_id: task_id.to_string(),
        baseline_path: path.display().to_string(),
        baseline_present: true,
        ok,
        drift,
    }
}

// ── Tests ──

#[cfg(test)]
fn seven_event_hooks_json(command: &str) -> String {
    format!(
        r#"{{
          "hooks": {{
            "beforeSubmitPrompt": [{{"command": "{command}", "timeout": 20}}],
            "stop": [{{"command": "{command}", "timeout": 20}}],
            "sessionStart": [{{"command": "{command}", "timeout": 5}}],
            "sessionEnd": [{{"command": "{command}", "timeout": 15}}],
            "postToolUse": [{{"command": "{command}", "timeout": 20}}],
            "subagentStart": [{{"command": "{command}", "timeout": 20}}],
            "subagentStop": [{{"command": "{command}", "timeout": 20}}]
          }}
        }}"#
    )
}

#[cfg(test)]
fn write_minimal_seven_event_hooks(repo: &Path, hooks_command: &str, template_command: &str) {
    fs::create_dir_all(repo.join(".cursor")).unwrap();
    fs::write(
        repo.join(".cursor/hooks.json"),
        seven_event_hooks_json(hooks_command),
    )
    .unwrap();
    fs::create_dir_all(repo.join("configs/framework")).unwrap();
    fs::write(
        repo.join("configs/framework/cursor-hooks.workspace-template.json"),
        seven_event_hooks_json(template_command),
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_repo(name: &str) -> PathBuf {
        let repo = std::env::temp_dir().join(format!(
            "router-rs-schema-drift-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        repo
    }

    fn seed_task_artifacts(repo: &Path, task: &str) {
        fs::create_dir_all(repo.join("artifacts/current").join(task)).unwrap();
        fs::write(
            repo.join("artifacts/current")
                .join(task)
                .join("REQUIREMENTS.md"),
            "## A\n### B\n",
        )
        .unwrap();
        fs::write(
            repo.join("artifacts/current").join(task).join("ROADMAP.md"),
            "## A\n### B\n",
        )
        .unwrap();
        fs::write(
            repo.join("artifacts/current")
                .join(task)
                .join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"path":"x"}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn baseline_roundtrip_and_check_ok() {
        let repo = temp_repo("roundtrip");
        write_minimal_seven_event_hooks(
            &repo,
            "x/cursor-router-rs-hook.sh",
            "x/cursor-router-rs-hook.sh",
        );
        let task = "t-schema";
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-schema"}"#,
        )
        .unwrap();
        seed_task_artifacts(&repo, task);

        let (_, path) = write_baseline(&repo, task).unwrap();
        assert!(path.is_file());
        let resp = check_against_baseline(&repo, task);
        assert!(resp.ok, "drift={:?}", resp.drift);
    }

    #[test]
    fn check_fails_on_hooks_template_command_mismatch() {
        let repo = temp_repo("cmd-mismatch");
        write_minimal_seven_event_hooks(
            &repo,
            "x/cursor-router-rs-hook.sh",
            "y/other-cursor-router-rs-hook.sh",
        );
        let task = "t-mismatch";
        seed_task_artifacts(&repo, task);
        write_baseline(&repo, task).unwrap();
        let snap = host_projection::hosts::host_extensions::cursor_impl::schema_drift::snapshot_cursor_hooks(&repo).unwrap();
        assert!(
            !snap.hooks_template_parity_issues.is_empty(),
            "parity issues={:?}",
            snap.hooks_template_parity_issues
        );
        assert!(!snap.matches_workspace_template);
        let resp = check_against_baseline(&repo, task);
        assert!(!resp.ok);
    }

    #[test]
    fn check_fails_on_baseline_task_id_mismatch() {
        let repo = temp_repo("task-mismatch");
        write_minimal_seven_event_hooks(
            &repo,
            "x/cursor-router-rs-hook.sh",
            "x/cursor-router-rs-hook.sh",
        );
        let task_a = "task-a";
        let task_b = "task-b";
        seed_task_artifacts(&repo, task_a);
        seed_task_artifacts(&repo, task_b);
        write_baseline(&repo, task_a).unwrap();
        let baseline_b_path = baseline_path(&repo, task_b);
        let baseline_a_path = baseline_path(&repo, task_a);
        fs::copy(&baseline_a_path, &baseline_b_path).unwrap();
        let resp = check_against_baseline(&repo, task_b);
        assert!(!resp.ok);
        assert!(
            resp.drift.iter().any(|d| d.field == "task_id_mismatch"),
            "drift={:?}",
            resp.drift
        );
    }

    #[test]
    fn resolve_task_id_requires_explicit_task_id() {
        let repo = temp_repo("no-fallback");
        fs::create_dir_all(repo.join("artifacts/current")).unwrap();
        fs::write(
            repo.join("artifacts/current/focus_task.json"),
            r#"{"task_id":"focus-only"}"#,
        )
        .unwrap();
        let err = resolve_task_id_for_schema_drift(&repo, None).unwrap_err();
        assert!(err.contains("provide --task-id"), "unexpected err: {err}");
    }
}
