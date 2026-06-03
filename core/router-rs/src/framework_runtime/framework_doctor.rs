//! Human-readable checks for `router-rs framework doctor`.

use crate::autopilot_goal::read_task_pointer_pair;
use crate::router_env_flags::router_rs_task_ledger_flock_enabled;
use crate::task_state::resolve_task_view;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

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

    println!("\n--- auto self-healing: cleaning broken symlinks ---");
    let _ = auto_clean_broken_symlinks(repo_root);

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
    match crate::runtime_registry::check_review_gate_registry_snapshot(repo_root) {
        Ok(()) => println!("RUNTIME_REGISTRY review_gate snapshot: ok"),
        Err(e) => {
            let msg = format!("RUNTIME_REGISTRY review_gate snapshot failed: {e}");
            println!("WARN: {msg}");
            warns.push(msg);
        }
    }
    let review_mode = match crate::review_gate_engine::cursor_review_gate_mode() {
        crate::review_gate_engine::CursorReviewGateMode::Lite => "lite",
        crate::review_gate_engine::CursorReviewGateMode::Strict => "strict",
    };
    println!(
        "ROUTER_RS_CURSOR_REVIEW_GATE_MODE: {review_mode} (env ROUTER_RS_CURSOR_REVIEW_GATE_MODE)"
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

    println!("\n--- host install projections (optional in framework source repo) ---");
    let host_install_checks = [
        (
            ".claude/settings.json (claude-code hooks)",
            repo_root.join(".claude").join("settings.json"),
        ),
        (
            ".claude/rules/framework.md",
            repo_root.join(".claude").join("rules").join("framework.md"),
        ),
        (
            ".gemini/antigravity/rules/framework.md",
            repo_root
                .join(".gemini")
                .join("antigravity")
                .join("rules")
                .join("framework.md"),
        ),
        (
            ".gemini/mcp.json",
            repo_root.join(".gemini").join("mcp.json"),
        ),
    ];
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
        println!(
            "hint: router-rs framework host-integration install --to claude-code|claude-desktop|antigravity --scope project"
        );
        let deprecated_shim = repo_root
            .join(".claude")
            .join("hooks")
            .join("router-rs-hook.sh");
        if deprecated_shim.is_file() {
            let msg = format!(
                "deprecated shim still present at {} — prefer .claude/settings.json hooks (see docs/hosts/claude.md)",
                deprecated_shim.display()
            );
            println!("WARN: {msg}");
            warns.push(msg);
        }
    }

    println!("\n--- Codex projection reminder ---");
    println!(
        "If you edited repo-root AGENTS.md or AGENTS_CODEX.md and rely on Codex hooks that embed policy from router-rs,"
    );
    println!("rebuild this binary then run:");
    println!("  router-rs framework sync-entrypoints --repo-root <repo>");
    println!(
        "(or `codex sync --repo-root` with the same sync engine). See AGENTS_CODEX.md (Codex Sync)."
    );

    println!("\n--- hook follow-up tokens (quick ref) ---");
    println!(
        "Host-injected machine lines start with ASCII `router-rs ` (e.g. REVIEW_GATE / AG_FOLLOWUP)."
    );
    println!(
        "Lines starting with RG_FOLLOWUP / RG FOLLOWUP without that prefix are not from this harness; see docs/framework_operator_primer.md."
    );

    println!("\n--- ephemeral Stop checkpoint rows (operator) ---");
    println!(
        "If task_registry.json lists many cursor-stop-* / session-checkpoint-* rows or focus drifted:"
    );
    println!(
        "  1) Pick the real task_id (active/focus or GOAL_STATE under artifacts/current/<id>/)."
    );
    println!("  2) Prune stale registry tasks and optional empty cursor-stop-* dirs.");
    println!("  3) Align artifacts/current/active_task.json, focus_task.json, and .supervisor_state.json.");
    println!("  4) Run: router-rs framework task-state-resolve --repo-root <repo>");
    println!("Solo defaults (2026-05): PostTool evidence / depth hint / journal / TASK_STATE auto-sync are OFF unless env =1.");
    println!("  Optional: ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1 (Stop checkpoint env is no-op; do not set ROUTER_RS_CONTINUITY_STOP_CHECKPOINT).");
    println!("  TASK_STATE projection: router-rs framework task-state-aggregate-sync --repo-root <repo>");
    println!("  Root artifact clutter: router-rs framework maint migrate-current-artifact-clutter --repo-root <repo>");

    println!("\n--- Codex hooks duplication (operator) ---");
    for line in super::codex_hooks_duplicate::collect_codex_hooks_duplicate_warnings(repo_root) {
        println!("{line}");
        // Lines from this helper are warnings by convention.
        warns.push(line);
    }

    println!("\n--- generated artifacts (manifest drift) ---");
    match crate::host_integration::generated_artifacts_status_for_repo(repo_root) {
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
        println!("ROUTER_RS_TASK_LEDGER_FLOCK: enabled (default) — cross-process GOAL/RFV/EVIDENCE writes serialize under artifacts/current/.router-rs.task-ledger.lock.");
    } else {
        let msg = "ROUTER_RS_TASK_LEDGER_FLOCK is disabled — parallel hook subprocesses may interleave writes to artifacts/current/**; treat TASK_STATE.json and rollups as best-effort until flock is re-enabled.".to_string();
        println!("WARN: {msg}");
        warns.push(msg);
    }

    println!("\n--- control plane (supervisor / pointers) ---");
    match super::build_framework_runtime_snapshot_envelope(repo_root, None, None) {
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

                    if let (Some(active_id), Some(focus_id)) =
                        (&active_task_id, &focus_task_id)
                    {
                        if active_id != focus_id {
                            let active_goal = crate::autopilot_goal::read_goal_state(
                                repo_root,
                                Some(active_id.as_str()),
                            )
                            .ok()
                            .flatten();
                            let focus_goal = crate::autopilot_goal::read_goal_state(
                                repo_root,
                                Some(focus_id.as_str()),
                            )
                            .ok()
                            .flatten();
                            let active_drives = active_goal
                                .as_ref()
                                .is_some_and(crate::autopilot_goal::goal_state_requests_continuation);
                            let focus_drives = focus_goal
                                .as_ref()
                                .is_some_and(crate::autopilot_goal::goal_state_requests_continuation);
                            if active_goal.is_some() && !active_drives && focus_drives {
                                issues.push(format!(
                                    "ACTIVE_NOT_DRIVING: active '{active_id}' GOAL does not request continuation but focus '{focus_id}' does; align pointers or complete focus GOAL"
                                ));
                            }
                        }
                    }

                    // Check for orphan task directories
                    if let Some(tasks) = registry.get("tasks").and_then(|v| v.as_array()) {
                        for row in tasks {
                            let tid = row
                                .get("task_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            if (tid.starts_with("cursor-stop-")
                                || tid.starts_with("session-checkpoint-"))
                                && tid
                                    != crate::framework_runtime::CONTINUITY_SESSION_CHECKPOINT_TASK_ID
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
            "has_dangling_focus_pointer": focus_dir.as_ref().map_or(false, |d| !d.is_dir()),
            "active_task_id": active_task_id,
            "focus_task_id": focus_task_id,
        }
    }))
}

