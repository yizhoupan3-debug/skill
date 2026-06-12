use crate::Node;
use rusqlite::{params, Connection};

pub fn search_symbols(
    conn: &Connection,
    query: &str,
    kind: Option<&str>,
    language: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Node>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    // Reject excessively long queries to prevent slow FTS/LIKE scans
    if trimmed.len() > 4096 {
        return Ok(Vec::new());
    }
    // Strip FTS5 special operators to prevent query injection: + - * ^ " ( ) :
    let sanitized: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '+' | '-' | '*' | '^' | '"' | '(' | ')' | ':'))
        .collect();
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }
    let fts_query = format!("\"{sanitized}\"*");

    // Push kind/language filtering into SQL WHERE instead of post-filtering in Rust.
    // This avoids the limit*4 over-fetch and ensures correct results even with narrow filters.
    let mut stmt = conn.prepare(
        r#"
        SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
        FROM nodes_fts f
        JOIN nodes n ON n.rowid = f.rowid
        WHERE nodes_fts MATCH ?1
          AND (?2 IS NULL OR n.kind = ?2)
          AND (?3 IS NULL OR n.language = ?3)
        ORDER BY bm25(nodes_fts)
        LIMIT ?4
        "#,
    )?;
    let rows = stmt.query_map(
        params![fts_query, kind, language, limit as i64],
        row_to_node,
    )?;
    let out: Vec<Node> = rows.collect::<Result<_, _>>()?;

    if out.is_empty() {
        return search_symbols_like(conn, trimmed, kind, language, limit);
    }
    Ok(out)
}

fn search_symbols_like(
    conn: &Connection,
    query: &str,
    kind: Option<&str>,
    language: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Node>> {
    // Escape LIKE wildcards: backslash first, then % and _
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let like = format!("%{escaped}%");
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line FROM nodes
         WHERE symbol LIKE ?1 ESCAPE '\\'
           AND (?2 IS NULL OR kind = ?2)
           AND (?3 IS NULL OR language = ?3)
         ORDER BY symbol LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![like, kind, language, limit as i64], row_to_node)?;
    rows.collect()
}

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    Ok(Node {
        id: row.get(0)?,
        symbol: row.get(1)?,
        kind: row.get(2)?,
        language: row.get(3)?,
        file_path: row.get(4)?,
        line: row.get::<_, i64>(5)? as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::search_symbols;
    use crate::db::index_ops::ingest_parsed_file;
    use crate::db::schema::init_schema;
    use crate::parser::{ParsedFile, ParsedSymbol};

    fn seed_test_data(conn: &rusqlite::Connection) {
        init_schema(conn).expect("initialize schema");
        ingest_parsed_file(
            conn,
            &ParsedFile {
                path: "a.rs".to_string(),
                language: "rust".to_string(),
                mtime_ns: 1,
                content_hash: "hash-a".to_string(),
                symbols: vec![
                    ParsedSymbol {
                        symbol: "search_me".to_string(),
                        kind: "fn".to_string(),
                        line: 1,
                    },
                    ParsedSymbol {
                        symbol: "search_struct".to_string(),
                        kind: "struct".to_string(),
                        line: 5,
                    },
                ],
                edges: vec![],
            },
        )
        .expect("should succeed");
        ingest_parsed_file(
            conn,
            &ParsedFile {
                path: "b.py".to_string(),
                language: "python".to_string(),
                mtime_ns: 2,
                content_hash: "hash-b".to_string(),
                symbols: vec![ParsedSymbol {
                    symbol: "search_py".to_string(),
                    kind: "function".to_string(),
                    line: 1,
                }],
                edges: vec![],
            },
        )
        .expect("should succeed");
    }

    #[test]
    fn search_returns_matching_symbols_via_fts() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed_test_data(&conn);
        let hits = search_symbols(&conn, "search", None, None, 10).expect("search symbols");
        assert!(hits.len() >= 2, "expected multiple search results");
    }

    #[test]
    fn search_filters_by_kind_in_sql() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed_test_data(&conn);
        let hits = search_symbols(&conn, "search", Some("fn"), None, 10).expect("search symbols");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "search_me");
        assert_eq!(hits[0].kind, "fn");
    }

    #[test]
    fn search_filters_by_language_in_sql() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed_test_data(&conn);
        let hits = search_symbols(&conn, "search", None, Some("python"), 10).expect("search symbols");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "search_py");
    }
}
