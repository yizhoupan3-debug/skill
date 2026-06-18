use crate::Node;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// A dead-code candidate node with caller count metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadCodeNode {
    pub id: String,
    pub symbol: String,
    pub kind: String,
    pub language: String,
    pub file_path: String,
    pub line: u32,
    pub callers_count: u32,
}

/// Find function/method nodes that have zero callers (in-degree = 0 in the call graph).
///
/// Uses a single SQL query with a LEFT JOIN on the edges table via the
/// `idx_edges_callee` index for performance (<100ms on typical repos).
///
/// - `language`: optional filter by language
/// - `min_lines`: optional minimum line number filter (applied in Rust post-filter)
pub fn find_dead_code(
    conn: &Connection,
    language: Option<&str>,
    min_lines: Option<u32>,
) -> rusqlite::Result<Vec<DeadCodeNode>> {
    let mut sql = String::from(
        "SELECT n.id, n.symbol, n.kind, n.language, n.file_path, n.line,
                COALESCE(cnt.callers, 0) AS callers_count
         FROM nodes n
         LEFT JOIN (
             SELECT callee_id, COUNT(*) AS callers
             FROM edges GROUP BY callee_id
         ) cnt ON cnt.callee_id = n.id
         WHERE n.kind IN ('fn', 'function', 'method')
           AND COALESCE(cnt.callers, 0) = 0",
    );
    if language.is_some() {
        sql.push_str("\n           AND n.language = ?1");
    }
    sql.push_str("\n         ORDER BY n.file_path, n.line");
    let mut stmt = conn.prepare(&sql)?;
    let rows = if let Some(lang) = language {
        stmt.query_map(params![lang], row_to_dead_code_node)?
    } else {
        stmt.query_map([], row_to_dead_code_node)?
    };
    let mut result: Vec<DeadCodeNode> = Vec::new();
    for row in rows {
        let node = row?;
        if let Some(min) = min_lines
            && node.line < min {
                continue;
            }
        result.push(node);
    }
    Ok(result)
}

/// Lightweight dead code count — fetches only COUNT(*), no node data.
/// Used by framework_snapshot hot path to avoid full row transfer.
pub fn count_dead_code_only(
    conn: &Connection,
    language: Option<&str>,
) -> rusqlite::Result<usize> {
    let mut sql = String::from(
        "SELECT COUNT(*)
         FROM nodes n
         LEFT JOIN (
             SELECT callee_id, COUNT(*) AS callers
             FROM edges GROUP BY callee_id
         ) cnt ON cnt.callee_id = n.id
         WHERE n.kind IN ('fn', 'function', 'method')
           AND COALESCE(cnt.callers, 0) = 0",
    );
    if language.is_some() {
        sql.push_str("\n           AND n.language = ?1");
    }
    let mut stmt = conn.prepare(&sql)?;
    if let Some(lang) = language {
        stmt.query_row(params![lang], |row| row.get::<_, i64>(0))
            .map(|c| c as usize)
    } else {
        stmt.query_row([], |row| row.get::<_, i64>(0))
            .map(|c| c as usize)
    }
}

