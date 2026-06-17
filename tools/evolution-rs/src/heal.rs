use chrono::{Duration, Utc};
use evolution_rs::EvolutionConfig;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::utils::{entry_is_recent, manifest_skill_slug_column};

pub fn heal_skills(
    journal: PathBuf,
    manifest: PathBuf,
    skills_root: PathBuf,
    dry_run: bool,
    cfg: &EvolutionConfig,
) -> anyhow::Result<()> {
    let entries = evolution_rs::load_audit_journal_entries(&journal)?;
    let cutoff = Utc::now() - Duration::days(cfg.evolution.audit_window_days);

    let recent_entries: Vec<_> = entries
        .iter()
        .filter(|e| entry_is_recent(e, cutoff))
        .collect();

    let active_skills: HashSet<&str> = recent_entries.iter().map(|e| e.final_skill.as_str()).collect();

    let content = std::fs::read_to_string(&manifest)?;
    let manifest_val: serde_json::Value = serde_json::from_str(&content)?;

    if let Some((skills, idx_slug)) = manifest_skill_slug_column(&manifest_val) {
        for s in skills {
            let Some(name) = s.get(idx_slug).and_then(|value| value.as_str()) else {
                continue;
            };
            if !active_skills.contains(name) && recent_entries.len() >= cfg.thresholds.min_entries_for_heal
            {
                let skill_path = skills_root.join(name);
                if skill_path.exists() {
                    if dry_run {
                        println!("Dry-Run: Would prune inactive skill `{}`", name);
                    } else {
                        let backup_path = PathBuf::from(".backups").join("pruned").join(name);
                        let backup_parent = backup_path
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("backup path has no parent"))?;
                        std::fs::create_dir_all(backup_parent)?;
                        std::fs::rename(skill_path, backup_path)?;
                        println!("Auto-Heal: Pruned inactive skill `{}`", name);
                    }
                }
            }
        }
    }
    Ok(())
}
