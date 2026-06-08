use serde::{Deserialize, Serialize};

/// Downstream gate label for extracted PDF content (T1–T8 artifact protocol).
///
/// Produced by [`crate::read::classify_content`] on full extract, or by
/// [`crate::read::shallow_scan_classify`] when batch `--skip-scanned` is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClass {
    /// Substantial extractable text (≥80 chars total or ≥80 chars/page on average).
    Text,
    /// No extractable text in the sampled/full window — typical scanned or image-only PDF.
    Scanned,
    /// Zero pages or otherwise empty document shell.
    Empty,
    /// Some text, but below density thresholds — treat like short OCR noise or sparse headers.
    Mixed,
    /// Load or extraction failed (`extraction_error` warning on full read).
    Error,
}

impl ContentClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Scanned => "scanned",
            Self::Empty => "empty",
            Self::Mixed => "mixed",
            Self::Error => "error",
        }
    }
}

/// Per-file extraction result (jsonl row + catalog entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: String,
    pub sha256: String,
    pub status: ProcessStatus,
    pub content_class: ContentClass,
    pub page_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_path: Option<String>,
    pub char_count: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Ok,
    Error,
    Skipped,
}

/// Batch catalog written to `catalog.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    pub out_dir: String,
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub entries: Vec<FileResult>,
}

/// Resume checkpoint persisted as `checkpoint.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

/// Compact stdout summary (target < 4 KB).
#[derive(Debug, Clone, Serialize)]
pub struct CatalogSummary {
    pub version: u32,
    pub total: usize,
    pub processed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub out_dir: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sample: Vec<CatalogSummaryEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogSummaryEntry {
    pub path: String,
    pub status: ProcessStatus,
    pub content_class: ContentClass,
    pub page_count: u32,
}
