use anyhow::Context;
use chrono::Utc;
use evolution_rs::EvolutionConfig;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::utils::{manifest_skill_columns, row_terms, truncate_ts_chars};

pub fn dump_skill(journal: PathBuf, skill: String) -> anyhow::Result<()> {
    let entries = evolution_rs::load_audit_journal_entries(&journal)?;
    println!("--- Evolution Path for Skill: `{}` ---", skill);
    let mut count = 0;
    for e in entries {
        if e.final_skill == skill {
            count += 1;
            println!(
                "[{}] R={:5} S={} | Task: {}",
                truncate_ts_chars(&e.ts, 19),
                e.reroute,
                e.struggle,
                e.task
            );
        }
    }
    println!("--- End of Path (Found {} entries) ---", count);
    Ok(())
}

pub fn detect_boundary_collisions(
    manifest_path: Option<PathBuf>,
    cfg: &EvolutionConfig,
) -> anyhow::Result<Vec<String>> {
    let mut collisions = Vec::new();
    if let Some(path) = manifest_path {
        let content = std::fs::read_to_string(path)?;
        let manifest: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(cols) = manifest_skill_columns(&manifest) {
            let skill_terms: Vec<(&str, HashSet<&str>)> = cols
                .skills
                .iter()
                .filter_map(|s| {
                    let slug = s.get(cols.idx_slug)?.as_str()?;
                    let hints = s.get(cols.idx_trigger_hints)?;
                    Some((slug, row_terms(hints)))
                })
                .collect();

            for i in 0..skill_terms.len() {
                for j in i + 1..skill_terms.len() {
                    let (s1_slug, t1) = &skill_terms[i];
                    let (s2_slug, t2) = &skill_terms[j];
                    let shared: Vec<_> = t1.iter().filter(|w| t2.contains(*w)).collect();
                    if shared.len() >= cfg.thresholds.boundary_collision_min_overlap {
                        collisions.push(format!(
                            "`{}` & `{}` overlap: {:?}",
                            s1_slug, s2_slug, shared
                        ));
                    }
                }
            }
        }
    }
    Ok(collisions)
}

pub fn sanitize_path(path: &Path) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolved = if path.exists() {
        std::fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?
    } else {
        let mut components = Vec::new();
        for comp in path.components() {
            match comp {
                std::path::Component::ParentDir => {
                    anyhow::bail!(
                        "Security violation: Path contains parent directory traversal '{}'",
                        path.display()
                    );
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    anyhow::bail!(
                        "Security violation: Absolute path {} escapes working directory",
                        path.display()
                    );
                }
                std::path::Component::CurDir => continue,
                std::path::Component::Normal(c) => components.push(c),
            }
        }
        let mut result = PathBuf::new();
        for comp in components {
            result.push(comp);
        }
        cwd.join(result)
    };
    let canonical_cwd = std::fs::canonicalize(&cwd)
        .with_context(|| format!("Failed to canonicalize CWD: {}", cwd.display()))?;
    if !resolved.starts_with(&canonical_cwd) {
        anyhow::bail!(
            "Security violation: Path {} resolves outside working directory",
            path.display()
        );
    }
    Ok(())
}

pub fn snapshot_skills(manifest_path: PathBuf, registry_path: PathBuf) -> anyhow::Result<()> {
    sanitize_path(&manifest_path)?;
    sanitize_path(&registry_path)?;
    let lock_file = File::open(&manifest_path)?;
    lock_file.lock_exclusive()?; // R38: Sync Lock

    let backup_dir = PathBuf::from(".backups");
    std::fs::create_dir_all(&backup_dir)?;
    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();

    let m_dest = backup_dir.join(format!("manifest_{}.json", ts));
    let r_dest = backup_dir.join(format!("registry_{}.md", ts));

    std::fs::copy(&manifest_path, m_dest)?;
    std::fs::copy(&registry_path, r_dest)?;
    println!("Snapshot created in .backups/ at {}", ts);
    Ok(())
}

pub fn calculate_dir_hash(path: &PathBuf) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    hash_dir_recursive(path, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "evo-inspect-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_boundary_collisions_no_overlap() {
        let dir = temp_dir("no-overlap");
        let manifest_path = dir.join("manifest.json");
        let manifest = serde_json::json!({
            "keys": ["slug", "trigger_hints"],
            "skills": [
                ["skill-a", "alpha bravo charlie"],
                ["skill-b", "delta echo foxtrot"]
            ]
        });
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let cfg = EvolutionConfig::default();
        let collisions =
            detect_boundary_collisions(Some(manifest_path), &cfg).unwrap();
        assert!(collisions.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detect_boundary_collisions_with_overlap() {
        let dir = temp_dir("overlap");
        let manifest_path = dir.join("manifest.json");
        let manifest = serde_json::json!({
            "keys": ["slug", "trigger_hints"],
            "skills": [
                ["skill-a", "alpha bravo charlie delta echo"],
                ["skill-b", "bravo charlie delta echo foxtrot"]
            ]
        });
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let cfg = EvolutionConfig::default();
        let collisions =
            detect_boundary_collisions(Some(manifest_path), &cfg).unwrap();
        assert!(!collisions.is_empty());
        assert!(collisions[0].contains("overlap"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detect_boundary_collisions_none_path() {
        let cfg = EvolutionConfig::default();
        let collisions = detect_boundary_collisions(None, &cfg).unwrap();
        assert!(collisions.is_empty());
    }

    #[test]
    fn sanitize_path_rejects_parent_traversal() {
        let result = sanitize_path(Path::new("../etc/passwd"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Security violation"));
    }

    #[test]
    fn sanitize_path_rejects_absolute_path() {
        let result = sanitize_path(Path::new("/etc/passwd"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Security violation"));
    }

    #[test]
    fn sanitize_path_accepts_relative_within_cwd() {
        let dir = temp_dir("sanitize-ok");
        let sub = dir.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        let saved = std::env::current_dir().ok();
        let _ = std::env::set_current_dir(&dir);
        let result = sanitize_path(Path::new("subdir"));
        if let Some(old) = saved {
            let _ = std::env::set_current_dir(old);
        }
        assert!(result.is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn calculate_dir_hash_is_deterministic() {
        let dir = temp_dir("hash-det");
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        std::fs::write(dir.join("b.txt"), "world").unwrap();
        let h1 = calculate_dir_hash(&dir).unwrap();
        let h2 = calculate_dir_hash(&dir).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn calculate_dir_hash_changes_with_content() {
        let dir1 = temp_dir("hash-chg1");
        let dir2 = temp_dir("hash-chg2");
        std::fs::write(dir1.join("f.txt"), "aaa").unwrap();
        std::fs::write(dir2.join("f.txt"), "bbb").unwrap();
        let h1 = calculate_dir_hash(&dir1).unwrap();
        let h2 = calculate_dir_hash(&dir2).unwrap();
        assert_ne!(h1, h2);
        let _ = std::fs::remove_dir_all(dir1);
        let _ = std::fs::remove_dir_all(dir2);
    }
}

fn hash_dir_recursive(path: &Path, hasher: &mut Sha256) -> anyhow::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let entry_path = entry.path();
        if entry.file_type()?.is_file() {
            let mut file = File::open(&entry_path)?;
            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
            }
        } else if entry.file_type()?.is_dir() {
            hash_dir_recursive(&entry_path, hasher)?;
        }
    }
    Ok(())
}
