use crate::Node;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolFilter {
    pub file_path: Option<String>,
    pub node_id: Option<String>,
}

impl SymbolFilter {
    pub fn from_options(file_path: Option<&str>, node_id: Option<&str>) -> Self {
        Self {
            file_path: file_path.map(str::to_string),
            node_id: node_id.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveOutcome {
    Found(Node),
    Ambiguous(Vec<Node>),
    NotFound,
}

pub fn get_node_by_id(conn: &Connection, id: &str) -> rusqlite::Result<Option<Node>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line FROM nodes WHERE id = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row_to_node(row)?));
    }
    Ok(None)
}

pub fn resolve_symbol(conn: &Connection, symbol: &str) -> rusqlite::Result<Option<Node>> {
    match resolve_symbol_filtered(conn, symbol, &SymbolFilter::from_options(None, None))? {
        ResolveOutcome::Found(node) => Ok(Some(node)),
        ResolveOutcome::Ambiguous(nodes) => Ok(nodes.into_iter().next()),
        ResolveOutcome::NotFound => Ok(None),
    }
}

pub fn resolve_symbol_filtered(
    conn: &Connection,
    symbol: &str,
    filter: &SymbolFilter,
) -> rusqlite::Result<ResolveOutcome> {
    if let Some(node_id) = filter.node_id.as_deref() {
        return match get_node_by_id(conn, node_id)? {
            Some(node) if node.symbol == symbol => Ok(ResolveOutcome::Found(node)),
            _ => Ok(ResolveOutcome::NotFound),
        };
    }

    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line
         FROM nodes
         WHERE symbol = ?1
           AND (?2 IS NULL OR file_path = ?2)
         ORDER BY file_path, line
         LIMIT 32",
    )?;
    let file_path = filter.file_path.as_deref();
    let rows = stmt.query_map(params![symbol, file_path], row_to_node)?;
    let nodes: Vec<Node> = rows.collect::<Result<_, _>>()?;
    if nodes.len() <= 1 {
        Ok(nodes
            .into_iter()
            .next()
            .map_or(ResolveOutcome::NotFound, ResolveOutcome::Found))
    } else {
        Ok(ResolveOutcome::Ambiguous(nodes))
    }
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
    use super::{
        get_node_by_id, resolve_symbol, resolve_symbol_filtered, ResolveOutcome, SymbolFilter,
    };
    use crate::db::schema::init_schema;

    fn seed(conn: &rusqlite::Connection) {
        init_schema(conn).expect("initialize schema");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["n1", "foo", "fn", "rust", "a.rs", 1],
        )
        .expect("should succeed");
    }

    #[test]
    fn resolve_symbol_finds_node() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let node = resolve_symbol(&conn, "foo").expect("resolve symbol").expect("resolve symbol");
        assert_eq!(node.id, "n1");
        assert_eq!(get_node_by_id(&conn, "n1").expect("resolve symbol").expect("resolve symbol").symbol, "foo");
    }

    #[test]
    fn resolve_symbol_reports_not_found() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let outcome = resolve_symbol(&conn, "nonexistent").expect("resolve symbol");
        assert!(outcome.is_none());
    }

    #[test]
    fn resolve_symbol_filtered_empty_result() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let outcome = resolve_symbol_filtered(
            &conn,
            "ghost",
            &SymbolFilter::from_options(None, None),
        )
        .expect("should succeed");
        assert!(matches!(outcome, ResolveOutcome::NotFound));
    }

    #[test]
    fn resolve_symbol_reports_ambiguous_matches() {
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        init_schema(&conn).expect("initialize schema");
        for (id, path) in [("n1", "a.rs"), ("n2", "b.rs")] {
            conn.execute(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, 'dup', 'fn', 'rust', ?2, 1)",
                rusqlite::params![id, path],
            )
            .expect("should succeed");
        }
        let outcome =
            resolve_symbol_filtered(&conn, "dup", &SymbolFilter::from_options(None, None)).expect("resolve symbol filtered");
        assert!(matches!(outcome, ResolveOutcome::Ambiguous(ref nodes) if nodes.len() == 2));

        let filtered =
            resolve_symbol_filtered(&conn, "dup", &SymbolFilter::from_options(Some("b.rs"), None))
                .expect("resolve symbol filtered");
        assert!(matches!(filtered, ResolveOutcome::Found(ref node) if node.id == "n2"));
    }

    #[test]
    fn get_node_by_id_propagates_db_error() {
        // Drop the connection so subsequent queries fail
        let conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        drop(conn);
        // Reopen a raw in-memory connection without schema, querying should error
        let bad_conn = rusqlite::Connection::open_in_memory().expect("rusqlite::Connection::open_in_memory should succeed");
        let result = get_node_by_id(&bad_conn, "n1");
        assert!(result.is_err(), "expected DB error for missing table");
    }
}
