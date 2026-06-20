//! PDF batch processing — delegates to batch_common::engine for shared logic.
use crate::read::{read_to_result, shallow_scan_classify, ReadOptions};
use crate::schema::{ContentClass, FileResult, ProcessStatus};
use batch_common::engine::{self, CatalogSummary, JOBS_AUTO};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub use batch_common::engine::{load_paths, print_catalog_summary, BatchOptions};

pub fn default_jobs() -> usize { resolve_jobs(JOBS_AUTO, &[]) }
pub fn resolve_jobs(jobs_arg: &str, hint_paths: &[PathBuf]) -> usize { engine::resolve_jobs("PDF", jobs_arg, hint_paths) }

fn skipped_scanned_file_result(path: &Path, pages: u32, class: ContentClass) -> FileResult {
    FileResult {
        common: batch_common::schema::CommonFileResult {
            path: path.display().to_string(),
            sha256: crate::read::file_sha256(path).unwrap_or_default(),
            status: ProcessStatus::Skipped, content_class: class,
            text_path: None, char_count: 0, truncated: false,
            warnings: vec!["skip_scanned".to_string()], error: None,
        },
        page_count: pages,
    }
}

/// Run a PDF batch job. The `skip_scanned` option uses a shallow
/// probe to skip image-only PDFs before full extraction.
pub fn run_batch(paths: Vec<PathBuf>, opts: &BatchOptions, skip_scanned: bool) -> Result<CatalogSummary> {
    let index_header = "# PDF batch index\n\n| path | status | class | pages | text |\n| --- | --- | --- | ---: | --- |\n";
    engine::run_batch(paths, opts, index_header, |path| {
        let read_opts = ReadOptions { max_chars: opts.max_chars, text_out_dir: Some(opts.out_dir.clone()) };
        if skip_scanned {
            match shallow_scan_classify(path) {
                Ok((pages, class, true)) => skipped_scanned_file_result(path, pages, class),
                Ok(_) => read_to_result(path, &read_opts),
                Err(e) => FileResult { common: batch_common::schema::CommonFileResult {
                    path: path.display().to_string(), sha256: String::new(),
                    status: ProcessStatus::Error, content_class: ContentClass::Error,
                    text_path: None, char_count: 0, truncated: false,
                    warnings: vec![], error: Some(format!("{e:#}")) },
                    page_count: 0 },
            }
        } else { read_to_result(path, &read_opts) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    static PDF_BATCH_JOBS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn with_pdf_batch_jobs_env<F: FnOnce()>(run: F) {
        let _guard = PDF_BATCH_JOBS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = "PDF_BATCH_JOBS";
        let prev = std::env::var(key).ok();
        unsafe { std::env::remove_var(key); }
        run();
        unsafe { match prev { Some(v) => std::env::set_var(key, v), None => std::env::remove_var(key) } }
    }
    #[test] fn resolve_jobs_auto_caps_at_eight() { with_pdf_batch_jobs_env(|| { let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4); assert_eq!(resolve_jobs(JOBS_AUTO, &[]), std::cmp::min(8, cpus).max(1)); }); }
    #[test] fn resolve_jobs_explicit_and_minimum_one() { with_pdf_batch_jobs_env(|| { assert_eq!(resolve_jobs("4", &[]), 4); assert_eq!(resolve_jobs("0", &[]), 1); }); }
    #[test] fn resolve_jobs_slow_fs_hint() { with_pdf_batch_jobs_env(|| { let slow = vec![PathBuf::from("/Volumes/Share/doc.pdf")]; let auto = resolve_jobs(JOBS_AUTO, &[]); let capped = resolve_jobs(JOBS_AUTO, &slow); assert!(capped <= auto); assert!(capped <= 2); }); }
    #[test] fn resolve_jobs_env_override() { with_pdf_batch_jobs_env(|| { unsafe { std::env::set_var("PDF_BATCH_JOBS", "3"); } assert_eq!(resolve_jobs(JOBS_AUTO, &[]), 3); }); }
}
