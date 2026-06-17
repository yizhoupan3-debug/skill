use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::models::*;

/// Initialize the SQLite database with FTS5 schema (4 tables + 1 FTS5 virtual table).
///
/// Schema: exploration_logs, exploration_decisions, exploration_insights,
/// barrier_reports, and exploration_fts (FTS5 external content on exploration_logs).
pub fn init_database(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("open database: {}", db_path.display()))?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS exploration_logs (
            id          TEXT PRIMARY KEY,
            direction   TEXT NOT NULL,
            question    TEXT NOT NULL,
            entry_point TEXT NOT NULL DEFAULT 'manual',
            barrier_id  TEXT,
            key_findings    TEXT NOT NULL DEFAULT '',
            open_questions  TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exploration_decisions (
            id          TEXT PRIMARY KEY,
            log_id      TEXT NOT NULL REFERENCES exploration_logs(id),
            decision    TEXT NOT NULL,
            rationale   TEXT NOT NULL DEFAULT '',
            outcome     TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exploration_insights (
            id          TEXT PRIMARY KEY,
            log_id      TEXT NOT NULL REFERENCES exploration_logs(id),
            text        TEXT NOT NULL,
            confidence  TEXT NOT NULL DEFAULT 'medium',
            cross_refs  TEXT NOT NULL DEFAULT '[]',
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS barrier_reports (
            id          TEXT PRIMARY KEY,
            barrier_id  TEXT NOT NULL,
            log_id      TEXT NOT NULL REFERENCES exploration_logs(id),
            loop_id     TEXT,
            report_path TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS exploration_fts USING fts5(
            direction, question, key_findings,
            content='exploration_logs',
            content_rowid='rowid',
            tokenize='unicode61'
        );
        ",
    )
    .context("initialize database schema")?;

    Ok(conn)
}

/// Record a new exploration log entry (DB + FTS5).
pub fn insert_log(conn: &Connection, log: &ExplorationLog) -> Result<()> {
    conn.execute(
        "INSERT INTO exploration_logs (id, direction, question, entry_point, barrier_id, key_findings, open_questions, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            log.id,
            log.direction,
            log.question,
            log.entry_point.as_str(),
            log.barrier_id,
            log.key_findings,
            log.open_questions,
            log.created_at,
            log.updated_at,
        ],
    )
    .context("insert exploration log")?;

    // Sync FTS5 index
    conn.execute(
        "INSERT INTO exploration_fts (rowid, direction, question, key_findings)
         VALUES (last_insert_rowid(), ?1, ?2, ?3)",
        params![log.direction, log.question, log.key_findings],
    )
    .context("insert FTS5 index")?;

    Ok(())
}

/// Add insight to an existing log.
pub fn insert_insight(conn: &Connection, insight: &ExplorationInsight) -> Result<()> {
    conn.execute(
        "INSERT INTO exploration_insights (id, log_id, text, confidence, cross_refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            insight.id,
            insight.log_id,
            insight.text,
            insight.confidence.as_str(),
            serde_json::to_string(&insight.cross_refs).unwrap_or_default(),
            insight.created_at,
        ],
    )
    .context("insert insight")?;

    // Append insight text to the log's key_findings for FTS coverage
    conn.execute(
        "UPDATE exploration_logs SET key_findings = key_findings || ?1 WHERE id = ?2",
        params![format!("\n## Insight\n\n{}", insight.text), insight.log_id],
    )
    .context("append insight to log")?;

    // Rebuild affected FTS index entry (INSERT with same rowid replaces in FTS5)
    conn.execute(
        "INSERT INTO exploration_fts (rowid, direction, question, key_findings)
         SELECT rowid, direction, question, key_findings FROM exploration_logs WHERE id = ?1",
        params![insight.log_id],
    )
    .context("rebuild FTS index entry")?;

    Ok(())
}

/// Full-text search across all logs.
pub fn search_logs(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.direction, e.question, snippet(exploration_fts, 0, '«', '»', '…', 48) AS snippet,
                    rank, e.created_at
             FROM exploration_fts
             JOIN exploration_logs e ON exploration_fts.rowid = e.rowid
             WHERE exploration_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )
        .context("prepare FTS search")?;

    let rows = stmt
        .query_map(params![query, limit as i64], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                direction: row.get(1)?,
                question: row.get(2)?,
                snippet: row.get(3)?,
                score: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .context("execute FTS search")?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.context("read search result")?);
    }
    Ok(results)
}

/// Connect two logs via a log-to-log cross-reference table.
pub fn connect_logs(conn: &Connection, log_id_a: &str, log_id_b: &str) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS log_connections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            log_id_a TEXT NOT NULL,
            log_id_b TEXT NOT NULL,
            relation TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(log_id_a, log_id_b)
        )",
        [],
    )
    .context("create log_connections table")?;

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO log_connections (log_id_a, log_id_b, created_at) VALUES (?1, ?2, ?3)",
        params![log_id_a, log_id_b, now],
    )
    .context("insert log connection")?;

    // Also insert reverse direction for bidirectional query
    conn.execute(
        "INSERT OR IGNORE INTO log_connections (log_id_a, log_id_b, created_at) VALUES (?1, ?2, ?3)",
        params![log_id_b, log_id_a, now],
    )
    .context("insert reverse log connection")?;

    Ok(())
}

