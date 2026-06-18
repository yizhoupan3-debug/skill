use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

use research_log_rs::cli::{Cli, Command, ExportFormat};
use research_log_rs::db;
use research_log_rs::models;
use research_log_rs::models::*;
use research_log_rs::text_layer;

fn main() -> Result<()> {
    let cli = <Cli as clap::Parser>::parse();
    let log_root = PathBuf::from("artifacts/research-log");

    match cli.command {
        Command::Record {
            direction,
            question,
            entry_point,
            barrier_id,
            importance,
            tags,
        } => cmd_record(&log_root, &direction, &question, &entry_point, barrier_id, importance, tags)?,

        Command::AddFinding {
            entry_id,
            kind,
            content,
            confidence,
        } => cmd_add_finding(&log_root, &entry_id, &kind, &content, confidence)?,

        Command::Search {
            query,
            direction,
            status,
            date_from,
            date_to,
            limit,
        } => cmd_search(&log_root, &query, direction.as_deref(), status.as_deref(), date_from.as_deref(), date_to.as_deref(), limit)?,

        Command::SearchFindings { query, kind, limit } => {
            cmd_search_findings(&log_root, &query, kind.as_deref(), limit)?
        }

        Command::Render {
            entry_id,
            write,
            output,
        } => cmd_render(&log_root, &entry_id, write, output)?,

        Command::Status => cmd_status(&log_root)?,

        Command::Consolidate => cmd_consolidate(&log_root)?,

        Command::Export { format, output } => cmd_export(&log_root, &format, output)?,

        Command::Connect {
            log_id_a,
            log_id_b,
            relation,
            notes,
        } => cmd_connect(&log_root, &log_id_a, &log_id_b, relation, notes)?,

        Command::Barrier { loop_id } => cmd_barrier(&log_root, loop_id)?,
    }
    Ok(())
}

fn cmd_record(
    log_root: &Path,
    direction: &str,
    question: &str,
    entry_point: &str,
    barrier_id: Option<String>,
    importance: i32,
    tags_opt: Option<String>,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    let conn = db::init_database(&db_path)?;

    let now = Utc::now();
    let log_id = format!("rl-{}", now.format("%Y%m%d%H%M%S"));

    let entry = Entry {
        id: log_id.clone(),
        direction: direction.to_string(),
        question: question.to_string(),
        context: None,
        entry_point: entry_point.to_string(),
        barrier_id,
        importance,
        status: STATUS_ACTIVE.to_string(),
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };

    db::insert_entry(&conn, &entry)?;

    if let Some(t) = tags_opt {
        let tag_list: Vec<String> = t.split(',').map(|s| s.trim().to_string()).collect();
        if !tag_list.is_empty() {
            db::insert_tags(&conn, &log_id, &tag_list)?;
        }
    }

    println!("Recorded research log entry: {}", log_id);
    Ok(())
}

fn cmd_add_finding(
    log_root: &Path,
    entry_id: &str,
    kind: &str,
    content: &str,
    confidence: Option<f64>,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    let conn = db::init_database(&db_path)?;

    if db::get_entry(&conn, entry_id)?.is_none() {
        anyhow::bail!("Entry not found: {}", entry_id);
    }

    let finding = Finding {
        id: 0,
        entry_id: entry_id.to_string(),
        kind: kind.to_string(),
        content: content.to_string(),
        confidence,
        metadata: None,
        created_at: Utc::now().to_rfc3339(),
    };
    db::insert_finding(&conn, &finding)?;
    println!("Added {} to entry {}", kind, entry_id);
    Ok(())
}

fn cmd_search(
    log_root: &Path,
    query: &str,
    direction: Option<&str>,
    status: Option<&str>,
    date_from: Option<&str>,
    date_to: Option<&str>,
    limit: usize,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database. Create one with `record`.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let results = db::search_entries(&conn, query, direction, status, date_from, date_to, limit)?;

    if results.is_empty() {
        println!("No results for: {}", query);
        return Ok(());
    }

    println!("Search results for \"{}\" ({} found):", query, results.len());
    for r in &results {
        println!("  [{:.30}] {}: {} (score: {:.2})", r.id, r.direction, r.snippet, r.score);
    }
    Ok(())
}

fn cmd_search_findings(
    log_root: &Path,
    query: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let results = db::search_findings(&conn, query, kind_filter, limit)?;

    if results.is_empty() {
        println!("No findings for: {}", query);
        return Ok(());
    }

    println!("Findings for \"{}\" ({} found):", query, results.len());
    for r in &results {
        println!("  [{}] {}: {}", r.id, r.kind, r.content.chars().take(80).collect::<String>());
    }
    Ok(())
}

