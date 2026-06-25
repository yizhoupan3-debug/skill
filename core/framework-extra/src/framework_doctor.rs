//! Human-readable checks for `router-rs framework doctor`.

use fr_exec::router_env_flags::router_rs_task_ledger_flock_enabled;
use core_state::task_state::resolve_task_view;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use core_policy::doc_registry;
use serde::Serialize;

/// Structured result for `router-rs framework doctor`.
#[derive(Debug, Serialize)]
pub struct DoctorResult {
    /// true if no warnings detected.
    pub ok: bool,
    /// Number of warnings found.
    pub warn_count: usize,
    /// Collected warning messages.
    pub warns: Vec<String>,
}

/// Run framework diagnostics. Returns structured `DoctorResult` (JSON printed to stdout).
/// If `warn_count > 0`, the caller should exit with code 1.
pub fn run_framework_doctor(repo_root: &Path) -> Result<DoctorResult, String> {
    println!("router-rs framework doctor");
    println!("repo_root: {}", repo_root.display());
    match std::env::current_exe() {
        Ok(p) => println!("router_rs_current_exe: {}", p.display()),
        Err(e) => println!("router_rs_current_exe: <unavailable: {e}>"),
    }

    let mut warns: Vec<String> = Vec::new();

    println!("\n--- symlink health check (report-only) ---");
    match report_broken_symlinks(repo_root) {
        Ok(0) => println!("INFO: No broken symlinks detected. Multi-host workspace is healthy."),
        Ok(n) => println!("WARN: {n} broken symlink(s) found. Run `router-rs framework clean-orphans` to fix."),
        Err(e) => tracing::warn!(error = %e, "failed to check broken symlinks"),
    }

    let checks: Vec<(String, std::path::PathBuf)> = vec![
        ("AGENTS.md".to_string(), repo_root.join("AGENTS.md")),
        (
            "skills/SKILL_ROUTING_RUNTIME.json".to_string(),
            repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json"),
        ),
        (
            "configs/framework/RUNTIME_REGISTRY.json".to_string(),
            repo_root
                .join("configs")
                .join("framework")
                .join("RUNTIME_REGISTRY.json"),
        ),
    ];

    println!("\n--- path checks ---");
    match core_policy::registry_review_gate::check_review_gate_registry_snapshot(repo_root) {
        Ok(()) => println!("RUNTIME_REGISTRY review_gate snapshot: ok"),
        Err(e) => {
            let msg = format!("RUNTIME_REGISTRY review_gate snapshot failed: {e}");
            println!("WARN: {msg}");
            warns.push(msg);
        }
    }
    let review_mode = match core_policy::review_gate_engine::review_gate_mode() {
        core_policy::review_gate_engine::ReviewGateMode::Lite => "lite",
        core_policy::review_gate_engine::ReviewGateMode::Strict => "strict",
    };
    println!(
        "ROUTER_RS_REVIEW_GATE_MODE: {review_mode} (env ROUTER_RS_REVIEW_GATE_MODE, legacy ROUTER_RS_CURSOR_REVIEW_GATE_MODE)"
    );
    for (label, path) in &checks {
        let status = if path.is_file() {
            "ok (file)"
        } else if path.exists() {
            "exists (not a regular file)"
        } else {
            "missing"
        };
        println!("{label}: {status} ({})", path.display());
    }

    println!("\n--- docs governance (reports/plans) ---");
    for dir_key in doc_registry::all_dirs() {
        let dir_path = repo_root.join(dir_key);
        if dir_path.is_dir() {
            println!("{dir_key}: ok (directory exists)");
        } else if dir_path.is_file() {
            let msg = format!("{dir_key}: is a file, should be a directory");
            println!("WARN: {msg}");
            warns.push(msg);
        } else {
            let msg = format!("{dir_key}: missing — create directory");
            println!("WARN: {msg}");
            warns.push(msg);
        }
    }
    for dir_key in doc_registry::all_dirs() {
        let dir_path = repo_root.join(dir_key);
        if !dir_path.is_dir() {
            continue;
        }
        match std::fs::read_dir(&dir_path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let fname_str = entry.file_name().to_string_lossy().into_owned();
                    if !fname_str.ends_with(".md") {
                        continue;
                    }
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    let stem_end = fname_str.len().saturating_sub(3);
                    if stem_end < 10 {
                        let msg = format!("{dir_key}/{fname_str}: filename too short for convention {{topic}}-{{YYYY-MM-DD}}.md");
                        println!("WARN: {msg}");
                        warns.push(msg);
                        continue;
                    }
                    let suffix = &fname_str[(stem_end - 10)..stem_end];
                    let is_date = suffix.len() == 10
                        && suffix.as_bytes()[0].is_ascii_digit()
                        && suffix.as_bytes()[1].is_ascii_digit()
                        && suffix.as_bytes()[2].is_ascii_digit()
                        && suffix.as_bytes()[3].is_ascii_digit()
                        && suffix.as_bytes()[4] == b'-'
                        && suffix.as_bytes()[5].is_ascii_digit()
                        && suffix.as_bytes()[6].is_ascii_digit()
                        && suffix.as_bytes()[7] == b'-'
                        && suffix.as_bytes()[8].is_ascii_digit()
                        && suffix.as_bytes()[9].is_ascii_digit();
                    if !is_date {
                        let msg = format!("{dir_key}/{fname_str}: does not follow convention {{topic}}-{{YYYY-MM-DD}}.md");
                        println!("WARN: {msg}");
                        warns.push(msg);
                    }
                }
            }
            Err(e) => {
                let msg = format!("{dir_key}: read error ({e})");
                println!("WARN: {msg}");
                warns.push(msg);
            }
        }
    }

    println!("\n--- host install projections (optional in framework source repo) ---");
    // Build check list dynamically from RUNTIME_REGISTRY.json host_entrypoints.
    let mut host_install_checks: Vec<(String, std::path::PathBuf)> = Vec::new();
    // Read host_entrypoints from registry for each supported host
    if let Ok(reg) = framework_kernel::runtime_registry::load_runtime_registry_json(repo_root)
        && let Ok(supported) = framework_kernel::framework_host_targets::host_targets_supported_host_ids(&reg)
        {
            for host_id in &supported {
                if let Ok(ep_value) =
                    framework_kernel::framework_host_targets::host_entrypoints_value_for_id(&reg, host_id)
                {
                    let paths: Vec<String> = match &ep_value {
                        Value::Array(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                        Value::String(s) => vec![s.clone()],
                        _ => vec![],
                    };
                    for ep in &paths {
                        // Only check file paths (skip agent policy names like AGENTS.md)
                        if ep.contains('/') || ep.contains('.') {
                            host_install_checks.push((ep.clone(), repo_root.join(ep)));
                        }
                    }
                }
            }
        }
    let mut host_install_missing = 0usize;
    for (label, path) in &host_install_checks {
        let status = if path.is_file() {
            "ok (file)"
        } else {
            host_install_missing += 1;
            "missing (run host-integration install)"
        };
        println!("{label}: {status} ({})", path.display());
    }
    if host_install_missing > 0 {
        let installable: Vec<&str> = host_projection::hosts::host_provider_registry()
            .iter()
            .filter(|p| p.capabilities().config_path.contains('/'))
            .map(|p| p.host_id())
            .collect();
        println!(
            "hint: router-rs framework host-integration install --to {} --scope project",
            installable.join("|")
        );
        // Data-driven deprecated shim check across all host directories (ADR §4)
        for host_dir in framework_kernel::runtime_registry::host_home_dirs().iter() {
            let deprecated_shim = repo_root
                .join(host_dir)
                .join("hooks")
                .join("router-rs-hook.sh");
            if deprecated_shim.is_file() {
                let msg = format!(
                    "deprecated shim still present at {} — prefer {}/settings.json hooks (see AGENTS.md)",
                    deprecated_shim.display(),
                    host_dir,
                );
                println!("WARN: {msg}");
                warns.push(msg);
            }
        }
    }

    println!("\n--- Codex projection reminder ---");
    println!(
        "If you edited repo-root AGENTS.md and rely on Codex hooks that embed policy from router-rs,"
    );
    println!("rebuild this binary then run:");
    println!("  router-rs framework sync-entrypoints --repo-root <repo>");
    println!("See AGENTS.md (Codex Sync) for host-specific details.");

    println!("\n--- hook follow-up tokens (quick ref) ---");
    println!(
        "Host-injected machine lines start with ASCII `router-rs ` (e.g. REVIEW_GATE / AG_FOLLOWUP)."
    );
    println!(
        "Lines starting with RG_FOLLOWUP / RG FOLLOWUP without that prefix are not from this harness."
    );

    println!("\n--- ephemeral Stop checkpoint rows (operator) ---");
    println!(
        "If task_registry.json lists many cursor-stop-* / session-checkpoint-* rows or focus drifted:"
    );
    println!(
        "  1) Pick the real task_id (active/focus or GOAL_STATE under artifacts/current/<id>/)."
    );
    println!("  2) Prune stale registry tasks and optional empty cursor-stop-* dirs.");
    println!(
        "  3) Align artifacts/current/active_task.json, focus_task.json, and .supervisor_state.json."
    );
    println!("  4) Run: router-rs framework task-state-resolve --repo-root <repo>");
    println!(
        "Solo defaults (2026-05): PostTool evidence / depth hint / journal / TASK_STATE auto-sync are OFF unless env =1."
    );
    println!(
        "  Optional: ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1 (Stop checkpoint env is no-op; do not set ROUTER_RS_CONTINUITY_STOP_CHECKPOINT)."
    );
    println!(
        "  TASK_STATE projection: router-rs framework task-state-aggregate-sync --repo-root <repo>"
    );
    println!(
        "  Root artifact clutter: router-rs framework maint migrate-current-artifact-clutter --repo-root <repo>"
    );

    println!("\n--- Codex hooks duplication (operator) ---");
    for line in framework_runtime_hooks::check_hook_duplicates(repo_root) {
        println!("{line}");
        // Lines from this helper are warnings by convention.
        warns.push(line);
    }

    println!("\n--- generated artifacts (manifest drift) ---");
    match host_projection::host_integration::generated_artifacts_status_for_repo(repo_root) {
        Ok(summary) => {
            let ok = summary.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            println!(
                "generated-artifacts-status: {}",
                if ok { "ok" } else { "DRIFT or FAIL" }
            );
            if let Some(arr) = summary
                .pointer("/manifest_status/drifted_artifacts")
                .and_then(|v| v.as_array())
            {
                let n = arr.len();
                if n > 0 {
                    println!("  drifted_count: {n}");
                }
            }
            if !ok {
                let msg = "generated-artifacts-status: DRIFT or FAIL (fix: cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot)".to_string();
                warns.push(msg);
            }
        }
        Err(e) => {
            let msg = format!("generated-artifacts-status: error ({e})");
            println!("{msg}");
            warns.push(msg);
        }
    }

    println!("\n--- continuity ledger ---");
    if router_rs_task_ledger_flock_enabled() {
        println!(
            "ROUTER_RS_TASK_LEDGER_FLOCK: enabled (default) — cross-process GOAL/RFV/EVIDENCE writes serialize under artifacts/current/.router-rs.task-ledger.lock."
        );
    } else {
        let msg = "ROUTER_RS_TASK_LEDGER_FLOCK is disabled — parallel hook subprocesses may interleave writes to artifacts/current/**; treat TASK_STATE.json and rollups as best-effort until flock is re-enabled.".to_string();
        println!("WARN: {msg}");
        warns.push(msg);
    }

    println!("\n--- control plane (supervisor / pointers) ---");
    match crate::snapshot::build_framework_runtime_snapshot_envelope_with_level(repo_root, None, None, "full")
    {
        Ok(envelope) => {
            let snapshot = &envelope["runtime_snapshot"];
            let state = snapshot
                .get("continuity")
                .and_then(|v| v.get("state"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            println!("continuity.state: {state}");
            let reasons = snapshot
                .get("continuity")
                .and_then(|v| v.get("inconsistency_reasons"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(
                    snapshot
                        .get("control_plane_inconsistency_reasons")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                );
            for reason in reasons.filter_map(Value::as_str).filter(|s| !s.is_empty()) {
                println!("WARN: {reason}");
                warns.push(reason.to_string());
            }
        }
        Err(e) => {
            let msg = format!("runtime snapshot unavailable: {e}");
            println!("WARN: {msg}");
            warns.push(msg);
        }
    }

    let warn_count = warns.len();
    let result = DoctorResult {
        ok: warn_count == 0,
        warn_count,
        warns,
    };
    println!(
        "\n--- doctor result (JSON) ---\n{}",
        serde_json::to_string_pretty(&result).unwrap_or_default()
    );
    Ok(result)
}

/// Read task pointer files directly from `artifacts/current/` (replaces removed `read_task_pointer_pair`).
/// Tries `TASK_POINTERS.json` first, then falls back to individual `active_task.json` / `focus_task.json`.
fn read_local_task_pointer_pair(repo_root: &Path) -> (Option<String>, Option<String>) {
    let current = repo_root.join("artifacts/current");
    // Phase 3C consolidated file
    let pointers_path = current.join("TASK_POINTERS.json");
    if pointers_path.is_file()
        && let Ok(raw) = fs::read_to_string(&pointers_path)
            && let Ok(data) = serde_json::from_str::<Value>(&raw) {
                let active = parse_pointer_task_id(data.get("active_task_id"));
                let focus = parse_pointer_task_id(data.get("focus_task_id"));
                if active.is_some() || focus.is_some() {
                    return (active, focus);
                }
            }
    // Legacy individual files
    let active = read_single_pointer(&current.join("active_task.json"));
    let focus = read_single_pointer(&current.join("focus_task.json"));
    (active, focus)
}

fn parse_pointer_task_id(value: Option<&Value>) -> Option<String> {
    let s = value?.as_str()?.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn read_single_pointer(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    parse_pointer_task_id(data.get("task_id"))
}

/// Continuity audit: check task pointers, registry consistency, and orphan directories.
pub fn run_continuity_audit(repo_root: &Path) -> Result<Value, String> {
    let mut issues: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    let current_dir = repo_root.join("artifacts/current");
    if !current_dir.is_dir() {
        return Ok(json!({
            "ok": false,
            "error": "artifacts/current directory missing",
            "issues": ["artifacts/current directory does not exist"],
        }));
    }

    // Read task pointers directly from pointer files (pointer helpers removed from core_state)
    let (active_task_id, focus_task_id) = read_local_task_pointer_pair(repo_root);
    let active_dir = active_task_id.as_ref().map(|id| current_dir.join(id));
    let focus_dir = focus_task_id.as_ref().map(|id| current_dir.join(id));

    // Validate active_task pointer
    if let (Some(id), Some(dir)) = (&active_task_id, &active_dir) {
        if !dir.is_dir() {
            issues.push(format!(
                "active_task.json points to non-existent directory: {}",
                id
            ));
        } else {
            info.push(format!("active_task.json: {} (valid)", id));
        }
    } else if active_task_id.is_none() {
        info.push("active_task.json: not set".to_string());
    }

    // Validate focus_task pointer
    if let (Some(id), Some(dir)) = (&focus_task_id, &focus_dir) {
        if !dir.is_dir() {
            issues.push(format!(
                "focus_task.json points to non-existent directory: {}",
                id
            ));
        } else {
            info.push(format!("focus_task.json: {} (valid)", id));
        }
    } else if focus_task_id.is_none() {
        info.push("focus_task.json: not set".to_string());
    }

    // Check for dangling focus pointer
    if let (Some(focus_id), false) = (
        &focus_task_id,
        focus_dir.as_ref().is_none_or(|d| d.is_dir()),
    ) {
        issues.push(format!(
            "DANGLING POINTER: focus_task.json references '{}' but directory does not exist",
            focus_id
        ));
        if let Some(task_id) = &active_task_id {
            warnings.push(format!(
                "Suggested fix: set focus_task.json to '{}' or clear it",
                task_id
            ));
        } else {
            warnings.push(
                "Suggested fix: clear focus_task.json (no active task to fallback to)".to_string(),
            );
        }
    }

    // Check task_registry.json consistency
    let registry_path = current_dir.join("task_registry.json");
    if registry_path.is_file() {
        match fs::read_to_string(&registry_path) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(registry) => {
                    let focus_in_registry = registry
                        .get("focus_task_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    if let (Some(reg_focus), Some(ptr_focus)) = (&focus_in_registry, &focus_task_id)
                    {
                        if reg_focus != ptr_focus {
                            issues.push(format!(
                                "REGISTRY MISMATCH: task_registry.json focus_task_id='{}' differs from focus_task.json='{}'",
                                reg_focus, ptr_focus
                            ));
                        } else {
                            info.push(
                                "task_registry.json focus_task_id: matches focus_task.json"
                                    .to_string(),
                            );
                        }
                    }

                    if let (Some(active_id), Some(focus_id)) = (&active_task_id, &focus_task_id)
                        && active_id != focus_id {
                            let active_goal = core_state::state_manager::read_goal_state(
                                repo_root,
                                Some(active_id.as_str()),
                            )
                            .ok()
                            .flatten();
                            let focus_goal = core_state::state_manager::read_goal_state(
                                repo_root,
                                Some(focus_id.as_str()),
                            )
                            .ok()
                            .flatten();
                            let active_drives = active_goal.as_ref().is_some_and(
                                core_state::state_manager::goal_state_requests_continuation,
                            );
                            let focus_drives = focus_goal.as_ref().is_some_and(
                                core_state::state_manager::goal_state_requests_continuation,
                            );
                            if active_goal.is_some() && !active_drives && focus_drives {
                                issues.push(format!(
                                    "ACTIVE_NOT_DRIVING: active '{active_id}' GOAL does not request continuation but focus '{focus_id}' does; align pointers or complete focus GOAL"
                                ));
                            }
                        }

                    // Check for orphan task directories
                    if let Some(tasks) = registry.get("tasks").and_then(|v| v.as_array()) {
                        for row in tasks {
                            let tid = row
                                .get("task_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            if framework_kernel::runtime_registry::is_ephemeral_task_id(tid)
                                && tid
                                    != core_state::state_manager::CONTINUITY_SESSION_CHECKPOINT_TASK_ID
                            {
                                warnings.push(format!(
                                    "EPHEMERAL CHECKPOINT ROW: task_registry lists '{tid}' (safe to prune if pointers point elsewhere; see framework doctor)"
                                ));
                            }
                        }
                        let registered_ids: std::collections::HashSet<_> = tasks
                            .iter()
                            .filter_map(|t| t.get("task_id").and_then(|v| v.as_str()))
                            .collect();

                        // Find directories not in registry
                        if let Ok(entries) = fs::read_dir(&current_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if !path.is_dir() {
                                    continue;
                                }
                                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                                // Skip known non-task files
                                if name.starts_with('.') || name == "task_registry.json" {
                                    continue;
                                }

                                if !registered_ids.contains(name) {
                                    let mtime = fs::metadata(&path)
                                        .and_then(|m| m.modified())
                                        .ok()
                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs());

                                    let age_days = mtime.map(|secs| {
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        (now - secs) / 86400
                                    });

                                    let age_str = age_days
                                        .map(|d| format!(" ({} days old)", d))
                                        .unwrap_or_default();

                                    warnings.push(format!(
                                        "ORPHAN DIRECTORY: '{}'{} not in task_registry.json and not referenced by any pointer",
                                        name, age_str
                                    ));
                                }
                            }
                        }

                        info.push(format!(
                            "task_registry.json: {} tasks registered",
                            tasks.len()
                        ));
                    } else {
                        info.push("task_registry.json: no tasks array found".to_string());
                    }
                }
                Err(e) => {
                    issues.push(format!("task_registry.json: invalid JSON: {}", e));
                }
            },
            Err(e) => {
                issues.push(format!("task_registry.json: read failed: {}", e));
            }
        }
    } else {
        warnings.push("task_registry.json: not found".to_string());
    }

    // Resolve task view for diagnostic info
    let task_view = resolve_task_view(repo_root, None);
    if let Some(tid) = &task_view.task_id {
        info.push(format!("resolved task_id: {}", tid));
        info.push(format!("control_mode: {:?}", task_view.control_mode));

        if let Some(dc) = &task_view.depth_compliance {
            info.push(format!("depth_score: {}/3", dc.depth_score));
        }
    } else {
        info.push("resolved task_id: none (idle)".to_string());
    }

    // Print report
    println!("\n=== Continuity Audit Report ===");
    println!("repo_root: {}", repo_root.display());

    if !issues.is_empty() {
        println!("\n[ISSUES] {} problem(s) found:", issues.len());
        for issue in &issues {
            println!("  - {}", issue);
        }
    }

    if !warnings.is_empty() {
        println!("\n[WARNINGS] {} warning(s):", warnings.len());
        for warning in &warnings {
            println!("  - {}", warning);
        }
    }

    if !info.is_empty() {
        println!("\n[INFO] {} item(s):", info.len());
        for item in &info {
            println!("  - {}", item);
        }
    }

    if issues.is_empty() && warnings.is_empty() {
        println!("\nNo issues found. Continuity state is healthy.");
    }

    Ok(json!({
        "ok": issues.is_empty(),
        "issues": issues,
        "warnings": warnings,
        "info": info,
        "summary": {
            "issue_count": issues.len(),
            "warning_count": warnings.len(),
            "has_dangling_focus_pointer": focus_dir.as_ref().is_some_and(|d| !d.is_dir()),
            "active_task_id": active_task_id,
            "focus_task_id": focus_task_id,
        }
    }))
}

/// Report broken symlinks without removing them (diagnostic-only).
pub fn report_broken_symlinks(repo_root: &Path) -> Result<usize, String> {
    let mut targets: Vec<&str> = framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS.to_vec();
    targets.push("artifacts");
    let mut broken_count = 0;
    for sub in &targets {
        let dir_path = repo_root.join(sub);
        if !dir_path.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(metadata) = fs::symlink_metadata(&path)
                    && metadata.is_symlink()
                    && !path.exists()
                {
                    println!("  BROKEN: {}", path.display());
                    broken_count += 1;
                }
            }
        }
    }
    Ok(broken_count)
}

#[cfg(test)]
mod tests {
    // removed: use crate::*
    use std::path::PathBuf;

    #[test]
    fn doctor_smoke_framework_repo_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("skill repo root");
        super::run_framework_doctor(&root).expect("doctor");
    }
}
