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
    // Strip FTS5 special operators to prevent query injection: + - * ^ " ( ) :
    let sanitized: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '+' | '-' | '*' | '^' | '"' | '(' | ')' | ':'))
        .collect();
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }
    let fts_query = format!("\"{sanitized}\"*");
    let mut stmt = conn.prepare(
        r#"
        SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
        FROM nodes_fts f
        JOIN nodes n ON n.rowid = f.rowid
        WHERE nodes_fts MATCH ?1
        ORDER BY bm25(nodes_fts)
        LIMIT ?2
        "#,
    )?;
    let rows = stmt.query_map(params![fts_query, (limit * 4) as i64], row_to_node)?;
    let mut out = Vec::new();
    for row in rows {
        let node = row?;
        if let Some(k) = kind {
            if node.kind != k {
                continue;
            }
        }
        if let Some(lang) = language {
            if node.language != lang {
                continue;
            }
        }
        out.push(node);
        if out.len() >= limit {
            break;
        }
    }
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
    // Escape LIKE wildcards % and _ to prevent injection
    let escaped = query.replace('%', "\\%").replace('_', "\\_");
    let like = format!("%{escaped}%");
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line FROM nodes WHERE symbol LIKE ?1 ESCAPE '\\' ORDER BY symbol LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![like, (limit * 4) as i64], row_to_node)?;
    let mut out = Vec::new();
    for row in rows {
        let node = row?;
        if let Some(k) = kind {
            if node.kind != k {
                continue;
            }
        }
        if let Some(lang) = language {
            if node.language != lang {
                continue;
            }
        }
        out.push(node);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
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

    #[test]
    fn search_returns_matching_symbols_via_fts() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        ingest_parsed_file(
            &conn,
            &ParsedFile {
                path: "a.rs".to_string(),
                language: "rust".to_string(),
                mtime_ns: 1,
                content_hash: "hash-a".to_string(),
                symbols: vec![ParsedSymbol {
                    symbol: "search_me".to_string(),
                    kind: "fn".to_string(),
                    line: 1,
                }],
                edges: vec![],
            },
        )
        .unwrap();
        let hits = search_symbols(&conn, "search", None, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "search_me");
    }
}
