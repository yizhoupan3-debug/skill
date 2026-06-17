mod audit;
mod heal;
mod inspect;
mod manifest;
mod sync;
mod utils;

use clap::{Parser, Subcommand};
use evolution_rs::{
    default_config_path, default_evolution_output_dir, default_telemetry_journal_path, load_config,
    run_analyze, run_health_score,
};
use std::path::PathBuf;
use std::time::Instant;

use audit::audit_journal;
use heal::heal_skills;
use inspect::{calculate_dir_hash, dump_skill, snapshot_skills};
use manifest::generate_manifest;
use sync::sync_feedback;
#[allow(unused_imports)]
use utils::{
    calculate_jaccard, canonical_skill_name, entry_is_recent, load_entries, stem,
    truncate_ts_chars,
};

use evolution_rs::EvolutionConfig;

#[derive(Parser)]
#[command(name = "evolution-rs")]
#[command(about = "High performance skill evolution core", long_about = None)]
struct Cli {
    /// TOML threshold config (defaults when omitted).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

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
        #[arg(long, default_value_t = 30)]
        days: i64,
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
    /// Analyze telemetry journal and write artifacts/evolution/analysis.json
    Analyze {
        #[arg(short, long, default_value = default_telemetry_journal_path())]
        journal: PathBuf,
        #[arg(short, long, default_value = default_evolution_output_dir())]
        output_dir: PathBuf,
        #[arg(short, long, default_value_t = 30)]
        days: i64,
    },
    /// Compute per-skill health scores from telemetry journal
    HealthScore {
        #[arg(short, long, default_value = default_telemetry_journal_path())]
        journal: PathBuf,
        #[arg(short, long, default_value = default_evolution_output_dir())]
        output_dir: PathBuf,
    },
}

fn resolve_window(days: i64, cfg: &EvolutionConfig) -> i64 {
    if days == 30 {
        cfg.evolution.audit_window_days
    } else {
        days
    }
}

