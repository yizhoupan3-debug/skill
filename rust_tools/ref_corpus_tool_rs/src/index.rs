use crate::chunk::chunk_text;
use crate::db::{clear_corpus, open};
use anyhow::Result;
use chrono::Utc;
use pdf_tool_rs::read::{file_sha256, page_count, ReadOptions, read_pdf};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub documents: u64,
    pub chunks: u64,
    pub db_path: String,
    pub corpus_dir: String,
}

pub struct IndexOptions {
    pub corpus_dir: PathBuf,
    pub db_path: PathBuf,
    pub max_chars: usize,
    pub overlap: usize,
    pub resume: bool,
}

pub fn default_db_path(project_root: &Path) -> PathBuf {
    project_root.join("artifacts/ref_corpus/index.sqlite")
}

pub fn index_corpus(opts: &IndexOptions) -> Result<IndexStats> {
    let conn = open(&opts.db_path)?;
    if !opts.resume {
        clear_corpus(&conn)?;
    }
    let mut doc_count = 0u64;
    let mut chunk_count = 0u64;
    for entry in WalkDir::new(&opts.corpus_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("pdf") {
            continue;
        }
        let sha = file_sha256(path)?;
        if opts.resume && document_up_to_date(&conn, path, &sha)? {
            continue;
        }
        let (n_chunks, _) = index_one_pdf(&conn, path, &sha, opts.max_chars, opts.overlap)?;
        doc_count += 1;
        chunk_count += n_chunks as u64;
    }
    Ok(IndexStats {
        documents: doc_count,
        chunks: chunk_count,
        db_path: opts.db_path.to_string_lossy().into_owned(),
        corpus_dir: opts.corpus_dir.to_string_lossy().into_owned(),
    })
}

fn document_up_to_date(conn: &Connection, path: &Path, sha: &str) -> Result<bool> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT sha256 FROM documents WHERE path = ?1",
            params![path.to_string_lossy().as_ref()],
            |row| row.get(0),
        )
        .ok();
    Ok(stored.as_deref() == Some(sha))
}

fn index_one_pdf(
    conn: &Connection,
    path: &Path,
    sha: &str,
    max_chars: usize,
    overlap: usize,
) -> Result<(usize, u32)> {
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    let pages = page_count(path).unwrap_or(0);
    let read = read_pdf(
        path,
        &ReadOptions {
            max_chars: 2_000_000,
            text_out_dir: None,
        },
    )?;
    let path_s = path.to_string_lossy().to_string();
    conn.execute("DELETE FROM documents WHERE path = ?1", params![path_s])?;
    conn.execute(
        "INSERT INTO documents(path, sha256, title, page_count, indexed_at) VALUES (?1,?2,?3,?4,?5)",
        params![path_s, sha, title, pages, Utc::now().to_rfc3339()],
    )?;
    let doc_id = conn.last_insert_rowid();
    let chunks = chunk_text(&read.text, max_chars, overlap);
    for (i, (page_hint, body)) in chunks.iter().enumerate() {
        conn.execute(
            "INSERT INTO chunks(doc_id, chunk_index, page_hint, body) VALUES (?1,?2,?3,?4)",
            params![doc_id, i as i64, *page_hint as i64, body],
        )?;
    }
    Ok((chunks.len(), pages))
}

pub fn corpus_stats(db_path: &Path) -> Result<IndexStats> {
    let conn = open(db_path)?;
    let documents: u64 = conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
    let chunks: u64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    Ok(IndexStats {
        documents,
        chunks,
        db_path: db_path.to_string_lossy().into_owned(),
        corpus_dir: String::new(),
    })
}

