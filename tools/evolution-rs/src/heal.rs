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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "evo-heal-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_journal_entry(ts: &str, skill: &str, reroute: bool, struggle: i32) -> String {
        serde_json::json!({
            "t": ts,
            "tk": "test task",
            "i": "none",
            "f": skill,
            "r": reroute,
            "s": struggle,
            "re": ""
        })
        .to_string()
    }

    fn make_manifest(skills: &[&str]) -> String {
        let manifest = serde_json::json!({
            "keys": ["slug", "trigger_hints"],
            "skills": skills.iter().map(|s| vec![serde_json::Value::String(s.to_string()), serde_json::Value::String("trigger".to_string())]).collect::<Vec<_>>()
        });
        serde_json::to_string_pretty(&manifest).unwrap()
    }

    #[test]
    fn heal_prunes_inactive_skill() {
        let dir = temp_dir("prune");
        let skills_root = dir.join("skills");
        let inactive = skills_root.join("inactive-skill");
        std::fs::create_dir_all(&inactive).unwrap();
        std::fs::write(inactive.join("SKILL.md"), "content").unwrap();

        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        for _ in 0..15 {
            lines.push(make_journal_entry(&ts, "active-skill", false, 0));
        }
        std::fs::write(&journal, lines.join("\n")).unwrap();

        let manifest = dir.join("manifest.json");
        std::fs::write(&manifest, make_manifest(&["active-skill", "inactive-skill"])).unwrap();

        let cfg = EvolutionConfig::default();
        heal_skills(journal, manifest, skills_root.clone(), false, &cfg).unwrap();

        assert!(!inactive.exists());
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(".backups/pruned");
    }

    #[test]
    fn heal_dry_run_does_not_move() {
        let dir = temp_dir("dryrun");
        let skills_root = dir.join("skills");
        let inactive = skills_root.join("inactive-skill");
        std::fs::create_dir_all(&inactive).unwrap();
        std::fs::write(inactive.join("SKILL.md"), "content").unwrap();

        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        for _ in 0..15 {
            lines.push(make_journal_entry(&ts, "active-skill", false, 0));
        }
        std::fs::write(&journal, lines.join("\n")).unwrap();

        let manifest = dir.join("manifest.json");
        std::fs::write(&manifest, make_manifest(&["active-skill", "inactive-skill"])).unwrap();

        let cfg = EvolutionConfig::default();
        heal_skills(journal, manifest, skills_root.clone(), true, &cfg).unwrap();

        assert!(inactive.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn heal_keeps_active_skill() {
        let dir = temp_dir("keep-active");
        let skills_root = dir.join("skills");
        let active = skills_root.join("active-skill");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::write(active.join("SKILL.md"), "content").unwrap();

        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        for _ in 0..15 {
            lines.push(make_journal_entry(&ts, "active-skill", false, 0));
        }
        std::fs::write(&journal, lines.join("\n")).unwrap();

        let manifest = dir.join("manifest.json");
        std::fs::write(&manifest, make_manifest(&["active-skill"])).unwrap();

        let cfg = EvolutionConfig::default();
        heal_skills(journal, manifest, skills_root.clone(), false, &cfg).unwrap();

        assert!(active.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
