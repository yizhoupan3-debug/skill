use crate::parser::ParsedFile;
use rusqlite::{params, Connection, Statement};

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1 LIMIT 1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row.get(0)?));
    }
    Ok(None)
}

struct DeleteFileStmts<'conn> {
    delete_edges_by_caller: Statement<'conn>,
    delete_edges_by_callee: Statement<'conn>,
    delete_nodes: Statement<'conn>,
    delete_files: Statement<'conn>,
}

impl<'conn> DeleteFileStmts<'conn> {
    fn prepare(conn: &'conn Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            delete_edges_by_caller: conn.prepare(
                "DELETE FROM edges WHERE caller_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            )?,
            delete_edges_by_callee: conn.prepare(
                "DELETE FROM edges WHERE callee_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            )?,
            delete_nodes: conn.prepare("DELETE FROM nodes WHERE file_path = ?1")?,
            delete_files: conn.prepare("DELETE FROM files WHERE path = ?1")?,
        })
    }

    fn execute(&mut self, file_path: &str) -> rusqlite::Result<()> {
        self.delete_edges_by_caller.execute(params![file_path])?;
        self.delete_edges_by_callee.execute(params![file_path])?;
        self.delete_nodes.execute(params![file_path])?;
        self.delete_files.execute(params![file_path])?;
        Ok(())
    }
}

pub struct IngestStmts<'conn> {
    insert_node: Statement<'conn>,
    insert_node_ignore: Statement<'conn>,
    insert_edge: Statement<'conn>,
    upsert_file: Statement<'conn>,
    delete: DeleteFileStmts<'conn>,
}

impl<'conn> IngestStmts<'conn> {
    pub fn prepare(conn: &'conn Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            insert_node: conn.prepare(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?,
            insert_node_ignore: conn.prepare(
                "INSERT OR IGNORE INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?,
            insert_edge: conn.prepare(
                "INSERT INTO edges (caller_id, callee_id) VALUES (?1, ?2)",
            )?,
            upsert_file: conn.prepare(
                "INSERT INTO files (path, mtime_ns, language) VALUES (?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET mtime_ns = excluded.mtime_ns, language = excluded.language",
            )?,
            delete: DeleteFileStmts::prepare(conn)?,
        })
    }
}

pub fn delete_file_index(conn: &Connection, file_path: &str) -> rusqlite::Result<()> {
    let mut stmts = DeleteFileStmts::prepare(conn)?;
    stmts.execute(file_path)
}

pub fn ingest_parsed_file(conn: &Connection, parsed: &ParsedFile) -> rusqlite::Result<(u64, u64)> {
    let mut stmts = IngestStmts::prepare(conn)?;
    ingest_parsed_file_with_stmts(conn, &mut stmts, parsed)
}

pub fn ingest_parsed_file_with_stmts(
    conn: &Connection,
    stmts: &mut IngestStmts<'_>,
    parsed: &ParsedFile,
) -> rusqlite::Result<(u64, u64)> {
    let tx = conn.unchecked_transaction()?;
    stmts.delete.execute(&parsed.path)?;
    let mut node_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for sym in &parsed.symbols {
        let id = format!("{}:{}:{}", parsed.path, sym.line, sym.symbol);
        stmts.insert_node.execute(params![
            id,
            sym.symbol,
            sym.kind,
            parsed.language,
            parsed.path,
            sym.line
        ])?;
        node_ids.insert(sym.symbol.clone(), id);
    }
    let mut edge_count = 0u64;
    for edge in &parsed.edges {
        let Some(caller_id) = node_ids.get(&edge.caller_symbol).cloned() else {
            continue;
        };
        let callee_id = node_ids
            .get(&edge.callee_symbol)
            .cloned()
            .unwrap_or_else(|| format!("{}:0:{}", parsed.path, edge.callee_symbol));
        if !node_ids.contains_key(&edge.callee_symbol) {
            stmts.insert_node_ignore.execute(params![
                callee_id,
                edge.callee_symbol,
                "ref",
                parsed.language,
                parsed.path,
                edge.line
            ])?;
        }
        stmts
            .insert_edge
            .execute(params![caller_id, callee_id])?;
        edge_count += 1;
    }
    stmts.upsert_file.execute(params![
        parsed.path,
        parsed.mtime_ns,
        parsed.language
    ])?;
    tx.commit()?;
    Ok((parsed.symbols.len() as u64, edge_count))
}

pub fn list_indexed_files(conn: &Connection) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare("SELECT path, mtime_ns FROM files ORDER BY path")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::{delete_file_index, ingest_parsed_file, IngestStmts};
    use crate::db::schema::init_schema;
    use crate::parser::{ParsedEdge, ParsedFile, ParsedSymbol};

    #[test]
    fn ingest_replaces_prior_file_nodes() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let parsed = ParsedFile {
            path: "src/a.rs".to_string(),
            language: "rust".to_string(),
            mtime_ns: 1,
            symbols: vec![ParsedSymbol {
                symbol: "foo".to_string(),
                kind: "fn".to_string(),
                line: 3,
            }],
            edges: vec![ParsedEdge {
                caller_symbol: "foo".to_string(),
                callee_symbol: "bar".to_string(),
                line: 4,
            }],
        };
        let (nodes, edges) = ingest_parsed_file(&conn, &parsed).unwrap();
        assert_eq!(nodes, 1);
        assert_eq!(edges, 1);
        delete_file_index(&conn, "src/a.rs").unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn ingest_stmts_reused_across_files() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut stmts = IngestStmts::prepare(&conn).unwrap();
        for (path, sym) in [("a.rs", "alpha"), ("b.rs", "beta")] {
            let parsed = ParsedFile {
                path: path.to_string(),
                language: "rust".to_string(),
                mtime_ns: 1,
                symbols: vec![ParsedSymbol {
                    symbol: sym.to_string(),
                    kind: "fn".to_string(),
                    line: 1,
                }],
                edges: vec![],
            };
            super::ingest_parsed_file_with_stmts(&conn, &mut stmts, &parsed).unwrap();
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
