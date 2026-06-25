//! OOXML batch processing — delegates to batch_common::engine for shared logic.
use crate::schema::{classify_text, FileKind, FileResult, ProcessStatus};
use crate::{detect_ooxml_kind, docx_read_text_string, OoxmlKind};
use crate::{read_docx_content, read_xlsx_content, read_pptx_content, pptx_read_text_string, xlsx_read_text_string};
use batch_common::engine;
pub use batch_common::engine::{BatchOptions, JOBS_AUTO};
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_jobs() -> usize {
    resolve_jobs(JOBS_AUTO, &[])
}

/// Resolve batch parallelism: `OOXML_BATCH_JOBS` env > explicit `--jobs N` > `auto`.
pub fn resolve_jobs(jobs_arg: &str, hint_paths: &[PathBuf]) -> usize {
    engine::resolve_jobs("OOXML", jobs_arg, hint_paths)
}

pub fn load_paths(manifest: Option<&Path>, stdin_paths: bool) -> anyhow::Result<Vec<PathBuf>> {
    engine::load_paths(manifest, stdin_paths)
}

fn read_to_result(path: &Path, opts: &BatchOptions, max_rows: usize, text_out_dir: Option<&PathBuf>) -> FileResult {
    let path_str = path.display().to_string();
    let kind = detect_ooxml_kind(path);
    let file_kind = match kind {
        OoxmlKind::Docx => FileKind::Docx,
        OoxmlKind::Xlsx => FileKind::Xlsx,
        OoxmlKind::Pptx => FileKind::Pptx,
        OoxmlKind::Unsupported => {
            return FileResult {
                common: batch_common::schema::CommonFileResult {
                    path: path_str,
                    sha256: mcp_stdio_common::util::file_sha256(path).unwrap_or_default(),
                    status: ProcessStatus::Error,
                    content_class: crate::schema::ContentClass::Error,
                    text_path: None,
                    char_count: 0,
                    truncated: false,
                    warnings: vec![],
                    error: Some(format!(
                        "unsupported extension: batch accepts .docx, .xlsx, and .pptx only"
                    )),
                },
                file_kind: FileKind::Unsupported,
                unit_count: 0,
            };
        }
    };

    let extract_result = match kind {
        OoxmlKind::Docx => read_docx_content(path).map(|out| {
            let unit_count = 0u32;
            (docx_read_text_string(&out), unit_count, Vec::new())
        }),
        OoxmlKind::Xlsx => read_xlsx_content(path, max_rows, &[]).map(|out| {
            let unit_count = out.sheets.len() as u32;
            let mut warnings = Vec::new();
            if out.sheets.iter().any(|s| s.truncated) {
                warnings.push("rows_truncated".to_string());
            }
            (xlsx_read_text_string(&out), unit_count, warnings)
        }),
        OoxmlKind::Pptx => read_pptx_content(path).map(|out| {
            let text = pptx_read_text_string(&out);
            let unit_count = out.slide_count as u32;
            (text, unit_count, Vec::new())
        }),
        OoxmlKind::Unsupported => unreachable!(),
    };

    match extract_result {
        Ok((raw_text, unit_count, mut warnings)) => {
            let sha = mcp_stdio_common::util::file_sha256(path).unwrap_or_default();
            let (text, truncated) = mcp_stdio_common::util::truncate_text(&raw_text, opts.max_chars);
            if truncated { warnings.push("text_truncated".to_string()); }
            let char_count = text.chars().count();
            let content_class = classify_text(char_count);

            let text_path = if let Some(out_dir) = text_out_dir {
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
                common: batch_common::schema::CommonFileResult {
                    path: path_str,
                    sha256: sha,
                    status: ProcessStatus::Ok,
                    content_class,
                    text_path,
                    char_count,
                    truncated,
                    warnings,
                    error: None,
                },
                file_kind,
                unit_count,
            }
        }
        Err(e) => FileResult {
            common: batch_common::schema::CommonFileResult {
                path: path_str,
                sha256: String::new(),
                status: ProcessStatus::Error,
                content_class: crate::schema::ContentClass::Error,
                text_path: None,
                char_count: 0,
                truncated: false,
                warnings: vec![],
                error: Some(format!("{e:#}")),
            },
            file_kind,
            unit_count: 0,
        },
    }
}

/// Run a OOXML batch job. `max_rows` controls the row limit for XLSX sheets;
/// the other options come from `BatchOptions`.
pub fn run_batch(
    paths: Vec<PathBuf>,
    opts: &BatchOptions,
    max_rows: usize,
) -> anyhow::Result<engine::CatalogSummary> {
    let index_header = "# OOXML batch index\n\n| path | kind | status | class | units | text |\n| --- | --- | --- | --- | ---: | --- |\n";
    let out_dir = opts.out_dir.clone();
    engine::run_batch(paths, opts, index_header, |path| {
        read_to_result(path, opts, max_rows, Some(&out_dir))
    })
}

pub fn print_catalog_summary(summary: &engine::CatalogSummary) -> anyhow::Result<()> {
    engine::print_catalog_summary(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_text_thresholds() {
        assert_eq!(classify_text(0), crate::schema::ContentClass::Empty);
        assert_eq!(classify_text(10), crate::schema::ContentClass::Mixed);
        assert_eq!(classify_text(80), crate::schema::ContentClass::Text);
    }
}