fn resolve_config(cli: &Cli) -> anyhow::Result<EvolutionConfig> {
    let path = cli
        .config
        .clone()
        .or_else(|| std::env::var("EVOLUTION_RS_CONFIG").ok().map(PathBuf::from))
        .or_else(|| {
            let default = PathBuf::from(default_config_path());
            if default.is_file() {
                Some(default)
            } else {
                None
            }
        });
    load_config(path.as_deref())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = resolve_config(&cli)?;
    let start = Instant::now();

    match cli.command {
        Commands::Audit {
            days,
            journal,
            json,
            manifest,
        } => {
            let window = resolve_window(days, &cfg);
            audit_journal(journal, window, json, manifest, &cfg)?;
        }
        Commands::Manifest {
            journal,
            scores,
            manifest,
            days,
        } => {
            let window = resolve_window(days, &cfg);
            generate_manifest(journal, scores, manifest, window, &cfg)?;
        }
        Commands::Dump { journal, skill } => dump_skill(journal, skill)?,
        Commands::Sync {
            journal,
            feedback,
            dry_run,
        } => sync_feedback(journal, feedback, dry_run)?,
        Commands::Snapshot { manifest, registry } => snapshot_skills(manifest, registry)?,
        Commands::Inspect { skill_dir } => {
            let hash = calculate_dir_hash(&skill_dir)?;
            println!("Skill Integrity (SHA-256): {}", hash);
        }
        Commands::Heal {
            journal,
            manifest,
            skills_root,
            dry_run,
        } => heal_skills(journal, manifest, skills_root, dry_run, &cfg)?,
        Commands::Analyze {
            journal,
            output_dir,
            days,
        } => {
            let window = resolve_window(days, &cfg);
            let out = run_analyze(&journal, &output_dir, window, &cfg)?;
            println!("Wrote {}", out.display());
        }
        Commands::HealthScore {
            journal,
            output_dir,
        } => {
            let out = run_health_score(&journal, &output_dir, &cfg)?;
            println!("Wrote {}", out.display());
        }
    }

    eprintln!("Execution completed in {:.2?}", start.elapsed());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashSet;

    #[test]
    fn missing_journal_loads_as_empty_entries() {
        let missing = std::env::temp_dir().join(format!(
            "evolution-rs-missing-journal-{}-{}.jsonl",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));

        let entries = load_entries(&missing).expect("missing journal should be empty");

        assert!(entries.is_empty());
    }

    #[test]
    fn truncate_ts_chars_handles_short_and_utf8() {
        assert_eq!(truncate_ts_chars("abc", 19), "abc");
        let s = "α".repeat(8);
        assert_eq!(truncate_ts_chars(&s, 3), "ααα");
    }

    #[test]
    fn stem_basic_suffixes() {
        assert_eq!(stem("running"), "run");
        assert_eq!(stem("testing"), "test");
        assert_eq!(stem("building"), "build");
        assert_eq!(stem("refactored"), "refactor");
        assert_eq!(stem("experiment"), "experi");
        assert_eq!(stem("cats"), "cat");
        assert_eq!(stem("glass"), "glass");
    }

    #[test]
    fn stem_short_words_unchanged() {
        assert_eq!(stem("the"), "the");
        assert_eq!(stem("ab"), "ab");
        assert_eq!(stem("a"), "a");
    }

    #[test]
    fn stem_minimum_root_length() {
        assert_eq!(stem("zing"), "zing");
        assert_eq!(stem("test"), "test");
    }

    #[test]
    fn canonical_skill_name_filters_invalid() {
        let known: HashSet<String> = ["pdf".into(), "csv".into()].into_iter().collect();
        assert_eq!(canonical_skill_name("", &known), None);
        assert_eq!(canonical_skill_name("none", &known), None);
        assert_eq!(canonical_skill_name("general", &known), None);
        assert_eq!(canonical_skill_name("  ", &known), None);
    }

    #[test]
    fn canonical_skill_name_exact_match() {
        let known: HashSet<String> = ["pdf".into(), "csv".into()].into_iter().collect();
        assert_eq!(canonical_skill_name("pdf", &known), Some("pdf".into()));
    }

    #[test]
    fn canonical_skill_name_multi_delimiter() {
        let known: HashSet<String> = ["pdf".into(), "csv".into()].into_iter().collect();
        assert_eq!(
            canonical_skill_name("pdf+csv+other", &known),
            Some("pdf".into())
        );
        assert_eq!(
            canonical_skill_name("other,pdf,more", &known),
            Some("pdf".into())
        );
        assert_eq!(
            canonical_skill_name("csv|pdf", &known),
            Some("csv".into())
        );
    }

    #[test]
    fn calculate_jaccard_case_insensitive() {
        let score = calculate_jaccard("PDF task", "pdf task");
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn calculate_jaccard_empty_strings() {
        assert_eq!(calculate_jaccard("", "hello"), 0.0);
        assert_eq!(calculate_jaccard("hello", ""), 0.0);
        assert_eq!(calculate_jaccard("", ""), 0.0);
    }

    #[test]
    fn calculate_jaccard_identical() {
        assert!((calculate_jaccard("hello world", "hello world") - 1.0).abs() < 0.001);
    }

    #[test]
    fn calculate_jaccard_disjoint() {
        assert_eq!(calculate_jaccard("abc def", "xyz uvw"), 0.0);
    }

    #[test]
    fn calculate_jaccard_partial_overlap() {
        let score = calculate_jaccard("a b c", "b c d");
        assert!((score - 0.5).abs() < 0.001);
    }

    #[test]
    fn calculate_jaccard_punctuation_normalized() {
        let score = calculate_jaccard("hello, world", "hello world");
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn entry_is_recent_rejects_malformed_ts() {
        let entry = evolution_rs::AuditJournalEntry {
            ts: "not-a-date".into(),
            ..Default::default()
        };
        let cutoff = Utc::now() - chrono::Duration::days(30);
        assert!(!entry_is_recent(&entry, cutoff));
    }

    #[test]
    fn entry_is_recent_accepts_recent_entry() {
        let entry = evolution_rs::AuditJournalEntry {
            ts: Utc::now().to_rfc3339(),
            ..Default::default()
        };
        let cutoff = Utc::now() - chrono::Duration::days(30);
        assert!(entry_is_recent(&entry, cutoff));
    }

    #[test]
    fn entry_is_recent_rejects_old_entry() {
        let entry = evolution_rs::AuditJournalEntry {
            ts: (Utc::now() - chrono::Duration::days(60)).to_rfc3339(),
            ..Default::default()
        };
        let cutoff = Utc::now() - chrono::Duration::days(30);
        assert!(!entry_is_recent(&entry, cutoff));
    }
}
