//! Clean operations: rust target dirs, hook-state files, orphan directories.
//!
//! All clean operations support `--dry-run` to preview changes before execution.
//! Symlink targets are skipped to avoid following bind mounts into unexpected locations.

use std::fs;
use std::path::Path;

use framework_kernel::runtime_registry::ALL_KNOWN_HOST_DIRS;

pub(super) fn clean_rust_target_dirs(repo_root: &Path, dry_run: bool) -> Result<(), String> {
    clean_targets_walk(repo_root, dry_run)?;
    Ok(())
}

fn clean_targets_walk(path: &Path, dry_run: bool) -> Result<(), String> {
    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
        return Ok(());
    }
    // Skip symlinks to avoid following bind mounts or symlinks into unexpected locations
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

        if files_to_clean.is_empty() {
            total_kept += kept;
            continue;
        }

        println!(
            "[{host_dir}] Found {} hook-state file(s) older than {} days ({} kept):",
            files_to_clean.len(),
            ttl_days,
            kept
        );

        for (path, mtime) in &files_to_clean {
            let age_days = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - mtime)
                / 86400;
            println!("  - {} ({} days old)", path.display(), age_days);
        }

        if !dry_run {
            for (path, _) in &files_to_clean {
                if let Err(e) = fs::remove_file(path) {
                    eprintln!("  Failed to delete {}: {}", path.display(), e);
                }
            }
        }

        total_cleaned += files_to_clean.len();
        total_kept += kept;
    }

    if total_cleaned == 0 {
        println!(
            "No hook-state files older than {} days across all host dirs. {} files kept.",
            ttl_days, total_kept
        );
    } else if dry_run {
        println!("\nDry-run mode: {total_cleaned} file(s) would be deleted ({total_kept} kept).");
    } else {
        println!("Done. {total_cleaned} hook-state file(s) deleted ({total_kept} kept).");
    }

    Ok(())
}

/// Clean orphan task directories not referenced by any pointer or registry.
pub(super) fn clean_orphan_directories(
    repo_root: &Path,
    dry_run: bool,
    ttl_days: u64,
) -> Result<(), String> {
    let current_dir = repo_root.join("artifacts/current");

    if !current_dir.is_dir() {
        println!("No artifacts/current directory found. Nothing to clean.");
        return Ok(());
    }

    // Gather referenced task IDs from registry
    let mut referenced_ids = std::collections::HashSet::new();

    // From task_registry.json
    let registry_path = current_dir.join("task_registry.json");
    if registry_path.is_file()
        && let Ok(raw) = fs::read_to_string(&registry_path)
            && let Ok(registry) = serde_json::from_str::<serde_json::Value>(&raw)
                && let Some(tasks) = registry.get("tasks").and_then(|t| t.as_array()) {
                    for task in tasks {
                        if let Some(id) = task.get("task_id").and_then(|v| v.as_str()) {
                            referenced_ids.insert(id.to_string());
                        }
                    }
                }

    // Find orphan directories
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs()
        .saturating_sub(ttl_days * 24 * 60 * 60);

    let mut orphans: Vec<(std::path::PathBuf, u64)> = Vec::new();

    let entries = fs::read_dir(&current_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Skip known non-task items
        if name.starts_with('.') {
            continue;
        }

        // Skip if referenced by any pointer or registry
        if referenced_ids.contains(name) {
            continue;
        }

        let mtime = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if mtime < cutoff {
            orphans.push((path, mtime));
        }
    }

    if orphans.is_empty() {
        println!("No orphan task directories found older than {ttl_days} days.");
        return Ok(());
    }

    println!(
        "Found {} orphan task director{} older than {ttl_days} days:",
        orphans.len(),
        if orphans.len() == 1 { "y" } else { "ies" }
    );

    for (path, mtime) in &orphans {
        let age_days = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - mtime)
            / 86400;
        println!("  - {} ({} days old)", path.display(), age_days);
    }

    if dry_run {
        println!("\nDry-run: {} orphan director{} would be removed.", orphans.len(),
            if orphans.len() == 1 { "y" } else { "ies" });
    } else {
        for (path, _) in &orphans {
            if let Err(e) = fs::remove_dir_all(path) {
                eprintln!("  Failed to remove {}: {}", path.display(), e);
            } else {
                println!("  removed {}", path.display());
            }
        }
        println!("Done. {} orphan director{} removed.", orphans.len(),
            if orphans.len() == 1 { "y" } else { "ies" });
    }

    Ok(())
}
