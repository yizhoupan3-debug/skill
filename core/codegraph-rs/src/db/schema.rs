use crate::db::index_ops::{get_meta, set_meta};
use crate::SCHEMA_VERSION;
use rusqlite::Connection;

pub const META_SCHEMA_VERSION_KEY: &str = "schema_version";
const LEGACY_SCHEMA_V1: &str = "codegraph-rs-v1";

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            mtime_ns INTEGER NOT NULL,
            language TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            symbol TEXT NOT NULL,
            kind TEXT NOT NULL,
            language TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_nodes_symbol ON nodes(symbol);
        CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file_path);
        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            caller_id TEXT NOT NULL,
            callee_id TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_edges_caller ON edges(caller_id);
        CREATE INDEX IF NOT EXISTS idx_edges_callee ON edges(callee_id);
        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            symbol, kind, language, file_path,
            content='nodes', content_rowid='rowid'
        );
        CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, symbol, kind, language, file_path)
            VALUES (new.rowid, new.symbol, new.kind, new.language, new.file_path);
        END;
        CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, symbol, kind, language, file_path)
            VALUES ('delete', old.rowid, old.symbol, old.kind, old.language, old.file_path);
        END;
        CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, symbol, kind, language, file_path)
            VALUES ('delete', old.rowid, old.symbol, old.kind, old.language, old.file_path);
            INSERT INTO nodes_fts(rowid, symbol, kind, language, file_path)
            VALUES (new.rowid, new.symbol, new.kind, new.language, new.file_path);
        END;
        "#,
    )?;
    if get_meta(conn, META_SCHEMA_VERSION_KEY)?.is_none() {
        set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
    }
    Ok(())
}

/// Upgrade legacy on-disk indexes (W4 minimal slice).
pub fn migrate_schema(conn: &Connection) -> rusqlite::Result<()> {
    match get_meta(conn, META_SCHEMA_VERSION_KEY)?.as_deref() {
        None => {
            migrate_v1_to_v2(conn)?;
            set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
        }
        Some(LEGACY_SCHEMA_V1) => {
            migrate_v1_to_v2(conn)?;
            set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
        }
        Some(v) if v == SCHEMA_VERSION => {}
        Some(other) => {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "unsupported codegraph schema version: {other}"
            )));
        }
    }
    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_edges_pair ON edges(caller_id, callee_id);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{init_schema, migrate_schema, META_SCHEMA_VERSION_KEY};
    use crate::db::index_ops::get_meta;
    use crate::SCHEMA_VERSION;

    #[test]
    fn schema_initializes_on_memory_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(count >= 4);
    }

    #[test]
    fn fresh_schema_stamps_current_version() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        migrate_schema(&conn).unwrap();
        assert_eq!(
            get_meta(&conn, META_SCHEMA_VERSION_KEY).unwrap().as_deref(),
            Some(SCHEMA_VERSION)
        );
    }

    #[test]
    fn legacy_v1_db_migrates_to_current() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE files (path TEXT PRIMARY KEY, mtime_ns INTEGER NOT NULL, language TEXT NOT NULL);
            CREATE TABLE nodes (
                id TEXT PRIMARY KEY, symbol TEXT NOT NULL, kind TEXT NOT NULL,
                language TEXT NOT NULL, file_path TEXT NOT NULL, line INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                caller_id TEXT NOT NULL, callee_id TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        migrate_schema(&conn).unwrap();
        assert_eq!(
            get_meta(&conn, META_SCHEMA_VERSION_KEY).unwrap().as_deref(),
            Some(SCHEMA_VERSION)
        );
        let pair_index: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_edges_pair'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pair_index, 1);
    }

    #[test]
    fn fts_trigger_syncs_on_insert() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["n1", "alpha_fn", "fn", "rust", "a.rs", 1],
        )
        .unwrap();
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH ?1",
                ["alpha"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }
}