fn row_to_dead_code_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeadCodeNode> {
    Ok(DeadCodeNode {
        id: row.get(0)?,
        symbol: row.get(1)?,
        kind: row.get(2)?,
        language: row.get(3)?,
        file_path: row.get(4)?,
        line: row.get::<_, i64>(5)? as u32,
        callers_count: row.get::<_, i64>(6)? as u32,
    })
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

/// Result of a definition lookup.
#[derive(Debug, Clone, Serialize)]
pub struct DefinitionResult {
    pub id: String,
    pub symbol: String,
    pub kind: String,
    pub language: String,
    pub file_path: String,
    pub line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Find symbol definitions by exact symbol name.
///
/// Prioritizes definitions (fn, function, method, class, struct, enum,
/// trait, interface, type) over usage/reference nodes.
pub fn find_definition(
    conn: &Connection,
    symbol: &str,
    file_path: Option<&str>,
) -> rusqlite::Result<Vec<DefinitionResult>> {
    let (where_clause, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(fp) = file_path {
            (
                "WHERE symbol = ?1 AND file_path = ?2".to_string(),
                vec![Box::new(symbol.to_string()), Box::new(fp.to_string())],
            )
        } else {
            (
                "WHERE symbol = ?1".to_string(),
                vec![Box::new(symbol.to_string())],
            )
        };
    let sql = format!(
        "SELECT id, symbol, kind, language, file_path, line, extra
         FROM nodes {}
         ORDER BY
           CASE WHEN kind IN ('fn','function','method','class','struct','enum','trait','interface','type') THEN 0 ELSE 1 END,
           file_path, line
         LIMIT 10",
        where_clause
    );
    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let extra_str: String = row.get(6)?;
        let (sc, el, ec) = parse_extra_position(&extra_str);
        Ok(DefinitionResult {
            id: row.get(0)?,
            symbol: row.get(1)?,
            kind: row.get(2)?,
            language: row.get(3)?,
            file_path: row.get(4)?,
            line: row.get::<_, i64>(5)? as u32,
            start_col: sc,
            end_line: el,
            end_col: ec,
        })
    })?;
    rows.collect()
}

/// Parse the `extra` column JSON blob into (start_col, end_line, end_col).
/// Returns (0, 0, 0) when the blob is empty or unparseable.
fn parse_extra_position(extra: &str) -> (u32, u32, u32) {
    if extra.is_empty() || extra == "{}" {
        return (0, 0, 0);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(extra) {
        let sc = v.get("sc").and_then(Value::as_u64).unwrap_or(0) as u32;
        let el = v.get("el").and_then(Value::as_u64).unwrap_or(0) as u32;
        let ec = v.get("ec").and_then(Value::as_u64).unwrap_or(0) as u32;
        return (sc, el, ec);
    }
    (0, 0, 0)
}

/// Find all symbols defined in a specific file.
pub fn find_symbols_by_file(
    conn: &Connection,
    file_path: &str,
) -> rusqlite::Result<Vec<Node>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line
         FROM nodes WHERE file_path = ?1
         ORDER BY line"
    )?;
    let rows = stmt.query_map([file_path], |row| {
        Ok(Node {
            id: row.get(0)?,
            symbol: row.get(1)?,
            kind: row.get(2)?,
            language: row.get(3)?,
            file_path: row.get(4)?,
            line: row.get::<_, i64>(5)? as u32,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ResolveOutcome, SymbolFilter, count_dead_code_only, find_dead_code,
        find_definition, find_symbols_by_file, get_node_by_id, resolve_symbol,
        resolve_symbol_filtered,
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
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let node = resolve_symbol(&conn, "foo")
            .expect("resolve symbol")
            .expect("resolve symbol");
        assert_eq!(node.id, "n1");
        assert_eq!(
            get_node_by_id(&conn, "n1")
                .expect("resolve symbol")
                .expect("resolve symbol")
                .symbol,
            "foo"
        );
    }

    #[test]
    fn resolve_symbol_reports_not_found() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let outcome = resolve_symbol(&conn, "nonexistent").expect("resolve symbol");
        assert!(outcome.is_none());
    }

    #[test]
    fn resolve_symbol_filtered_empty_result() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        let outcome =
            resolve_symbol_filtered(&conn, "ghost", &SymbolFilter::from_options(None, None))
                .expect("should succeed");
        assert!(matches!(outcome, ResolveOutcome::NotFound));
    }

    #[test]
    fn resolve_symbol_reports_ambiguous_matches() {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        init_schema(&conn).expect("initialize schema");
        for (id, path) in [("n1", "a.rs"), ("n2", "b.rs")] {
            conn.execute(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, 'dup', 'fn', 'rust', ?2, 1)",
                rusqlite::params![id, path],
            )
            .expect("should succeed");
        }
        let outcome =
            resolve_symbol_filtered(&conn, "dup", &SymbolFilter::from_options(None, None))
                .expect("resolve symbol filtered");
        assert!(matches!(outcome, ResolveOutcome::Ambiguous(ref nodes) if nodes.len() == 2));

        let filtered = resolve_symbol_filtered(
            &conn,
            "dup",
            &SymbolFilter::from_options(Some("b.rs"), None),
        )
        .expect("resolve symbol filtered");
        assert!(matches!(filtered, ResolveOutcome::Found(ref node) if node.id == "n2"));
    }

    #[test]
    fn get_node_by_id_propagates_db_error() {
        // Drop the connection so subsequent queries fail
        let conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        seed(&conn);
        drop(conn);
        // Reopen a raw in-memory connection without schema, querying should error
        let bad_conn = rusqlite::Connection::open_in_memory()
            .expect("rusqlite::Connection::open_in_memory should succeed");
        let result = get_node_by_id(&bad_conn, "n1");
        assert!(result.is_err(), "expected DB error for missing table");
    }

    #[test]
    fn find_dead_code_returns_uncalled_functions() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        // caller -> callee
        for (id, sym, kind) in [
            ("n1", "caller", "fn"),
            ("n2", "callee", "fn"),
            ("n3", "orphan", "fn"),
        ] {
            conn.execute(
                "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES (?1, ?2, ?3, 'rust', 'a.rs', ?4)",
                rusqlite::params![id, sym, kind, id.strip_prefix('n').unwrap().parse::<u32>().unwrap()],
            )
            .expect("should succeed");
        }
        conn.execute(
            "INSERT INTO edges (caller_id, callee_id) VALUES ('n1', 'n2')",
            [],
        )
        .expect("should succeed");

        let dead = find_dead_code(&conn, None, None).expect("find dead code");
        // caller has no callers (entry point), orphan has no callers
        // callee has 1 caller so is NOT dead
        let symbols: Vec<&str> = dead.iter().map(|n| n.symbol.as_str()).collect();
        assert!(
            symbols.contains(&"caller"),
            "caller should be dead (no callers)"
        );
        assert!(symbols.contains(&"orphan"), "orphan should be dead");
        assert!(!symbols.contains(&"callee"), "callee has a caller");
    }

    #[test]
    fn find_dead_code_filters_by_language() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n1', 'rust_fn', 'fn', 'rust', 'a.rs', 1)",
            [],
        )
        .expect("should succeed");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n2', 'py_fn', 'function', 'python', 'b.py', 1)",
            [],
        )
        .expect("should succeed");

        let rust_only = find_dead_code(&conn, Some("rust"), None).expect("find dead code");
        assert_eq!(rust_only.len(), 1);
        assert_eq!(rust_only[0].symbol, "rust_fn");

        let py_only = find_dead_code(&conn, Some("python"), None).expect("find dead code");
        assert_eq!(py_only.len(), 1);
        assert_eq!(py_only[0].symbol, "py_fn");
    }

    #[test]
    fn find_dead_code_respects_min_lines() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n1', 'early', 'fn', 'rust', 'a.rs', 5)",
            [],
        )
        .expect("should succeed");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n2', 'late', 'fn', 'rust', 'a.rs', 50)",
            [],
        )
        .expect("should succeed");

        let all = find_dead_code(&conn, None, None).expect("find dead code");
        assert_eq!(all.len(), 2);

        let filtered = find_dead_code(&conn, None, Some(10)).expect("find dead code");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].symbol, "late");
    }

    #[test]
    fn find_dead_code_excludes_structs_and_traits() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n1', 'MyStruct', 'struct', 'rust', 'a.rs', 1)",
            [],
        )
        .expect("should succeed");
        conn.execute(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n2', 'MyTrait', 'trait', 'rust', 'a.rs', 5)",
            [],
        )
        .expect("should succeed");

        let dead = find_dead_code(&conn, None, None).expect("find dead code");
        assert!(
            dead.is_empty(),
            "structs and traits should not appear in dead code"
        );
    }

    #[test]
    fn find_definition_returns_function_kinds_first() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute_batch(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line, extra) VALUES ('n1', 'doWork', 'fn', 'typescript', 'src/work.ts', 10, '{\"sc\":1,\"el\":15,\"ec\":1}');
             INSERT INTO nodes (id, symbol, kind, language, file_path, line, extra) VALUES ('n2', 'doWork', 'ref', 'typescript', 'src/test.ts', 5, '');"
        ).expect("should succeed");

        let defs = find_definition(&conn, "doWork", None).expect("find definition");
        assert_eq!(defs.len(), 2, "should find both fn and ref nodes");
        // fn kind should come first (ORDER BY CASE WHEN kind IN ('fn',...) THEN 0 ELSE 1 END)
        assert_eq!(defs[0].kind, "fn", "definition-kind should be sorted first");
        assert_eq!(defs[0].line, 10);
        assert_eq!(defs[0].start_col, 1);
        assert_eq!(defs[0].end_line, 15);
        assert_eq!(defs[0].end_col, 1);
        // ref kind should be second and have column defaults (0,0,0) from empty extra
        assert_eq!(defs[1].kind, "ref");
        assert_eq!(defs[1].start_col, 0);
    }

    #[test]
    fn find_definition_respects_file_path_filter() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute_batch(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line, extra) VALUES ('n1', 'helper', 'fn', 'go', 'a.go', 5, '');
             INSERT INTO nodes (id, symbol, kind, language, file_path, line, extra) VALUES ('n2', 'helper', 'fn', 'go', 'b.go', 3, '');"
        ).expect("should succeed");

        let all = find_definition(&conn, "helper", None).expect("find all");
        assert_eq!(all.len(), 2);

        let filtered = find_definition(&conn, "helper", Some("a.go")).expect("find filtered");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].file_path, "a.go");
    }

    #[test]
    fn find_symbols_by_file_returns_all_symbols_in_file() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute_batch(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n1', 'Config', 'struct', 'rust', 'config.rs', 3);
             INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n2', 'build', 'fn', 'rust', 'config.rs', 10);
             INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n3', 'other', 'fn', 'rust', 'other.rs', 1);"
        ).expect("should succeed");

        let symbols = find_symbols_by_file(&conn, "config.rs").expect("find symbols by file");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].symbol, "Config");
        assert_eq!(symbols[1].symbol, "build");
    }

    #[test]
    fn find_symbols_by_file_empty_when_no_match() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        let symbols = find_symbols_by_file(&conn, "nonexistent.rs")
            .expect("should return empty vec, not error");
        assert!(symbols.is_empty());
    }

    #[test]
    fn count_dead_code_only_matches_find_dead_code() {
        let conn = rusqlite::Connection::open_in_memory().expect("should succeed");
        init_schema(&conn).expect("initialize schema");
        conn.execute_batch(
            "INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n1', 'orphan', 'fn', 'rust', 'lib.rs', 1);
             INSERT INTO nodes (id, symbol, kind, language, file_path, line) VALUES ('n2', 'caller', 'fn', 'rust', 'lib.rs', 5);
             INSERT INTO edges (caller_id, callee_id) VALUES ('n2', 'n1');"
        ).expect("should succeed");

        let full = find_dead_code(&conn, None, None).expect("find dead code");
        let count = count_dead_code_only(&conn, None).expect("count dead code only");
        assert_eq!(count, full.len(), "count should match find_dead_code result length");
    }
}
