use super::paths::normalize_runtime_path;
use super::SQLITE_TABLE_NAME;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn env_checkpoint_storage_db_path() -> Option<PathBuf> {
    std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE")
        .ok()
        .and_then(|value| normalize_runtime_path(&value).ok())
}

pub fn runtime_storage_db_name_candidates() -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for candidate in [
        std::env::var("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE").ok(),
        Some("runtime_checkpoint_store.sqlite3".to_string()),
    ]
    .into_iter()
    .flatten()
    {
        if seen.insert(candidate.clone()) {
            ordered.push(candidate);
        }
    }
    ordered
}

pub fn sqlite_connection(path: &Path) -> Result<std::rc::Rc<Connection>, String> {
    use std::cell::RefCell;
    use std::rc::Rc;
    thread_local! {
        static CACHED: RefCell<Option<(PathBuf, Rc<Connection>)>> = const { RefCell::new(None) };
    }
    CACHED.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some((ref cached_path, ref cached_conn)) = *slot {
            if cached_path == path {
                return Ok(Rc::clone(cached_conn));
            }
        }
        let conn = Connection::open(path).map_err(|err| {
            format!(
                "open sqlite runtime storage failed for {}: {err}",
                path.display()
            )
        })?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| format!("enable sqlite runtime storage WAL failed: {err}"))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|err| format!("set sqlite runtime storage synchronous mode failed: {err}"))?;
        ensure_runtime_storage_sqlite_schema(&conn)?;
        let shared = Rc::new(conn);
        let result = Rc::clone(&shared);
        *slot = Some((path.to_path_buf(), shared));
        Ok(result)
    })
}

pub fn ensure_runtime_storage_sqlite_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {SQLITE_TABLE_NAME} (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
        ),
        [],
    )
    .map_err(|err| format!("ensure sqlite runtime storage schema failed: {err}"))?;
    Ok(())
}

pub fn sqlite_lookup_key(path: &Path, storage_root: &Path) -> Result<String, String> {
    let resolved_path = normalize_runtime_path(&path.display().to_string())?;
    let resolved_root = normalize_runtime_path(&storage_root.display().to_string())?;
    let relative_path = resolved_path
        .strip_prefix(&resolved_root)
        .map_err(|_| {
            format!(
                "sqlite runtime storage path {} must stay under storage root {}",
                resolved_path.display(),
                resolved_root.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    // Namescape keys by resolved storage root so one sqlite db can safely serve
    // multiple session roots without payload collisions.
    let root_scope = resolved_root.display().to_string().replace('\\', "/");
    Ok(format!("{root_scope}::{relative_path}"))
}

const SQLITE_EXISTS_SQL: &str =
    "SELECT 1 FROM runtime_storage_payloads WHERE payload_key = ?1 LIMIT 1";
const SQLITE_READ_SQL: &str =
    "SELECT payload_text FROM runtime_storage_payloads WHERE payload_key = ?1 LIMIT 1";
const SQLITE_WRITE_SQL: &str =
    "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)
     ON CONFLICT(payload_key) DO UPDATE SET payload_text = excluded.payload_text";
const SQLITE_APPEND_SQL: &str =
    "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?1, ?2)
     ON CONFLICT(payload_key) DO UPDATE
     SET payload_text = runtime_storage_payloads.payload_text || excluded.payload_text";

pub fn sqlite_payload_exists(path: &Path, db_path: &Path, storage_root: &Path) -> Result<bool, String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare_cached(SQLITE_EXISTS_SQL)
        .map_err(|err| format!("prepare sqlite exists query failed: {err}"))?;
    let exists = stmt
        .query_row(params![stable_key], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(|err| format!("run sqlite exists query failed: {err}"))?
        .is_some();
    Ok(exists)
}

pub fn sqlite_read_text(path: &Path, db_path: &Path, storage_root: &Path) -> Result<String, String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    let conn = sqlite_connection(db_path)?;
    let mut stmt = conn
        .prepare_cached(SQLITE_READ_SQL)
        .map_err(|err| format!("prepare sqlite read query failed: {err}"))?;
    stmt.query_row(params![stable_key], |row| row.get::<_, String>(0))
        .map_err(|err| format!("read sqlite payload failed for {}: {err}", path.display()))
}

pub fn sqlite_write_text(
    path: &Path,
    db_path: &Path,
    storage_root: &Path,
    payload_text: &str,
) -> Result<(), String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create sqlite parent directory for {} failed: {err}",
                db_path.display()
            )
        })?;
    }
    let conn = sqlite_connection(db_path)?;
    conn.execute(
        SQLITE_WRITE_SQL,
        params![stable_key, payload_text],
    )
    .map_err(|err| format!("write sqlite payload failed for {}: {err}", path.display()))?;
    Ok(())
}

pub fn sqlite_append_text(
    path: &Path,
    db_path: &Path,
    storage_root: &Path,
    payload_text: &str,
) -> Result<(), String> {
    let stable_key = sqlite_lookup_key(path, storage_root)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create sqlite parent directory for {} failed: {err}",
                db_path.display()
            )
        })?;
    }
    let conn = sqlite_connection(db_path)?;
    conn.execute(
        SQLITE_APPEND_SQL,
        params![stable_key, payload_text],
    )
    .map_err(|err| format!("append sqlite payload failed for {}: {err}", path.display()))?;
    Ok(())
}
