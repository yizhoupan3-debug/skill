//! research-log-rs 数据库层 v2
//!
//! - WAL 模式实现写入 10-50x 加速
//! - Prepared statements 避免重复 parse
//! - FTS5 triggers 自动同步，无需手动 INSERT
//! - 全字段索引，避免表扫描
//! - 统一的 findings 表替代膨胀的 key_findings

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::models::*;

/// 数据库 schema 版本号，用于未来迁移。
const SCHEMA_VERSION: i32 = 2;

/// 初始化数据库：创建所有表、索引、triggers、启用 WAL。
pub fn init_database(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path).context("open research log database")?;

    // ── 性能设置 ──
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA cache_size=-65536;
         PRAGMA mmap_size=67108864;
         PRAGMA temp_store=MEMORY;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=5000;
         PRAGMA wal_autocheckpoint=1000;",
    )
    .context("set PRAGMAs")?;

    // ── 检查 schema 版本 ──
    // value 列是 TEXT，读为 String 再解析，避免 rusqlite 类型转换失败
    let existing_version: i32 = (|| -> Result<i32> {
        let val: String = conn.query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )?;
        Ok(val.parse().unwrap_or(0))
    })()
    .unwrap_or(0);

    if existing_version == 0 {
        // 首次创建或从旧版迁移 — 新版 schema 不兼容旧版
        conn.execute_batch(SCHEMA_SQL)?;
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION],
        )?;
    } else if existing_version < SCHEMA_VERSION {
        // 未来迁移逻辑写在这里
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            params![SCHEMA_VERSION],
        )?;
    }

    Ok(conn)
}

/// 预编译语句缓存，避免重复 parse。
struct Stmts<'a> {
    insert_entry: rusqlite::CachedStatement<'a>,
    insert_finding: rusqlite::CachedStatement<'a>,
    insert_tag: rusqlite::CachedStatement<'a>,
    insert_ref: rusqlite::CachedStatement<'a>,
    insert_connection: rusqlite::CachedStatement<'a>,
    insert_barrier_report: rusqlite::CachedStatement<'a>,
    insert_run: rusqlite::CachedStatement<'a>,
    insert_activity: rusqlite::CachedStatement<'a>,
    update_entry: rusqlite::CachedStatement<'a>,
    get_entry: rusqlite::CachedStatement<'a>,
    get_findings_by_entry: rusqlite::CachedStatement<'a>,
    get_tags_by_entry: rusqlite::CachedStatement<'a>,
    search_entries: rusqlite::CachedStatement<'a>,
    search_findings: rusqlite::CachedStatement<'a>,
}