/// Get barrier reports for a loop (or all if no loop_id given).
pub fn list_barrier_reports(conn: &Connection, loop_id: Option<&str>) -> Result<Vec<BarrierReport>> {
    let sql = if loop_id.is_some() {
        "SELECT id, barrier_id, log_id, loop_id, report_path, created_at FROM barrier_reports WHERE loop_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, barrier_id, log_id, loop_id, report_path, created_at FROM barrier_reports ORDER BY created_at DESC"
    };

    let mut stmt = conn.prepare(sql).context("prepare barrier query")?;
    let rows = if let Some(lid) = loop_id {
        stmt.query_map(params![lid], barrier_report_mapper)
            .context("query barrier reports")?
    } else {
        stmt.query_map([], barrier_report_mapper)
            .context("query barrier reports")?
    };

    let mut results = Vec::new();
    for row in rows {
        results.push(row.context("read barrier report")?);
    }
    Ok(results)
}

fn barrier_report_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<BarrierReport> {
    Ok(BarrierReport {
        id: row.get(0)?,
        barrier_id: row.get(1)?,
        log_id: row.get(2)?,
        loop_id: row.get(3)?,
        report_path: row.get(4)?,
        created_at: row.get(5)?,
    })
}

/// Trace research path from a barrier: barrier → log → insights.
pub fn trace_barrier_route(conn: &Connection, barrier_id: &str) -> Result<Vec<SearchResult>> {
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.direction, e.question, e.key_findings, 0.0, e.created_at
             FROM exploration_logs e
             WHERE e.barrier_id = ?1
                OR e.id IN (
                    SELECT log_id FROM barrier_reports WHERE barrier_id = ?1
                )
             ORDER BY e.created_at",
        )
        .context("prepare barrier trace")?;

    let rows = stmt
        .query_map(params![barrier_id], |row| {
            let findings: String = row.get(3)?;
            let snippet = if findings.len() > 120 {
                format!("{}...", &findings[..120])
            } else {
                findings.clone()
            };
            Ok(SearchResult {
                id: row.get(0)?,
                direction: row.get(1)?,
                question: row.get(2)?,
                snippet,
                score: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .context("execute barrier trace")?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.context("read barrier trace result")?);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EntryPoint, ExplorationInsight, ExplorationLog};

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE exploration_logs (
                id TEXT PRIMARY KEY, direction TEXT, question TEXT,
                entry_point TEXT, barrier_id TEXT, key_findings TEXT,
                open_questions TEXT, created_at TEXT, updated_at TEXT
            );
            CREATE TABLE exploration_insights (
                id TEXT PRIMARY KEY, log_id TEXT, text TEXT,
                confidence TEXT, cross_refs TEXT, created_at TEXT
            );
            CREATE TABLE barrier_reports (
                id TEXT PRIMARY KEY, barrier_id TEXT, log_id TEXT,
                loop_id TEXT, report_path TEXT, created_at TEXT
            );
            CREATE VIRTUAL TABLE exploration_fts USING fts5(
                direction, question, key_findings, content='exploration_logs', content_rowid='rowid'
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_insert_and_search() {
        let conn = test_db();
        let log = ExplorationLog {
            id: "test-1".into(),
            direction: "attention-optimization".into(),
            question: "如何优化 attention 的计算复杂度".into(),
            entry_point: EntryPoint::Manual,
            barrier_id: None,
            key_findings: "Flash Attention 2 可以显著减少显存占用".into(),
            open_questions: "是否适用于长序列".into(),
            created_at: "2026-06-18T10:00:00".into(),
            updated_at: "2026-06-18T10:00:00".into(),
        };
        insert_log(&conn, &log).unwrap();

        let results = search_logs(&conn, "attention", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].direction, "attention-optimization");
    }

    #[test]
    fn test_insight_appends_to_fts() {
        let conn = test_db();
        let log = ExplorationLog {
            id: "test-2".into(),
            direction: "test-direction".into(),
            question: "测试问题".into(),
            entry_point: EntryPoint::Manual,
            barrier_id: None,
            key_findings: "初始发现".into(),
            open_questions: "待解决".into(),
            created_at: "2026-06-18T10:00:00".into(),
            updated_at: "2026-06-18T10:00:00".into(),
        };
        insert_log(&conn, &log).unwrap();

        let insight = ExplorationInsight {
            id: "insight-1".into(),
            log_id: "test-2".into(),
            text: "important finding: this direction can try sparse attention".into(),
            confidence: crate::models::Confidence::High,
            cross_refs: vec![],
            created_at: "2026-06-18T11:00:00".into(),
        };
        insert_insight(&conn, &insight).unwrap();

        let results = search_logs(&conn, "sparse", 10).unwrap();
        assert_eq!(results.len(), 1, "FTS should find the insight text via key_findings");
    }

    #[test]
    fn test_barrier_trace() {
        let conn = test_db();
        let log = ExplorationLog {
            id: "barrier-log-1".into(),
            direction: "convergence-issue".into(),
            question: "模型不收敛的根因".into(),
            entry_point: EntryPoint::BarrierEscalation,
            barrier_id: Some("br-20260618001".into()),
            key_findings: "学习率过大导致梯度爆炸".into(),
            open_questions: "是否有更好的 warmup 策略".into(),
            created_at: "2026-06-18T10:00:00".into(),
            updated_at: "2026-06-18T10:00:00".into(),
        };
        insert_log(&conn, &log).unwrap();

        let results = trace_barrier_route(&conn, "br-20260618001").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].direction, "convergence-issue");
    }
}
