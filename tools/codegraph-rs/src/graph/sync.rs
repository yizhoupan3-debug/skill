//! Incremental index sync + filesystem watcher.

use crate::CodeGraphIndex;
use crate::db::index_ops::{
    IndexedFileMeta, IngestStmts, ingest_parsed_file_with_stmts, list_indexed_files, set_meta,
};
use crate::parser::{self, ParsedFile, common::hex_encode, parse_file};
use anyhow::Context;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

const WATCHER_DEBOUNCE: Duration = Duration::from_millis(400);

const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "artifacts",
    ".cursor",
    ".claude",
    ".codex",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".cache",
    ".mypy_cache",
    ".pytest_cache",
    ".tox",
    "coverage",
    ".turbo",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub files_scanned: u64,
    pub files_updated: u64,
    pub files_removed: u64,
    pub nodes_added: u64,
    pub edges_added: u64,
}

pub fn build_full_index(index: &CodeGraphIndex, repo_root: &Path) -> anyhow::Result<SyncReport> {
    incremental_sync(index, repo_root, true)
}

/// Incremental sync: mtime fast-path first, then content-hash comparison.
///
/// Performance: most unchanged files are skipped with a single stat() syscall
/// (mtime check) instead of reading the full file content for SHA-256.
pub fn incremental_sync(
    index: &CodeGraphIndex,
    repo_root: &Path,
    force_all: bool,
) -> anyhow::Result<SyncReport> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut report = SyncReport::default();
    let mut seen = HashSet::new();
    let indexed: HashMap<String, IndexedFileMeta> = list_indexed_files(index.connection())?
        .into_iter()
        .map(|meta| (meta.path.clone(), meta))
        .collect();

    // Phase 1: fast mtime check — skip files whose mtime hasn't changed (O(1) per file).
    // Only files with changed mtime need content hash comparison.
    let mut pending: Vec<FileWorkItem> = Vec::new();
    for path in discover_source_files(&repo_root)? {
        report.files_scanned += 1;
        let rel = relative_path(&repo_root, &path);
        seen.insert(rel.clone());

        if !force_all {
            // Fast path: mtime unchanged → file definitely unchanged, skip without I/O
            if let Some(stored) = indexed.get(&rel) {
                let mtime_ns = file_mtime_ns(&path)?;
                if stored.mtime_ns == mtime_ns {
                    continue; // stat-only fast path
                }
                // mtime changed — need content hash to verify
                let content_hash = file_content_hash(&path)?;
                if stored.content_hash == content_hash {
                    continue; // mtime changed but content identical
                }
                pending.push(FileWorkItem {
                    path,
                    rel,
                    mtime_ns,
                    content_hash,
                });
            } else {
                // New file — always index
                let mtime_ns = file_mtime_ns(&path)?;
                let content_hash = file_content_hash(&path)?;
                pending.push(FileWorkItem {
                    path,
                    rel,
                    mtime_ns,
                    content_hash,
                });
            }
        } else {
            // Force-all mode: always read and parse
            let mtime_ns = file_mtime_ns(&path)?;
            let content_hash = file_content_hash(&path)?;
            pending.push(FileWorkItem {
                path,
                rel,
                mtime_ns,
                content_hash,
            });
        }
    }

    // Phase 2: parallel parse unchanged files
    let parsed: Vec<ParsedFile> = pending
        .par_iter()
        .filter_map(|item| parse_work_item(item).ok().flatten())
        .collect();

    // Phase 3: skill registry sync — index skill metadata into FTS
    sync_skill_registry(index, &repo_root, force_all, &mut report)?;

    // Phase 4: sequential DB ingest (source files)
    let conn = index.connection();
    let mut ingest_stmts = IngestStmts::prepare(conn)?;
    for parsed in parsed {
        let (nodes, edges) = ingest_parsed_file_with_stmts(conn, &mut ingest_stmts, &parsed)?;
        report.files_updated += 1;
        report.nodes_added += nodes;
        report.edges_added += edges;
    }

    // Phase 5: remove stale files (atomic transaction)
    //
    // Note: DeleteFileStmts are prepared on `conn`, so we use execute_batch
    // for the BEGIN/COMMIT wrapper rather than rusqlite Transaction.
    conn.execute_batch("BEGIN")?;
    for (path, _) in indexed {
        if !seen.contains(&path) {
            ingest_stmts.delete.execute(&path)?;
            report.files_removed += 1;
        }
    }
    conn.execute_batch("COMMIT")?;

    let indexed_at = chrono::Local::now().to_rfc3339();
    set_meta(conn, "indexed_at", &indexed_at)?;
    Ok(report)
}

