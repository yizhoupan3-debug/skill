//! Cross-host worktree auto-save and audit utilities.
//!
//! All functions operate through `std::process::Command` and silently
//! handle errors — no hook event is ever blocked by a failed git operation.
//!
//! Called from `router_command_dispatch.rs::dispatch_hook_command()` as a
//! cross-cutting side effect before routing to the host-specific handler,
//! ensuring all 4 hosts (cursor/claude/opencode/codex) are covered.
//!
//! State files are stored per-host under `<config_dir>/hook-state/` to
//! prevent cross-host state pollution.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Map host_id to its config directory via registry.
/// Returns empty string for unknown hosts.
fn host_config_dir(host_id: &str) -> &'static str {
    framework_kernel::runtime_registry::host_private_config_dir(host_id)
}

/// Return the per-host hook-state directory leaf, e.g. ".claude/hook-state".
fn host_state_dir_leaf(host_id: &str) -> String {
    format!("{}/hook-state", host_config_dir(host_id))
}

fn dirty_count_rel(host_id: &str) -> String {
    format!("{}/dirty_count", host_state_dir_leaf(host_id))
}

fn audit_result_rel(host_id: &str) -> String {
    format!("{}/session_audit.txt", host_state_dir_leaf(host_id))
}

/// After N consecutive dirty PostToolUse checks, auto-stash.
const DIRTY_THRESHOLD: usize = 5;

/// Default branch names to skip during auto-save.
const DEFAULT_BRANCHES: &[&str] = &["main", "master"];

/// git stash auto-save message prefix.
const STASH_PREFIX: &str = "auto-save";

// ── Public API ───────────────────────────────────────────────────

/// Returns `true` when `cwd/.git` is a file (git worktree marker).
pub fn is_worktree_path(cwd: &Path) -> bool {
    cwd.join(".git").is_file()
}

/// Returns `true` when `branch` is a default/protected branch name.
pub fn is_default_branch(branch: &str) -> bool {
    DEFAULT_BRANCHES.contains(&branch)
}

/// Parse `git worktree list --porcelain` output.
///
/// Each worktree block:
/// ```text
/// worktree /path
/// HEAD abc123...
/// branch refs/heads/name
/// locked              ← optional
/// ```
pub fn parse_worktree_list(output: &str) -> Vec<(PathBuf, String, bool)> {
    output
        .split("\n\n")
        .filter_map(|block| {
            let mut path: Option<PathBuf> = None;
            let mut branch: Option<String> = None;
            let mut locked = false;
            for line in block.lines() {
                if let Some(p) = line.strip_prefix("worktree ") {
                    path = Some(PathBuf::from(p));
                } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                    branch = Some(b.to_string());
                } else if line == "locked" || line.starts_with("locked ") {
                    locked = true;
                }
            }
            Some((path?, branch?, locked))
        })
        .collect()
}

/// Collect all git worktrees from `repo_root` that have uncommitted changes.
///
/// Returns `Vec<(worktree_path, branch_name, dirty_file_count)>`.
/// Skips worktrees whose directory no longer exists on disk.
pub fn collect_dirty_worktrees(repo_root: &Path) -> Vec<(PathBuf, String, usize)> {
    let output = match git(repo_root, &["worktree", "list", "--porcelain"]) {
        Some(o) => o,
        None => return vec![],
    };
    let list = parse_worktree_list(&output);
    let mut dirty = Vec::new();
    for (wt_path, branch, _locked) in &list {
        if !wt_path.is_dir() {
            continue;
        }
        let count = dirty_file_count(repo_root, wt_path);
        if count > 0 {
            dirty.push((wt_path.clone(), branch.clone(), count));
        }
    }
    dirty
}

/// Auto-save dirty worktrees: `git stash push -m "auto-save:<ts>-<label>"`.
///
/// Skips worktrees on `main`/`master` branches.
/// Silently succeeds if there is nothing to save.
pub fn auto_save_worktrees(repo_root: &Path, label: &str) {
    let ts = unix_ms();
    let dirty_list = collect_dirty_worktrees(repo_root);
    for (wt_path, branch, _count) in &dirty_list {
        if is_default_branch(branch) {
            continue;
        }
        let msg = format!("{STASH_PREFIX}:{ts}-{label}-{branch}");
        git(
            repo_root,
            &[
                "-C",
                &wt_path.to_string_lossy(),
                "stash",
                "push",
                "-m",
                &msg,
                "--include-untracked",
            ],
        );
    }
}

