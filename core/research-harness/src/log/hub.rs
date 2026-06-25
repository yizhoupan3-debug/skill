// Migrated from tools/research-log-rs/src/hub.rs

//! Cross-workspace research knowledge hub.
//!
//! Maintains a central SQLite database at `~/.claude/research-knowledge-hub.db`
//! that indexes entries across all registered research workspaces,
//! enabling cross-workspace search and discovery.
//!
//! # Schema
//!
//! - `workspace_index`: tracks known workspaces (path, name, entry_count)
//! - `hub_entries`: denormalized, materialized index of all entries
//! - `hub_entries_fts`: FTS5 virtual table for cross-workspace search

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

use crate::log::db;

const HUB_FILENAME: &str = ".router-rs/research-knowledge-hub.db";

fn hub_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Cannot determine home directory (set $HOME or $USERPROFILE)")?;
    Ok(Path::new(&home).join(HUB_FILENAME))
}

/// Hub search result with workspace provenance.
#[derive(Debug, Clone)]
pub struct HubSearchResult {
    pub workspace_name: String,
    pub workspace_path: String,
    pub local_entry_id: String,
    pub direction: String,
    pub question: String,
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub score: f64,
}

/// Workspace info from the hub index.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub entry_count: i64,
    pub last_indexed_at: Option<String>,
}