fn cmd_render(
    log_root: &Path,
    entry_id: &str,
    write: bool,
    output: Option<String>,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    let conn = db::init_database(&db_path)?;

    let entry = db::get_entry(&conn, entry_id)?.with_context(|| format!("Entry not found: {}", entry_id))?;
    let findings = db::get_findings(&conn, entry_id)?;
    let tags = db::get_tags(&conn, entry_id)?;

    let md = text_layer::render_entry(&entry, &findings, &tags);
    println!("{}", md);

    if write {
        let dest = output.map(PathBuf::from).unwrap_or_else(|| log_root.to_path_buf());
        let path = text_layer::write_entry_md(&dest, &entry, &findings, &tags)?;
        println!("Written to: {}", path.display());
    }
    Ok(())
}

fn cmd_status(log_root: &Path) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }

    let size = std::fs::metadata(&db_path)?.len();
    let conn = db::init_database(&db_path)?;
    let ids = db::list_entry_ids(&conn)?;
    let is_wal: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap_or_default();

    println!("Research Log Database Status:");
    println!("  Path: {}", db_path.display());
    println!("  Size: {} KB", size / 1024);
    println!("  Entries: {}", ids.len());
    println!("  WAL: {}", if is_wal.to_lowercase() == "wal" { "enabled ✓" } else { "disabled" });
    Ok(())
}

fn cmd_consolidate(log_root: &Path) -> Result<()> {
    let (count, files) = research_log_rs::consolidate_activity_log(log_root)?;
    println!("Consolidated {} activities from {} file(s)", count, files);
    Ok(())
}

fn cmd_export(
    log_root: &Path,
    format: &ExportFormat,
    output: Option<String>,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    let conn = db::init_database(&db_path)?;

    match format {
        ExportFormat::Json => {
            let entries = research_log_rs::export::export_json(&conn)?;
            let json = serde_json::to_string_pretty(&entries)?;
            match output {
                Some(path) => std::fs::write(&path, &json)?,
                None => println!("{}", json),
            }
            println!("Exported {} entries (JSON)", entries.len());
        }
        ExportFormat::Csv => {
            let csv = research_log_rs::export::export_csv(&conn)?;
            match output {
                Some(path) => std::fs::write(&path, &csv)?,
                None => println!("{}", csv),
            }
            let lines = csv.lines().count().saturating_sub(1); // 减表头
            println!("Exported {} entries (CSV)", lines);
        }
        ExportFormat::Obsidian => {
            let files = research_log_rs::export::export_obsidian(&conn)?;
            let dir = output.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("obsidian-export"));
            std::fs::create_dir_all(&dir)?;
            for (filename, md) in &files {
                std::fs::write(dir.join(filename), md)?;
            }
            println!("Exported {} entries to {} (Obsidian)", files.len(), dir.display());
        }
    }
    Ok(())
}

fn cmd_connect(
    log_root: &Path,
    log_id_a: &str,
    log_id_b: &str,
    relation: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    let conn = db::init_database(&db_path)?;

    if db::get_entry(&conn, log_id_a)?.is_none() {
        anyhow::bail!("Entry not found: {}", log_id_a);
    }
    if db::get_entry(&conn, log_id_b)?.is_none() {
        anyhow::bail!("Entry not found: {}", log_id_b);
    }

    let log_conn = models::LogConnection {
        id: 0,
        entry_id_a: log_id_a.to_string(),
        entry_id_b: log_id_b.to_string(),
        relation,
        notes,
        created_at: Utc::now().to_rfc3339(),
    };
    db::insert_connection(&conn, &log_conn)?;
    println!("Connected: {} <-> {}", log_id_a, log_id_b);
    Ok(())
}

fn cmd_barrier(log_root: &Path, loop_id: Option<String>) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;

    if let Some(ref lid) = loop_id {
        let mut stmt = conn.prepare(
            "SELECT barrier_id, loop_id, created_at FROM barrier_reports WHERE loop_id = ?1 ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(rusqlite::params![lid], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?))
        })?;
        let mut count = 0;
        for r in rows {
            let (bid, lid_opt, ts) = r?;
            println!("  {} (loop: {:?}) @ {}", bid, lid_opt, ts);
            count += 1;
        }
        if count == 0 {
            println!("No barrier reports for loop: {}", lid);
        }
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT barrier_id, loop_id, created_at FROM barrier_reports ORDER BY created_at DESC LIMIT 20"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut count = 0;
    for r in rows {
        let (bid, lid_opt, ts) = r?;
        println!("  {} (loop: {:?}) @ {}", bid, lid_opt, ts);
        count += 1;
    }
    if count == 0 {
        println!("No barrier reports.");
    }
    Ok(())
}
