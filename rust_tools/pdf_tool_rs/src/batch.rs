use crate::read::{read_to_result, shallow_scan_classify, ReadOptions};
use crate::schema::{
    Catalog, CatalogSummary, CatalogSummaryEntry, Checkpoint, ContentClass, FileResult,
    ProcessStatus,
};
use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

pub struct BatchOptions {
    pub out_dir: PathBuf,
    pub jobs: usize,
    pub resume: bool,
    /// When true, run [`crate::read::shallow_scan_classify`] before full extract; image-only
    /// or blank PDFs get `status: skipped`, `content_class: scanned|empty`, and warning
    /// `skip_scanned` (no `text/<sha>.txt` written).
    pub skip_scanned: bool,
    pub fail_fast: bool,
    pub max_chars: usize,
}

pub const JOBS_AUTO: &str = "auto";

pub fn default_jobs() -> usize {
    resolve_jobs(JOBS_AUTO, &[])
}

/// Resolve batch parallelism: `PDF_BATCH_JOBS` env > explicit `--jobs N` > `auto`.
pub fn resolve_jobs(jobs_arg: &str, hint_paths: &[PathBuf]) -> usize {
    if let Ok(raw) = std::env::var("PDF_BATCH_JOBS") {
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
    if slow_fs_hint(hint_paths) {
        jobs = std::cmp::min(2, jobs);
    }
    jobs.max(1)
}

fn slow_fs_hint(hint_paths: &[PathBuf]) -> bool {
    if std::env::var("PDF_BATCH_SLOW_FS").is_ok_and(|v| !v.is_empty() && v != "0") {
        return true;
    }
    hint_paths.iter().any(|p| {
        p.to_string_lossy()
            .split('/')
            .nth(1)
            .is_some_and(|seg| seg == "Volumes" || seg == "mnt" || seg == "net")
    })
}

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

const JSONL_BUF_CAPACITY: usize = 64 * 1024;

fn skipped_scanned_file_result(path: &Path, pages: u32, class: ContentClass) -> FileResult {
    FileResult {
        path: path.display().to_string(),
        sha256: crate::read::file_sha256(path).unwrap_or_default(),
        status: ProcessStatus::Skipped,
        content_class: class,
        page_count: pages,
        text_path: None,
        char_count: 0,
        truncated: false,
        warnings: vec!["skip_scanned".to_string()],
        error: None,
    }
}

fn spawn_jsonl_writer(
    out_dir: PathBuf,
    mut checkpoint: Checkpoint,
    fail_fast: bool,
    rx: mpsc::Receiver<(String, FileResult)>,
) -> thread::JoinHandle<Result<(Checkpoint, Vec<FileResult>)>> {
    thread::spawn(move || -> Result<(Checkpoint, Vec<FileResult>)> {
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
            checkpoint.mark(&key, result.status);
            results.push(result);
            if fail_fast && results.last().is_some_and(|r| r.status == ProcessStatus::Error) {
                stop = true;
            }
        }

        jsonl.flush()?;
        save_checkpoint(&out_dir, &checkpoint)?;
        Ok((checkpoint, results))
    })
}

fn write_catalog(out_dir: &Path, catalog: &Catalog) -> Result<()> {
    let path = out_dir.join("catalog.json");
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, catalog)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn write_index(out_dir: &Path, entries: &[FileResult]) -> Result<()> {
    let path = out_dir.join("index.md");
    let mut file = File::create(&path)?;
    writeln!(file, "# PDF batch index\n")?;
    writeln!(file, "| path | status | class | pages | text |")?;
    writeln!(file, "| --- | --- | --- | ---: | --- |")?;
    for e in entries {
        let text_ref = e.text_path.as_deref().unwrap_or("-");
        writeln!(
            file,
            "| {} | {:?} | {} | {} | {} |",
            e.path,
            e.status,
            e.content_class.as_str(),
            e.page_count,
            text_ref
        )?;
    }
    Ok(())
}

