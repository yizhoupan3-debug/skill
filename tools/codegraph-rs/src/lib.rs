pub mod db;
pub mod graph;
pub mod mcp;
pub mod parser;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: &str = "codegraph-rs-v4";

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
        let db_path = repo_root.as_ref().join("artifacts/codegraph/index.sqlite");
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
        Ok(db::node_ops::resolve_symbol_filtered(
            &self.conn, symbol, filter,
        )?)
    }

    pub fn index_stats(&self) -> anyhow::Result<IndexStats> {
        Ok(db::stats::index_stats(&self.conn, &self.db_path)?)
    }

    pub fn list_files(&self) -> anyhow::Result<Vec<FileRecord>> {
        Ok(db::stats::list_files(&self.conn)?)
    }

    pub fn find_dead_code(
        &self,
        language: Option<&str>,
        min_lines: Option<u32>,
    ) -> anyhow::Result<Vec<db::node_ops::DeadCodeNode>> {
        Ok(db::node_ops::find_dead_code(
            &self.conn, language, min_lines,
        )?)
    }

    /// Lightweight dead code count (COUNT(*) only, no row data).
    /// Preferred for hot-path queries that only need the number.
    pub fn count_dead_code_only(
        &self,
        language: Option<&str>,
    ) -> anyhow::Result<usize> {
        Ok(db::node_ops::count_dead_code_only(
            &self.conn, language,
        )?)
    }

    pub fn find_definition(
        &self,
        symbol: &str,
        file_path: Option<&str>,
    ) -> anyhow::Result<Vec<db::node_ops::DefinitionResult>> {
        Ok(db::node_ops::find_definition(&self.conn, symbol, file_path)?)
    }

    /// Find all indexed symbols in a specific file.
    pub fn find_symbols_by_file(&self, file_path: &str) -> anyhow::Result<Vec<Node>> {
        Ok(db::node_ops::find_symbols_by_file(&self.conn, file_path)?)
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

    /// Ingest MCP tool registry entries into the index as `mcp_tool`-kinded nodes.
    ///
    /// Idempotent: deletes existing `mcp_tool` nodes before inserting.
    /// Returns the number of tools ingested.
    pub fn ingest_mcp_tools(&self, registry: &serde_json::Value) -> anyhow::Result<usize> {
        Ok(db::mcp_tool_ops::ingest_mcp_tools(&self.conn, registry)?)
    }

    /// Resolve a tool name to its managed server ID via indexed lookup.
    ///
    /// Returns `None` if the tool is not found.
    pub fn search_mcp_tool(&self, tool_name: &str) -> Option<String> {
        db::mcp_tool_ops::resolve_mcp_tool_server_id(&self.conn, tool_name)
    }

    /// List all MCP tool nodes in the index.
    pub fn list_mcp_tools(&self) -> anyhow::Result<Vec<Node>> {
        Ok(db::mcp_tool_ops::list_mcp_tools(&self.conn)?)
    }

    /// Ingest skill metadata from SKILL_MANIFEST.json into the index.
    ///
    /// Idempotent: deletes existing `skill` nodes before inserting.
    /// Returns the number of skills ingested.
    pub fn ingest_skills(&self, manifest: &serde_json::Value) -> anyhow::Result<usize> {
        Ok(db::skill_ops::ingest_skills(&self.conn, manifest)?)
    }

    /// Find a skill by exact slug match.
    pub fn find_skill(&self, slug: &str) -> Option<Node> {
        db::skill_ops::find_skill_by_slug(&self.conn, slug)
    }

    /// List all skill nodes in the index.
    pub fn list_skills(&self) -> anyhow::Result<Vec<Node>> {
        Ok(db::skill_ops::list_skills(&self.conn)?)
    }
}
