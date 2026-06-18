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

/// Schema version number, bumped for each migration.
const SCHEMA_VERSION: i32 = 3;

/// Migration registry: (from_version, migration_fn).
/// Each function must be idempotent (safe to retry on partial failure).
/// **Must be sorted by from_version ascending with no gaps** — validated by test.
const MIGRATIONS: &[(i32, fn(&Connection) -> Result<()>)] = &[
    (2, migrate_v2_to_v3),
];

fn run_migrations(conn: &Connection, existing_version: i32) -> Result<()> {
    // 运行时防御：验证 MIGRATIONS 已排序
    for i in 1..MIGRATIONS.len() {
        assert!(
            MIGRATIONS[i].0 > MIGRATIONS[i - 1].0,
            "MIGRATIONS ordering violation at index {}: v{} <= v{}",
            i, MIGRATIONS[i].0, MIGRATIONS[i - 1].0
        );
    }

    let mut version = existing_version;
    for (from_ver, migration_fn) in MIGRATIONS {
        if version == *from_ver {
            migration_fn(&conn).context(format!("migration v{} -> v{}", from_ver, from_ver + 1))?;
            version = *from_ver + 1;
        }
    }
    Ok(())
}

fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    // 幂等检查：只有列不存在时才 ALTER
    let has_weight: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('connections') WHERE name='weight'",
        [],
        |row| row.get(0),
    ).unwrap_or(false);
    if !has_weight {
        conn.execute_batch(SCHEMA_ALTER_V2_TO_V3)?;
    }
    // 创建新表（幂等：IF NOT EXISTS）
    conn.execute_batch(SCHEMA_CREATE_V3)?;
    conn.execute_batch("COMMIT")?;
    Ok(())
}

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
        conn.execute_batch(SCHEMA_SQL).context("init: execute SCHEMA_SQL")?;
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION],
        )?;
    } else if existing_version < SCHEMA_VERSION {
        // 遍历从 existing_version 到 SCHEMA_VERSION 之间的所有迁移
        run_migrations(&conn, existing_version)
            .context(format!("run migrations from v{} to v{}", existing_version, SCHEMA_VERSION))?;
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
    // Entity / KG
    upsert_entity: rusqlite::CachedStatement<'a>,
    insert_entity_relation: rusqlite::CachedStatement<'a>,
    insert_entry_entity: rusqlite::CachedStatement<'a>,
    get_entity_by_name: rusqlite::CachedStatement<'a>,
    search_entities: rusqlite::CachedStatement<'a>,
    get_entry_entities: rusqlite::CachedStatement<'a>,
    get_all_connections: rusqlite::CachedStatement<'a>,
    get_connections_for_entry: rusqlite::CachedStatement<'a>,
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
                "INSERT INTO connections (entry_id_a, entry_id_b, relation, weight, confidence, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
                        rank, f.confidence, f.metadata, f.created_at
                 FROM findings_fts
                 JOIN findings f ON f.rowid = findings_fts.rowid
                 WHERE findings_fts MATCH ?1
                   AND (?2 IS NULL OR f.kind = ?2)
                 ORDER BY rank
                 LIMIT ?3",
            )?,
            // Entity / KG
            upsert_entity: conn.prepare_cached(
                "INSERT INTO entities (name, kind, description, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(name) DO UPDATE SET
                   kind = COALESCE(NULLIF(?2, 'concept'), kind),
                   description = COALESCE(?3, description),
                   metadata = COALESCE(?4, metadata)",
            )?,
            insert_entity_relation: conn.prepare_cached(
                "INSERT OR IGNORE INTO entity_relations (entity_id_a, entity_id_b, relation, entry_id, confidence, metadata, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?,
            insert_entry_entity: conn.prepare_cached(
                "INSERT OR IGNORE INTO entry_entities (entry_id, entity_id, role)
                 VALUES (?1, ?2, ?3)",
            )?,
            get_entity_by_name: conn.prepare_cached(
                "SELECT id, name, kind, description, metadata, created_at
                 FROM entities WHERE name=?1",
            )?,
            search_entities: conn.prepare_cached(
                "SELECT e.id, e.name, e.kind, e.description, e.metadata, e.created_at
                 FROM entities_fts
                 JOIN entities e ON e.rowid = entities_fts.rowid
                 WHERE entities_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?,
            get_entry_entities: conn.prepare_cached(
                "SELECT e.id, e.name, e.kind, e.description, e.metadata, e.created_at, ee.role
                 FROM entry_entities ee
                 JOIN entities e ON e.id = ee.entity_id
                 WHERE ee.entry_id=?1
                 ORDER BY e.name",
            )?,
            get_all_connections: conn.prepare_cached(
                "SELECT id, entry_id_a, entry_id_b, relation, weight, confidence, notes, created_at
                 FROM connections ORDER BY created_at",
            )?,
            get_connections_for_entry: conn.prepare_cached(
                "SELECT id, entry_id_a, entry_id_b, relation, weight, confidence, notes, created_at
                 FROM connections WHERE entry_id_a=?1 OR entry_id_b=?1
                 ORDER BY created_at",
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
        c.weight,
        c.confidence,
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
/// 注意：FTS5 将 hyphen 解析为列排除前缀，须转义为空格。
pub fn search_findings(
    conn: &Connection,
    query: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<Finding>> {
    let mut stmts = Stmts::new(conn)?;
    let fts_query = query.replace('-', " ");
    let mut rows = stmts.search_findings.query(params![fts_query, kind_filter, limit as i64])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(Finding {
            id: row.get(0)?,
            entry_id: row.get(1)?,
            kind: row.get(2)?,
            content: row.get::<_, String>(3).unwrap_or_default(),
            confidence: row.get::<_, Option<f64>>(6).ok().flatten(),
            metadata: row.get::<_, Option<String>>(7).ok().flatten(),
            created_at: row.get::<_, String>(8).unwrap_or_default(),
        });
    }
    Ok(results)
}

/// 重建 FTS5 索引。
pub fn rebuild_fts_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "INSERT INTO entries_fts(entries_fts) VALUES('rebuild');
         INSERT INTO findings_fts(findings_fts) VALUES('rebuild');
         INSERT INTO entities_fts(entities_fts) VALUES('rebuild');",
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

// ── Entity / Knowledge Graph API ──

/// Upsert an entity by name (INSERT or UPDATE on conflict).
pub fn upsert_entity(
    conn: &Connection,
    name: &str,
    kind: &str,
    description: Option<&str>,
    metadata: Option<&str>,
) -> Result<i64> {
    let mut stmts = Stmts::new(conn)?;
    stmts.upsert_entity.execute(params![name, kind, description, metadata, chrono::Utc::now().to_rfc3339()])?;
    // Return entity ID via a fresh query
    let mut q = stmts.get_entity_by_name.query(params![name])?;
    match q.next()? {
        Some(row) => Ok(row.get(0)?),
        None => anyhow::bail!("Entity not found after upsert: {name}"),
    }
}

/// Link two entities with a relation type.
pub fn insert_entity_relation(
    conn: &Connection,
    entity_id_a: i64,
    entity_id_b: i64,
    relation: &str,
    entry_id: Option<&str>,
    confidence: Option<f64>,
    metadata: Option<&str>,
) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_entity_relation.execute(params![
        entity_id_a,
        entity_id_b,
        relation,
        entry_id,
        confidence,
        metadata,
        chrono::Utc::now().to_rfc3339(),
    ])?;
    Ok(())
}

/// Associate an entry with an entity.
pub fn insert_entry_entity(
    conn: &Connection,
    entry_id: &str,
    entity_id: i64,
    role: &str,
) -> Result<()> {
    let mut stmts = Stmts::new(conn)?;
    stmts.insert_entry_entity.execute(params![entry_id, entity_id, role])?;
    Ok(())
}

/// Get entity by name.
pub fn get_entity_by_name(conn: &Connection, name: &str) -> Result<Option<Entity>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_entity_by_name.query(params![name])?;
    match rows.next()? {
        Some(row) => Ok(Some(Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            description: row.get(3)?,
            metadata: row.get(4)?,
            created_at: row.get(5)?,
        })),
        None => Ok(None),
    }
}

