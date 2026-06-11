use crate::db::index_ops::get_meta;
use crate::{FileRecord, IndexStats};
use rusqlite::Connection;
use std::path::Path;

pub fn index_stats(conn: &Connection, db_path: &Path) -> rusqlite::Result<IndexStats> {
    let node_count: i64 = conn.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))?;
    let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))?;
    let file_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM nodes",
        [],
        |r| r.get(0),
    )?;
    let indexed_at = get_meta(conn, "indexed_at")?;
    let db_size_bytes = std::fs::metadata(db_path)
        .ok()
        .map(|m| m.len());
    Ok(IndexStats {
        node_count: node_count as u64,
        edge_count: edge_count as u64,
        file_count: file_count as u64,
        indexed_at,
        db_size_bytes,
    })
}

pub fn list_files(conn: &Connection) -> rusqlite::Result<Vec<FileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, language, COUNT(*) as c FROM nodes GROUP BY file_path, language ORDER BY file_path",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(FileRecord {
            path: row.get(0)?,
            language: row.get(1)?,
            symbol_count: row.get::<_, i64>(2)? as u64,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::{index_stats, list_files};
    use crate::db::schema::init_schema;

    #[test]
    fn stats_reflect_empty_index() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        // Use a temp path for db_path
        let tmp = std::env::temp_dir().join("codegraph-stats-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let db_path = tmp.join("index.sqlite");
        // Create empty file so metadata works
        std::fs::write(&db_path, "").unwrap();
        let stats = index_stats(&conn, &db_path).unwrap();
        assert_eq!(stats.node_count, 0);
        assert!(stats.db_size_bytes.is_some());
        assert!(list_files(&conn).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(tmp);
    }
}
