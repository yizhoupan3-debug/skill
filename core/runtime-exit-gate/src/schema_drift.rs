//! Task / harness schema drift: capture baselines and compare on verify.
//!
//! All hooks snapshot logic is shared via
//! `host-projection/src/hosts/host_extensions/schema_drift.rs` (L0).
//! No host-specific code lives in this crate — paths, events, and timeouts are
//! derived from the `HostProvider` registry.


use hex;
use host_projection::hosts::host_extensions::schema_drift as shared_schema_drift;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_DRIFT_BASELINE_SCHEMA_VERSION: &str = "schema-drift-baseline-v1";
pub const SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION: &str = "schema-drift-check-response-v1";
pub const SCHEMA_DRIFT_CONTRACT_SCHEMA_VERSION: &str = "schema-drift-contract-v1";
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
        schema_version: SCHEMA_DRIFT_CONTRACT_SCHEMA_VERSION.to_string(),
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
    pub host_hooks: Value,
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
        core_state_utils::path_guard::safe_task_id_component(id)
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

// ── Hooks snapshot via L0 shared schema_drift module ──

/// Fallback hooks snapshot used when the real snapshot fails.
/// The `["snapshot_failed"]` array ensures `host_hooks_json_ok` returns false.
fn fallback_host_hooks_json() -> Value {
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

/// Clean empty snapshot for hosts without hooks_manifest_path (passes validation).
fn noop_host_hooks_json() -> Value {
    json!({
        "registered_events": [],
        "forbidden_still_registered": [],
        "missing_required": [],
        "matches_workspace_template": false,
        "hook_command_issues": [],
        "gate_timeout_issues": [],
        "hooks_template_parity_issues": [],
    })
}

/// Snapshot all hosts' hooks using the shared L0 function.
/// Iterates ALL_HOST_IDS from the registry — no host-specific hardcoding.
fn snapshot_all_host_hooks(repo_root: &Path) -> Value {
    let mut map = serde_json::Map::new();
    for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
        let cmd_fragment = format!("{host_id}-router-rs-hook.sh");
        let provider = host_projection::hosts::host_provider_for_id(host_id);
        let hooks_path = provider.and_then(|p| p.hooks_manifest_path());
        let template_path = format!("configs/framework/{}-hooks.workspace-template.json", host_id);
        match hooks_path {
            Some(hooks_rel) => {
                let snap = shared_schema_drift::snapshot_host_hooks_json(
                    repo_root,
                    Path::new(hooks_rel),
                    Path::new(&template_path),
                    host_projection::hosts::host_extensions::host_registered_hook_events(host_id),
                    &[],
                    &cmd_fragment,
                )
                .unwrap_or_else(|_| fallback_host_hooks_json());
                map.insert(host_id.to_string(), snap);
            }
            None => {
                map.insert(host_id.to_string(), noop_host_hooks_json());
            }
        }
    }
    Value::Object(map)
}