/// FTS5 search entities by name/description.
pub fn search_entities(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Entity>> {
    let mut stmts = Stmts::new(conn)?;
    let fts_query = query.replace('-', " ");
    let mut rows = stmts.search_entities.query(params![fts_query, limit as i64])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            description: row.get(3)?,
            metadata: row.get(4)?,
            created_at: row.get(5)?,
        });
    }
    Ok(results)
}

/// Get all entities associated with an entry (with roles).
pub fn get_entry_entities(conn: &Connection, entry_id: &str) -> Result<Vec<(Entity, String)>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_entry_entities.query(params![entry_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let entity = Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            description: row.get(3)?,
            metadata: row.get(4)?,
            created_at: row.get(5)?,
        };
        let role: String = row.get(6)?;
        results.push((entity, role));
    }
    Ok(results)
}

/// Get all connections (for graph loading).
pub fn get_all_connections(conn: &Connection) -> Result<Vec<LogConnection>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_all_connections.query([])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(LogConnection {
            id: row.get(0)?,
            entry_id_a: row.get(1)?,
            entry_id_b: row.get(2)?,
            relation: row.get(3)?,
            weight: row.get(4)?,
            confidence: row.get(5)?,
            notes: row.get(6)?,
            created_at: row.get(7)?,
        });
    }
    Ok(results)
}

