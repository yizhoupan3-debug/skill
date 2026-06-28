use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

use crate::read::{classify_content, extract_text, file_sha256, page_count};

#[derive(Debug, Serialize)]
pub struct PdfInfo {
    pub path: String,
    pub sha256: String,
    pub page_count: u32,
    pub file_size_bytes: u64,
    pub content_class: String,
    pub text_preview_chars: usize,
    pub warnings: Vec<String>,
}

pub fn pdf_info(path: &Path, preview_chars: usize) -> Result<PdfInfo> {
    let path_str = path.display().to_string();
    let sha256 = file_sha256(path)?;
    let pages = page_count(path)?;
    let file_size_bytes = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .len();

    let mut warnings = Vec::new();
    let text = match extract_text(path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!("extraction_error: {e:#}"));
            String::new()
        }
    };
    let class = classify_content(&text, pages);

    Ok(PdfInfo {
        path: path_str,
        sha256,
        page_count: pages,
        file_size_bytes,
        content_class: class.as_str().to_string(),
        text_preview_chars: text.chars().take(preview_chars).count(),
        warnings,
    })
}