/// Sync skill registry metadata into the codegraph index.
///
/// Reads `skills/SKILL_ROUTING_RUNTIME.json`, checks mtime/content-hash for incremental
/// updates, and ingests skill + keyword nodes into the DB with FTS indexing.
fn sync_skill_registry(
    index: &CodeGraphIndex,
    repo_root: &Path,
    force_all: bool,
    report: &mut SyncReport,
) -> anyhow::Result<()> {
    let registry_path = repo_root.join(parser::skill::RUNTIME_REL_PATH);
    if !registry_path.is_file() {
        return Ok(());
    }

    let conn = index.connection();

    // Fast path: mtime check without reading the file
    if !force_all {
        let mtime_ns = file_mtime_ns(&registry_path)?;
        let indexed = list_indexed_files(conn)?
            .into_iter()
            .find(|m| m.path == parser::skill::RUNTIME_REL_PATH);
        if let Some(ref stored) = indexed
            && stored.mtime_ns == mtime_ns {
                return Ok(()); // mtime unchanged, skip entirely
            }
    }

    // mtime changed or new file: read once, share between hash and parser
    let content = std::fs::read_to_string(&registry_path)
        .context("read skill registry")?;
    let mtime_ns = file_mtime_ns(&registry_path)?;
    let content_hash = hex_encode(
        Sha256::digest(content.as_bytes()).as_slice(),
    );

    // Content-hash check for mtime-noise (touch without content change)
    if !force_all {
        let indexed = list_indexed_files(conn)?
            .into_iter()
            .find(|m| m.path == parser::skill::RUNTIME_REL_PATH);
        if let Some(ref stored) = indexed
            && stored.content_hash == content_hash {
                return Ok(());
            }
    }

    let Some(parsed) = parser::skill::parse_skill_registry_with_content(
        &content, mtime_ns, content_hash,
    ) else {
        return Ok(());
    };

    let mut stmts = IngestStmts::prepare(conn)?;
    let (nodes, edges) = ingest_parsed_file_with_stmts(conn, &mut stmts, &parsed)?;
    report.nodes_added += nodes;
    report.edges_added += edges;
    report.files_updated += 1;
    report.files_scanned += 1;

    Ok(())
}

#[derive(Debug, Clone)]
struct FileWorkItem {
    path: PathBuf,
    rel: String,
    mtime_ns: i64,
    content_hash: String,
}

fn parse_work_item(item: &FileWorkItem) -> anyhow::Result<Option<ParsedFile>> {
    let contents = fs::read_to_string(&item.path)
        .with_context(|| format!("read source file {}", item.path.display()))?;
    let Some(mut parsed) = parse_file(Path::new(&item.rel), &contents, item.mtime_ns) else {
        return Ok(None);
    };
    parsed.path = item.rel.clone();
    parsed.content_hash = item.content_hash.clone();
    Ok(Some(parsed))
}

fn file_content_hash(path: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read file for hash {}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(hex_encode(digest.as_slice()))
}

fn discover_source_files(repo_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_sources(repo_root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_sources(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.iter().any(|skip| name == *skip) {
                continue;
            }
            walk_sources(&path, out)?;
            continue;
        }
        if parser::common::detect_language(&path.to_string_lossy()).is_some() {
            out.push(path);
        }
    }
    Ok(())
}

