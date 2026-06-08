use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Docx,
    Xlsx,
    Unsupported,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClass {
    Text,
    Empty,
    Mixed,
    Error,
}

impl ContentClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Empty => "empty",
            Self::Mixed => "mixed",
            Self::Error => "error",
        }
    }
}

pub fn classify_text(char_count: usize) -> ContentClass {
    if char_count == 0 {
        ContentClass::Empty
    } else if char_count >= 80 {
        ContentClass::Text
    } else {
        ContentClass::Mixed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: String,
    pub sha256: String,
    pub file_kind: FileKind,
    pub status: ProcessStatus,
    pub content_class: ContentClass,
    /// Sheet count for xlsx; 0 for docx.
    pub unit_count: u32,
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
    pub file_kind: FileKind,
    pub status: ProcessStatus,
    pub content_class: ContentClass,
    pub unit_count: u32,
}
