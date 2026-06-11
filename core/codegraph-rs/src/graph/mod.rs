pub mod sync;

use crate::db::node_ops::SymbolFilter;
use crate::{ImpactReport, Node};
use rusqlite::{params, Connection, Statement};

pub use sync::{build_full_index, incremental_sync, IndexWatcher, SyncReport};

/// Cached prepared statements for graph traversal queries.
/// Avoids re-preparing SQL on every BFS hop.
struct GraphStmts<'conn> {
    direct_callers: Statement<'conn>,
    direct_callees: Statement<'conn>,
}

impl<'conn> GraphStmts<'conn> {
    fn prepare(conn: &'conn Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            direct_callers: conn.prepare(
                r#"
                SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
                FROM edges e
                JOIN nodes callee ON callee.id = e.callee_id
                JOIN nodes n ON n.id = e.caller_id
                WHERE callee.symbol = ?1
                  AND (?2 IS NULL OR callee.file_path = ?2)
                  AND (?3 IS NULL OR callee.id = ?3)
                LIMIT 64
                "#,
            )?,
            direct_callees: conn.prepare(
                r#"
                SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line
                FROM edges e
                JOIN nodes caller ON caller.id = e.caller_id
                JOIN nodes n ON n.id = e.callee_id
                WHERE caller.symbol = ?1
                  AND (?2 IS NULL OR caller.file_path = ?2)
                  AND (?3 IS NULL OR caller.id = ?3)
                LIMIT 64
                "#,
            )?,
        })
    }

    fn direct_callers(
        &mut self,
        symbol: &str,
        filter: &SymbolFilter,
    ) -> rusqlite::Result<Vec<Node>> {
        let rows = self.direct_callers.query_map(
            params![symbol, filter.file_path, filter.node_id],
            map_row,
        )?;
        let result: rusqlite::Result<Vec<Node>> = rows.collect();
        result
    }

    fn direct_callees(
        &mut self,
        symbol: &str,
        filter: &SymbolFilter,
    ) -> rusqlite::Result<Vec<Node>> {
        let rows = self.direct_callees.query_map(
            params![symbol, filter.file_path, filter.node_id],
            map_row,
        )?;
        let result: rusqlite::Result<Vec<Node>> = rows.collect();
        result
    }
}

pub fn find_callers(
    conn: &Connection,
    symbol: &str,
    depth: u32,
    filter: &SymbolFilter,
) -> rusqlite::Result<Vec<Node>> {
    let depth = depth.max(1);
    let mut seen = std::collections::HashSet::new();
    let mut frontier = vec![(symbol.to_string(), filter.clone())];
    let mut out = Vec::new();
    let mut stmts = GraphStmts::prepare(conn)?;
    for _ in 0..depth {
        let mut next = Vec::new();
        for (sym, hop_filter) in &frontier {
            for node in stmts.direct_callers(sym, hop_filter)? {
                if seen.insert(node.id.clone()) {
                    next.push((
                        node.symbol.clone(),
                        SymbolFilter::from_options(Some(&node.file_path), None),
                    ));
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

pub fn find_callees(
    conn: &Connection,
    symbol: &str,
    depth: u32,
    filter: &SymbolFilter,
) -> rusqlite::Result<Vec<Node>> {
    let depth = depth.max(1);
    let mut seen = std::collections::HashSet::new();
    let mut frontier = vec![(symbol.to_string(), filter.clone())];
    let mut out = Vec::new();
    let mut stmts = GraphStmts::prepare(conn)?;
    for _ in 0..depth {
        let mut next = Vec::new();
        for (sym, hop_filter) in &frontier {
            for node in stmts.direct_callees(sym, hop_filter)? {
                if seen.insert(node.id.clone()) {
                    next.push((
                        node.symbol.clone(),
                        SymbolFilter::from_options(Some(&node.file_path), None),
                    ));
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

pub fn impact_radius(
    conn: &Connection,
    symbol: &str,
    depth: u32,
    filter: &SymbolFilter,
) -> rusqlite::Result<ImpactReport> {
    let callers = find_callers(conn, symbol, depth, filter)?;
    let callees = find_callees(conn, symbol, depth, filter)?;
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
    use super::{find_callers, find_callees, impact_radius};
    use crate::db::node_ops::SymbolFilter;
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
        let filter = SymbolFilter::from_options(None, None);
        let callers = find_callers(&conn, "target", 1, &filter).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].symbol, "caller");
    }

    #[test]
    fn find_callees_supports_depth() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        seed_graph(&conn);
        let filter = SymbolFilter::from_options(None, None);
        // depth=2 from "caller" should reach "target" (d1) and "leaf" (d2)
        let callees = find_callees(&conn, "caller", 2, &filter).unwrap();
        assert_eq!(callees.len(), 2);
        let symbols: Vec<&str> = callees.iter().map(|n| n.symbol.as_str()).collect();
        assert!(symbols.contains(&"target"));
        assert!(symbols.contains(&"leaf"));
    }

    #[test]
    fn impact_radius_includes_both_directions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        seed_graph(&conn);
        let filter = SymbolFilter::from_options(None, None);
        let report = impact_radius(&conn, "target", 2, &filter).unwrap();
        assert_eq!(report.callers.len(), 1);
        // callees now supports BFS depth=2
        assert_eq!(report.callees.len(), 1);
        assert_eq!(report.callees[0].symbol, "leaf");
    }

    #[test]
    fn duplicate_symbol_callers_do_not_cross_files() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        for (id, path, caller) in [
            ("a1", "a.rs", "caller_a"),
            ("a2", "a.rs", "shared"),
            ("b1", "b.rs", "caller_b"),
            ("b2", "b.rs", "shared"),
        ] {
            conn.execute(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, 'fn', 'rust', ?3, 1)",
                rusqlite::params![id, caller, path],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO edges (caller_id, callee_id) VALUES ('a1', 'a2'), ('b1', 'b2')",
            [],
        )
        .unwrap();

        let a_callers = find_callers(
            &conn,
            "shared",
            1,
            &SymbolFilter::from_options(Some("a.rs"), None),
        )
        .unwrap();
        assert_eq!(a_callers.len(), 1);
        assert_eq!(a_callers[0].symbol, "caller_a");

        let b_callees = find_callees(
            &conn,
            "caller_b",
            1,
            &SymbolFilter::from_options(Some("b.rs"), None),
        )
        .unwrap();
        assert_eq!(b_callees.len(), 1);
        assert_eq!(b_callees[0].symbol, "shared");
        assert_eq!(b_callees[0].file_path, "b.rs");
    }
}
