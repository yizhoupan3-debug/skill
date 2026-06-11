pub mod db;
pub mod graph;
pub mod mcp;
pub mod parser;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: &str = "codegraph-rs-v3";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub symbol: String,
    pub kind: String,
    pub language: String,
    pub file_path: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ImpactReport {
    pub symbol: String,
    pub depth: u32,
    pub callers: Vec<Node>,
    pub callees: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IndexStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub file_count: u64,
    pub indexed_at: Option<String>,
    /// SQLite database file size in bytes (None if stat fails)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    pub path: String,
    pub language: String,
    pub symbol_count: u64,
}

pub struct CodeGraphIndex {
    db_path: PathBuf,
    conn: rusqlite::Connection,
}

impl CodeGraphIndex {
    pub fn open(repo_root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_path = repo_root
            .as_ref()
            .join("artifacts/codegraph/index.sqlite");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = rusqlite::Connection::open(&db_path)?;
        db::schema::init_schema(&conn)?;
        db::schema::migrate_schema(&conn)?;
        Ok(Self { db_path, conn })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub(crate) fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }

    pub fn search_symbols(
        &self,
        query: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<Node>> {
        Ok(db::fts_ops::search_symbols(
            &self.conn, query, kind, language, limit,
        )?)
    }

    pub fn find_callers(
        &self,
        symbol: &str,
        depth: u32,
        filter: &db::node_ops::SymbolFilter,
    ) -> anyhow::Result<Vec<Node>> {
        Ok(graph::find_callers(&self.conn, symbol, depth, filter)?)
    }

    pub fn find_callees(
        &self,
        symbol: &str,
        depth: u32,
        filter: &db::node_ops::SymbolFilter,
    ) -> anyhow::Result<Vec<Node>> {
        Ok(graph::find_callees(&self.conn, symbol, depth, filter)?)
    }

    pub fn impact_radius(
        &self,
        symbol: &str,
        depth: u32,
        filter: &db::node_ops::SymbolFilter,
    ) -> anyhow::Result<ImpactReport> {
        Ok(graph::impact_radius(&self.conn, symbol, depth, filter)?)
    }

    pub fn get_node_by_id(&self, id: &str) -> anyhow::Result<Option<Node>> {
        Ok(db::node_ops::get_node_by_id(&self.conn, id)?)
    }

    pub fn resolve_symbol(&self, symbol: &str) -> anyhow::Result<Option<Node>> {
        Ok(db::node_ops::resolve_symbol(&self.conn, symbol)?)
    }

    pub fn resolve_symbol_filtered(
        &self,
        symbol: &str,
        filter: &db::node_ops::SymbolFilter,
    ) -> anyhow::Result<db::node_ops::ResolveOutcome> {
        Ok(db::node_ops::resolve_symbol_filtered(&self.conn, symbol, filter)?)
    }

    pub fn index_stats(&self) -> anyhow::Result<IndexStats> {
        Ok(db::stats::index_stats(&self.conn, &self.db_path)?)
    }

    pub fn list_files(&self) -> anyhow::Result<Vec<FileRecord>> {
        Ok(db::stats::list_files(&self.conn)?)
    }

    pub fn build_full_index(&self, repo_root: &Path) -> anyhow::Result<graph::SyncReport> {
        graph::build_full_index(self, repo_root)
    }

    pub fn incremental_sync(
        &self,
        repo_root: &Path,
        force_all: bool,
    ) -> anyhow::Result<graph::SyncReport> {
        graph::incremental_sync(self, repo_root, force_all)
    }

    pub fn spawn_watcher(&self, repo_root: PathBuf) -> anyhow::Result<graph::IndexWatcher> {
        let _ = self.db_path();
        graph::IndexWatcher::spawn(repo_root)
    }
}
