//! Human-readable checks for `router-rs framework doctor`.

use crate::autopilot_goal::read_task_pointer_pair;
use crate::router_env_flags::router_rs_task_ledger_flock_enabled;
use crate::task_state::resolve_task_view;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Print diagnostics to stdout (plain text, not JSON).
pub fn run_framework_doctor(repo_root: &Path) -> Result<(), String> {
    println!("router-rs framework doctor");
    println!("repo_root: {}", repo_root.display());
    match std::env::current_exe() {
        Ok(p) => println!("router_rs_current_exe: {}", p.display()),
        Err(e) => println!("router_rs_current_exe: <unavailable: {e}>"),
    }

    let checks = [
        ("AGENTS.md", repo_root.join("AGENTS.md")),
        (
            "skills/SKILL_ROUTING_RUNTIME.json",
            repo_root.join("skills").join("SKILL_ROUTING_RUNTIME.json"),
        ),
        (
            "configs/framework/RUNTIME_REGISTRY.json",
            repo_root
                .join("configs")
                .join("framework")
                .join("RUNTIME_REGISTRY.json"),
        ),
        (
            ".cursor/hooks.json",
            repo_root.join(".cursor").join("hooks.json"),
        ),
        (
            ".codex/hooks.json",
            repo_root.join(".codex").join("hooks.json"),
        ),
    ];

    println!("\n--- path checks ---");
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

    println!("\n--- Codex projection reminder ---");
    println!(
        "If you edited repo-root AGENTS.md and rely on Codex hooks that embed policy from router-rs,"
    );
    println!("rebuild this binary then run:");
    println!("  router-rs framework sync-entrypoints --repo-root <repo>");
    println!(
        "(or `codex sync --repo-root` with the same sync engine). See AGENTS.md (Codex Sync)."
    );

    println!("\n--- hook follow-up tokens (quick ref) ---");
    println!(
        "Host-injected machine lines start with ASCII `router-rs ` (e.g. REVIEW_GATE / AG_FOLLOWUP)."
    );
    println!(
        "Lines starting with RG_FOLLOWUP / RG FOLLOWUP without that prefix are not from this harness; see docs/framework_operator_primer.md."
    );

    println!("\n--- continuity ledger ---");
    if router_rs_task_ledger_flock_enabled() {
        println!("ROUTER_RS_TASK_LEDGER_FLOCK: enabled (default) — cross-process GOAL/RFV/EVIDENCE writes serialize under artifacts/current/.router-rs.task-ledger.lock.");
    } else {
        println!("WARN: ROUTER_RS_TASK_LEDGER_FLOCK is disabled.");
        println!("      Parallel hook subprocesses may interleave writes to artifacts/current/**;");
        println!(
            "      treat TASK_STATE.json and rollups as best-effort until flock is re-enabled."
        );
    }

    Ok(())
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

    // Check task pointers
    let (active_task_id, focus_task_id) = read_task_pointer_pair(repo_root);
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

    // Check for dangling focus pointer (points to non-existent task)
    if let (Some(focus_id), false) = (
        &focus_task_id,
        focus_dir.as_ref().map_or(true, |d| d.is_dir()),
    ) {
        issues.push(format!(
            "DANGLING POINTER: focus_task.json references '{}' but directory does not exist",
            focus_id
        ));
        // Suggest fix
        if active_task_id.is_some() {
            warnings.push(format!(
                "Suggested fix: set focus_task.json to '{}' or clear it",
                active_task_id.as_ref().unwrap()
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

                    // Check for orphan task directories
                    if let Some(tasks) = registry.get("tasks").and_then(|v| v.as_array()) {
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
            "has_dangling_focus_pointer": focus_dir.as_ref().map_or(false, |d| !d.is_dir()),
            "active_task_id": active_task_id,
            "focus_task_id": focus_task_id,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn doctor_smoke_framework_repo_root() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("skill repo root");
        run_framework_doctor(&root).expect("doctor");
    }
}
