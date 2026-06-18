//! Common utility functions shared across tool crates.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Compute SHA-256 hex digest of a file.
pub fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// Expand `~/` prefix to `$HOME`.
pub fn expand_path(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(input)
}

/// Case-insensitive extension check.
pub fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

/// Parse a range spec like "1-5", "3", "1,3,7-10", "all" into sorted unique 1-based indices.
pub fn parse_range(spec: &str, total: u64) -> Result<Vec<u64>> {
    if spec == "all" || spec.is_empty() {
        return Ok((1..=total).collect());
    }
    let mut indices = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: u64 = start
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid range start: {start}"))?;
            let end: u64 = end
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid range end: {end}"))?;
            if start == 0 || end == 0 || start > end || end > total {
                return Err(anyhow::anyhow!(
                    "Range {start}-{end} out of bounds (total: {total})"
                ));
            }
            indices.extend(start..=end);
        } else {
            let idx: u64 = part
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number: {part}"))?;
            if idx == 0 || idx > total {
                return Err(anyhow::anyhow!(
                    "Number {idx} out of bounds (total: {total})"
                ));
            }
            indices.push(idx);
        }
    }
    indices.sort();
    indices.dedup();
    if indices.is_empty() {
        return Err(anyhow::anyhow!("No valid indices specified"));
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_range_all() {
        let result = parse_range("all", 10).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_parse_range_empty_returns_all() {
        let result = parse_range("", 10).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_parse_range_single() {
        let result = parse_range("3", 10).unwrap();
        assert_eq!(result, vec![3]);
    }

    #[test]
    fn test_parse_range_range() {
        let result = parse_range("2-5", 10).unwrap();
        assert_eq!(result, vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_range_comma_separated() {
        let result = parse_range("1,3,5", 10).unwrap();
        assert_eq!(result, vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_range_mixed() {
        let result = parse_range("1,3-5,9", 10).unwrap();
        assert_eq!(result, vec![1, 3, 4, 5, 9]);
    }

    #[test]
    fn test_parse_range_dedup_and_sort() {
        let result = parse_range("5,3,5,1", 10).unwrap();
        assert_eq!(result, vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_range_out_of_bounds() {
        assert!(parse_range("0", 10).is_err());
        assert!(parse_range("11", 10).is_err());
        assert!(parse_range("1-11", 10).is_err());
        assert!(parse_range("6-3", 10).is_err());
    }

    #[test]
    fn test_parse_range_invalid_format() {
        assert!(parse_range("abc", 10).is_err());
        assert!(parse_range("1-a", 10).is_err());
    }

    #[test]
    fn test_has_extension() {
        assert!(has_extension(Path::new("foo.txt"), "txt"));
        assert!(has_extension(Path::new("foo.TXT"), "txt"));
        assert!(has_extension(Path::new("foo.Txt"), "TXT"));
        assert!(!has_extension(Path::new("foo.pdf"), "txt"));
        assert!(!has_extension(Path::new("foo"), "txt"));
        assert!(has_extension(Path::new("foo.docx"), "DOCX"));
    }

    #[test]
    fn test_expand_path_relative_stays() {
        assert_eq!(expand_path("relative/path"), Path::new("relative/path"));
    }

    #[test]
    fn test_expand_path_absolute_stays() {
        assert_eq!(expand_path("/absolute/path"), Path::new("/absolute/path"));
    }

    #[test]
    fn test_expand_path_tilde() {
        if let Ok(home) = std::env::var("HOME") {
            let expanded = expand_path("~/documents");
            assert!(expanded.starts_with(&home));
            assert!(expanded.ends_with("documents"));
        }
    }
}