/// SessionStart audit: unlock & prune orphaned worktrees, return report.
///
/// Scans `git worktree list --porcelain` for locked worktrees whose
/// directories no longer exist, then auto-unlocks and prunes them.
/// Also lists any surviving dirty worktrees.
pub fn audit_orphan_worktrees(repo_root: &Path) -> Option<String> {
    let output = git(repo_root, &["worktree", "list", "--porcelain"])?;
    let list = parse_worktree_list(&output);

    let mut orphaned: Vec<String> = Vec::new();
    let mut dirty_warnings: Vec<String> = Vec::new();

    for (wt_path, branch, locked) in &list {
        if !wt_path.is_dir() && *locked {
            // Directory gone + locked = orphan. Unlock then prune.
            let path_str = wt_path.to_string_lossy();
            git(repo_root, &["worktree", "unlock", &path_str]);
            orphaned.push(format!("{path_str} (branch: {branch})"));
        } else if wt_path.is_dir() {
            let count = dirty_file_count(repo_root, wt_path);
            if count > 0 {
                dirty_warnings.push(format!(
                    "{} has {} dirty file(s) (branch: {})",
                    wt_path.display(),
                    count,
                    branch
                ));
            }
        }
    }

    if !orphaned.is_empty() {
        git(repo_root, &["worktree", "prune"]);
    }

    let mut parts: Vec<String> = Vec::new();
    if !orphaned.is_empty() {
        parts.push(format!(
            "[worktree-audit] Cleaned up {} orphan worktree(s).",
            orphaned.len()
        ));
    }
    if !dirty_warnings.is_empty() {
        parts.push("[worktree-audit] Warning: dirty worktree(s):".to_string());
        parts.extend(dirty_warnings);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Lightweight dirty tracking, called from PostToolUse.
///
/// Reads/writes `.claude/hook-state/dirty_count`.
/// Counts dirty files across ALL worktrees (not just current dir).
/// Stashes when the consecutive dirty count reaches `DIRTY_THRESHOLD`.
pub fn track_and_maybe_stash(repo_root: &Path, host_id: &str, label: &str) {
    let count_path = repo_root.join(dirty_count_rel(host_id));
    let count = collect_dirty_worktrees(repo_root)
        .iter()
        .map(|(_, _, c)| c)
        .sum::<usize>();

    let accum: usize = std::fs::read_to_string(&count_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let new_accum = if count > 0 { accum + 1 } else { 0 };
    let _ = std::fs::write(&count_path, format!("{new_accum}\n"));

    if new_accum >= DIRTY_THRESHOLD {
        auto_save_worktrees(repo_root, label);
        let _ = std::fs::write(&count_path, "0\n");
    }
}

/// Persist the SessionStart audit result to a state file so it can be
/// consumed by the first `userpromptsubmit` event on any host.
pub fn save_audit_result(repo_root: &Path, host_id: &str, audit: Option<String>) {
    let path = repo_root.join(audit_result_rel(host_id));
    match audit {
        Some(msg) => { let _ = std::fs::write(&path, &msg); }
        None => { let _ = std::fs::remove_file(&path); }
    }
}

/// Read and remove the pending audit result. Called once from
/// `hook_dispatch.rs::dispatch()` on the first event after SessionStart.
pub fn take_audit_result(repo_root: &Path, host_id: &str) -> Option<String> {
    let path = repo_root.join(audit_result_rel(host_id));
    let content = std::fs::read_to_string(&path).ok()?;
    if content.trim().is_empty() {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let _ = std::fs::remove_file(&path);
    Some(content)
}

// ── Internal helpers ─────────────────────────────────────────────

/// Run `git <args>` in `repo_root`. Returns stdout text on success.
fn git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            exit_code = ?output.status.code(),
            stderr = %stderr,
            "git command failed"
        );
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Count dirty files via `git status --porcelain` in `cwd`.
fn dirty_file_count(_repo_root: &Path, cwd: &Path) -> usize {
    std::process::Command::new("git")
        .args([
            "-C",
            &cwd.to_string_lossy(),
            "status",
            "--porcelain",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8(o.stdout).unwrap_or_default())
        .map(|text| text.lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0)
}

/// Current unix epoch milliseconds.
fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default_branch() {
        assert!(is_default_branch("main"));
        assert!(is_default_branch("master"));
        assert!(!is_default_branch("feature-x"));
        assert!(!is_default_branch("develop"));
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        assert!(parse_worktree_list("").is_empty());
    }

    #[test]
    fn test_parse_worktree_list_single() {
        let input = "worktree /repo/.claude/worktrees/wt1\n\
                      HEAD a1b2c3d4e5f6...\n\
                      branch refs/heads/feature-x\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, PathBuf::from("/repo/.claude/worktrees/wt1"));
        assert_eq!(result[0].1, "feature-x");
        assert!(!result[0].2);
    }

    #[test]
    fn test_parse_worktree_list_locked() {
        let input = "worktree /repo/.claude/worktrees/wt1\n\
                      HEAD a1b2c3...\n\
                      branch refs/heads/feature\n\
                      locked claude agent\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].2);
    }

    #[test]
    fn test_parse_worktree_list_multiple_with_empty() {
        let input = "worktree /repo\n\
                      HEAD abc...\n\
                      branch refs/heads/main\n\
                      \n\
                      worktree /repo/.claude/worktrees/wt2\n\
                      HEAD def...\n\
                      branch refs/heads/feature\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "main");
        assert_eq!(result[1].1, "feature");
    }

    #[test]
    fn test_is_worktree_path_regular_repo() {
        let dir = std::env::temp_dir().join("wt_test_regular");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // `.git` as DIR → NOT worktree
        let dot_git = dir.join(".git");
        std::fs::create_dir_all(&dot_git).unwrap();
        assert!(!is_worktree_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_worktree_path_worktree() {
        let dir = std::env::temp_dir().join("wt_test_worktree");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // `.git` as FILE → is worktree
        let dot_git = dir.join(".git");
        std::fs::write(&dot_git, "gitdir: /tmp/fake/.git/worktrees/test\n").unwrap();
        assert!(is_worktree_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
