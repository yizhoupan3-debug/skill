//! Process-local cache for `terminals/*.txt` scans within one hook subprocess.
//!
//! Cache hits avoid repeat `read_dir` scans; callers still receive an owned `Vec` clone per call.
//! The main win is skipping duplicate directory walks within one hook subprocess (not zero-copy).

use super::{TerminalObservation, parse_terminal_header};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

static SCAN_COUNT: AtomicU64 = AtomicU64::new(0);

/// Max distinct `terminals_dir` keys per hook subprocess (daemon would need revisiting).
const MAX_TERMINAL_CACHE_DIRS: usize = 8;

struct CacheEntry {
    dir_mtime: Option<SystemTime>,
    observations: Arc<Vec<TerminalObservation>>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, CacheEntry>>> = Mutex::new(None);

fn dir_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn scan_terminals_dir(terminals_dir: &Path) -> Vec<TerminalObservation> {
    SCAN_COUNT.fetch_add(1, Ordering::Relaxed);
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(terminals_dir) else {
        return out;
    };
    let mut buf = String::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        buf.clear();
        if let Ok(file) = fs::File::open(&path) {
            let _ = file.take(4096).read_to_string(&mut buf);
        }
        let Some(header) = parse_terminal_header(&buf) else {
            continue;
        };
        let (Some(pid), Some(cwd)) = (header.pid, header.cwd) else {
            continue;
        };
        out.push(TerminalObservation {
            pid,
            cwd,
            active_command: header.active_command,
            last_command: header.last_command,
            started_at_ms: header.started_at_ms,
        });
    }
    out
}

pub fn collect_terminal_observations_cached(terminals_dir: &Path) -> Vec<TerminalObservation> {
    let mtime = dir_mtime(terminals_dir);
    let mut guard = CACHE.lock().expect("terminal cache mutex");
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(entry) = map.get(terminals_dir) {
        if entry.dir_mtime == mtime {
            return (*entry.observations).clone();
        }
    }
    let observations = scan_terminals_dir(terminals_dir);
    let shared = Arc::new(observations);
    if map.len() >= MAX_TERMINAL_CACHE_DIRS {
        map.clear();
    }
    map.insert(
        terminals_dir.to_path_buf(),
        CacheEntry {
            dir_mtime: mtime,
            observations: Arc::clone(&shared),
        },
    );
    (*shared).clone()
}

#[cfg(test)]
pub fn terminal_scan_count_for_tests() -> u64 {
    SCAN_COUNT.load(Ordering::Relaxed)
}

#[cfg(test)]
pub fn reset_terminal_cache_for_tests() {
    SCAN_COUNT.store(0, Ordering::Relaxed);
    let mut guard = CACHE.lock().expect("terminal cache mutex");
    guard.take();
}
