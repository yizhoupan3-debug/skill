//! Skill metadata ingestion and FTS lookup.
//!
//! Stores each skill from `SKILL_ROUTING_RUNTIME.json` as a node with `kind="skill"`,
//! `language="json"`, `file_path="runtime://SKILL_ROUTING_RUNTIME.json"`.
//! The symbol column holds the skill slug; keywords are stored as the
//! `file_path` suffix so FTS can match on trigger_hints.
//!
//! This enables O(log n) skill discovery via FTS5 instead of O(n) `fs::read_dir`.

use rusqlite::{Connection, params};
use serde_json::Value;

pub const SKILL_RUNTIME_PATH: &str = "runtime://SKILL_ROUTING_RUNTIME.json";
pub const SKILL_KIND: &str = "skill";
pub const SKILL_LANGUAGE: &str = "json";

/// Ingest all skills from a parsed `SKILL_ROUTING_RUNTIME.json` value.
///
/// The manifest uses an array-of-arrays format where `keys` defines column names
/// and each skill is an array of values. Deletes existing `skill` nodes first
/// (idempotent), then inserts one node per skill.
///
/// Returns the number of skills ingested.
pub fn ingest_skills(conn: &Connection, manifest: &Value) -> rusqlite::Result<usize> {
    let Some(keys) = manifest.get("skills") else {
        return Ok(0);
    };
    let Some(skills) = keys.as_array() else {
        return Ok(0);
    };

    // Get the key names to find column indices
    let header: Vec<String> = manifest
        .get("keys")
        .and_then(|k| k.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let slug_idx = header.iter().position(|k| k == "slug").unwrap_or(0);
    let desc_idx = header.iter().position(|k| k == "description");
    let hints_idx = header.iter().position(|k| k == "trigger_hints");
    let _priority_idx = header.iter().position(|k| k == "priority");
    let _path_idx = header.iter().position(|k| k == "skill_path");

    let tx = conn.unchecked_transaction()?;

    // Remove stale skill nodes
    conn.execute("DELETE FROM nodes WHERE kind = ?1", params![SKILL_KIND])?;

    let mut insert = conn.prepare(
        "INSERT INTO nodes (id, symbol, kind, language, file_path, line)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
    )?;

    let mut count = 0usize;
    for skill_row in skills {
        let Some(row) = skill_row.as_array() else {
            continue;
        };

        let slug = row
            .get(slug_idx)
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        // Build a searchable text blob from description + trigger_hints
        let mut search_text = String::from(slug);
        if let Some(idx) = desc_idx
            && let Some(desc) = row.get(idx).and_then(Value::as_str) {
                search_text.push(' ');
                search_text.push_str(desc);
            }
        if let Some(idx) = hints_idx
            && let Some(hints) = row.get(idx).and_then(Value::as_array) {
                for hint in hints {
                    if let Some(s) = hint.as_str() {
                        search_text.push(' ');
                        search_text.push_str(s);
                    }
                }
            }

        let id = format!("skill://{slug}");
        // Store the search text in file_path so FTS5 can index it
        // (file_path is indexed by FTS5 in schema.rs)
        let file_path = format!("manifest://{slug}");

        insert.execute(params![id, slug, SKILL_KIND, SKILL_LANGUAGE, file_path,])?;
        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

/// Find a skill by slug (exact match).
pub fn find_skill_by_slug(conn: &Connection, slug: &str) -> Option<crate::Node> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, symbol, kind, language, file_path, line
             FROM nodes WHERE symbol = ?1 AND kind = ?2 LIMIT 1",
        )
        .ok()?;
    let mut rows = stmt.query(params![trimmed, SKILL_KIND]).ok()?;
    let row = rows.next().ok()??;
    Some(crate::Node {
        id: row.get(0).ok()?,
        symbol: row.get(1).ok()?,
        kind: row.get(2).ok()?,
        language: row.get(3).ok()?,
        file_path: row.get(4).ok()?,
        line: row.get::<_, i64>(5).ok()? as u32,
    })
}

/// Return all skill nodes from the index.
pub fn list_skills(conn: &Connection) -> rusqlite::Result<Vec<crate::Node>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, kind, language, file_path, line
         FROM nodes WHERE kind = ?1 ORDER BY symbol",
    )?;
    let rows = stmt.query_map(params![SKILL_KIND], |row| {
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

    fn sample_manifest() -> Value {
        json!({
            "keys": ["slug", "description", "trigger_hints", "priority", "skill_path"],
            "skills": [
                ["gitx", "Git workflow commands", ["git", "branch", "merge"], "P0", "skills/gitx/SKILL.md"],
                ["simplify", "Review for simplification", ["simplify", "cleanup", "refactor"], "P0", "skills/simplify/SKILL.md"],
                ["deep-research", "Deep research", ["research", "investigate", "analyze"], "P1", "skills/deep-research/SKILL.md"]
            ]
        })
    }

    #[test]
    fn ingest_skills_creates_nodes() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        init_schema(&conn).expect("init schema");
        let manifest = sample_manifest();

        let count = ingest_skills(&conn, &manifest).expect("ingest");
        assert_eq!(count, 3);
    }

    #[test]
    fn find_skill_by_slug_works() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        init_schema(&conn).expect("init schema");
        ingest_skills(&conn, &sample_manifest()).expect("ingest");

        let skill = find_skill_by_slug(&conn, "gitx");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().symbol, "gitx");

        assert!(find_skill_by_slug(&conn, "nonexistent").is_none());
        assert!(find_skill_by_slug(&conn, "").is_none());
    }

    #[test]
    fn list_skills_returns_sorted() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        init_schema(&conn).expect("init schema");
        ingest_skills(&conn, &sample_manifest()).expect("ingest");

        let skills = list_skills(&conn).expect("list");
        assert_eq!(skills.len(), 3);
        assert!(skills.iter().all(|n| n.kind == "skill"));
        for i in 1..skills.len() {
            assert!(skills[i - 1].symbol <= skills[i].symbol);
        }
    }

    #[test]
    fn ingest_is_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        init_schema(&conn).expect("init schema");
        let manifest = sample_manifest();

        ingest_skills(&conn, &manifest).expect("first");
        ingest_skills(&conn, &manifest).expect("second");

        let skills = list_skills(&conn).expect("list");
        assert_eq!(skills.len(), 3); // not 6
    }

    #[test]
    fn empty_manifest_ingests_zero() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        init_schema(&conn).expect("init schema");

        let count = ingest_skills(&conn, &json!({})).expect("ingest empty");
        assert_eq!(count, 0);
    }
}