/// Check whether a host hooks snapshot JSON blob is valid.
/// Delegates to the shared L0 function.
fn host_hooks_json_ok(hooks: &Value) -> bool {
    let Some(map) = hooks.as_object() else { return false };
    map.values().all(|v| shared_schema_drift::host_hooks_json_ok(v))
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
        host_hooks: snapshot_all_host_hooks(repo_root),
        task_artifacts: snapshot_task_artifacts(repo_root, task_id),
        contracts: ContractVersionsSnapshot {
            closeout_record: fr_contracts::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
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
    let value =
        serde_json::to_value(&baseline).map_err(|e| format!("serialize baseline: {e}"))?;
    core_state_utils::atomic_write::write_atomic_json(&path, &value)?;
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

    let current_hooks = snapshot_all_host_hooks(repo_root);
    let current_artifacts = snapshot_task_artifacts(repo_root, task_id);
    let current_contracts = ContractVersionsSnapshot {
        closeout_record: fr_contracts::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
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

    // Acquire shared lock on the baseline file before reading, to coordinate
    // with concurrent writers (e.g. write_baseline using write_atomic_json).
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => {
            return SchemaDriftCheckResponse {
                schema_version: SCHEMA_DRIFT_CHECK_RESPONSE_SCHEMA_VERSION.to_string(),
                task_id: task_id.to_string(),
                baseline_path: path.display().to_string(),
                baseline_present: true,
                ok: false,
                drift: vec![SchemaDriftDriftItem {
                    field: "baseline_read".to_string(),
                    baseline: json!(null),
                    current: json!({"error": "open_failed"}),
                }],
            };
        }
    };
    let _lock_guard = file.lock_shared().ok();
    let raw = {
        use std::io::Read;
        let mut s = String::new();
        if (&file).take(10_000_000).read_to_string(&mut s).is_err() {
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
        }
        s
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

    if let Some(item) = drift_item("host_hooks", &baseline.host_hooks, &current_hooks) {
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
        && host_hooks_json_ok(&baseline.host_hooks)
        && host_hooks_json_ok(&current_hooks)
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

    fn write_host_hooks_fixture(repo: &Path, host_id: &str) {
        let provider = host_projection::hosts::host_provider_for_id(host_id)
            .expect("write_host_hooks_fixture: unknown host");
        let hooks_rel = provider
            .hooks_manifest_path()
            .expect("write_host_hooks_fixture: host has no hooks_manifest_path");
        let events = provider.registered_hook_events();

        let mut pairs = String::new();
        for (i, event) in events.iter().enumerate() {
            let timeout = match *event {
                "sessionStart"
                | "SessionStart"
                | "session.idle"
                | "session.created"
                | "session.deleted" => 5,
                "sessionEnd" => 15,
                _ => 20,
            };
            if i > 0 {
                pairs.push_str(",\n");
            }
            pairs.push_str(&format!(
                r#"      "{event}": [{{"command": "x/{host_id}-router-rs-hook.sh", "timeout": {timeout}}}]"#
            ));
        }

        let hooks_json = format!(
            r#"{{
  "hooks": {{
{pairs}
  }}
}}"#
        );

        let hooks_abs = repo.join(hooks_rel);
        if let Some(parent) = hooks_abs.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&hooks_abs, hooks_json.as_bytes()).unwrap();

        let template_rel = format!("configs/framework/{host_id}-hooks.workspace-template.json");
        let template_abs = repo.join(&template_rel);
        if let Some(parent) = template_abs.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&template_abs, hooks_json.as_bytes()).unwrap();
    }

    fn write_all_hosts_hooks_fixtures(repo: &Path) {
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            write_host_hooks_fixture(repo, host_id);
        }
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
        write_all_hosts_hooks_fixtures(&repo);
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
        write_all_hosts_hooks_fixtures(&repo);
        // Pick the first host with hooks_manifest_path to test template parity
        let host_id = *framework_kernel::runtime_registry::ALL_HOST_IDS
            .iter()
            .find(|id| host_projection::hosts::host_provider_for_id(id)
                .and_then(|p| p.hooks_manifest_path())
                .is_some())
            .expect("no host with hooks_manifest_path");
        let provider = host_projection::hosts::host_provider_for_id(host_id).unwrap();
        let hooks_rel = provider.hooks_manifest_path().unwrap();
        let events = provider.registered_hook_events();
        let launcher = format!("{host_id}-router-rs-hook.sh");

        // Override template with mismatched command to test template parity
        let template_rel = format!("configs/framework/{host_id}-hooks.workspace-template.json");
        let template_abs = repo.join(&template_rel);
        let template_text = std::fs::read_to_string(&template_abs).unwrap();
        let wrong_launcher = format!("y/other-{launcher}");
        let patched = template_text.replace(&launcher, &wrong_launcher);
        std::fs::write(&template_abs, patched.as_bytes()).unwrap();

        let task = "t-mismatch";
        seed_task_artifacts(&repo, task);
        write_baseline(&repo, task).unwrap();
        let snap = host_projection::hosts::host_extensions::schema_drift::snapshot_host_hooks(
            &repo,
            Path::new(hooks_rel),
            Path::new(&template_rel),
            events,
            &[],
            &launcher,
        )
        .unwrap();
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
        write_all_hosts_hooks_fixtures(&repo);
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
