//! Shared batch processing engine for tool crates.
//!
//! Provides `run_batch`, `resolve_jobs`, `load_paths`, checkpoint/catalog
//! persistence, and the `BatchResult` trait for per-crate result types.

use crate::schema::ProcessStatus;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

// ---------------------------------------------------------------------------
// BatchResult trait
// ---------------------------------------------------------------------------

pub trait BatchResult: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static {
    fn path(&self) -> &str;
    fn status(&self) -> ProcessStatus;
    fn write_index_row(&self, writer: &mut dyn Write) -> std::io::Result<()>;
    fn to_summary_entry(&self) -> serde_json::Value;
}

// ---------------------------------------------------------------------------
// BatchOptions (shared)
// ---------------------------------------------------------------------------

pub struct BatchOptions {
    pub out_dir: PathBuf,
    pub jobs: usize,
    pub resume: bool,
    pub fail_fast: bool,
    pub max_chars: usize,
}

// ---------------------------------------------------------------------------
// Job resolution
// ---------------------------------------------------------------------------

pub const JOBS_AUTO: &str = "auto";

/// Resolve batch parallelism: `{PREFIX}_BATCH_JOBS` env > explicit `--jobs N` > `auto`.
/// The `env_prefix` identifies the tool (e.g. "PDF", "OOXML").
pub fn resolve_jobs(env_prefix: &str, jobs_arg: &str, hint_paths: &[PathBuf]) -> usize {
    let env_key = format!("{env_prefix}_BATCH_JOBS");
    if let Ok(raw) = std::env::var(&env_key) {
        if let Ok(n) = raw.parse::<usize>() {
            return n.max(1);
        }
    }

    if !jobs_arg.eq_ignore_ascii_case(JOBS_AUTO) {
        if let Ok(n) = jobs_arg.parse::<usize>() {
            return n.max(1);
        }
        return 1;
    }

    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut jobs = std::cmp::min(8, cpus);
    if slow_fs_hint(env_prefix, hint_paths) {
        jobs = std::cmp::min(2, jobs);
    }
    jobs.max(1)
}

fn slow_fs_hint(env_prefix: &str, hint_paths: &[PathBuf]) -> bool {
    let env_key = format!("{env_prefix}_BATCH_SLOW_FS");
    if std::env::var(&env_key).is_ok_and(|v| !v.is_empty() && v != "0") {
        return true;
    }
    hint_paths.iter().any(|p| {
        p.to_string_lossy()
            .split('/')
            .nth(1)
            .is_some_and(|seg| seg == "Volumes" || seg == "mnt" || seg == "net")
    })
}

// ---------------------------------------------------------------------------
// Path loading
// ---------------------------------------------------------------------------

pub fn load_paths(manifest: Option<&Path>, stdin_paths: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if stdin_paths {
        let stdin = BufReader::new(std::io::stdin());
        for line in stdin.lines() {
            let line = line?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                paths.push(PathBuf::from(trimmed));
            }
        }
    } else if let Some(manifest_path) = manifest {
        let raw = fs::read_to_string(manifest_path)
            .with_context(|| format!("read manifest {}", manifest_path.display()))?;
        let value: Value = serde_json::from_str(&raw)
            .with_context(|| format!("parse manifest {}", manifest_path.display()))?;
        match value {
            Value::Array(arr) => {
                for item in arr {
                    match item {
                        Value::String(s) => paths.push(PathBuf::from(s)),
                        other => bail!("manifest array items must be strings, got {other}"),
                    }
                }
            }
            Value::Object(mut map) => {
                if let Some(Value::Array(arr)) = map.remove("paths") {
                    for item in arr {
                        match item {
                            Value::String(s) => paths.push(PathBuf::from(s)),
                            other => bail!("paths items must be strings, got {other}"),
                        }
                    }
                } else {
                    bail!("manifest object must contain a \"paths\" array");
                }
            }
            other => bail!("manifest must be a JSON array or object, got {other}"),
        }
    } else {
        bail!("batch requires --manifest or --stdin-paths");
    }
    Ok(paths)
}

// ---------------------------------------------------------------------------
// Checkpoint / Catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub completed: Vec<String>,
    pub failed: Vec<String>,
    pub skipped: Vec<String>,
}

impl Checkpoint {
    pub fn is_done(&self, path_key: &str) -> bool {
        self.completed.contains(&path_key.to_string())
            || self.failed.contains(&path_key.to_string())
            || self.skipped.contains(&path_key.to_string())
    }

    pub fn mark(&mut self, path_key: &str, status: ProcessStatus) {
        let key = path_key.to_string();
        self.completed.retain(|p| p != &key);
        self.failed.retain(|p| p != &key);
        self.skipped.retain(|p| p != &key);
        match status {
            ProcessStatus::Ok => self.completed.push(key),
            ProcessStatus::Error => self.failed.push(key),
            ProcessStatus::Skipped => self.skipped.push(key),
        }
    }
}

fn path_key(path: &Path) -> Result<String> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("canonicalize {}", path.display()))?;
    Ok(canonical.display().to_string())
}

