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
