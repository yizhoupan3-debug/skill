//! OOXML-specific batch schema types.
pub use batch_common::schema::{classify_text, ContentClass, ProcessStatus};
use batch_common::engine::BatchResult;
use serde::{Deserialize, Serialize};
use std::io::{Result as IoResult, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind { Docx, Xlsx, Pptx, Unsupported }
impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self { Self::Docx => "docx", Self::Xlsx => "xlsx", Self::Pptx => "pptx", Self::Unsupported => "unsupported" }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: String, pub sha256: String, pub file_kind: FileKind, pub status: ProcessStatus,
    pub content_class: ContentClass, pub unit_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")] pub text_path: Option<String>,
    pub char_count: usize, pub truncated: bool, pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<String>,
}

impl BatchResult for FileResult {
    fn path(&self) -> &str { &self.path }
    fn status(&self) -> ProcessStatus { self.status }
    fn write_index_row(&self, writer: &mut dyn Write) -> IoResult<()> {
        let text_ref = self.text_path.as_deref().unwrap_or("-");
        writeln!(writer, "| {} | {} | {:?} | {} | {} | {} |",
            self.path, self.file_kind.as_str(), self.status, self.content_class.as_str(), self.unit_count, text_ref)
    }
    fn to_summary_entry(&self) -> serde_json::Value {
        serde_json::json!({"path": self.path, "file_kind": self.file_kind, "status": self.status, "content_class": self.content_class, "unit_count": self.unit_count})
    }
}