pub fn run_batch(paths: Vec<PathBuf>, opts: &BatchOptions) -> Result<CatalogSummary> {
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

    let read_opts = ReadOptions {
        max_chars: opts.max_chars,
        text_out_dir: Some(opts.out_dir.clone()),
    };

    let mut pending: Vec<(String, PathBuf)> = Vec::new();
    for path in paths {
        let key = path_key(&path)?;
        if opts.resume && checkpoint.is_done(&key) {
            continue;
        }
        pending.push((key, path));
    }

    let (tx, rx) = mpsc::channel();
    let writer = spawn_jsonl_writer(
        opts.out_dir.clone(),
        checkpoint,
        opts.fail_fast,
        rx,
    );

    pool.install(|| {
        pending.par_iter().for_each(|(key, path)| {
            let result = if opts.skip_scanned {
                match shallow_scan_classify(path) {
                    Ok((pages, class, true)) => {
                        skipped_scanned_file_result(path, pages, class)
                    }
                    Ok(_) => read_to_result(path, &read_opts),
                    Err(e) => FileResult {
                        path: path.display().to_string(),
                        sha256: String::new(),
                        status: ProcessStatus::Error,
                        content_class: ContentClass::Error,
                        page_count: 0,
                        text_path: None,
                        char_count: 0,
                        truncated: false,
                        warnings: vec![],
                        error: Some(format!("{e:#}")),
                    },
                }
            } else {
                read_to_result(path, &read_opts)
            };
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
                seen.insert(e.path.clone());
            }
            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let prev: FileResult = serde_json::from_str(&line)?;
                if seen.insert(prev.path.clone()) {
                    all_entries.push(prev);
                }
            }
        }
    }

    let processed = all_entries
        .iter()
        .filter(|e| e.status == ProcessStatus::Ok)
        .count();
    let failed = all_entries
        .iter()
        .filter(|e| e.status == ProcessStatus::Error)
        .count();
    let skipped = all_entries
        .iter()
        .filter(|e| e.status == ProcessStatus::Skipped)
        .count();

    let catalog = Catalog {
        version: 1,
        out_dir: opts.out_dir.display().to_string(),
        total: all_entries.len(),
        processed,
        failed,
        skipped,
        entries: all_entries.clone(),
    };
    write_catalog(&opts.out_dir, &catalog)?;
    write_index(&opts.out_dir, &all_entries)?;

    let sample: Vec<CatalogSummaryEntry> = all_entries
        .iter()
        .take(8)
        .map(|e| CatalogSummaryEntry {
            path: e.path.clone(),
            status: e.status,
            content_class: e.content_class,
            page_count: e.page_count,
        })
        .collect();

    Ok(CatalogSummary {
        version: 1,
        total: catalog.total,
        processed,
        failed,
        skipped,
        out_dir: catalog.out_dir,
        sample,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_jobs_auto_caps_at_eight() {
        with_pdf_batch_jobs_env(|| {
            let cpus = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            let expected = std::cmp::min(8, cpus).max(1);
            assert_eq!(resolve_jobs(JOBS_AUTO, &[]), expected);
        });
    }

    static PDF_BATCH_JOBS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_pdf_batch_jobs_env<F: FnOnce()>(run: F) {
        let _guard = PDF_BATCH_JOBS_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let key = "PDF_BATCH_JOBS";
        let prev = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        run();
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn resolve_jobs_explicit_and_minimum_one() {
        with_pdf_batch_jobs_env(|| {
            assert_eq!(resolve_jobs("4", &[]), 4);
            assert_eq!(resolve_jobs("0", &[]), 1);
        });
    }

    #[test]
    fn resolve_jobs_slow_fs_hint() {
        with_pdf_batch_jobs_env(|| {
            let slow = vec![PathBuf::from("/Volumes/Share/doc.pdf")];
            let auto = resolve_jobs(JOBS_AUTO, &[]);
            let capped = resolve_jobs(JOBS_AUTO, &slow);
            assert!(capped <= auto);
            assert!(capped <= 2);
        });
    }

    #[test]
    fn resolve_jobs_env_override() {
        with_pdf_batch_jobs_env(|| {
            unsafe {
                std::env::set_var("PDF_BATCH_JOBS", "3");
            }
            assert_eq!(resolve_jobs(JOBS_AUTO, &[]), 3);
        });
    }
}
