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
use rusqlite::{Connection, params};
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
    let now = framework_core::time::now_iso();
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
pub fn index_workspace(hub: &Connection, workspace_id: i64, log_root: &Path) -> Result<usize> {
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
    let now = framework_core::time::now_iso();
    hub.execute(
        "UPDATE workspace_index SET entry_count=?1, last_indexed_at=?2 WHERE id=?3",
        params![count as i64, now, workspace_id],
    )?;

    Ok(count)
}

/// Index all registered workspaces. Returns workspace_name -> entry_count.
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