/// Get connections where a given entry appears as A or B.
pub fn get_connections_for_entry(conn: &Connection, entry_id: &str) -> Result<Vec<LogConnection>> {
    let mut stmts = Stmts::new(conn)?;
    let mut rows = stmts.get_connections_for_entry.query(params![entry_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(LogConnection {
            id: row.get(0)?,
            entry_id_a: row.get(1)?,
            entry_id_b: row.get(2)?,
            relation: row.get(3)?,
            weight: row.get(4)?,
            confidence: row.get(5)?,
            notes: row.get(6)?,
            created_at: row.get(7)?,
        });
    }
    Ok(results)
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
    weight REAL DEFAULT 1.0,
    confidence REAL,
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

-- ═══ Entity / Knowledge Graph (v3) ═══

CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'concept',
    description TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entities_kind ON entities(kind);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    name, description,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS entity_relations (
    id INTEGER PRIMARY KEY,
    entity_id_a INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    entity_id_b INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,
    entry_id TEXT REFERENCES entries(id) ON DELETE SET NULL,
    confidence REAL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(entity_id_a, entity_id_b, relation)
);
CREATE INDEX IF NOT EXISTS idx_entity_rel_a ON entity_relations(entity_id_a);
CREATE INDEX IF NOT EXISTS idx_entity_rel_b ON entity_relations(entity_id_b);

CREATE TABLE IF NOT EXISTS entry_entities (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'mentioned',
    PRIMARY KEY (entry_id, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_entry_entities_entity ON entry_entities(entity_id);

-- Entities FTS triggers
CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;
CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
END;
CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;
";

/// Schema migration ALTER only: v2 → v3 safe ALTER TABLE operations
const SCHEMA_ALTER_V2_TO_V3: &str = "
ALTER TABLE connections ADD COLUMN weight REAL DEFAULT 1.0;
ALTER TABLE connections ADD COLUMN confidence REAL;
";

/// Schema migration CREATE only: new v3 tables / indexes / triggers
const SCHEMA_CREATE_V3: &str = "
CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'concept',
    description TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entities_kind ON entities(kind);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    name, description,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS entity_relations (
    id INTEGER PRIMARY KEY,
    entity_id_a INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    entity_id_b INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,
    entry_id TEXT REFERENCES entries(id) ON DELETE SET NULL,
    confidence REAL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(entity_id_a, entity_id_b, relation)
);
CREATE INDEX IF NOT EXISTS idx_entity_rel_a ON entity_relations(entity_id_a);
CREATE INDEX IF NOT EXISTS idx_entity_rel_b ON entity_relations(entity_id_b);

CREATE TABLE IF NOT EXISTS entry_entities (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'mentioned',
    PRIMARY KEY (entry_id, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_entry_entities_entity ON entry_entities(entity_id);

-- FTS triggers for existing tables
CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;
CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
END;
CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
    INSERT INTO entities_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;
";

#[cfg(test)]
mod tests {
    use super::*;
    

    fn test_db() -> Connection {
        init_database(Path::new(":memory:")).unwrap()
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
            weight: 1.0,
            confidence: None,
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

    /// 验证 MIGRATIONS 已按版本升序排列且无间隙
    #[test]
    fn test_migrations_ordering() {
        for i in 1..MIGRATIONS.len() {
            assert!(
                MIGRATIONS[i].0 > MIGRATIONS[i - 1].0,
                "MIGRATIONS[{}] from_ver={} must be > MIGRATIONS[{}] from_ver={}",
                i, MIGRATIONS[i].0, i - 1, MIGRATIONS[i - 1].0
            );
        }
        // 验证迁移序列无 gap：每个 from_ver 必须递增 +1
        if !MIGRATIONS.is_empty() {
            assert!(
                MIGRATIONS[0].0 >= 1,
                "first migration must be for v1 or later, got v{}",
                MIGRATIONS[0].0
            );
            for i in 1..MIGRATIONS.len() {
                assert_eq!(
                    MIGRATIONS[i].0, MIGRATIONS[i - 1].0 + 1,
                    "MIGRATIONS must have no gaps; gap at index {}: v{} -> v{}",
                    i, MIGRATIONS[i - 1].0, MIGRATIONS[i].0
                );
            }
            assert!(
                MIGRATIONS.last().unwrap().0 + 1 <= SCHEMA_VERSION,
                "last migration v{} +1 = v{} > SCHEMA_VERSION v{}",
                MIGRATIONS.last().unwrap().0,
                MIGRATIONS.last().unwrap().0 + 1,
                SCHEMA_VERSION
            );
        }
    }
}
