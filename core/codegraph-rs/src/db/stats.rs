use crate::db::index_ops::get_meta;
use crate::{FileRecord, IndexStats};
use rusqlite::Connection;

pub fn index_stats(conn: &Connection) -> rusqlite::Result<IndexStats> {
    let node_count: i64 = conn.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))?;
    let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))?;
    let file_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM nodes",
        [],
        |r| r.get(0),
    )?;
    let indexed_at = get_meta(conn, "indexed_at")?;
    Ok(IndexStats {
        node_count: node_count as u64,
        edge_count: edge_count as u64,
        file_count: file_count as u64,
        indexed_at,
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
        let stats = index_stats(&conn).unwrap();
        assert_eq!(stats.node_count, 0);
        assert!(list_files(&conn).unwrap().is_empty());
    }
}