fn relative_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn file_mtime_ns(path: &Path) -> anyhow::Result<i64> {
    let meta = fs::metadata(path)?;
    let modified = match meta.modified() {
        Ok(m) => m,
        Err(e) => {
            // Filesystems like FUSE/NFS may not support mtime; fall back to epoch.
            eprintln!("[codegraph] mtime unavailable for {} ({}); using epoch", path.display(), e);
            SystemTime::UNIX_EPOCH
        }
    };
    let duration = modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(duration.as_nanos() as i64)
}

/// Filesystem watcher that triggers incremental sync on file changes.
///
/// The watcher reuses a single SQLite connection across sync cycles and
/// properly joins its background thread on Drop.
pub struct IndexWatcher {
    _watcher: RecommendedWatcher,
    _handle: Option<JoinHandle<()>>,
}

impl IndexWatcher {
    pub fn spawn(repo_root: PathBuf) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            RecommendedWatcher::new(tx, Config::default()).context("create filesystem watcher")?;
        watcher
            .watch(&repo_root, RecursiveMode::Recursive)
            .with_context(|| format!("watch {}", repo_root.display()))?;

        let handle = thread::spawn(move || {
            // Open a single DB connection for the watcher lifetime.
            let index = match CodeGraphIndex::open(&repo_root) {
                Ok(idx) => idx,
                Err(err) => {
                    eprintln!("codegraph IndexWatcher: failed to open index: {err}");
                    return;
                }
            };
            let mut pending = false;
            let mut last_event = Instant::now();
            loop {
                let timeout = if pending {
                    WATCHER_DEBOUNCE.saturating_sub(last_event.elapsed())
                } else {
                    Duration::from_secs(3600)
                };
                match rx.recv_timeout(timeout) {
                    Ok(Ok(event)) => {
                        let relevant = matches!(
                            event.kind,
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                        );
                        if relevant {
                            pending = true;
                            last_event = Instant::now();
                        }
                    }
                    Ok(Err(_)) => continue,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if !pending {
                            continue;
                        }
                        pending = false;
                        if let Err(err) = incremental_sync(&index, &repo_root, false) {
                            eprintln!(
                                "codegraph IndexWatcher: incremental_sync failed for {}: {err}",
                                repo_root.display()
                            );
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            _handle: Some(handle),
        })
    }
}

impl Drop for IndexWatcher {
    fn drop(&mut self) {
        // Replace the watcher with a no-op watcher to close the mpsc channel
        // without dropping the receiver (which would cause the thread to panic).
        // Then join the thread for clean shutdown.
        let (tx, _) = mpsc::channel();
        self._watcher = RecommendedWatcher::new(tx, Config::default())
            .unwrap_or_else(|_| {
                // If we can't create a replacement, the original will be dropped
                // when this struct is dropped, closing the channel.
                RecommendedWatcher::new(mpsc::channel().0, Config::default())
                    .expect("fallback watcher")
            });
        if let Some(handle) = self._handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_full_index, incremental_sync};
    use crate::CodeGraphIndex;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> (std::path::PathBuf, CodeGraphIndex) {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time since epoch")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("codegraph-sync-{suffix}"));
        fs::create_dir_all(&root).expect("create temp directory");
        fs::write(
            root.join("lib.rs"),
            "fn alpha() {}\nfn beta() { alpha(); }\n",
        )
        .expect("should succeed");
        let index = CodeGraphIndex::open(&root).expect("open test index");
        (root, index)
    }

    #[test]
    fn incremental_sync_indexes_rust_sources() {
        let (root, index) = temp_repo();
        let report = build_full_index(&index, &root).expect("build full index");
        assert!(report.files_updated >= 1);
        assert!(report.nodes_added >= 2);
        let stats = index.index_stats().expect("build full index");
        assert!(stats.node_count >= 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn second_sync_skips_unchanged_files() {
        let (root, index) = temp_repo();
        build_full_index(&index, &root).expect("build full index");
        let report = incremental_sync(&index, &root, false).expect("build full index");
        assert_eq!(report.files_updated, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_reindexes_when_content_hash_stale_despite_matching_mtime() {
        let (root, index) = temp_repo();
        build_full_index(&index, &root).expect("build full index");
        let conn = index.connection();
        let (mtime_ns, stale_hash): (i64, String) = conn
            .query_row(
                "SELECT mtime_ns, content_hash FROM files WHERE path = 'lib.rs'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("should succeed");
        fs::write(root.join("lib.rs"), "fn gamma() {}\n").expect("write test file");
        conn.execute(
            "UPDATE files SET content_hash = ?1, mtime_ns = ?2 WHERE path = 'lib.rs'",
            rusqlite::params![stale_hash, mtime_ns],
        )
        .expect("should succeed");
        let report = incremental_sync(&index, &root, false).expect("incremental sync");
        assert!(
            report.files_updated >= 1,
            "expected re-index when on-disk content hash differs: {:?}",
            report
        );
        let search = index
            .search_symbols("gamma", None, None, 10)
            .expect("search symbols");
        assert!(search.iter().any(|n| n.symbol == "gamma"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_removes_deleted_files_from_index() {
        let (root, index) = temp_repo();
        build_full_index(&index, &root).expect("build full index");
        let lib_rs = root.join("lib.rs");
        assert!(
            lib_rs.is_file(),
            "expected lib.rs before delete sync test: {}",
            lib_rs.display()
        );
        fs::remove_file(&lib_rs).expect("remove test file");
        let report = incremental_sync(&index, &root, false).expect("remove test file");
        assert!(report.files_removed >= 1);
        let stats = index.index_stats().expect("remove test file");
        assert_eq!(stats.node_count, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parallel_sync_indexes_multiple_files() {
        let (root, index) = temp_repo();
        for i in 0..8 {
            fs::write(
                root.join(format!("module_{i}.rs")),
                format!("fn sym_{i}() {{}}\n"),
            )
            .expect("should succeed");
        }
        let report = build_full_index(&index, &root).expect("build full index");
        assert!(
            report.files_updated >= 9,
            "expected parallel ingest of all modules"
        );
        let stats = index.index_stats().expect("build full index");
        assert!(stats.node_count >= 10);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watcher_spawns_without_error() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-watch-{suffix}"));
        fs::create_dir_all(&root).expect("create temp directory");
        let index = CodeGraphIndex::open(&root).expect("create temp directory");
        let watcher = super::IndexWatcher::spawn(root.clone()).expect("create temp directory");
        // Drop watcher before removing directory to avoid filesystem race
        drop(watcher);
        let _ = fs::remove_dir_all(root);
        let _ = index;
    }

    #[test]
    fn index_builds_markdown_headings_under_docs() {
        let (root, index) = temp_repo();
        let docs_dir = root.join("docs").join("spec");
        fs::create_dir_all(&docs_dir).expect("create docs dir");
        fs::write(
            docs_dir.join("test-spec.md"),
            "# Core Crates\n\nThis is about Rust crates.\n\n## Module Structure\n\nDetails here.\n",
        )
        .expect("write test md");
        let report = build_full_index(&index, &root).expect("build full index");
        assert!(
            report.files_updated >= 1,
            "expected at least 1 file indexed"
        );
        let hits = index
            .search_symbols("Core Crates", None, Some("markdown"), 10)
            .expect("search symbols");
        assert!(
            hits.iter().any(|n| n.symbol == "Core Crates"),
            "expected heading 'Core Crates' in search results"
        );
        let hits2 = index
            .search_symbols("Module Structure", None, None, 10)
            .expect("search symbols");
        assert!(hits2.iter().any(|n| n.symbol == "Module Structure"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn index_skips_markdown_outside_docs() {
        let (root, index) = temp_repo();
        fs::write(root.join("README.md"), "# Project\n\nDesc.\n")
            .expect("write readme");
        let _report = build_full_index(&index, &root).expect("build full index");
        let hits = index
            .search_symbols("Project", None, None, 10)
            .expect("search symbols");
        assert!(
            !hits.iter().any(|n| n.symbol == "Project"),
            "README.md should not be indexed"
        );
        let _ = fs::remove_dir_all(root);
    }
}