fn load_checkpoint(out_dir: &Path) -> Result<Checkpoint> {
    let cp_path = out_dir.join("checkpoint.json");
    if !cp_path.exists() {
        return Ok(Checkpoint::default());
    }
    let raw = fs::read_to_string(&cp_path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_checkpoint(out_dir: &Path, checkpoint: &Checkpoint) -> Result<()> {
    let cp_path = out_dir.join("checkpoint.json");
    let file = File::create(&cp_path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, checkpoint)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CatalogSummary (returned to caller for stdout)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogSummary {
    pub version: u32,
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub out_dir: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sample: Vec<Value>,
}

// ---------------------------------------------------------------------------
// Core run_batch
// ---------------------------------------------------------------------------

const JSONL_BUF_CAPACITY: usize = 64 * 1024;

/// Run a batch job over `paths`, calling `process` on each. The
/// `index_header` is written to the top of `index.md`.
pub fn run_batch<R, F>(
    paths: Vec<PathBuf>,
    opts: &BatchOptions,
    index_header: &str,
    process: F,
) -> Result<CatalogSummary>
where
    R: BatchResult,
    F: Fn(&Path) -> R + Send + Sync,
{
    fs::create_dir_all(&opts.out_dir)?;
    fs::create_dir_all(opts.out_dir.join("text"))?;

    let checkpoint = if opts.resume {
        load_checkpoint(&opts.out_dir)?
    } else {
        Checkpoint::default()
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.jobs)
        .build()
        .context("build rayon thread pool")?;

    let mut pending: Vec<(String, PathBuf)> = Vec::new();
    for path in paths {
        let key = path_key(&path)?;
        if opts.resume && checkpoint.is_done(&key) {
            continue;
        }
        pending.push((key, path));
    }

    let (tx, rx) = mpsc::channel();
    let writer = spawn_jsonl_writer::<R>(opts.out_dir.clone(), checkpoint, opts.fail_fast, rx);

    pool.install(|| {
        pending.par_iter().for_each(|(key, path)| {
            let result = process(path);
            let _ = tx.send((key.clone(), result));
        });
    });
    drop(tx);

    let (_checkpoint, results) = writer
        .join()
        .map_err(|_| anyhow::anyhow!("jsonl writer thread panicked"))??;

    // Merge prior checkpoint entries into catalog when resuming.
    let mut all_entries = results;
    if opts.resume {
        let jsonl_path = opts.out_dir.join("results.jsonl");
        if jsonl_path.exists() {
            let file = File::open(&jsonl_path)?;
            let reader = BufReader::new(file);
            let mut seen = std::collections::HashSet::new();
            for e in &all_entries {
                seen.insert(e.path().to_string());
            }
            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let prev: R = serde_json::from_str(&line)?;
                if seen.insert(prev.path().to_string()) {
                    all_entries.push(prev);
                }
            }
        }
    }

    let processed = all_entries.iter().filter(|e| e.status() == ProcessStatus::Ok).count();
    let failed = all_entries.iter().filter(|e| e.status() == ProcessStatus::Error).count();
    let skipped = all_entries.iter().filter(|e| e.status() == ProcessStatus::Skipped).count();

    // Write catalog.json
    let catalog = serde_json::json!({
        "version": 1,
        "out_dir": opts.out_dir.display().to_string(),
        "total": all_entries.len(),
        "processed": processed,
        "failed": failed,
        "skipped": skipped,
        "entries": all_entries.iter().map(|e| serde_json::to_value(e).unwrap_or_default()).collect::<Vec<_>>(),
    });
    write_catalog(&opts.out_dir, &catalog)?;
    write_index(&opts.out_dir, &all_entries, index_header)?;

    let sample: Vec<Value> = all_entries
        .iter()
        .take(8)
        .map(|e| e.to_summary_entry())
        .collect();

    Ok(CatalogSummary {
        version: 1,
        total: all_entries.len(),
        processed,
        failed,
        skipped,
        out_dir: opts.out_dir.display().to_string(),
        sample,
    })
}

fn spawn_jsonl_writer<R: BatchResult>(
    out_dir: PathBuf,
    mut checkpoint: Checkpoint,
    fail_fast: bool,
    rx: mpsc::Receiver<(String, R)>,
) -> thread::JoinHandle<Result<(Checkpoint, Vec<R>)>> {
    thread::spawn(move || -> Result<(Checkpoint, Vec<R>)> {
        let jsonl_path = out_dir.join("results.jsonl");
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jsonl_path)?;
        let mut jsonl = BufWriter::with_capacity(JSONL_BUF_CAPACITY, file);
        let mut results = Vec::new();
        let mut stop = false;

        for (key, result) in rx {
            if stop {
                continue;
            }
            serde_json::to_writer(&mut jsonl, &result)?;
            jsonl.write_all(b"\n")?;
            checkpoint.mark(&key, result.status());
            results.push(result);
            if fail_fast && results.last().is_some_and(|r| r.status() == ProcessStatus::Error) {
                stop = true;
            }
        }

        jsonl.flush()?;
        save_checkpoint(&out_dir, &checkpoint)?;
        Ok((checkpoint, results))
    })
}

fn write_catalog(out_dir: &Path, catalog: &serde_json::Value) -> Result<()> {
    let path = out_dir.join("catalog.json");
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, catalog)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn write_index<R: BatchResult>(out_dir: &Path, entries: &[R], header: &str) -> Result<()> {
    let path = out_dir.join("index.md");
    let mut file = File::create(&path)?;
    file.write_all(header.as_bytes())?;
    for e in entries {
        e.write_index_row(&mut file)?;
    }
    Ok(())
}

pub fn print_catalog_summary(summary: &CatalogSummary) -> Result<()> {
    let json = serde_json::to_string(summary)?;
    let max_stdout = 4096;
    if json.len() <= max_stdout {
        println!("{json}");
    } else {
        let truncated = CatalogSummary {
            sample: summary.sample.iter().take(3).cloned().collect(),
            ..summary.clone()
        };
        let compact = serde_json::to_string(&truncated)?;
        println!("{}", &compact[..compact.len().min(max_stdout)]);
    }
    Ok(())
}
