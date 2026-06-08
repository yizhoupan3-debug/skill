use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

pub const SCHEMA_VERSION: &str = "ref-corpus-v1";

pub fn open(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path).with_context(|| format!("open {}", db_path.display()))?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            sha256 TEXT NOT NULL,
            title TEXT NOT NULL,
            page_count INTEGER NOT NULL DEFAULT 0,
            indexed_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            doc_id INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            page_hint INTEGER NOT NULL DEFAULT 0,
            body TEXT NOT NULL,
            FOREIGN KEY(doc_id) REFERENCES documents(id) ON DELETE CASCADE,
            UNIQUE(doc_id, chunk_index)
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            title,
            body,
            doc_path UNINDEXED,
            chunk_index UNINDEXED,
            page_hint UNINDEXED
        );
        CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
            INSERT INTO chunks_fts(rowid, title, body, doc_path, chunk_index, page_hint)
            SELECT NEW.id, (SELECT title FROM documents WHERE id = NEW.doc_id), NEW.body,
                   (SELECT path FROM documents WHERE id = NEW.doc_id), NEW.chunk_index, NEW.page_hint;
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, title, body, doc_path, chunk_index, page_hint)
            VALUES('delete', OLD.id, '', '', '', 0, 0);
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, title, body, doc_path, chunk_index, page_hint)
            VALUES('delete', OLD.id, '', '', '', 0, 0);
            INSERT INTO chunks_fts(rowid, title, body, doc_path, chunk_index, page_hint)
            SELECT NEW.id, (SELECT title FROM documents WHERE id = NEW.doc_id), NEW.body,
                   (SELECT path FROM documents WHERE id = NEW.doc_id), NEW.chunk_index, NEW.page_hint;
        END;
        "#,
    )?;
    conn.execute(
        "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_VERSION],
    )?;
    Ok(())
}

pub fn clear_corpus(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM chunks_fts; DELETE FROM chunks; DELETE FROM documents;",
    )?;
    Ok(())
}
