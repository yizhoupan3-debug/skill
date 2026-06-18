use anyhow::{Context, Result};
use chrono::Utc;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use research_log_rs::cli::{Command, ExportFormat, GraphFormat};
use research_log_rs::db;
use research_log_rs::graph;
use research_log_rs::models;
use research_log_rs::models::*;
use research_log_rs::text_layer;

fn main() -> Result<()> {
    let cli = <research_log_rs::cli::Cli as clap::Parser>::parse();
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

        // ── Knowledge Graph ──
        Command::Neighbors {
            entry_id,
            relation,
            limit,
        } => cmd_neighbors(&log_root, &entry_id, relation.as_deref(), limit)?,

        Command::Path {
            from,
            to,
            max_depth,
        } => cmd_path(&log_root, &from, &to, max_depth)?,

        Command::Subgraph {
            entry_id,
            max_depth,
            format,
        } => cmd_subgraph(&log_root, &entry_id, max_depth, &format)?,

        Command::GraphStats => cmd_graph_stats(&log_root)?,

        Command::Viz {
            entry_id,
            max_depth,
            min_connections,
            format,
        } => cmd_viz(&log_root, entry_id.as_deref(), max_depth, min_connections, &format)?,

        Command::Route {
            barrier_id,
            max_depth,
        } => cmd_route(&log_root, &barrier_id, max_depth)?,

        // ── Entity Management ──
        Command::ExtractEntities { entry_id } => cmd_extract_entities(&log_root, &entry_id)?,

        Command::AddEntity {
            name,
            kind,
            description,
        } => cmd_add_entity(&log_root, &name, &kind, description.as_deref())?,

        Command::LinkEntities {
            entity_a,
            entity_b,
            relation,
            entry_id,
        } => cmd_link_entities(&log_root, &entity_a, &entity_b, &relation, entry_id.as_deref())?,

        Command::SearchEntities { query, limit } => cmd_search_entities(&log_root, &query, limit)?,

        Command::EntryEntities { entry_id } => cmd_entry_entities(&log_root, &entry_id)?,

        // ── Cross-Workspace Hub ──
        Command::HubRegister { path, name } => cmd_hub_register(path.as_deref(), name.as_deref())?,

        Command::HubIndex { path } => cmd_hub_index(path.as_deref())?,

        Command::HubSearch { query, limit } => cmd_hub_search(&query, limit)?,

        Command::HubList => cmd_hub_list()?,
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
        weight: 1.0,
        confidence: None,
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

// ── Knowledge Graph command handlers ──

fn cmd_neighbors(log_root: &Path, entry_id: &str, relation_filter: Option<&str>, limit: usize) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let g = graph::load_full_graph(&conn)?;

    let entry = db::get_entry(&conn, entry_id)?
        .with_context(|| format!("Entry not found: {}", entry_id))?;
    let filter: Option<Vec<&str>> = relation_filter
        .map(|r| r.split(',').map(|s| s.trim()).collect());
    let neighbors = graph::get_neighbors(&g, entry_id, filter.as_deref());

    println!("Neighbors of [{}] {}: {}", entry.id, entry.direction, entry.question);
    println!("  ({} connection(s))", neighbors.len());
    println!();
    for (nid, rel, _w, conf) in neighbors.iter().take(limit) {
        let ne = db::get_entry(&conn, nid)?;
        match ne {
            Some(e) => println!("  {} --[{}{}]--> [{}] {}",
                entry_id,
                rel.unwrap_or("related"),
                if let Some(c) = conf { format!(" conf={}", c) } else { String::new() },
                e.id,
                e.question.chars().take(60).collect::<String>(),
            ),
            None => println!("  {} --[{}]--> {} (deleted?)",
                entry_id, rel.unwrap_or("related"), nid),
        }
    }
    Ok(())
}

fn cmd_path(log_root: &Path, from: &str, to: &str, max_depth: usize) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let g = graph::load_full_graph(&conn)?;

    let _from_entry = db::get_entry(&conn, from)?
        .with_context(|| format!("Start entry not found: {}", from))?;
    let _to_entry = db::get_entry(&conn, to)?
        .with_context(|| format!("Target entry not found: {}", to))?;

    println!("Finding path from [{}] to [{}]", from, to);
    let path = graph::find_path(&g, from, to, max_depth);

    match path {
        Some(nodes) => {
            println!("Path ({} hops):", nodes.len() - 1);
            for (i, (nid, rel, w)) in nodes.iter().enumerate() {
                let e = db::get_entry(&conn, nid)?;
                let label = e.map(|x| format!("{}: {}", x.direction, x.question.chars().take(50).collect::<String>()))
                    .unwrap_or_else(|| nid.clone());
                if i == 0 {
                    println!("  START [{}] {}", nid, label);
                } else {
                    let rel_label = rel.as_deref().unwrap_or("related");
                    println!("    --[{} w={:.1}]-->", rel_label, w);
                    println!("  [{}] {}", nid, label);
                }
            }
        }
        None => println!("No path found between [{}] and [{}] (max depth: {})", from, to, max_depth),
    }
    Ok(())
}

