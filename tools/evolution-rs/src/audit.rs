use chrono::{Duration, Utc};
use evolution_rs::EvolutionConfig;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::inspect::detect_boundary_collisions;
use crate::utils::{
    STOP_WORDS, calculate_jaccard, entry_is_recent, manifest_skill_columns, row_text, stem,
};

pub fn audit_journal(
    path: PathBuf,
    days: i64,
    json: bool,
    manifest_path: Option<PathBuf>,
    cfg: &EvolutionConfig,
) -> anyhow::Result<()> {
    let entries = evolution_rs::load_audit_journal_entries(&path)?;
    let cutoff = Utc::now() - Duration::days(days);

    let filtered: Vec<_> = entries
        .iter()
        .filter(|e| entry_is_recent(e, cutoff))
        .collect();

    let total = filtered.len();
    let reroute_count = filtered.iter().filter(|e| e.reroute).count();
    let struggle_count = filtered.iter().filter(|e| e.struggle > 0).count();

    if !json {
        println!("Evolution Audit (R21 Parallel) - Core-RS");
        println!("========================================");
        println!("Total Decisions: {}", total);
        println!("Reroutes: {}", reroute_count);
        println!("Struggles: {}", struggle_count);
    }

    // Pattern Detection (R11/R12)
    let mut ngrams: HashMap<String, i32> = HashMap::new();
    let mut ngram_doc_freq: HashMap<String, i32> = HashMap::new();

    for e in filtered
        .iter()
        .filter(|e| e.init == "none" || e.init == "general")
    {
        let task_lower = e.task.to_lowercase();
        let words: Vec<String> = task_lower
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|w| {
                w.len() >= cfg.thresholds.min_word_length && !STOP_WORDS.contains(w.as_str())
            })
            .map(|w| stem(&w))
            .collect();

        let mut seen_in_entry: HashSet<String> = HashSet::new();
        for i in 0..words.len().saturating_sub(1) {
            let bi = format!("{} {}", words[i], words[i + 1]);
            *ngrams.entry(bi.clone()).or_insert(0) += 1;
            if seen_in_entry.insert(bi.clone()) {
                *ngram_doc_freq.entry(bi).or_insert(0) += 1;
            }
        }
    }

    let mut common: Vec<_> = ngrams.into_iter().collect();
    common.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

    let mut new_skill_candidates = Vec::new();
    for (phrase, count) in common.iter().take(cfg.audit.top_ngram_candidates) {
        if *count >= cfg.evolution.min_candidate_frequency {
            new_skill_candidates.push(serde_json::json!({
                "phrase": phrase,
                "count": count,
                "suggested_name": format!("skill-{}", phrase.replace(" ", "-")),
                "reason": format!("Pattern '{}' repeated {}x.", phrase, count)
            }));
        }
    }

    if json {
        let collisions = detect_boundary_collisions(manifest_path.clone(), cfg)?;
        let mut repair_suggestions = Vec::new();
        for col in &collisions {
            repair_suggestions.push(format!("Boundary conflict: {}", col));
        }

        // R29: Correlation Analysis (A -> B Reroutes)
        let mut correlations: HashMap<(String, String), i32> = HashMap::new();
        for e in filtered.iter().filter(|e| e.reroute) {
            if !e.init.is_empty() && e.init != "none" && e.init != e.final_skill {
                *correlations
                    .entry((e.init.clone(), e.final_skill.clone()))
                    .or_insert(0) += 1;
            }
        }
        for ((from, to), count) in correlations {
            if count >= cfg.thresholds.min_correlation_count {
                repair_suggestions.push(format!("High correlation: `{}` frequently reroutes to `{}` ({}x). Consider merging or adjusting triggers.", from, to, count));
            }
        }

        // R31-33: Advanced Refactoring Suggestions
        if let Some(path) = manifest_path
            && let Ok(content) = std::fs::read_to_string(path)
                && let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content)
                    && let Some(cols) = manifest_skill_columns(&manifest)
                    {
                        let active_skills: HashSet<_> =
                            filtered.iter().map(|e| e.final_skill.as_str()).collect();

                        let none_general_entries: Vec<_> = filtered
                            .iter()
                            .filter(|e| e.init == "none" || e.init == "general")
                            .collect();

                        for s in cols.skills {
                            let Some(name) = s.get(cols.idx_slug).and_then(|value| value.as_str())
                            else {
                                continue;
                            };
                            let triggers = row_text(&s[cols.idx_trigger_hints]);

                            // R33: Pruning Suggestion (Zero usage)
                            if !active_skills.contains(name)
                                && total >= cfg.thresholds.min_usage_for_pruning_hint
                            {
                                repair_suggestions.push(format!("Pruning: Skill `{}` has zero usage in last {} days. Consider deleting.", name, days));
                            }

                            let triggers_lower = triggers.to_lowercase();
                            for e in &none_general_entries {
                                let score = calculate_jaccard(&e.task, &triggers);
                                if score > cfg.thresholds.jaccard_near_match {
                                    repair_suggestions.push(format!("Near-miss: Task '{}' likely belongs to `{}`, but trigger missed (Jaccard={:.2})", e.task, name, score));
                                    let task_lower = e.task.to_lowercase();
                                    let keywords: Vec<_> = task_lower
                                        .split_whitespace()
                                        .filter(|w| w.len() > 4 && !triggers_lower.contains(w))
                                        .collect();
                                    if !keywords.is_empty() {
                                        repair_suggestions.push(format!(
                                            "Learning: Consider adding triggers {:?} to `{}`",
                                            keywords, name
                                        ));
                                    }
                                }
                            }
                        }
                    }

        // R28: TF-IDF
        let total_docs = filtered.len() as f32;
        let mut tf_idf_candidates = new_skill_candidates;
        for c in &mut tf_idf_candidates {
            let phrase = c["phrase"].as_str().unwrap_or("");
            let total_freq = c["count"].as_f64().unwrap_or(1.0) as f32;
            let df = *ngram_doc_freq.get(phrase).unwrap_or(&1) as f32;
            let tf = total_freq / total_docs;
            let idf = (total_docs / (1.0 + df)).ln();
            c["tf_idf"] = serde_json::json!(tf * idf);
        }
        tf_idf_candidates.sort_by(|a, b| {
            b["tf_idf"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&a["tf_idf"].as_f64().unwrap_or(0.0))
        });

        let report = serde_json::json!({
            "total_decisions": total,
            "reroute_count": reroute_count,
            "struggle_count": struggle_count,
            "new_skill_candidates": tf_idf_candidates,
            "repair_suggestions": repair_suggestions,
            "boundary_collisions": collisions,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "evo-audit-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_entry(ts: &str, task: &str, init: &str, skill: &str, reroute: bool, struggle: i32) -> String {
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
    fn audit_journal_runs_with_no_entries() {
        let dir = temp_dir("empty");
        let journal = dir.join("journal.jsonl");
        std::fs::write(&journal, "").unwrap();
        let cfg = EvolutionConfig::default();
        audit_journal(journal, 30, false, None, &cfg).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_journal_ngram_extraction() {
        let dir = temp_dir("ngram");
        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        for _ in 0..5 {
            lines.push(make_entry(&ts, "convert pdf to csv", "none", "pdf", true, 0));
        }
        std::fs::write(&journal, lines.join("\n")).unwrap();
        let cfg = EvolutionConfig::default();
        audit_journal(journal, 30, true, None, &cfg).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_journal_correlation_analysis() {
        let dir = temp_dir("corr");
        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        for _ in 0..5 {
            lines.push(make_entry(&ts, "test task", "skill-a", "skill-b", true, 0));
        }
        std::fs::write(&journal, lines.join("\n")).unwrap();
        let cfg = EvolutionConfig::default();
        audit_journal(journal, 30, true, None, &cfg).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_journal_struggle_count() {
        let dir = temp_dir("struggle");
        let journal = dir.join("journal.jsonl");
        let ts = chrono::Utc::now().to_rfc3339();
        let mut lines = Vec::new();
        lines.push(make_entry(&ts, "task1", "none", "pdf", false, 3));
        lines.push(make_entry(&ts, "task2", "none", "csv", false, 0));
        lines.push(make_entry(&ts, "task3", "none", "pdf", true, 1));
        std::fs::write(&journal, lines.join("\n")).unwrap();
        let cfg = EvolutionConfig::default();
        audit_journal(journal, 30, true, None, &cfg).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stem_reduces_words() {
        assert_eq!(crate::utils::stem("running"), "run");
        assert_eq!(crate::utils::stem("created"), "creat");
        assert_eq!(crate::utils::stem("assessment"), "assess");
        assert_eq!(crate::utils::stem("cats"), "cat");
        assert_eq!(crate::utils::stem("ab"), "ab");
        assert_eq!(crate::utils::stem("the"), "the");
    }

    #[test]
    fn jaccard_similar_strings_high_score() {
        let score = crate::utils::calculate_jaccard(
            "convert pdf to csv",
            "convert pdf to csv format",
        );
        assert!(score > 0.5);
    }

    #[test]
    fn jaccard_different_strings_low_score() {
        let score = crate::utils::calculate_jaccard("hello world", "foo bar baz");
        assert!(score < 0.1);
    }
}