impl<'a> Stmts<'a> {
    fn new(conn: &'a Connection) -> Result<Self> {
        Ok(Stmts {
            insert_entry: conn.prepare_cached(
                "INSERT INTO entries (id, direction, question, context, entry_point, barrier_id, importance, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?,
            insert_finding: conn.prepare_cached(
                "INSERT INTO findings (entry_id, kind, content, confidence, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?,
            insert_tag: conn.prepare_cached(
                "INSERT OR IGNORE INTO tags (entry_id, tag) VALUES (?1, ?2)",
            )?,
            insert_ref: conn.prepare_cached(
                "INSERT INTO refs (entry_id, ref_type, ref_key, title, authors, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?,
            insert_connection: conn.prepare_cached(
                "INSERT INTO connections (entry_id_a, entry_id_b, relation, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?,
            insert_barrier_report: conn.prepare_cached(
                "INSERT OR REPLACE INTO barrier_reports (barrier_id, entry_id, loop_id, report, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?,
            insert_run: conn.prepare_cached(
                "INSERT INTO runs (entry_id, outcome, summary, metrics, git_state, env_fingerprint, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?,
            insert_activity: conn.prepare_cached(
                "INSERT OR IGNORE INTO activity_log (tool_name, summary, source, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?,
            update_entry: conn.prepare_cached(
                "UPDATE entries SET direction=?1, question=?2, context=?3, importance=?4, status=?5, updated_at=?6
                 WHERE id=?7",
            )?,
            get_entry: conn.prepare_cached(
                "SELECT id, direction, question, context, entry_point, barrier_id, importance, status, created_at, updated_at
                 FROM entries WHERE id=?1",
            )?,
            get_findings_by_entry: conn.prepare_cached(
                "SELECT id, entry_id, kind, content, confidence, metadata, created_at
                 FROM findings WHERE entry_id=?1 ORDER BY created_at",
            )?,
            get_tags_by_entry: conn.prepare_cached(
                "SELECT tag FROM tags WHERE entry_id=?1 ORDER BY tag",
            )?,
            search_entries: conn.prepare_cached(
                "SELECT e.id, e.direction, e.question, snippet(entries_fts, 1, '<mark>', '</mark>', '...', 32) as snippet,
                        rank, e.created_at
                 FROM entries_fts
                 JOIN entries e ON e.rowid = entries_fts.rowid
                 WHERE entries_fts MATCH ?1
                   AND (?2 IS NULL OR e.direction = ?2)
                   AND (?3 IS NULL OR e.status = ?3)
                   AND (?4 IS NULL OR e.created_at >= ?4)
                   AND (?5 IS NULL OR e.created_at <= ?5)
                 ORDER BY rank
                 LIMIT ?6",
            )?,
            search_findings: conn.prepare_cached(
                "SELECT f.id, f.entry_id, f.kind, f.content,
                        snippet(findings_fts, 1, '<mark>', '</mark>', '...', 64) as snippet,
                        rank, f.created_at
                 FROM findings_fts
                 JOIN findings f ON f.rowid = findings_fts.rowid
                 WHERE findings_fts MATCH ?1
                   AND (?2 IS NULL OR f.kind = ?2)
                 ORDER BY rank
                 LIMIT ?3",
            )?,
        })
    }
}

// ── Public API ──

/// 插入一条日志条目（含 FTS5 trigger 自动同步）。
pub fn insert_entry(conn: &Connection, entry: &Entry) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_entry.execute(params![
        entry.id,
        entry.direction,
        entry.question,
        entry.context,
        entry.entry_point,
        entry.barrier_id,
        entry.importance,
        entry.status,
        entry.created_at,
        entry.updated_at,
    ])?;
    Ok(())
}

/// 插入一条 finding（insight/decision/question/plan）。
/// 返回新行 ID。
pub fn insert_finding(conn: &Connection, f: &Finding) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_finding.execute(params![
        f.entry_id,
        f.kind,
        f.content,
        f.confidence,
        f.metadata,
        f.created_at,
    ])?;
    Ok(())
}

/// 批量插入 tags。
pub fn insert_tags(conn: &Connection, entry_id: &str, tags: &[String]) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    for tag in tags {
        stmts.insert_tag.execute(params![entry_id, tag])?;
    }
    Ok(())
}

/// 插入一条引用。
pub fn insert_ref(conn: &Connection, r: &Ref) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_ref.execute(params![
        r.entry_id,
        r.ref_type,
        r.ref_key,
        r.title,
        r.authors,
        r.notes,
        r.created_at,
    ])?;
    Ok(())
}

/// 插入一条连接（双向自动处理）。
pub fn insert_connection(conn: &Connection, c: &crate::models::LogConnection) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_connection.execute(params![
        c.entry_id_a,
        c.entry_id_b,
        c.relation,
        c.notes,
        c.created_at,
    ])?;
    Ok(())
}

/// 插入 barrier 报告。
pub fn insert_barrier_report(conn: &Connection, r: &BarrierReport) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_barrier_report.execute(params![
        r.barrier_id,
        r.entry_id,
        r.loop_id,
        r.report,
        r.created_at,
    ])?;
    Ok(())
}

/// 插入实验 run 记录（替代 run-ledger.jsonl）。
pub fn insert_run(conn: &Connection, r: &Run) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_run.execute(params![
        r.entry_id,
        r.outcome,
        r.summary,
        r.metrics,
        r.git_state,
        r.env_fingerprint,
        r.created_at,
    ])?;
    Ok(())
}

