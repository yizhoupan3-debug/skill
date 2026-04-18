use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;
use chrono::{DateTime, Utc, Duration};
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use memmap2::Mmap;
use sha2::{Sha256, Digest};
use fs2::FileExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct JournalEntry {
    #[serde(rename = "t")]
    ts: String,
    #[serde(rename = "tk")]
    task: String,
    #[serde(rename = "i")]
    init: String,
    #[serde(rename = "f")]
    final_skill: String,
    #[serde(rename = "c", default)]
    conf: f32,
    #[serde(rename = "d", default)]
    diff: i32,
    #[serde(rename = "r", default)]
    reroute: bool,
    #[serde(rename = "s", default)]
    struggle: i32,
    #[serde(rename = "re", default)]
    reason: String,
    #[serde(rename = "ft", default)]
    failed_trigger: String,
    #[serde(rename = "n", default)]
    notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyEntry {
    ts: String,
    task: String,
    init: String,
    #[serde(rename = "final")]
    final_skill: String,
    #[serde(default)]
    conf: f32,
    #[serde(default)]
    diff: i32,
    #[serde(default)]
    reroute: bool,
    #[serde(default)]
    struggle: i32,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    failed_trigger: String,
    #[serde(default)]
    notes: String,
}

#[derive(Parser)]
#[command(name = "evolution-rs")]
#[command(about = "High performance skill evolution core", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Audit the journal and suggest repairs / new skills
    Audit {
        #[arg(short, long, default_value_t = 30)]
        days: i64,
        #[arg(short, long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Calculate health scores for all skills and output a blended manifest
    Manifest {
        #[arg(short, long)]
        journal: PathBuf,
        #[arg(long)]
        scores: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Dump all entries for a specific skill (R19)
    Dump {
        #[arg(short, long)]
        journal: PathBuf,
        #[arg(short, long)]
        skill: String,
    },
    /// Sync JSONL entries to the Markdown feedback table with deduplication
    Sync {
        #[arg(short, long)]
        journal: PathBuf,
        #[arg(short, long)]
        feedback: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a versioned snapshot of skills (R34)
    Snapshot {
        #[arg(short, long)]
        manifest: PathBuf,
        #[arg(short, long)]
        registry: PathBuf,
    },
    /// Inspect skill integrity (R37)
    Inspect {
        #[arg(short, long)]
        skill_dir: PathBuf,
    },
    /// Automatically apply pruning and merging suggestions (R46)
    Heal {
        #[arg(short, long)]
        journal: PathBuf,
        #[arg(short, long)]
        manifest: PathBuf,
        #[arg(short, long)]
        skills_root: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

fn stem(word: &str) -> String {
    let mut s = word.to_string();
    if s.len() <= 4 { return s; }
    if s.ends_with("ing") { s.truncate(s.len() - 3); }
    else if s.ends_with("ed") { s.truncate(s.len() - 2); }
    else if s.ends_with("ment") { s.truncate(s.len() - 4); }
    else if s.ends_with("s") && !s.ends_with("ss") { s.truncate(s.len() - 1); }
    s
}

fn load_entries_parallel(path: &PathBuf) -> anyhow::Result<Vec<JournalEntry>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    // R21/R24: Split by newlines and process in parallel
    let entries: Vec<JournalEntry> = mmap.as_parallel_slice()
        .par_split(|&b| b == b'\n')
        .filter_map(|line_bytes| {
            if line_bytes.is_empty() { return None; }
            let line = std::str::from_utf8(line_bytes).ok()?.trim();
            if line.is_empty() { return None; }

            if let Ok(e) = serde_json::from_str::<JournalEntry>(line) {
                Some(e)
            } else if let Ok(l) = serde_json::from_str::<LegacyEntry>(line) {
                Some(JournalEntry {
                    ts: l.ts, task: l.task, init: l.init, final_skill: l.final_skill,
                    conf: l.conf, diff: l.diff, reroute: l.reroute, struggle: l.struggle,
                    reason: l.reason, failed_trigger: l.failed_trigger, notes: l.notes,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(entries)
}

fn audit_journal(path: PathBuf, days: i64, json: bool, manifest_path: Option<PathBuf>) -> anyhow::Result<()> {
    let entries = load_entries_parallel(&path)?;
    let cutoff = Utc::now() - Duration::days(days);

    let filtered: Vec<_> = entries.iter().filter(|e| {
        if let Ok(ts) = DateTime::parse_from_rfc3339(&e.ts) {
            ts.with_timezone(&Utc) >= cutoff
        } else { true } // Accept if date parsing fails for legacy
    }).collect();

    let total = filtered.len();
    let reroutes: Vec<_> = filtered.iter().filter(|e| e.reroute).collect();
    let struggles: Vec<_> = filtered.iter().filter(|e| e.struggle > 0).collect();

    if !json {
        println!("Evolution Audit (R21 Parallel) - Core-RS");
        println!("========================================");
        println!("Total Decisions: {}", total);
        println!("Reroutes: {}", reroutes.len());
        println!("Struggles: {}", struggles.len());
    }

    // Pattern Detection (R11/R12)
    let mut ngrams: HashMap<String, i32> = HashMap::new();
    let stop_words: HashSet<&str> = ["the", "and", "for", "with", "this", "help", "how", "give", "can", "you"].iter().cloned().collect();

    for e in filtered.iter().filter(|e| e.init == "none" || e.init == "general") {
        let task_lower = e.task.to_lowercase();
        let words: Vec<String> = task_lower
            .split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
            .filter(|w| w.len() > 3 && !stop_words.contains(w.as_str()))
            .map(|w| stem(&w))
            .collect();

        for i in 0..words.len().saturating_sub(1) {
            let bi = format!("{} {}", words[i], words[i+1]);
            *ngrams.entry(bi).or_insert(0) += 1;
        }
    }

    let mut common: Vec<_> = ngrams.into_iter().collect();
    common.sort_by(|a, b| b.1.cmp(&a.1));

    let mut new_skill_candidates = Vec::new();
    for (phrase, count) in common.iter().take(10) {
        if *count >= 2 {
            new_skill_candidates.push(serde_json::json!({
                "phrase": phrase,
                "count": count,
                "suggested_name": format!("skill-{}", phrase.replace(" ", "-")),
                "reason": format!("Pattern '{}' repeated {}x.", phrase, count)
            }));
        }
    }

    if json {
        let collisions = detect_boundary_collisions(manifest_path.clone())?;
        let mut repair_suggestions = Vec::new();
        for col in &collisions {
            repair_suggestions.push(format!("Boundary conflict: {}", col));
        }

        // R29: Correlation Analysis (A -> B Reroutes)
        let mut correlations: HashMap<(String, String), i32> = HashMap::new();
        for e in reroutes.iter() {
            if !e.init.is_empty() && e.init != "none" && e.init != e.final_skill {
                *correlations.entry((e.init.clone(), e.final_skill.clone())).or_insert(0) += 1;
            }
        }
        for ((from, to), count) in correlations {
            if count >= 2 {
                repair_suggestions.push(format!("High correlation: `{}` frequently reroutes to `{}` ({}x). Consider merging or adjusting triggers.", from, to, count));
            }
        }

        // R31-33: Advanced Refactoring Suggestions
        if let Some(path) = manifest_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(skills) = manifest["skills"].as_array() {
                        let keys = manifest["keys"].as_array().unwrap();
                        let idx_slug = keys.iter().position(|k| k == "slug").unwrap();
                        let idx_trigger_hints = keys
                            .iter()
                            .position(|k| k == "trigger_hints" || k == "triggers")
                            .unwrap();

                        let active_skills: HashSet<_> = filtered.iter().map(|e| e.final_skill.as_str()).collect();

                        for s in skills {
                            let name = s[idx_slug].as_str().unwrap();
                            let triggers = row_text(&s[idx_trigger_hints]);

                            // R33: Pruning Suggestion (Zero usage)
                            if !active_skills.contains(name) && total > 5 {
                                repair_suggestions.push(format!("Pruning: Skill `{}` has zero usage in last {} days. Consider deleting.", name, days));
                            }

                            for e in filtered.iter().filter(|e| e.init == "none" || e.init == "general") {
                                let score = calculate_jaccard(&e.task, triggers);
                                if score > 0.25 {
                                     repair_suggestions.push(format!("Near-miss: Task '{}' likely belongs to `{}`, but trigger missed (Jaccard={:.2})", e.task, name, score));
                                     let task_lower = e.task.to_lowercase();
                                     let triggers_lower = triggers.to_lowercase();
                                     let keywords: Vec<_> = task_lower.split_whitespace()
                                         .filter(|w| w.len() > 4 && !triggers_lower.contains(w))
                                         .collect();
                                     if !keywords.is_empty() {
                                         repair_suggestions.push(format!("Learning: Consider adding triggers {:?} to `{}`", keywords, name));
                                     }
                                }
                            }
                        }
                    }
                }
            }
        }

        // R28: TF-IDF pseudo-logic (Simple frequency / diversity)
        let total_docs = filtered.len() as f32;
        let mut tf_idf_candidates = new_skill_candidates;
        for c in &mut tf_idf_candidates {
             let count = c["count"].as_f64().unwrap_or(1.0) as f32;
             let tf = count / total_docs;
             let idf = (total_docs / (1.0 + count)).ln();
             c["tf_idf"] = serde_json::json!(tf * idf);
        }
        tf_idf_candidates.sort_by(|a, b| b["tf_idf"].as_f64().unwrap_or(0.0).partial_cmp(&a["tf_idf"].as_f64().unwrap_or(0.0)).unwrap());

        let report = serde_json::json!({
            "total_decisions": total,
            "reroute_count": reroutes.len(),
            "struggle_count": struggles.len(),
            "new_skill_candidates": tf_idf_candidates,
            "repair_suggestions": repair_suggestions,
            "boundary_collisions": collisions,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    Ok(())
}

fn sanitize_path(path: &Path) -> anyhow::Result<()> {
    if path.to_string_lossy().contains("..") {
        anyhow::bail!("Security violation: Path contains parent directory traversal '..'");
    }
    Ok(())
}

fn snapshot_skills(manifest_path: PathBuf, registry_path: PathBuf) -> anyhow::Result<()> {
    sanitize_path(&manifest_path)?;
    sanitize_path(&registry_path)?;
    let lock_file = File::open(&manifest_path)?;
    lock_file.lock_exclusive()?; // R38: Sync Lock

    let backup_dir = PathBuf::from(".backups");
    if !backup_dir.exists() { std::fs::create_dir(&backup_dir)?; }
    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();

    let m_dest = backup_dir.join(format!("manifest_{}.json", ts));
    let r_dest = backup_dir.join(format!("registry_{}.md", ts));

    std::fs::copy(&manifest_path, m_dest)?;
    std::fs::copy(&registry_path, r_dest)?;
    println!("Snapshot created in .backups/ at {}", ts);
    Ok(())
}

fn generate_manifest(journal: PathBuf, scores_json: Option<PathBuf>, manifest_path: Option<PathBuf>) -> anyhow::Result<()> {
    let entries = load_entries_parallel(&journal)?;
    let cutoff = Utc::now() - Duration::days(30);

    let mut skill_stats: HashMap<String, (i32, i32)> = HashMap::new();
    for e in entries {
        if let Ok(ts) = DateTime::parse_from_rfc3339(&e.ts) {
            if ts.with_timezone(&Utc) < cutoff { continue; }
        }
        let s = skill_stats.entry(e.final_skill.clone()).or_insert((0, 0));
        s.0 += 1;
        if e.reroute { s.1 += 1; }
    }

    let mut static_scores: HashMap<String, f32> = HashMap::new();
    if let Some(path) = scores_json {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(skills) = payload["skills"].as_array() {
                    for entry in skills {
                        if let (Some(name), Some(total)) = (entry["name"].as_str(), entry["total"].as_f64()) {
                            static_scores.insert(name.to_string(), total as f32);
                        }
                    }
                } else if let Some(obj) = payload.as_object() {
                    for (k, v) in obj {
                        if let Some(val) = v.as_f64() {
                            static_scores.insert(k.clone(), val as f32);
                        }
                    }
                }
            }
        }
    }

    let mut all_skills = HashSet::new();
    if let Some(path) = manifest_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(skills) = payload["skills"].as_array() {
                    for row in skills {
                        if let Some(name) = row[0].as_str() {
                            all_skills.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }

    for s in skill_stats.keys() { all_skills.insert(s.clone()); }
    for s in static_scores.keys() { all_skills.insert(s.clone()); }

    let mut skills_map = HashMap::new();
    let mut critical_outliers = Vec::new();
    let mut blended_scores = Vec::new();

    for skill in all_skills {
        let (total, reroutes) = skill_stats.get(&skill).cloned().unwrap_or((0, 0));
        let dynamic_base = if total > 0 { 100.0 * (1.0 - (reroutes as f32 / total as f32)) } else { 100.0 };
        let static_score = *static_scores.get(&skill).unwrap_or(&85.0);
        let blended = (((dynamic_base * 0.6) + (static_score * 0.4)) * 10.0).round() / 10.0;

        let status = if blended >= 85.0 { "Healthy" } else if blended >= 60.0 { "Stable" } else { "Critical" };
        if blended < 60.0 { critical_outliers.push(skill.clone()); }
        blended_scores.push(blended);

        skills_map.insert(skill, serde_json::json!({
            "dynamic_score": blended,
            "static_score": (static_score * 10.0).round() / 10.0,
            "usage_30d": total,
            "reroutes_30d": reroutes,
            "health_status": status
        }));
    }

    let avg_health = if !blended_scores.is_empty() { (blended_scores.iter().sum::<f32>() / blended_scores.len() as f32 * 10.0).round() / 10.0 } else { 0.0 };

    let manifest = serde_json::json!({
        "ts": Utc::now().to_rfc3339(),
        "summary": {
            "total_skills": skills_map.len(),
            "critical_skills": critical_outliers.len(),
            "avg_health": avg_health,
        },
        "skills": skills_map,
        "critical_outliers": critical_outliers,
    });

    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}

fn dump_skill(journal: PathBuf, skill: String) -> anyhow::Result<()> {
    let entries = load_entries_parallel(&journal)?;
    println!("--- Evolution Path for Skill: `{}` ---", skill);
    let mut count = 0;
    for e in entries {
        if e.final_skill == skill {
            count += 1;
            println!("[{}] R={:5} S={} | Task: {}", &e.ts[..19], e.reroute, e.struggle, e.task);
        }
    }
    println!("--- End of Path (Found {} entries) ---", count);
    Ok(())
}

fn detect_boundary_collisions(manifest_path: Option<PathBuf>) -> anyhow::Result<Vec<String>> {
    let mut collisions = Vec::new();
    if let Some(path) = manifest_path {
        let content = std::fs::read_to_string(path)?;
        let manifest: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(skills) = manifest["skills"].as_array() {
            let keys = manifest["keys"].as_array().ok_or_else(|| anyhow::anyhow!("Invalid manifest"))?;
            let idx_slug = keys.iter().position(|k| k == "slug").unwrap_or(0);
            let idx_trigger_hints = keys
                .iter()
                .position(|k| k == "trigger_hints" || k == "triggers")
                .unwrap_or(7);

            for i in 0..skills.len() {
                for j in i+1..skills.len() {
                    let s1 = &skills[i];
                    let s2 = &skills[j];
                    let t1: HashSet<_> = row_terms(&s1[idx_trigger_hints]);
                    let t2: HashSet<_> = row_terms(&s2[idx_trigger_hints]);
                    let intersection: HashSet<_> = t1.intersection(&t2).cloned().collect();
                    if intersection.len() > 3 {
                        collisions.push(format!("`{}` & `{}` overlap: {:?}", s1[idx_slug], s2[idx_slug], intersection));
                    }
                }
            }
        }
    }
    Ok(collisions)
}

fn sync_feedback(journal: PathBuf, feedback: PathBuf, dry_run: bool) -> anyhow::Result<()> {
    let entries = load_entries_parallel(&journal)?;

    // R51: Load existing to deduplicate
    let mut seen = HashSet::new();
    if feedback.exists() {
        let reader = BufReader::new(File::open(&feedback)?);
        for line in reader.lines() {
            if let Ok(l) = line {
                if l.starts_with("|") {
                    seen.insert(l);
                }
            }
        }
    }

    let mut output = if !dry_run {
        Some(OpenOptions::new().create(true).append(true).open(&feedback)?)
    } else {
        None
    };

    for e in entries.iter().filter(|e| e.reroute || e.struggle > 0) {
        let line = format!("| {} | `{}` | `{}` | {} |", &e.ts[..10], e.final_skill, e.init, e.reason);
        if seen.insert(line.clone()) {
            if let Some(ref mut out) = output {
                writeln!(out, "{}", line)?;
            } else {
                println!("Dry-Run: Would sync `{}`", line);
            }
        }
    }
    Ok(())
}


fn calculate_jaccard(s1: &str, s2: &str) -> f32 {
    let t1: HashSet<&str> = s1.split_whitespace().collect();
    let t2: HashSet<&str> = s2.split_whitespace().collect();
    if t1.is_empty() || t2.is_empty() { return 0.0; }

    let intersection = t1.iter().filter(|&&w| t2.contains(w)).count() as f32;
    let union = (t1.len() + t2.len()) as f32 - intersection;
    intersection / union
}

fn row_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::String(text) => text.clone(),
        _ => String::new(),
    }
}

fn row_terms<'a>(value: &'a serde_json::Value) -> HashSet<&'a str> {
    match value {
        serde_json::Value::Array(items) => items.iter().filter_map(|item| item.as_str()).collect(),
        serde_json::Value::String(text) => text.split_whitespace().collect(),
        _ => HashSet::new(),
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let start = Instant::now();

    match cli.command {
        Commands::Audit { days, journal, json, manifest } => audit_journal(journal, days, json, manifest)?,
        Commands::Manifest { journal, scores, manifest } => generate_manifest(journal, scores, manifest)?,
        Commands::Dump { journal, skill } => dump_skill(journal, skill)?,
        Commands::Sync { journal, feedback, dry_run } => sync_feedback(journal, feedback, dry_run)?,
        Commands::Snapshot { manifest, registry } => snapshot_skills(manifest, registry)?,
        Commands::Inspect { skill_dir } => {
            let hash = calculate_dir_hash(&skill_dir)?;
            println!("Skill Integrity (SHA-256): {}", hash);
        }
        Commands::Heal { journal, manifest, skills_root, dry_run } => heal_skills(journal, manifest, skills_root, dry_run)?,
    }

    eprintln!("Execution completed in {:.2?}", start.elapsed());
    Ok(())
}

fn heal_skills(journal: PathBuf, manifest: PathBuf, skills_root: PathBuf, dry_run: bool) -> anyhow::Result<()> {
    let entries = load_entries_parallel(&journal)?;
    let active_skills: HashSet<&str> = entries.iter().map(|e| e.final_skill.as_str()).collect();

    let content = std::fs::read_to_string(&manifest)?;
    let manifest_val: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(skills) = manifest_val["skills"].as_array() {
        let keys = manifest_val["keys"].as_array().unwrap();
        let idx_slug = keys.iter().position(|k| k == "slug").unwrap();

        for s in skills {
            let name = s[idx_slug].as_str().unwrap();
            // R46: Automatic Pruning of Zero-usage skills
            if !active_skills.contains(name) && entries.len() > 10 {
                let skill_path = skills_root.join(name);
                if skill_path.exists() {
                    if dry_run {
                        println!("Dry-Run: Would prune inactive skill `{}`", name);
                    } else {
                        let backup_path = PathBuf::from(".backups").join("pruned").join(name);
                        std::fs::create_dir_all(backup_path.parent().unwrap())?;
                        std::fs::rename(skill_path, backup_path)?;
                        println!("Auto-Heal: Pruned inactive skill `{}`", name);
                    }
                }
            }
        }
    }
    Ok(())
}

fn calculate_dir_hash(path: &PathBuf) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    let entries = std::fs::read_dir(path)?;
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    files.sort_by_key(|e| e.path());

    for entry in files {
        if entry.file_type()?.is_file() {
            let mut file = File::open(entry.path())?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            hasher.update(&buffer);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}
