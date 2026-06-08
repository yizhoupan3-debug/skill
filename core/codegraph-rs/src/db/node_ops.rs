use crate::Node;
use rusqlite::{params, Connection};

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
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line FROM nodes WHERE symbol = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(params![symbol])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row_to_node(row)?));
    }
    Ok(None)
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
    use super::{get_node_by_id, resolve_symbol};
    use crate::db::schema::init_schema;

    fn seed(conn: &rusqlite::Connection) {
        init_schema(conn).unwrap();
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["n1", "foo", "fn", "rust", "a.rs", 1],
        )
        .unwrap();
    }

    #[test]
    fn resolve_symbol_finds_node() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        seed(&conn);
        let node = resolve_symbol(&conn, "foo").unwrap().unwrap();
        assert_eq!(node.id, "n1");
        assert_eq!(get_node_by_id(&conn, "n1").unwrap().unwrap().symbol, "foo");
    }
}