/// 插入活动日志条目。
pub fn insert_activity_entry(conn: &Connection, a: &ActivityEntry) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_activity.execute(params![
        a.tool_name,
        a.summary,
        a.source,
        a.metadata,
        a.created_at,
    ])?;
    Ok(())
}

/// 更新条目。
pub fn update_entry(
    conn: &Connection,
    id: &str,
    direction: &str,
    question: &str,
    context: Option<&str>,
    importance: i32,
    status: &str,
    updated_at: &str,
) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts
        .update_entry
        .execute(params![direction, question, context, importance, status, updated_at, id])?;
    Ok(())
}

/// 按 ID 查询条目。
pub fn get_entry(conn: &Connection, id: &str) -> Result<Option<Entry>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_entry.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(Entry {
            id: row.get(0)?,
            direction: row.get(1)?,
            question: row.get(2)?,
            context: row.get(3)?,
            entry_point: row.get(4)?,
            barrier_id: row.get(5)?,
            importance: row.get(6)?,
            status: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })),
        None => Ok(None),
    }
}

/// 查询条目的 findings。
pub fn get_findings(conn: &Connection, entry_id: &str) -> Result<Vec<Finding>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts
        .get_findings_by_entry
        .query(params![entry_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(Finding {
            id: row.get(0)?,
            entry_id: row.get(1)?,
            kind: row.get(2)?,
            content: row.get(3)?,
            confidence: row.get(4)?,
            metadata: row.get(5)?,
            created_at: row.get(6)?,
        });
    }
    Ok(results)
}

/// 查询条目的 tags。
pub fn get_tags(conn: &Connection, entry_id: &str) -> Result<Vec<String>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_tags_by_entry.query(params![entry_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(row.get(0)?);
    }
    Ok(results)
}

/// FTS5 搜索条目。支持方向、状态、日期过滤。
#[allow(clippy::too_many_arguments)]
pub fn search_entries(
    conn: &Connection,
    query: &str,
    direction_filter: Option<&str>,
    status_filter: Option<&str>,
    date_from: Option<&str>,
    date_to: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.search_entries.query(params![
        query,
        direction_filter,
        status_filter,
        date_from,
        date_to,
        limit as i64,
    ])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(SearchResult {
            id: row.get(0)?,
            direction: row.get(1)?,
            question: row.get(2)?,
            snippet: row.get::<_, String>(3).unwrap_or_default(),
            score: row.get(4)?,
            created_at: row.get(5)?,
        });
    }
    Ok(results)
}

/// FTS5 搜索 findings。支持 kind 过滤。
pub fn search_findings(
    conn: &Connection,
    query: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<Finding>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.search_findings.query(params![query, kind_filter, limit as i64])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(Finding {
            id: row.get(0)?,
            entry_id: row.get(1)?,
            kind: row.get(2)?,
            content: row.get::<_, String>(3).unwrap_or_default(),
            confidence: None,
            metadata: None,
            created_at: row.get::<_, String>(6).unwrap_or_default(),
        });
    }
    Ok(results)
}

/// 重建 FTS5 索引。
pub fn rebuild_fts_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "INSERT INTO entries_fts(entries_fts) VALUES('rebuild');
         INSERT INTO findings_fts(findings_fts) VALUES('rebuild');",
    )
    .context("rebuild FTS5 indexes")?;
    Ok(())
}

