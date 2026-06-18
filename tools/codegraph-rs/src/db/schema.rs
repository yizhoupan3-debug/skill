use crate::SCHEMA_VERSION;
use crate::db::index_ops::{get_meta, set_meta};
use rusqlite::Connection;

pub const META_SCHEMA_VERSION_KEY: &str = "schema_version";
const LEGACY_SCHEMA_V1: &str = "codegraph-rs-v1";
const LEGACY_SCHEMA_V2: &str = "codegraph-rs-v2";
const LEGACY_SCHEMA_V3: &str = "codegraph-rs-v3";

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
            language TEXT NOT NULL,
            content_hash TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            symbol TEXT NOT NULL,
            kind TEXT NOT NULL,
            language TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT 0,
            extra TEXT NOT NULL DEFAULT ''
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
            content='nodes', content_rowid='rowid',
            prefix='1 2 3'
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
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
        }
        Some(LEGACY_SCHEMA_V1) => {
            migrate_v1_to_v2(conn)?;
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
        }
        Some(LEGACY_SCHEMA_V2) => {
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            set_meta(conn, META_SCHEMA_VERSION_KEY, SCHEMA_VERSION)?;
        }
        Some(LEGACY_SCHEMA_V3) => {
            migrate_v3_to_v4(conn)?;
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

fn migrate_v2_to_v3(conn: &Connection) -> rusqlite::Result<()> {
    let has_column: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'content_hash'",
        [],
        |row| row.get(0),
    )?;
    if has_column == 0 {
        conn.execute(
            "ALTER TABLE files ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

fn migrate_v3_to_v4(conn: &Connection) -> rusqlite::Result<()> {
    let has_column: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('nodes') WHERE name = 'extra'",
        [],
        |row| row.get(0),
    )?;
    if has_column == 0 {
        conn.execute(
            "ALTER TABLE nodes ADD COLUMN extra TEXT NOT NULL DEFAULT ''",
            [],
        )?;
        // Check if there are existing nodes that need backfill
        let existing: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap_or(0);
        if existing > 0 {
            eprintln!(
                "[codegraph] schema v4: added extra column for {existing} existing nodes. \
                 Run `build_full_index` to populate column-precision position data."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{META_SCHEMA_VERSION_KEY, init_schema, migrate_schema};
    use crate::SCHEMA_VERSION;
    use crate::db::index_ops::get_meta;

    #[test]
    fn schema_initializes_on_memory_db() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        init_schema(&conn).expect("initialize schema");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .expect("query row from DB");
        assert!(count >= 4);
    }

    #[test]
    fn fresh_schema_stamps_current_version() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        init_schema(&conn).expect("initialize schema");
        migrate_schema(&conn).expect("initialize schema");
        assert_eq!(
            get_meta(&conn, META_SCHEMA_VERSION_KEY)
                .expect("initialize schema")
                .as_deref(),
            Some(SCHEMA_VERSION)
        );
    }

    #[test]
    fn legacy_v2_db_migrates_content_hash_column() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        conn.execute_batch(
            r#"
            CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('schema_version', 'codegraph-rs-v2');
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
        .expect("should succeed");
        migrate_schema(&conn).expect("migrate schema");
        assert_eq!(
            get_meta(&conn, META_SCHEMA_VERSION_KEY)
                .expect("migrate schema")
                .as_deref(),
            Some(SCHEMA_VERSION)
        );
        let has_hash: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'content_hash'",
                [],
                |r| r.get(0),
            )
            .expect("should succeed");
        assert_eq!(has_hash, 1);
    }

    #[test]
    fn legacy_v1_db_migrates_to_current() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
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
        .expect("should succeed");
        migrate_schema(&conn).expect("migrate schema");
        assert_eq!(
            get_meta(&conn, META_SCHEMA_VERSION_KEY)
                .expect("migrate schema")
                .as_deref(),
            Some(SCHEMA_VERSION)
        );
        let pair_index: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_edges_pair'",
                [],
                |r| r.get(0),
            )
            .expect("should succeed");
        assert_eq!(pair_index, 1);
    }

    #[test]
    fn fts_trigger_syncs_on_insert() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["n1", "alpha_fn", "fn", "rust", "a.rs", 1],
        )
        .expect("should succeed");
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH ?1",
                ["alpha"],
                |r| r.get(0),
            )
            .expect("should succeed");
        assert_eq!(hits, 1);
    }
}
