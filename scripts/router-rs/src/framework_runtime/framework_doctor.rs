//! Human-readable checks for `router-rs framework doctor`.

use crate::router_env_flags::router_rs_task_ledger_flock_enabled;
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