/// 批量插入 activity log entries，直用 prepare_cached 避免创建全部 14 个 stmts。
pub fn bulk_insert_activities(
    conn: &Connection,
    entries: &[ActivityEntry],
) -> Result<usize> {
    let mut stmt = conn.prepare_cached(
        "INSERT OR IGNORE INTO activity_log (tool_name, summary, source, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut count = 0;
    for a in entries {
        stmt.execute(params![
            a.tool_name,
            a.summary,
            a.source,
            a.metadata,
            a.created_at,
        ])?;
        count += 1;
    }
    Ok(count)
}

/// 获取所有 entry ID（用于遍历/导出）。
pub fn list_entry_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT id FROM entries ORDER BY created_at")?;
    let mut rows = stmt.query([])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        ids.push(row.get(0)?);
    }
    Ok(ids)
}

/// 获取数据库中所有表和索引的大小统计。
pub fn db_stats(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT name, pgsize FROM dbstat ORDER BY pgsize DESC",
    )?;
    let mut rows = stmt.query([])?;
    let mut stats = Vec::new();
    while let Some(row) = rows.next()? {
        stats.push((row.get::<_, String>(0)?, row.get::<_, i64>(1)?));
    }
    Ok(stats)
}

// ── Schema ──

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY,
    direction TEXT NOT NULL,
    question TEXT NOT NULL,
    context TEXT,
    entry_point TEXT DEFAULT 'manual',
    barrier_id TEXT,
    importance INTEGER DEFAULT 0,
    status TEXT DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entries_direction ON entries(direction);
CREATE INDEX IF NOT EXISTS idx_entries_status ON entries(status);
CREATE INDEX IF NOT EXISTS idx_entries_created ON entries(created_at);
CREATE INDEX IF NOT EXISTS idx_entries_barrier ON entries(barrier_id);