fn cmd_graph_stats(log_root: &Path) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let g = graph::load_full_graph(&conn)?;
    let stats = graph::get_graph_stats(&g);

    println!("╔═══ Research Knowledge Graph Stats ═══╗");
    println!("║ Nodes:              {:>6}", stats.node_count);
    println!("║ Edges:              {:>6}", stats.edge_count);
    println!("║ Avg degree:         {:>6.2}", stats.avg_degree);
    println!("║ Density:            {:>6.4}", stats.density);
    println!("║ Isolated nodes:     {:>6}", stats.isolated_nodes);
    if !stats.relation_counts.is_empty() {
        println!("║ ───────────────────────────────── ║");
        for (rel, count) in &stats.relation_counts {
            println!("║  {}: {:>6}", rel, count);
        }
    }
    println!("╚═══════════════════════════════════════╝");
    Ok(())
}

fn cmd_viz(log_root: &Path, entry_id: Option<&str>, max_depth: usize, min_connections: usize, format: &GraphFormat) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let g = match entry_id {
        Some(eid) => graph::load_subgraph(&conn, eid, max_depth)?,
        None => graph::load_full_graph(&conn)?,
    };

    // Load entry data for labels
    let mut labels: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for node in &g.nodes {
        if let Some(entry) = db::get_entry(&conn, node)? {
            labels.insert(node.clone(), format!("{}:{}", entry.direction, entry.question.chars().take(40).collect::<String>()));
        }
    }

    match format {
        GraphFormat::Text => {
            let viz = render_ascii_viz(&g, entry_id, max_depth, min_connections, &labels)?;
            println!("{}", viz);
        }
        GraphFormat::Dot => {
            let dot = render_dot_viz(&g, &labels)?;
            println!("{}", dot);
        }
    }
    Ok(())
}

fn cmd_subgraph(log_root: &Path, entry_id: &str, max_depth: usize, format: &GraphFormat) -> Result<()> {
    cmd_viz(log_root, Some(entry_id), max_depth, 0, format)
}

fn cmd_route(log_root: &Path, barrier_id: &str, max_depth: usize) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let route = graph::trace_barrier_route(&conn, barrier_id, max_depth)?;

    println!("═══ Barrier Route: {} ═══", route.barrier.barrier_id);
    println!("  Loop: {:?}, Created: {}", route.barrier.loop_id, route.barrier.created_at);
    println!();
    for ewf in &route.root_entries {
        println!("  Entry: [{}] {}", ewf.entry.id, ewf.entry.question);
        for f in &ewf.findings {
            println!("    {}: {} (conf: {:?})", f.kind, f.content.chars().take(60).collect::<String>(), f.confidence);
        }
        for c in &ewf.connections {
            println!("    Edge: {} <-> {} [{}]", c.entry_id_a, c.entry_id_b, c.relation.as_deref().unwrap_or("related"));
        }
    }
    println!();
    let stats = graph::get_graph_stats(&route.subgraph);
    println!("  Subgraph: {} nodes, {} edges", stats.node_count, stats.edge_count);
    Ok(())
}

// ── Entity Management command handlers ──

fn cmd_extract_entities(log_root: &Path, entry_id: &str) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let entry = db::get_entry(&conn, entry_id)?
        .with_context(|| format!("Entry not found: {}", entry_id))?;

    // Build text from question + findings
    let mut text = entry.question.clone();
    let findings = db::get_findings(&conn, entry_id)?;
    for f in &findings {
        text.push_str(" ");
        text.push_str(&f.content);
    }
    let tags = db::get_tags(&conn, entry_id)?;
    for t in &tags {
        text.push_str(" ");
        text.push_str(t);
    }

    let found = research_log_rs::extract::extract_entities_from_text(&text);
    if found.is_empty() {
        println!("No entities found in entry [{}].", entry_id);
        return Ok(());
    }

    let mut entity_ids = Vec::new();
    for (name, kind) in &found {
        let eid = db::upsert_entity(&conn, name, kind, None, None)?;
        db::insert_entry_entity(&conn, entry_id, eid, ENTRY_ENTITY_ROLE_MENTIONED)?;
        entity_ids.push((name.clone(), kind.clone()));
    }

    println!("Extracted {} entities from [{}]:", entity_ids.len(), entry_id);
    for (name, kind) in &entity_ids {
        println!("  [{}] {}", kind, name);
    }
    Ok(())
}

