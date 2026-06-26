use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::utils::truncate_ts_chars;

pub fn sync_feedback(journal: PathBuf, feedback: PathBuf, dry_run: bool) -> anyhow::Result<()> {
    let entries = observer_rs::load_audit_journal_entries(&journal)?;

    // R51: Load existing to deduplicate
    let mut seen = HashSet::new();
    if feedback.exists() {
        let reader = BufReader::new(File::open(&feedback)?);
        for l in reader.lines().map_while(Result::ok) {
            if l.starts_with("|") {
                seen.insert(l);
            }
        }
    }

    let mut output = if !dry_run {
        Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&feedback)?,
        )
    } else {
        None
    };

    for e in entries.iter().filter(|e| e.reroute || e.struggle > 0) {
        let line = format!(
            "| {} | `{}` | `{}` | {} |",
            truncate_ts_chars(&e.ts, 10),
            e.final_skill,
            e.init,
            e.reason
        );
        if seen.contains(&line) {
            continue;
        }
        seen.insert(line.clone());
        if let Some(ref mut out) = output {
            writeln!(out, "{}", line)?;
        } else {
            println!("Dry-Run: Would sync `{}`", line);
        }
    }
    Ok(())
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "obs-sync-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn legacy_entry(ts: &str, task: &str, init: &str, skill: &str, reroute: bool, struggle: i32) -> String {
        serde_json::json!({
            "t": ts,
            "tk": task,
            "i": init,
            "f": skill,
            "r": reroute,
            "s": struggle,
            "re": ""
        })
        .to_string()
    }

    #[test]
    fn sync_feedback_writes_reroute_entries() {
        let dir = temp_dir("write");
        let journal = dir.join("journal.jsonl");
        let feedback = dir.join("feedback.md");
        let ts = chrono::Utc::now().to_rfc3339();
        let entry = legacy_entry(&ts, "test task", "none", "pdf", true, 0);
        std::fs::write(&journal, format!("{}\n", entry)).unwrap();
        sync_feedback(journal, feedback.clone(), false).unwrap();
        let content = std::fs::read_to_string(&feedback).unwrap();
        assert!(content.contains("pdf"));
        assert!(content.contains("none"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_feedback_deduplicates_existing_lines() {
        let dir = temp_dir("dedup");
        let journal = dir.join("journal.jsonl");
        let feedback = dir.join("feedback.md");
        let ts = chrono::Utc::now().to_rfc3339();
        let entry = legacy_entry(&ts, "test task", "none", "pdf", true, 0);
        std::fs::write(&journal, format!("{}\n", entry)).unwrap();
        sync_feedback(journal.clone(), feedback.clone(), false).unwrap();
        let content_before = std::fs::read_to_string(&feedback).unwrap();
        sync_feedback(journal, feedback.clone(), false).unwrap();
        let content_after = std::fs::read_to_string(&feedback).unwrap();
        assert_eq!(content_before, content_after);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_feedback_dry_run_does_not_write() {
        let dir = temp_dir("dryrun");
        let journal = dir.join("journal.jsonl");
        let feedback = dir.join("feedback.md");
        let ts = chrono::Utc::now().to_rfc3339();
        let entry = legacy_entry(&ts, "test", "none", "csv", true, 0);
        std::fs::write(&journal, format!("{}\n", entry)).unwrap();
        sync_feedback(journal, feedback.clone(), true).unwrap();
        assert!(!feedback.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_feedback_skips_non_reroute_non_struggle() {
        let dir = temp_dir("skip");
        let journal = dir.join("journal.jsonl");
        let feedback = dir.join("feedback.md");
        let ts = chrono::Utc::now().to_rfc3339();
        let entry = legacy_entry(&ts, "test", "none", "pdf", false, 0);
        std::fs::write(&journal, format!("{}\n", entry)).unwrap();
        sync_feedback(journal, feedback.clone(), false).unwrap();
        let content = std::fs::read_to_string(&feedback).unwrap();
        assert!(content.trim().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
