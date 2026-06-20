//! Shared schema types for batch processing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Ok,
    Error,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClass {
    Text,
    Scanned,
    Empty,
    Mixed,
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

/// Classify text by character count (shared threshold: 80 chars).
pub fn classify_text(char_count: usize) -> ContentClass {
    if char_count == 0 {
        ContentClass::Empty
    } else if char_count >= 80 {
        ContentClass::Text
    } else {
        ContentClass::Mixed
    }
}

/// Common fields shared by all batch `FileResult` types (PDF, OOXML, etc.).
///
/// Tool-specific crates extend this via `#[serde(flatten)]` to add
/// domain-specific fields (e.g. `page_count`, `file_kind`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonFileResult {
    pub path: String,
    pub sha256: String,
    pub status: ProcessStatus,
    pub content_class: ContentClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_path: Option<String>,
    pub char_count: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_status_ok_eq() {
        assert_eq!(ProcessStatus::Ok, ProcessStatus::Ok);
        assert_ne!(ProcessStatus::Ok, ProcessStatus::Error);
        assert_ne!(ProcessStatus::Ok, ProcessStatus::Skipped);
    }

    #[test]
    fn test_content_class_as_str() {
        assert_eq!(ContentClass::Text.as_str(), "text");
        assert_eq!(ContentClass::Scanned.as_str(), "scanned");
        assert_eq!(ContentClass::Empty.as_str(), "empty");
        assert_eq!(ContentClass::Mixed.as_str(), "mixed");
        assert_eq!(ContentClass::Error.as_str(), "error");
    }

    #[test]
    fn test_classify_text_thresholds() {
        assert_eq!(classify_text(0), ContentClass::Empty);
        assert_eq!(classify_text(10), ContentClass::Mixed);
        assert_eq!(classify_text(79), ContentClass::Mixed);
        assert_eq!(classify_text(80), ContentClass::Text);
        assert_eq!(classify_text(100_000), ContentClass::Text);
    }

    #[test]
    fn test_process_status_serialize_roundtrip() {
        let json = serde_json::to_string(&ProcessStatus::Ok).unwrap();
        assert_eq!(json, "\"ok\"");
        let deserialized: ProcessStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProcessStatus::Ok);

        let json = serde_json::to_string(&ProcessStatus::Error).unwrap();
        assert_eq!(json, "\"error\"");
        let deserialized: ProcessStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProcessStatus::Error);
    }
}
