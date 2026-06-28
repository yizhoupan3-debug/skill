//! OOXML-specific batch schema types.
use batch_common::engine::BatchResult;
pub use batch_common::schema::{CommonFileResult, ContentClass, ProcessStatus, classify_text};
use serde::{Deserialize, Serialize};
use std::io::{Result as IoResult, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Docx,
    Xlsx,
    Pptx,
    Unsupported,
}
impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    #[serde(flatten)]
    pub common: CommonFileResult,
    pub file_kind: FileKind,
    pub unit_count: u32,
}

impl BatchResult for FileResult {
    fn path(&self) -> &str {
        &self.common.path
    }
    fn status(&self) -> ProcessStatus {
        self.common.status
    }
    fn write_index_row(&self, writer: &mut dyn Write) -> IoResult<()> {
        let text_ref = self.common.text_path.as_deref().unwrap_or("-");
        writeln!(
            writer,
            "| {} | {} | {:?} | {} | {} | {} |",
            self.common.path,
            self.file_kind.as_str(),
            self.common.status,
            self.common.content_class.as_str(),
            self.unit_count,
            text_ref
        )
    }
    fn to_summary_entry(&self) -> serde_json::Value {
        serde_json::json!({"path": self.common.path, "file_kind": self.file_kind, "status": self.common.status, "content_class": self.common.content_class, "unit_count": self.unit_count})
    }
}
