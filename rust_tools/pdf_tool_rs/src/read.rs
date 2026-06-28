use crate::schema::{ContentClass, FileResult, ProcessStatus};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub use mcp_stdio_common::util::file_sha256;

pub struct ReadOptions {
    pub max_chars: usize,
    pub text_out_dir: Option<PathBuf>,
}

pub struct ReadOutput {
    pub text: String,
    pub page_count: u32,
    pub content_class: ContentClass,
    pub warnings: Vec<String>,
    pub file_sha256: String,
    pub text_path: Option<String>,
    pub truncated: bool,
}

pub fn page_count(path: &Path) -> Result<u32> {
    let doc =
        lopdf::Document::load(path).with_context(|| format!("load pdf {}", path.display()))?;
    Ok(doc.get_pages().len() as u32)
}

pub fn extract_text(path: &Path) -> Result<String> {
    pdf_extract::extract_text(path).with_context(|| format!("extract text from {}", path.display()))
}

/// Sample text from the first `min(SHALLOW_SAMPLE_PAGES, page_count)` pages only.
pub fn extract_text_sample(path: &Path, page_count: u32) -> Result<String> {
    if page_count == 0 {
        return Ok(String::new());
    }
    let sample_pages = page_count.min(SHALLOW_SAMPLE_PAGES) as usize;
    let pages = pdf_extract::extract_text_by_pages(path)
        .with_context(|| format!("shallow extract from {}", path.display()))?;
    let joined: String = pages
        .into_iter()
        .take(sample_pages)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(joined)
}

/// Shallow scan for batch [`--skip-scanned`](crate::batch::BatchOptions::skip_scanned).
///
/// Reads page count via `lopdf`, samples text from the first [`SHALLOW_SAMPLE_PAGES`] pages
/// only (no `pdfinfo` subprocess). Returns `(page_count, content_class, should_skip)` where
/// `should_skip` is true when the probe finds no text (`Scanned` / `Empty`) so batch can
/// emit `status: skipped` + `warnings: ["skip_scanned"]` without a full-document extract.
pub fn shallow_scan_classify(path: &Path) -> Result<(u32, ContentClass, bool)> {
    let pages = page_count(path)?;
    if pages == 0 {
        return Ok((0, ContentClass::Empty, true));
    }
    let sample = extract_text_sample(path, pages).unwrap_or_default();
    let trimmed = sample.trim();
    if trimmed.is_empty() {
        return Ok((pages, ContentClass::Scanned, true));
    }
    let sample_pages = pages.min(SHALLOW_SAMPLE_PAGES).max(1);
    let class = classify_content(trimmed, sample_pages);
    Ok((pages, class, false))
}

/// Pages sampled for `--skip-scanned` shallow probe (no full-document extract).
pub const SHALLOW_SAMPLE_PAGES: u32 = 3;

/// Density threshold (chars/page) mirrored by [`classify_content`] for full reads.
pub const SHALLOW_CHARS_PER_PAGE_EPS: f64 = 80.0;

/// Classify already-extracted text for `content_class` on `pdf read` / batch full extract.
///
/// | Condition | `ContentClass` |
/// | --- | --- |
/// | `page_count == 0` | `Empty` |
/// | trimmed text empty | `Scanned` |
/// | ≥80 total chars **or** ≥80 chars/page | `Text` |
/// | otherwise | `Mixed` |
pub fn classify_content(text: &str, page_count: u32) -> ContentClass {
    if page_count == 0 {
        return ContentClass::Empty;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ContentClass::Scanned;
    }
    let total_chars = trimmed.chars().count();
    let chars_per_page = total_chars as f64 / page_count.max(1) as f64;
    // Non-empty extraction ⇒ not scanned (short 1-page docs are common).
    if total_chars >= 80 || chars_per_page >= 80.0 {
        ContentClass::Text
    } else {
        ContentClass::Mixed
    }
}

pub fn read_pdf(path: &Path, opts: &ReadOptions) -> Result<ReadOutput> {
    let mut warnings = Vec::new();
    let file_sha256 = file_sha256(path)?;
    let page_count = page_count(path)?;

    let raw_text = match extract_text(path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!("extraction_error: {e:#}"));
            String::new()
        }
    };

    let content_class = if warnings.iter().any(|w| w.starts_with("extraction_error")) {
        ContentClass::Error
    } else {
        classify_content(&raw_text, page_count)
    };

    let normalized = raw_text.replace('\r', "");
    let char_count = normalized.chars().count();
    let truncated = char_count > opts.max_chars;
    let text: String = normalized.chars().take(opts.max_chars).collect();
    if truncated {
        warnings.push("text_truncated".to_string());
    }

    let text_path = if let Some(out_dir) = &opts.text_out_dir {
        let rel = format!("text/{file_sha256}.txt");
        let full = out_dir.join(&rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full, &text)?;
        Some(rel)
    } else {
        None
    };

    Ok(ReadOutput {
        text,
        page_count,
        content_class,
        warnings,
        file_sha256,
        text_path,
        truncated,
    })
}

pub fn read_to_result(path: &Path, opts: &ReadOptions) -> FileResult {
    let path_str = path.display().to_string();
    match read_pdf(path, opts) {
        Ok(out) => FileResult {
            common: batch_common::schema::CommonFileResult {
                path: path_str,
                sha256: out.file_sha256,
                status: ProcessStatus::Ok,
                content_class: out.content_class,
                text_path: out.text_path,
                char_count: out.text.chars().count(),
                truncated: out.truncated,
                warnings: out.warnings,
                error: None,
            },
            page_count: out.page_count,
        },
        Err(e) => FileResult {
            common: batch_common::schema::CommonFileResult {
                path: path_str,
                sha256: String::new(),
                status: ProcessStatus::Error,
                content_class: ContentClass::Error,
                text_path: None,
                char_count: 0,
                truncated: false,
                warnings: vec![],
                error: Some(format!("{e:#}")),
            },
            page_count: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_empty_pages() {
        assert_eq!(classify_content("", 0), ContentClass::Empty);
    }

    #[test]
    fn classify_scanned() {
        assert_eq!(classify_content("  ", 3), ContentClass::Scanned);
    }

    #[test]
    fn classify_text() {
        let text = "a".repeat(500);
        assert_eq!(classify_content(&text, 2), ContentClass::Text);
    }

    #[test]
    fn shallow_sample_pages_constant() {
        assert_eq!(SHALLOW_SAMPLE_PAGES, 3);
    }
}