fn cmd_add_entity(log_root: &Path, name: &str, kind: &str, description: Option<&str>) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let eid = db::upsert_entity(&conn, name, kind, description, None)?;
    println!("Added entity: [{}] {} (id={})", kind, name, eid);
    Ok(())
}

fn cmd_link_entities(log_root: &Path, entity_a: &str, entity_b: &str, relation: &str, entry_id: Option<&str>) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;

    // Resolve by name (or try as numeric ID)
    let resolve = |name_or_id: &str| -> Result<i64> {
        if let Ok(num) = name_or_id.parse::<i64>() {
            return Ok(num);
        }
        let e = db::get_entity_by_name(&conn, name_or_id)?;
        match e {
            Some(ent) => Ok(ent.id),
            None => anyhow::bail!("Entity not found: {}", name_or_id),
        }
    };
    let id_a = resolve(entity_a)?;
    let id_b = resolve(entity_b)?;

    db::insert_entity_relation(&conn, id_a, id_b, relation, entry_id, None, None)?;
    println!("Linked entities: {} --[{}]--> {}", entity_a, relation, entity_b);
    Ok(())
}

fn cmd_search_entities(log_root: &Path, query: &str, limit: usize) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let results = db::search_entities(&conn, query, limit)?;

    println!("Entity search results for \"{}\" ({} found):", query, results.len());
    for e in &results {
        println!("  [{}] {} (id={})", e.kind, e.name, e.id);
        if let Some(ref desc) = e.description {
            println!("         {}", desc.chars().take(80).collect::<String>());
        }
    }
    Ok(())
}

fn cmd_entry_entities(log_root: &Path, entry_id: &str) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        println!("No research log database.");
        return Ok(());
    }
    let conn = db::init_database(&db_path)?;
    let entry = db::get_entry(&conn, entry_id)?
        .with_context(|| format!("Entry not found: {}", entry_id))?;
    let entities = db::get_entry_entities(&conn, entry_id)?;

    println!("Entities for [{}] {}: {}", entry.id, entry.direction, entry.question);
    if entities.is_empty() {
        println!("  (none — run `extract-entities {}` to auto-extract)", entry_id);
    } else {
        for (e, role) in &entities {
            println!("  [{}] {} (role: {})", e.kind, e.name, role);
        }
    }
    Ok(())
}

// ── ASCII / DOT rendering (lightweight, no external dep) ──

fn render_ascii_viz(
    g: &research_log_rs::graph::KnowledgeGraph,
    center: Option<&str>,
    max_depth: usize,
    min_connections: usize,
    labels: &std::collections::HashMap<String, String>,
) -> Result<String> {
    let mut out = String::new();
    out.push_str("┌─ Research Knowledge Graph");
    if let Some(c) = center {
        write!(out, " (centered on {})", c)?;
    }
    out.push_str(" ─────────────────────┐\n");

    // Determine which nodes to show
    let nodes_to_show: Vec<&String> = if let Some(c) = center {
        let g2 = {
            // We need a subgraph for ordering; reuse the full graph's adjacency
            let adj = &g.adjacency;
            let mut visited = std::collections::HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            visited.insert(c.to_string());
            queue.push_back((c.to_string(), 0usize));
            while let Some((node, depth)) = queue.pop_front() {
                if depth >= max_depth { continue; }
                if let Some(edges) = adj.get(&node) {
                    for (nbor, _, _, _) in edges {
                        if visited.insert(nbor.clone()) {
                            queue.push_back((nbor.clone(), depth + 1));
                        }
                    }
                }
            }
            visited
        };
        g.nodes.iter().filter(|n| g2.contains(*n)).collect()
    } else {
        g.nodes.iter().collect()
    };

    // Filter by min_connections
    let candidates: Vec<&String> = nodes_to_show.into_iter().filter(|n| {
        if min_connections == 0 { return true; }
        let deg = g.adjacency.get(*n).map_or(0, |v| v.len());
        deg >= min_connections
    }).collect();

    // Sort by number of connections (most connected first)
    let mut sorted: Vec<_> = candidates.into_iter().collect();
    sorted.sort_by_key(|n| {
        g.adjacency.get(*n).map_or(0, |v| v.len())
    });
    sorted.reverse();

    // Deduplicate edges for ASCII (each connection appears twice in adjacency)
    let mut seen_edges: std::collections::HashSet<String> = std::collections::HashSet::new();

    for node in &sorted {
        let label = labels.get(*node).map(|s| s.as_str()).unwrap_or(node);
        write!(out, "  [{}] {}", node, label)?;
        out.push('\n');

        if let Some(edges) = g.adjacency.get(*node) {
            for (nbor, rel, w, _) in edges {
                let edge_key = if node.as_str() < nbor.as_str() {
                    format!("{}->{}", node, nbor)
                } else {
                    format!("{}->{}", nbor, node)
                };
                if !seen_edges.insert(edge_key) {
                    continue;
                }
                if sorted.contains(&&nbor) {
                    let rel_label = rel.as_deref().unwrap_or("related");
                    write!(out, "   └──[{} w={:.1}]──> [{}]\n", rel_label, w, nbor)?;
                }
            }
        }
        out.push('\n');
    }

    let stats = research_log_rs::graph::get_graph_stats(g);
    writeln!(out, "  Stats: {} nodes, {} edges, density {:.4}", stats.node_count, stats.edge_count, stats.density)?;
    out.push_str("└────────────────────────────────────────────────────────────┘\n");
    Ok(out)
}