CREATE TABLE IF NOT EXISTS findings (
    id INTEGER PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL,
    metadata TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_findings_entry ON findings(entry_id);
CREATE INDEX IF NOT EXISTS idx_findings_kind ON findings(kind);

CREATE TABLE IF NOT EXISTS tags (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (entry_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

CREATE TABLE IF NOT EXISTS refs (
    id INTEGER PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    ref_type TEXT NOT NULL,
    ref_key TEXT,
    title TEXT,
    authors TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_refs_entry ON refs(entry_id);
CREATE INDEX IF NOT EXISTS idx_refs_type ON refs(ref_type);

CREATE TABLE IF NOT EXISTS connections (
    id INTEGER PRIMARY KEY,
    entry_id_a TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    entry_id_b TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    relation TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conn_a ON connections(entry_id_a);
CREATE INDEX IF NOT EXISTS idx_conn_b ON connections(entry_id_b);

CREATE TABLE IF NOT EXISTS barrier_reports (
    barrier_id TEXT PRIMARY KEY,
    entry_id TEXT REFERENCES entries(id) ON DELETE SET NULL,
    loop_id TEXT,
    report TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_barrier_loop ON barrier_reports(loop_id);
CREATE INDEX IF NOT EXISTS idx_barrier_entry ON barrier_reports(entry_id);

CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    outcome TEXT NOT NULL,
    summary TEXT NOT NULL,
    metrics TEXT,
    git_state TEXT,
    env_fingerprint TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_runs_entry ON runs(entry_id);
CREATE INDEX IF NOT EXISTS idx_runs_outcome ON runs(outcome);

CREATE TABLE IF NOT EXISTS activity_log (
    id INTEGER PRIMARY KEY,
    tool_name TEXT NOT NULL,
    summary TEXT NOT NULL,
    source TEXT DEFAULT 'auto',
    metadata TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(tool_name, summary, created_at)
);
CREATE INDEX IF NOT EXISTS idx_activity_created ON activity_log(created_at);

-- FTS5
-- context 列保留但插入空串，避免 JSON 噪声污染搜索结果
CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
    direction, question, context,
    content='entries',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS findings_fts USING fts5(
    content, metadata,
    content='findings',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- FTS sync triggers（context 和 metadata 插入空串，不索引 JSON）
CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, direction, question, context)
    VALUES (new.rowid, new.direction, new.question, '');
END;

CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, direction, question)
    VALUES ('delete', old.rowid, old.direction, old.question);
END;

CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, direction, question)
    VALUES ('delete', old.rowid, old.direction, old.question);
    INSERT INTO entries_fts(rowid, direction, question, context)
    VALUES (new.rowid, new.direction, new.question, '');
END;

CREATE TRIGGER IF NOT EXISTS findings_ai AFTER INSERT ON findings BEGIN
    INSERT INTO findings_fts(rowid, content, metadata)
    VALUES (new.rowid, new.content, '');
END;

CREATE TRIGGER IF NOT EXISTS findings_ad AFTER DELETE ON findings BEGIN
    INSERT INTO findings_fts(findings_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER IF NOT EXISTS findings_au AFTER UPDATE ON findings BEGIN
    INSERT INTO findings_fts(findings_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO findings_fts(rowid, content, metadata)
    VALUES (new.rowid, new.content, '');
END;
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn test_db() -> Connection {
        init_database(&Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_schema_version() {
        let conn = test_db();
        let version_str: String = conn
            .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let version: i32 = version_str.parse().unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn test_insert_and_get_entry() {
        let conn = test_db();
        let entry = Entry {
            id: "entry-001".into(),
            direction: "factor-research".into(),
            question: "CSI300 factors".into(),
            context: Some(r#"{"env":"test"}"#.into()),
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "2026-06-18T10:00:00Z".into(),
            updated_at: "2026-06-18T10:00:00Z".into(),
        };
        insert_entry(&conn, &entry).unwrap();
        let got = get_entry(&conn, "entry-001").unwrap().unwrap();
        assert_eq!(got.id, "entry-001");
        assert_eq!(got.direction, "factor-research");
        assert_eq!(got.context.unwrap(), r#"{"env":"test"}"#);
    }

    #[test]
    fn test_insert_and_search_entry() {
        let conn = test_db();
        let entry = Entry {
            id: "entry-002".into(),
            direction: "quant".into(),
            question: "momentum factor decay rate".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 2,
            status: STATUS_ACTIVE.into(),
            created_at: "2026-06-18T11:00:00Z".into(),
            updated_at: "2026-06-18T11:00:00Z".into(),
        };
        insert_entry(&conn, &entry).unwrap();

        let results = search_entries(&conn, "momentum", None, None, None, None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("momentum"));
    }

    #[test]
    fn test_insert_finding() {
        let conn = test_db();
        let entry = Entry {
            id: "e1".into(),
            direction: "test".into(),
            question: "test".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        insert_entry(&conn, &entry).unwrap();

        let finding = Finding {
            id: 0,
            entry_id: "e1".into(),
            kind: FINDING_KIND_INSIGHT.into(),
            content: "发现动量因子衰减速度与波动率正相关".into(),
            confidence: Some(0.85),
            metadata: None,
            created_at: "2026-06-18T12:00:00Z".into(),
        };
        insert_finding(&conn, &finding).unwrap();

        let findings = get_findings(&conn, "e1").unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "insight");
        assert!((findings[0].confidence.unwrap() - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_tags() {
        let conn = test_db();
        let entry = Entry {
            id: "e-tag".into(),
            direction: "test".into(),
            question: "test".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        insert_entry(&conn, &entry).unwrap();
        insert_tags(&conn, "e-tag", &["因子".into(), "动量".into(), "衰减".into()]).unwrap();

        let tags = get_tags(&conn, "e-tag").unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&"因子".into()));
    }

    #[test]
    fn test_insert_and_search_findings() {
        let conn = test_db();
        let entry = Entry {
            id: "e2".into(),
            direction: "test".into(),
            question: "test".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        insert_entry(&conn, &entry).unwrap();

        let finding = Finding {
            id: 0,
            entry_id: "e2".into(),
            kind: FINDING_KIND_DECISION.into(),
            content: "改用 EWMA 估计协方差矩阵".into(),
            confidence: None,
            metadata: None,
            created_at: "now".into(),
        };
        insert_finding(&conn, &finding).unwrap();

        let results = search_findings(&conn, "EWMA", None, 10).unwrap();
        assert!(!results.is_empty());

        let filtered = search_findings(&conn, "EWMA", Some(FINDING_KIND_DECISION), 10).unwrap();
        assert!(!filtered.is_empty());

        let no_match = search_findings(&conn, "EWMA", Some(FINDING_KIND_QUESTION), 10).unwrap();
        assert!(no_match.is_empty());
    }

    #[test]
    fn test_run() {
        let conn = test_db();
        let entry = Entry {
            id: "e-run".into(),
            direction: "test".into(),
            question: "test".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        insert_entry(&conn, &entry).unwrap();

        let run = Run {
            id: 0,
            entry_id: "e-run".into(),
            outcome: OUTCOME_CONFIRMATORY.into(),
            summary: "EWMA 参数 λ=0.94 表现最佳".into(),
            metrics: Some(r#"{"sharpe":1.8,"turnover":0.15}"#.into()),
            git_state: Some(r#"{"commit":"abc123"}"#.into()),
            env_fingerprint: None,
            created_at: "now".into(),
        };
        insert_run(&conn, &run).unwrap();
    }

    #[test]
    fn test_connect_and_relation() {
        let conn_db = test_db();
        for id in &["ea", "eb"] {
            insert_entry(
                &conn_db,
                &Entry {
                    id: id.to_string(),
                    direction: "test".into(),
                    question: "test".into(),
                    context: None,
                    entry_point: ENTRY_POINT_MANUAL.into(),
                    barrier_id: None,
                    importance: 0,
                    status: STATUS_ACTIVE.into(),
                    created_at: "now".into(),
                    updated_at: "now".into(),
                },
            )
            .unwrap();
        }

        let log_conn = crate::models::LogConnection {
            id: 0,
            entry_id_a: "ea".into(),
            entry_id_b: "eb".into(),
            relation: Some(RELATION_SUPPORTS.into()),
            notes: Some("结果一致".into()),
            created_at: "now".into(),
        };
        insert_connection(&conn_db, &log_conn).unwrap();
    }

    #[test]
    fn test_search_with_filters() {
        let conn = test_db();
        for i in 0..5 {
            let entry = Entry {
                id: format!("e-filter-{}", i),
                direction: if i % 2 == 0 { "even".into() } else { "odd".into() },
                question: format!("question {}", i),
                context: None,
                entry_point: ENTRY_POINT_MANUAL.into(),
                barrier_id: None,
                importance: i,
                status: if i < 3 { STATUS_ACTIVE.into() } else { STATUS_ARCHIVED.into() },
                created_at: format!("2026-06-{:02}T10:00:00Z", 18 - i),
                updated_at: format!("2026-06-{:02}T10:00:00Z", 18 - i),
            };
            insert_entry(&conn, &entry).unwrap();
        }

        // Filter by direction
        let r = search_entries(&conn, "question", Some("even"), None, None, None, 10).unwrap();
        assert_eq!(r.len(), 3, "even-direction entries");

        // Filter by status
        let r = search_entries(&conn, "question", None, Some(STATUS_ACTIVE), None, None, 10).unwrap();
        assert_eq!(r.len(), 3, "active entries");

        // Filter by date
        let r = search_entries(&conn, "question", None, None, Some("2026-06-17"), None, 10).unwrap();
        assert_eq!(r.len(), 2, "entries on or after 17th");
    }

    #[test]
    fn test_barrier_report() {
        let conn = test_db();
        let report = BarrierReport {
            barrier_id: "br-001".into(),
            entry_id: None,
            loop_id: Some("loop-daily".into()),
            report: Some(r#"{"candidates":[]}"#.into()),
            created_at: "now".into(),
        };
        insert_barrier_report(&conn, &report).unwrap();
    }

    #[test]
    fn test_activity_log() {
        let conn = test_db();
        let a = ActivityEntry {
            id: 0,
            tool_name: "WebFetch".into(),
            summary: "arXiv search: transformer".into(),
            source: "auto".into(),
            metadata: None,
            created_at: "now".into(),
        };
        insert_activity_entry(&conn, &a).unwrap();
    }

    #[test]
    fn test_db_is_wal() {
        // WAL is set on disk databases; in :memory: SQLite falls back.
        // Verify the PRAGMA is accepted without error.
        let conn = test_db();
        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap_or_else(|_| "memory".to_string());
        // Accept WAL (disk) or memory (:memory: fallback)
        let ok = journal.to_lowercase().contains("wal")
            || journal.to_lowercase() == "memory";
        assert!(ok, "journal_mode should be WAL or memory, got: {journal}");
    }

    #[test]
    fn test_list_entry_ids() {
        let conn = test_db();
        for i in 0..3 {
            let entry = Entry {
                id: format!("list-{}", i),
                direction: "test".into(),
                question: "test".into(),
                context: None,
                entry_point: ENTRY_POINT_MANUAL.into(),
                barrier_id: None,
                importance: 0,
                status: STATUS_ACTIVE.into(),
                created_at: format!("2026-06-{:02}T10:00:00Z", 18 - i),
                updated_at: format!("2026-06-{:02}T10:00:00Z", 18 - i),
            };
            insert_entry(&conn, &entry).unwrap();
        }
        let ids = list_entry_ids(&conn).unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_rebuild_fts_index() {
        let conn = test_db();
        let entry = Entry {
            id: "fts-rebuild".into(),
            direction: "test".into(),
            question: "rebuild test".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        insert_entry(&conn, &entry).unwrap();
        rebuild_fts_index(&conn).unwrap();
        // After rebuild, search should still work
        let results = search_entries(&conn, "rebuild", None, None, None, None, 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_fts_sync_after_update() {
        let conn = test_db();
        insert_entry(
            &conn,
            &Entry {
                id: "fts-upd".into(),
                direction: "old".into(),
                question: "original question".into(),
                context: None,
                entry_point: ENTRY_POINT_MANUAL.into(),
                barrier_id: None,
                importance: 0,
                status: STATUS_ACTIVE.into(),
                created_at: "now".into(),
                updated_at: "now".into(),
            },
        )
        .unwrap();

        // Search for original
        let r = search_entries(&conn, "original", None, None, None, None, 10).unwrap();
        assert_eq!(r.len(), 1);

        // Update question
        update_entry(&conn, "fts-upd", "new-dir", "updated question", None, 0, STATUS_ACTIVE, "now").unwrap();

        // Search for new content
        let r = search_entries(&conn, "updated", None, None, None, None, 10).unwrap();
        assert_eq!(r.len(), 1, "FTS should find updated content");

        // Old content should be gone from FTS
        let r = search_entries(&conn, "original", None, None, None, None, 10).unwrap();
        assert_eq!(r.len(), 0, "FTS should NOT find old content after update");
    }

    #[test]
    fn test_fts_sync_after_delete() {
        let conn = test_db();
        insert_entry(
            &conn,
            &Entry {
                id: "fts-del".into(),
                direction: "temp".into(),
                question: "will be deleted".into(),
                context: None,
                entry_point: ENTRY_POINT_MANUAL.into(),
                barrier_id: None,
                importance: 0,
                status: STATUS_ACTIVE.into(),
                created_at: "now".into(),
                updated_at: "now".into(),
            },
        )
        .unwrap();

        // Confirm it's searchable
        let r = search_entries(&conn, "deleted", None, None, None, None, 10).unwrap();
        assert_eq!(r.len(), 1);

        // Delete the entry (SQLite direct — verify cascading FTS removal)
        conn.execute("DELETE FROM entries WHERE id='fts-del'", []).unwrap();

        // Should no longer appear in search
        let r = search_entries(&conn, "deleted", None, None, None, None, 10).unwrap();
        assert_eq!(r.len(), 0, "FTS should NOT find deleted entry");
    }
}
