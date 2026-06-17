use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::utils::truncate_ts_chars;

pub fn sync_feedback(journal: PathBuf, feedback: PathBuf, dry_run: bool) -> anyhow::Result<()> {
    let entries = evolution_rs::load_audit_journal_entries(&journal)?;

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