fn render_dot_viz(
    g: &research_log_rs::graph::KnowledgeGraph,
    labels: &std::collections::HashMap<String, String>,
) -> Result<String> {
    let mut out = String::new();
    out.push_str("digraph ResearchKnowledgeGraph {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded];\n\n");

    for node in &g.nodes {
        let label = labels.get(node).map(|s| s.as_str()).unwrap_or(node);
        // Escape quotes
        let escaped = label.replace('"', "\\\"");
        write!(out, "    \"{}\" [label=\"{}\\n{}\"];\n", node, node, escaped)?;
    }
    out.push('\n');

    // Deduplicate edges for DOT (each connection appears twice in adjacency)
    let mut seen = std::collections::HashSet::new();
    for (node, edges) in &g.adjacency {
        for (nbor, rel, _, _) in edges {
            let key = if node < nbor {
                format!("{}->{}", node, nbor)
            } else {
                format!("{}->{}", nbor, node)
            };
            if seen.insert(key) {
                let style = match rel.as_deref() {
                    Some("supports" | "extends") => "style=solid",
                    Some("contradicts") => "style=dotted, color=red",
                    _ => "style=dashed",
                };
                let label = rel.as_deref().unwrap_or("related");
                write!(out, "    \"{}\" -> \"{}\" [label=\"{}\", {}];\n", node, nbor, label, style)?;
            }
        }
    }

    out.push_str("}\n");
    Ok(out)
}

// ── Cross-Workspace Hub ──

fn cmd_hub_register(path: Option<&str>, name: Option<&str>) -> Result<()> {
    let cwd = path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let ws_name = name
        .map(|s| s.to_string())
        .unwrap_or_else(|| cwd.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".to_string()));

    let hub = research_log_rs::hub::init_hub()?;
    let ws_id = research_log_rs::hub::register_workspace(&hub, &cwd, &ws_name)?;
    let log_root = cwd.join("artifacts").join("research-log");
    let count = research_log_rs::hub::index_workspace(&hub, ws_id, &log_root)?;
    println!("Registered workspace '{}' at {} ({} entries)", ws_name, cwd.display(), count);
    Ok(())
}

fn cmd_hub_index(path: Option<&str>) -> Result<()> {
    let hub = research_log_rs::hub::init_hub()?;
    if let Some(p) = path {
        let ws_path = PathBuf::from(p);
        let ws_name = ws_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ws_id = research_log_rs::hub::register_workspace(&hub, &ws_path, &ws_name)?;
        let log_root = ws_path.join("artifacts").join("research-log");
        let count = research_log_rs::hub::index_workspace(&hub, ws_id, &log_root)?;
        println!("Indexed '{}': {} entries", ws_name, count);
    } else {
        let results = research_log_rs::hub::index_all(&hub)?;
        println!("Indexed {} workspace(s):", results.len());
        for (name, count) in &results {
            println!("  {}: {} entries", name, count);
        }
    }
    Ok(())
}

fn cmd_hub_search(query: &str, limit: usize) -> Result<()> {
    let hub = research_log_rs::hub::init_hub()?;
    let results = research_log_rs::hub::hub_search(&hub, query, limit)?;

    if results.is_empty() {
        println!("No cross-workspace results for: {}", query);
        println!("  (Run `hub-index` first to index workspaces)");
        return Ok(());
    }

    println!("Cross-workspace search results for \"{}\" ({} found):", query, results.len());
    for r in &results {
        println!("  [{}.{}] {}: {}",
            r.workspace_name,
            r.local_entry_id,
            r.direction,
            r.question.chars().take(60).collect::<String>(),
        );
    }
    Ok(())
}

fn cmd_hub_list() -> Result<()> {
    let hub = research_log_rs::hub::init_hub()?;
    let workspaces = research_log_rs::hub::list_workspaces(&hub)?;

    if workspaces.is_empty() {
        println!("No workspaces registered. Run `hub-register` first.");
        return Ok(());
    }

    println!("Registered workspaces:");
    for w in &workspaces {
        let last_idx = w.last_indexed_at.as_deref().unwrap_or("never");
        println!("  [{}] {} ({} entries, indexed: {})", w.id, w.name, w.entry_count, last_idx);
    }
    Ok(())
}
