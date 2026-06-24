//! Clean operations: rust target dirs, hook-state files, orphan directories.
//!
//! All clean operations support `--dry-run` to preview changes before execution.
//! Symlink targets are skipped to avoid following bind mounts into unexpected locations.

use std::fs;
use std::path::Path;

use framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS;

#[allow(dead_code)]
pub(super) fn clean_rust_target_dirs(repo_root: &Path, dry_run: bool) -> Result<(), String> {
    clean_targets_walk(repo_root, dry_run)?;
    Ok(())
}

fn clean_targets_walk(path: &Path, dry_run: bool) -> Result<(), String> {
    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
        return Ok(());
    }
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.is_symlink()
    {
        return Ok(());
    }
    if path.file_name().and_then(|n| n.to_str()) == Some("target") && path.is_dir() {
        if dry_run {
            println!("  DRY-RUN: would remove {}", path.display());
        } else {
            fs::remove_dir_all(path).map_err(|e| e.to_string())?;
            println!("  removed {}", path.display());
        }
        return Ok(());
    }
    if path.is_dir() {
        let read = fs::read_dir(path).map_err(|e| e.to_string())?;
        for ent in read {
            clean_targets_walk(&ent.map_err(|e| e.to_string())?.path(), dry_run)?;
        }
    }
    Ok(())
}

/// Clean hook-state files older than TTL days across all host directories.
#[allow(dead_code)]
pub(super) fn clean_hook_state_files(
    repo_root: &Path,
    dry_run: bool,
    ttl_days: u64,
) -> Result<(), String> {
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs()
        .saturating_sub(ttl_days * 24 * 60 * 60);

    let mut total_cleaned: usize = 0;
    let mut total_kept: usize = 0;

    for host_dir in ALL_KNOWN_HOST_DIRS {
        let hook_state_dir = repo_root.join(host_dir).join("hook-state");
        if !hook_state_dir.is_dir() {
            continue;
        }

        let mut files_to_clean: Vec<(std::path::PathBuf, u64)> = Vec::new();
        let mut kept: usize = 0;

        let entries = fs::read_dir(&hook_state_dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            let is_target = name.starts_with("review-subagent-")
                || name.starts_with("session-terminals-")
                || name.starts_with("adversarial-loop-")
                || name.starts_with(".tmp-");

            if !is_target {
                kept += 1;
                continue;
            }

            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if mtime < cutoff {
                files_to_clean.push((path, mtime));
            } else {
                kept += 1;
            }
        }

        if files_to_clean.is_empty() && kept > 0 {
            total_kept += kept;
            continue;
        }

        // Sort by mtime descending to clean newest-first on tty (useful for
        // debugging when ttl is too aggressive).
        files_to_clean.sort_by(|a, b| b.1.cmp(&a.1));

        if dry_run {
            for (p, _) in &files_to_clean {
                println!("  DRY-RUN: would remove {}", p.display());
            }
        } else {
            for (p, _) in &files_to_clean {
                if let Err(e) = fs::remove_file(p) {
                    eprintln!("  warn: remove {} failed: {e}", p.display());
                    continue;
                }
                println!("  removed {}", p.display());
            }
        }

        total_cleaned += files_to_clean.len();
        total_kept += kept;
    }

    let summary = format!(
        "hook-state: cleaned {total_cleaned} files, kept {total_kept} files across {} host dirs",
        ALL_KNOWN_HOST_DIRS.len()
    );
    if dry_run {
        println!("  DRY-RUN: {summary}");
    } else {
        eprintln!("{summary}");
    }

    Ok(())
}

/// Remove directories that are older than TTL and match known patterns
/// (e.g. `.claude/projects/`, `.cursor/tmp/`) OR are empty host dirs.
#[allow(dead_code)]
pub(super) fn clean_orphan_directories(
    repo_root: &Path,
    dry_run: bool,
    ttl_days: u64,
) -> Result<(), String> {
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs()
        .saturating_sub(ttl_days * 24 * 60 * 60);

    let mut total_removed: usize = 0;
    let mut total_skipped: usize = 0;

    for host_dir in ALL_KNOWN_HOST_DIRS {
        let dir = repo_root.join(host_dir);
        if !dir.is_dir() {
            continue;
        }

        // Check if the host dir itself is older than TTL.
        if let Ok(meta) = fs::metadata(&dir) {
            if let Ok(modified) = meta.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    if duration.as_secs() < cutoff {
                        continue; // Too new to clean.
                    }
                }
            }
        }

        // Walk entries under the host dir, looking for empty dirs or known patterns.
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let is_known_orphan = matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("projects" | "tmp" | "sandbox" | "cache" | "session-state" | "checkpoints")
            );

            if !is_known_orphan {
                continue;
            }

            // Check age of the dir.
            let mtime = match fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
            {
                Some(t) => t,
                None => continue,
            };

            if mtime >= cutoff {
                total_skipped += 1;
                continue;
            }

            // Check if dir is empty or only contains stale content.
            let is_empty = fs::read_dir(&path)
                .map(|mut r| r.next().is_none())
                .unwrap_or(false);

            if !is_empty {
                total_skipped += 1;
                continue;
            }

            if dry_run {
                println!("  DRY-RUN: would remove empty or stale dir {}", path.display());
            } else {
                fs::remove_dir(&path).map_err(|e| e.to_string())?;
                println!("  removed empty/stale dir {}", path.display());
            }
            total_removed += 1;
        }
    }

    let summary = format!(
        "orphans: removed {total_removed} dirs, skipped {total_skipped} dirs"
    );
    if dry_run {
        println!("  DRY-RUN: {summary}");
    } else {
        eprintln!("{summary}");
    }

    Ok(())
}
