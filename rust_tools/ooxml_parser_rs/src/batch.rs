use crate::schema::{
    classify_text, Catalog, CatalogSummary, CatalogSummaryEntry, Checkpoint, ContentClass, FileKind,
    FileResult, ProcessStatus,
};
use crate::{
    docx_read_text_string, file_sha256, read_docx_content, read_xlsx_content, xlsx_read_text_string,
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
    pub fail_fast: bool,
    pub max_chars: usize,
    pub max_rows: usize,
}

pub const JOBS_AUTO: &str = "auto";

pub fn resolve_jobs(jobs_arg: &str, hint_paths: &[PathBuf]) -> usize {
    if let Ok(raw) = std::env::var("OOXML_BATCH_JOBS") {
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
    if std::env::var("OOXML_BATCH_SLOW_FS").is_ok_and(|v| !v.is_empty() && v != "0") {
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

fn detect_file_kind(path: &Path) -> FileKind {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| match ext.as_str() {
            "docx" => FileKind::Docx,
            "xlsx" => FileKind::Xlsx,
            _ => FileKind::Unsupported,
        })
        .unwrap_or(FileKind::Unsupported)
}

struct ReadOpts {
    max_chars: usize,
    max_rows: usize,
    text_out_dir: Option<PathBuf>,
}

fn truncate_text(text: &str, max_chars: usize) -> (String, bool, Vec<String>) {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return (text.to_string(), false, Vec::new());
    }
    let truncated: String = text.chars().take(max_chars).collect();
    (truncated, true, vec!["text_truncated".to_string()])
}

fn read_to_result(path: &Path, opts: &ReadOpts) -> FileResult {
    let path_str = path.display().to_string();
    let kind = detect_file_kind(path);

    if kind == FileKind::Unsupported {
        return FileResult {
            path: path_str,
            sha256: file_sha256(path).unwrap_or_default(),
            file_kind: kind,
            status: ProcessStatus::Error,
            content_class: ContentClass::Error,
            unit_count: 0,
            text_path: None,
            char_count: 0,
            truncated: false,
            warnings: vec![],
            error: Some(
                "unsupported extension: batch accepts .docx and .xlsx only (use ppt read-full for .pptx)"
                    .to_string(),
            ),
        };
    }

    let extract = match kind {
        FileKind::Docx => read_docx_content(path).map(|out| {
            let unit_count = 0u32;
            (docx_read_text_string(&out), unit_count, Vec::new())
        }),
        FileKind::Xlsx => read_xlsx_content(path, opts.max_rows, &[]).map(|out| {
            let unit_count = out.sheets.len() as u32;
            let mut warnings = Vec::new();
            if out.sheets.iter().any(|s| s.truncated) {
                warnings.push("rows_truncated".to_string());
            }
            (xlsx_read_text_string(&out), unit_count, warnings)
        }),
        FileKind::Unsupported => unreachable!(),
    };

    match extract {
        Ok((raw_text, unit_count, mut warnings)) => {
            let sha = file_sha256(path).unwrap_or_default();
            let (text, truncated, mut trunc_warnings) = truncate_text(&raw_text, opts.max_chars);
            warnings.append(&mut trunc_warnings);
            let char_count = text.chars().count();
            let content_class = classify_text(char_count);

            let text_path = if let Some(out_dir) = &opts.text_out_dir {
                let rel = format!("text/{sha}.txt");
                let full = out_dir.join(&rel);
                if let Some(parent) = full.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::write(&full, &text).is_ok() {
                    Some(rel)
                } else {
                    warnings.push("text_write_failed".to_string());
                    None
                }
            } else {
                None
            };

            FileResult {
                path: path_str,
                sha256: sha,
                file_kind: kind,
                status: ProcessStatus::Ok,
                content_class,
                unit_count,
                text_path,
                char_count,
                truncated,
                warnings,
                error: None,
            }
        }
        Err(e) => FileResult {
            path: path_str,
            sha256: String::new(),
            file_kind: kind,
            status: ProcessStatus::Error,
            content_class: ContentClass::Error,
            unit_count: 0,
            text_path: None,
            char_count: 0,
            truncated: false,
            warnings: vec![],
            error: Some(format!("{e:#}")),
        },
    }
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
    writeln!(file, "# OOXML batch index\n")?;
    writeln!(file, "| path | kind | status | class | units | text |")?;
    writeln!(file, "| --- | --- | --- | --- | ---: | --- |")?;
    for e in entries {
        let text_ref = e.text_path.as_deref().unwrap_or("-");
        writeln!(
            file,
            "| {} | {} | {:?} | {} | {} | {} |",
            e.path,
            e.file_kind.as_str(),
            e.status,
            e.content_class.as_str(),
            e.unit_count,
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

    let read_opts = ReadOpts {
        max_chars: opts.max_chars,
        max_rows: opts.max_rows,
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
            let result = read_to_result(path, &read_opts);
            let _ = tx.send((key.clone(), result));
        });
    });
    drop(tx);

    let (_checkpoint, results) = writer
        .join()
        .map_err(|_| anyhow::anyhow!("jsonl writer thread panicked"))??;

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
            file_kind: e.file_kind,
            status: e.status,
            content_class: e.content_class,
            unit_count: e.unit_count,
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
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let expected = std::cmp::min(8, cpus).max(1);
        assert_eq!(resolve_jobs(JOBS_AUTO, &[]), expected);
    }

    #[test]
    fn classify_text_thresholds() {
        assert_eq!(classify_text(0), ContentClass::Empty);
        assert_eq!(classify_text(10), ContentClass::Mixed);
        assert_eq!(classify_text(80), ContentClass::Text);
    }

    #[test]
    fn detect_file_kind_by_extension() {
        assert_eq!(
            detect_file_kind(Path::new("report.docx")),
            FileKind::Docx
        );
        assert_eq!(
            detect_file_kind(Path::new("data.XLSX")),
            FileKind::Xlsx
        );
        assert_eq!(
            detect_file_kind(Path::new("deck.pptx")),
            FileKind::Unsupported
        );
    }
}
