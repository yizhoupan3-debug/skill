//! PDF-specific batch schema types.
pub use batch_common::schema::{classify_text, CommonFileResult, ContentClass, ProcessStatus};
use batch_common::engine::BatchResult;
use serde::{Deserialize, Serialize};
use std::io::{Result as IoResult, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    #[serde(flatten)]
    pub common: CommonFileResult,
    pub page_count: u32,
}

impl BatchResult for FileResult {
    fn path(&self) -> &str { &self.common.path }
    fn status(&self) -> ProcessStatus { self.common.status }
    fn write_index_row(&self, writer: &mut dyn Write) -> IoResult<()> {
        let text_ref = self.common.text_path.as_deref().unwrap_or("-");
        writeln!(writer, "| {} | {:?} | {} | {} | {} |",
            self.common.path, self.common.status, self.common.content_class.as_str(), self.page_count, text_ref)
    }
    fn to_summary_entry(&self) -> serde_json::Value {
        serde_json::json!({"path": self.common.path, "status": self.common.status, "content_class": self.common.content_class, "page_count": self.page_count})
    }
}
