use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

use research_log_rs::cli::{Cli, Command};
use research_log_rs::db;
use research_log_rs::models::{
    Confidence, EntryPoint, ExplorationInsight, ExplorationLog,
};
use research_log_rs::text_layer;
use research_log_rs::ARTIFACTS_LOG_DIR;

fn main() -> Result<()> {
    let cli: Cli = clap::Parser::parse();
    let log_root = PathBuf::from(ARTIFACTS_LOG_DIR);

    match cli.command {
        Command::Record {
            direction,
            question,
            entry_point,
            barrier_id,
        } => {
            let now = Utc::now();
            let log_id = uuid::Uuid::new_v4().to_string();
            let timestamp = now.to_rfc3339();

            // Write text layer
            let file_path = text_layer::write_daily_log(
                &log_root,
                &now.date_naive(),
                &direction,
                &question,
                &log_id,
            )
            .context("write text layer")?;
            text_layer::update_index(&log_root, &direction, &now.date_naive())?;

            // Write DB layer
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;
            let log = ExplorationLog {
                id: log_id.clone(),
                direction,
                question,
                entry_point: EntryPoint::from_str(&entry_point),
                barrier_id,
                key_findings: String::new(),
                open_questions: String::new(),
                created_at: timestamp.clone(),
                updated_at: timestamp,
            };
            db::insert_log(&conn, &log)?;

            println!("Recorded: {} → {}", log_id, file_path.display());
        }

        Command::Search { query, limit } => {
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;
            let results = db::search_logs(&conn, &query, limit)?;

            if results.is_empty() {
                println!("No results for: {}", query);
            } else {
                for r in &results {
                    println!(
                        "[{}] {}: {} (score: {:.2})",
                        r.id, r.direction, r.snippet, r.score
                    );
                }
            }
        }

        Command::Insight {
            log_id,
            text,
            confidence,
        } => {
            let now = Utc::now();
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;

            let insight = ExplorationInsight {
                id: uuid::Uuid::new_v4().to_string(),
                log_id: log_id.clone(),
                text: text.clone(),
                confidence: Confidence::from_str(&confidence),
                cross_refs: vec![],
                created_at: now.to_rfc3339(),
            };
            db::insert_insight(&conn, &insight)?;

            // Also append to text layer if file exists
            let insight_path = log_root.join("insights").join(format!("{}.md", log_id));
            if let Some(parent) = insight_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(
                &insight_path,
                format!("# Insight ({})\n\n{} [confidence: {}]\n", log_id, text, confidence),
            )
            .ok();

            println!("Insight added to log: {}", log_id);
        }

        Command::Connect {
            log_id_a,
            log_id_b,
            relation: _,
        } => {
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;
            db::connect_logs(&conn, &log_id_a, &log_id_b)?;
            println!("Connected: {} ↔ {}", log_id_a, log_id_b);
        }

        Command::Barrier { loop_id } => {
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;
            let reports = db::list_barrier_reports(&conn, loop_id.as_deref())?;

            if reports.is_empty() {
                println!("No barrier reports found.");
            } else {
                for r in &reports {
                    println!(
                        "[{}] barrier={} loop={:?} path={}",
                        r.id, r.barrier_id, r.loop_id, r.report_path
                    );
                }
            }
        }

        Command::Route { barrier_id } => {
            let db_path = log_root.join("research-log.db");
            let conn = db::init_database(&db_path)?;
            let results = db::trace_barrier_route(&conn, &barrier_id)?;

            if results.is_empty() {
                println!("No research path found for barrier: {}", barrier_id);
            } else {
                for r in &results {
                    println!(
                        "[{}] {} → {} (snippet: {})",
                        r.id, r.direction, r.created_at, r.snippet
                    );
                }
            }
        }
    }

    Ok(())
}
