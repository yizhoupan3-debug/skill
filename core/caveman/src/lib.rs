pub mod compress;
pub mod validate;
pub mod cli;

use serde::{Deserialize, Serialize};

/// Caveman compression intensity levels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CavemanMode {
    Lite,
    Full,
    Ultra,
    WenyanLite,
    WenyanFull,
    WenyanUltra,
}

impl Default for CavemanMode {
    fn default() -> Self {
        CavemanMode::Full
    }
}

/// Result of a compression operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompressResult {
    pub original_size: usize,
    pub compressed_size: usize,
    pub backup_path: Option<String>,
    pub token_estimate_saved: u64,
    pub valid: bool,
}

impl CompressResult {
    pub fn savings_percent(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        ((self.original_size - self.compressed_size) as f64 / self.original_size as f64) * 100.0
    }
}

/// Validation outcome after compression.
#[derive(Debug)]
pub struct ValidationResult {
    pub code_blocks_match: bool,
    pub urls_preserved: bool,
    pub file_paths_preserved: bool,
    pub heading_structure_preserved: bool,
    pub inline_code_preserved: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.code_blocks_match
            && self.urls_preserved
            && self.file_paths_preserved
            && self.heading_structure_preserved
            && self.inline_code_preserved
            && self.errors.is_empty()
    }
}

/// Rough token estimate (characters / 4, per common approximation).
pub fn estimate_tokens(text: &str) -> u64 {
    (text.len() / 4) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 1);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_savings_percent() {
        let r = CompressResult {
            original_size: 100,
            compressed_size: 40,
            backup_path: None,
            token_estimate_saved: 15,
            valid: true,
        };
        assert!((r.savings_percent() - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_caveman_mode_default() {
        assert_eq!(CavemanMode::default(), CavemanMode::Full);
    }
}
