use crate::db::open;
use anyhow::{Context, Result};
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub doc_path: String,
    pub title: String,
    pub chunk_index: i64,
    pub page_hint: i64,
    pub snippet: String,
    pub rank: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub db_path: String,
}

pub fn search_corpus(db_path: &Path, query: &str, limit: usize) -> Result<SearchResult> {
    let conn = open(db_path)?;
    let fts = build_fts_query(query);
    if fts.is_empty() {
        return Ok(SearchResult {
            query: query.to_string(),
            hits: vec![],
            db_path: db_path.to_string_lossy().into_owned(),
        });
    }
    let mut stmt = conn.prepare(
        "SELECT doc_path, title, chunk_index, page_hint,
                snippet(chunks_fts, 1, '>>', '<<', '…', 24) AS snip,
                bm25(chunks_fts) AS rank
         FROM chunks_fts
         WHERE chunks_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;
    let hits = stmt
        .query_map(params![fts, limit as i64], |row| {
            Ok(SearchHit {
                doc_path: row.get(0)?,
                title: row.get(1)?,
                chunk_index: row.get(2)?,
                page_hint: row.get(3)?,
                snippet: row.get(4)?,
                rank: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .context("search query")?;
    Ok(SearchResult {
        query: query.to_string(),
        hits,
        db_path: db_path.to_string_lossy().into_owned(),
    })
}

fn build_fts_query(user: &str) -> String {
    user.split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| {
            let escaped = w.replace('"', "");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::build_fts_query;

    #[test]
    fn fts_joins_terms() {
        assert_eq!(build_fts_query("foo bar"), "\"foo\" OR \"bar\"");
    }
}
