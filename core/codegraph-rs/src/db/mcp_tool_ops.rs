//! MCP tool registry ingestion and FTS lookup (v6.5 codegraph integration).
//!
//! Stores each managed MCP tool as a node with `kind="mcp_tool"`,
//! `language="json"`, `file_path="registry://RUNTIME_REGISTRY.json"`.
//! The symbol column holds the tool name; `id` is formatted as
//! `registry://<server_id>:<tool_name>`. Re-uses the existing FTS5
//! index so lookups go through `search_symbols` with `kind="mcp_tool"`.

use rusqlite::{params, Connection};
use serde_json::Value;

/// Virtual file path used for all MCP tool registry nodes.
pub const MCP_REGISTRY_PATH: &str = "registry://RUNTIME_REGISTRY.json";
pub const MCP_TOOL_KIND: &str = "mcp_tool";
pub const MCP_TOOL_LANGUAGE: &str = "json";

/// Ingest all managed MCP tools from a parsed `RUNTIME_REGISTRY.json` value.
///
/// Deletes any existing `mcp_tool` nodes first (idempotent), then inserts
/// one node per tool with the server_id encoded in the `id` and stored
/// as the node's `line` field (unused for registry nodes; server_id goes
/// into a separate lookup via `resolve_mcp_tool_server_id`).
///
/// Returns the number of tools ingested.
pub fn ingest_mcp_tools(conn: &Connection, registry: &Value) -> rusqlite::Result<usize> {
    let Some(managed) = registry.get("managed_mcp_servers").and_then(Value::as_object) else {
        return Ok(0);
    };

    let tx = conn.unchecked_transaction()?;

    // Remove stale mcp_tool nodes
    conn.execute("DELETE FROM nodes WHERE kind = ?1", params![MCP_TOOL_KIND])?;

    let mut insert = conn.prepare(
        "INSERT INTO nodes (id, symbol, kind, language, file_path, line)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
    )?;

    let mut count = 0usize;
    for (server_id, entry) in managed {
        let Some(tools) = entry.get("tools").and_then(Value::as_array) else {
            continue;
        };
        for tool in tools {
            let Some(tool_name) = tool.as_str() else {
                continue;
            };
            let id = format!("registry://{server_id}:{tool_name}");
            insert.execute(params![
                id,
                tool_name,
                MCP_TOOL_KIND,
                MCP_TOOL_LANGUAGE,
                MCP_REGISTRY_PATH,
            ])?;
            count += 1;
        }
    }

    tx.commit()?;
    Ok(count)
}

/// Resolve a tool name to its server_id using FTS.
///
/// Searches the `mcp_tool`-kinded nodes via the FTS5 index and returns
/// the `server_id` parsed from the node `id` field (`registry://<server_id>:<tool_name>`).
/// Returns `None` if the tool is not found in the index.
pub fn resolve_mcp_tool_server_id(conn: &Connection, tool_name: &str) -> Option<String> {
    let trimmed = tool_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Use exact symbol match (not FTS) for O(1) lookup
    let mut stmt = conn
        .prepare(
            "SELECT id FROM nodes WHERE symbol = ?1 AND kind = ?2 LIMIT 1",
        )
        .ok()?;
    let mut rows = stmt.query(params![trimmed, MCP_TOOL_KIND]).ok()?;
    let row = rows.next().ok()??;
    let id: String = row.get(0).ok()?;
    // Parse server_id from `registry://<server_id>:<tool_name>`
    let rest = id.strip_prefix("registry://")?;
    let (server_id, _) = rest.split_once(':')?;
    Some(server_id.to_string())
}

/// Return all MCP tool nodes from the index.
pub fn list_mcp_tools(conn: &Connection) -> rusqlite::Result<Vec<crate::Node>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line FROM nodes WHERE kind = ?1 ORDER BY symbol",
    )?;
    let rows = stmt.query_map(params![MCP_TOOL_KIND], |row| {
        Ok(crate::Node {
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
    use super::*;
    use crate::db::schema::init_schema;
    use serde_json::json;

    fn registry_with_tools() -> Value {
        json!({
            "managed_mcp_servers": {
                "router-rs-framework": {
                    "tools": ["framework_snapshot", "skill_route", "skill_read"]
                },
                "browser-mcp": {
                    "tools": ["browser_open", "browser_click", "browser_screenshot"]
                },
                "mcp-codegraph": {
                    "tools": ["codegraph_search", "codegraph_status"]
                }
            }
        })
    }

    #[test]
    fn ingest_mcp_tools_creates_nodes() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let registry = registry_with_tools();

        let count = ingest_mcp_tools(&conn, &registry).expect("ingest");
        assert_eq!(count, 8); // 3 + 3 + 2
    }

    #[test]
    fn resolve_mcp_tool_server_id_finds_tool() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let registry = registry_with_tools();
        ingest_mcp_tools(&conn, &registry).expect("ingest");

        assert_eq!(
            resolve_mcp_tool_server_id(&conn, "framework_snapshot"),
            Some("router-rs-framework".to_string())
        );
        assert_eq!(
            resolve_mcp_tool_server_id(&conn, "browser_open"),
            Some("browser-mcp".to_string())
        );
        assert_eq!(
            resolve_mcp_tool_server_id(&conn, "codegraph_search"),
            Some("mcp-codegraph".to_string())
        );
    }

    #[test]
    fn resolve_mcp_tool_returns_none_for_unknown() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let registry = registry_with_tools();
        ingest_mcp_tools(&conn, &registry).expect("ingest");

        assert_eq!(resolve_mcp_tool_server_id(&conn, "grep"), None);
        assert_eq!(resolve_mcp_tool_server_id(&conn, ""), None);
    }

    #[test]
    fn ingest_is_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let registry = registry_with_tools();

        ingest_mcp_tools(&conn, &registry).expect("first ingest");
        ingest_mcp_tools(&conn, &registry).expect("second ingest");

        // Should still have exactly 8 nodes (not 16)
        let tools = list_mcp_tools(&conn).expect("list");
        assert_eq!(tools.len(), 8);
    }

    #[test]
    fn list_mcp_tools_returns_sorted_nodes() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");
        let registry = registry_with_tools();
        ingest_mcp_tools(&conn, &registry).expect("ingest");

        let tools = list_mcp_tools(&conn).expect("list");
        assert!(tools.iter().all(|n| n.kind == "mcp_tool"));
        assert!(tools.iter().all(|n| n.file_path == MCP_REGISTRY_PATH));
        // Should be sorted by symbol
        for i in 1..tools.len() {
            assert!(tools[i - 1].symbol <= tools[i].symbol);
        }
    }

    #[test]
    fn empty_registry_ingests_zero() {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");

        let count = ingest_mcp_tools(&conn, &json!({})).expect("ingest empty");
        assert_eq!(count, 0);
    }
}
