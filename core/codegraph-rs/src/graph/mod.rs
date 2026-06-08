pub mod sync;

use crate::{ImpactReport, Node};
use rusqlite::{params, Connection};

pub use sync::{
    build_full_index, incremental_sync, IndexWatcher, SyncReport,
};

pub fn find_callers(conn: &Connection, symbol: &str, depth: u32) -> rusqlite::Result<Vec<Node>> {
    let depth = depth.max(1);
    let mut seen = std::collections::HashSet::new();
    let mut frontier = vec![symbol.to_string()];
    let mut out = Vec::new();
    for _ in 0..depth {
        let mut next = Vec::new();
        for sym in frontier {
            for node in direct_callers(conn, &sym)? {
                if seen.insert(node.id.clone()) {
                    next.push(node.symbol.clone());
                    out.push(node);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(out)
}

fn direct_callers(conn: &Connection, symbol: &str) -> rusqlite::Result<Vec<Node>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
        FROM edges e
        JOIN nodes callee ON callee.id = e.callee_id
        JOIN nodes n ON n.id = e.caller_id
        WHERE callee.symbol = ?1
        LIMIT 64
        "#,
    )?;
    let rows = stmt.query_map(params![symbol], map_row)?;
    rows.collect()
}

pub fn find_callees(conn: &Connection, symbol: &str) -> rusqlite::Result<Vec<Node>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
        FROM edges e
        JOIN nodes caller ON caller.id = e.caller_id
        JOIN nodes n ON n.id = e.callee_id
        WHERE caller.symbol = ?1
        LIMIT 64
        "#,
    )?;
    let rows = stmt.query_map(params![symbol], map_row)?;
    rows.collect()
}

pub fn impact_radius(conn: &Connection, symbol: &str, depth: u32) -> rusqlite::Result<ImpactReport> {
    let callers = find_callers(conn, symbol, depth)?;
    let callees = find_callees(conn, symbol)?;
    Ok(ImpactReport {
        symbol: symbol.to_string(),
        depth,
        callers,
        callees,
    })
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
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
    use super::{find_callers, impact_radius};
    use crate::db::schema::init_schema;

    fn seed_graph(conn: &rusqlite::Connection) {
        init_schema(conn).unwrap();
        for (id, sym) in [("a", "caller"), ("b", "target"), ("c", "leaf")] {
            conn.execute(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, 'fn', 'rust', 'f.rs', 1)",
                rusqlite::params![id, sym],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO edges (caller_id, callee_id) VALUES ('a', 'b'), ('b', 'c')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn find_callers_returns_direct_caller() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        seed_graph(&conn);
        let callers = find_callers(&conn, "target", 1).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].symbol, "caller");
    }

    #[test]
    fn impact_radius_includes_both_directions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        seed_graph(&conn);
        let report = impact_radius(&conn, "target", 2).unwrap();
        assert_eq!(report.callers.len(), 1);
        assert_eq!(report.callees.len(), 1);
    }
}