const HUB_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS workspace_index (
    id INTEGER PRIMARY KEY,
    workspace_path TEXT NOT NULL UNIQUE,
    workspace_name TEXT NOT NULL,
    last_indexed_at TEXT,
    entry_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS hub_entries (
    id INTEGER PRIMARY KEY,
    workspace_id INTEGER NOT NULL REFERENCES workspace_index(id) ON DELETE CASCADE,
    local_entry_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    question TEXT NOT NULL,
    tags TEXT,
    status TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(workspace_id, local_entry_id)
);
CREATE INDEX IF NOT EXISTS idx_hub_entries_created ON hub_entries(created_at);
CREATE INDEX IF NOT EXISTS idx_hub_entries_direction ON hub_entries(direction);

CREATE VIRTUAL TABLE IF NOT EXISTS hub_entries_fts USING fts5(
    question, direction, tags,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS hub_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- FTS5 sync triggers
CREATE TRIGGER IF NOT EXISTS hub_entries_ai AFTER INSERT ON hub_entries BEGIN
    INSERT INTO hub_entries_fts(rowid, question, direction, tags)
    VALUES (new.rowid, new.question, new.direction, COALESCE(new.tags, ''));
END;

CREATE TRIGGER IF NOT EXISTS hub_entries_ad AFTER DELETE ON hub_entries BEGIN
    INSERT INTO hub_entries_fts(hub_entries_fts, rowid, question, direction, tags)
    VALUES ('delete', old.rowid, old.question, old.direction, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS hub_entries_au AFTER UPDATE ON hub_entries BEGIN
    INSERT INTO hub_entries_fts(hub_entries_fts, rowid, question, direction, tags)
    VALUES ('delete', old.rowid, old.question, old.direction, old.tags);
    INSERT INTO hub_entries_fts(rowid, question, direction, tags)
    VALUES (new.rowid, new.question, new.direction, COALESCE(new.tags, ''));
END;
";

/// Initialize the hub database (create if not exists) at the default path.
pub fn init_hub() -> Result<Connection> {
    let hub_path = hub_path()?;
    init_hub_at(&hub_path)
}

/// Initialize a hub database at a specific path (useful for testing).
pub fn init_hub_at(hub_path: &Path) -> Result<Connection> {
    // Ensure parent directory exists
    if let Some(parent) = hub_path.parent() {
        std::fs::create_dir_all(parent).context("Create hub directory")?;
    }

    let conn = Connection::open(hub_path).context("Open hub database")?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;",
    )
    .context("Set hub PRAGMAs")?;

    conn.execute_batch(HUB_SCHEMA)
        .context("Create hub schema")?;

    // Set or update schema version
    conn.execute(
        "INSERT OR REPLACE INTO hub_meta (key, value) VALUES ('schema_version', '1')",
        [],
    )?;

    Ok(conn)
}

/// Register or update a workspace in the hub.
pub fn register_workspace(hub: &Connection, workspace_path: &Path, name: &str) -> Result<i64> {
    let now = framework_kernel::time::now_iso();
    hub.execute(
        "INSERT INTO workspace_index (workspace_path, workspace_name, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(workspace_path) DO UPDATE SET
           workspace_name = COALESCE(NULLIF(?2, ''), workspace_name)",
        params![workspace_path.display().to_string(), name, now],
    )?;

    let id: i64 = hub.query_row(
        "SELECT id FROM workspace_index WHERE workspace_path=?1",
        params![workspace_path.display().to_string()],
        |row| row.get(0),
    )?;
    Ok(id)
}

/// Index all entries from a workspace's research-log.db into the hub.
pub fn index_workspace(
    hub: &Connection,
    workspace_id: i64,
    log_root: &Path,
) -> Result<usize> {
    let db_path = log_root.join("research-log.db");
    if !db_path.exists() {
        return Ok(0);
    }

    let source = db::init_database(&db_path)?;

    // Fetch all entries from the source DB
    let ids = db::list_entry_ids(&source)?;
    let mut count = 0usize;

    for eid in &ids {
        let entry = match db::get_entry(&source, eid)? {
            Some(e) => e,
            None => continue,
        };
        let tags = db::get_tags(&source, eid)?;
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

        // Upsert into hub: reindexing refreshes existing entries
        hub.execute(
            "INSERT INTO hub_entries (workspace_id, local_entry_id, direction, question, tags, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(workspace_id, local_entry_id) DO UPDATE SET
               direction=excluded.direction,
               question=excluded.question,
               tags=excluded.tags,
               status=excluded.status",
            params![
                workspace_id,
                entry.id,
                entry.direction,
                entry.question,
                tags_json,
                entry.status,
                entry.created_at,
            ],
        )?;
        count += 1;
    }

    // Update entry count and last indexed timestamp
    let now = framework_kernel::time::now_iso();
    hub.execute(
        "UPDATE workspace_index SET entry_count=?1, last_indexed_at=?2 WHERE id=?3",
        params![count as i64, now, workspace_id],
    )?;

    Ok(count)
}

/// Index all registered workspaces. Returns workspace_name -> entry_count.
pub fn index_all(hub: &Connection) -> Result<std::collections::HashMap<String, usize>> {
    // Collect all workspaces first (avoid borrowing conflict with hub inside the loop)
    let workspaces: Vec<(i64, String, String)> = {
        let mut stmt = hub.prepare(
            "SELECT id, workspace_path, workspace_name FROM workspace_index",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut ws = Vec::new();
        for row in rows {
            ws.push(row?);
        }
        ws
    };

    let mut results = std::collections::HashMap::new();
    for (ws_id, ws_path, ws_name) in workspaces {
        let log_root = Path::new(&ws_path).join("artifacts").join("research-log");
        match index_workspace(hub, ws_id, &log_root) {
            Ok(count) => {
                results.insert(ws_name, count);
            }
            Err(e) => {
                tracing::warn!("Warning: failed to index workspace '{ws_name}': {e}");
            }
        }
    }
    Ok(results)
}

/// Cross-workspace FTS5 search.
///
/// Handles FTS5 query syntax: hyphens in user queries are escaped to
/// prevent FTS5 from interpreting them as column exclusion prefixes.
pub fn hub_search(
    hub: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<HubSearchResult>> {
    // Escape hyphens that would be interpreted as FTS5 column exclusion
    let fts_query = db::sanitize_fts_query(query);
    let sql = "
        SELECT wi.workspace_name, wi.workspace_path,
               he.local_entry_id, he.direction, he.question,
               he.tags, he.status, he.created_at,
               rank
        FROM hub_entries_fts
        JOIN hub_entries he ON he.rowid = hub_entries_fts.rowid
        JOIN workspace_index wi ON wi.id = he.workspace_id
        WHERE hub_entries_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    ";

    let mut stmt = hub.prepare_cached(sql)?;
    let mut rows = stmt.query(params![fts_query, limit as i64])?;
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let tags_str: Option<String> = row.get(5)?;
        let tags: Vec<String> = tags_str.as_deref().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();

        results.push(HubSearchResult {
            workspace_name: row.get(0)?,
            workspace_path: row.get(1)?,
            local_entry_id: row.get(2)?,
            direction: row.get(3)?,
            question: row.get(4)?,
            tags,
            status: row.get(6)?,
            created_at: row.get(7)?,
            score: row.get::<_, f64>(8).unwrap_or(0.0),
        });
    }

    Ok(results)
}

/// List registered workspaces.
pub fn list_workspaces(hub: &Connection) -> Result<Vec<WorkspaceInfo>> {
    let mut stmt = hub.prepare(
        "SELECT id, workspace_name, workspace_path, entry_count, last_indexed_at
         FROM workspace_index ORDER BY last_indexed_at DESC",
    )?;

    let mut rows = stmt.query([])?;
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        results.push(WorkspaceInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            entry_count: row.get(3)?,
            last_indexed_at: row.get(4)?,
        });
    }

    Ok(results)
}

/// Register the current working directory as a workspace and index it.
pub fn register_and_index_current() -> Result<()> {
    let cwd = std::env::current_dir().context("Get current directory")?;
    let name = cwd
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let hub = init_hub()?;
    let ws_id = register_workspace(&hub, &cwd, &name)?;
    let log_root = cwd.join("artifacts").join("research-log");
    let count = index_workspace(&hub, ws_id, &log_root)?;
    println!("Registered workspace '{}' at {} ({} entries)", name, cwd.display(), count);
    Ok(())
}

/// 兼容骨架 API：同步单个工作区的日志到 Hub。
pub fn sync_workspace(hub_path: &Path, workspace: &Path) -> Result<()> {
    let conn = if hub_path.exists() || hub_path.parent().is_some_and(|p| p.exists()) {
        init_hub_at(hub_path)?
    } else {
        init_hub()?
    };
    let name = workspace
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let ws_id = register_workspace(&conn, workspace, &name)?;
    let log_root = workspace.join("artifacts").join("research-log");
    let count = index_workspace(&conn, ws_id, &log_root)?;
    println!("Synced workspace '{}' ({} entries)", name, count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_search_result_struct() {
        let result = HubSearchResult {
            workspace_name: "test-ws".into(),
            workspace_path: "/tmp/test".into(),
            local_entry_id: "e1".into(),
            direction: "deepen".into(),
            question: "test q".into(),
            tags: vec!["ml".into()],
            status: "active".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            score: 0.95,
        };
        assert_eq!(result.workspace_name, "test-ws");
        assert_eq!(result.score, 0.95);
    }

    #[test]
    fn workspace_info_struct() {
        let info = WorkspaceInfo {
            id: 1,
            name: "my-project".into(),
            path: "/home/user/project".into(),
            entry_count: 42,
            last_indexed_at: None,
        };
        assert_eq!(info.entry_count, 42);
    }
}