/// Auto-detect and safely clean broken symlinks inside multi-host client directories.
pub fn auto_clean_broken_symlinks(repo_root: &Path) -> Result<(), String> {
    let targets = [
        ".antigravitycli",
        ".claude",
        ".cursor",
        ".codex",
        ".gemini",
        "artifacts",
    ];
    let mut cleaned_count = 0;
    for sub in &targets {
        let dir_path = repo_root.join(sub);
        if !dir_path.is_dir() {
            continue;
        }
        if let Err(e) = clean_broken_symlinks_in_dir(&dir_path, &mut cleaned_count) {
            println!("WARN: failed to clean broken symlinks in {}: {e}", dir_path.display());
        }
    }
    if cleaned_count > 0 {
        println!("INFO: Successfully auto-cleaned {cleaned_count} broken symlink(s) to secure system integrity.");
    } else {
        println!("INFO: No broken symlinks detected. Multi-host workspace is healthy.");
    }
    Ok(())
}

fn clean_broken_symlinks_in_dir(dir: &Path, cleaned_count: &mut usize) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(metadata) = fs::symlink_metadata(&path) {
                if metadata.is_symlink() {
                    // Check if the symlink target exists
                    if !path.exists() {
                        if let Err(e) = fs::remove_file(&path) {
                            println!("WARN: failed to remove broken symlink {}: {e}", path.display());
                        } else {
                            println!("REPAIRED: Removed broken symlink {}", path.display());
                            *cleaned_count += 1;
                        }
                    }
                } else if metadata.is_dir() {
                    let _ = clean_broken_symlinks_in_dir(&path, cleaned_count);
                }
            }
        }
    }
    Ok(())
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
