//! Common utility functions shared across tool crates.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Compute SHA-256 hex digest of a file.
pub fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// Expand `~/` prefix to `$HOME`.
pub fn expand_path(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return Path::new(&home).join(rest);
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

/// Truncate a string to at most `max_chars` Unicode characters.
///
/// Returns the (possibly truncated) string and whether truncation occurred.
/// If the input exceeds `max_chars`, it is sliced at the character boundary
/// and `"…"` is appended (1 char), so the result is at most `max_chars` chars.
pub fn truncate_text(text: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), !text.is_empty());
    }
    let char_count = text.chars().count();
    if char_count <= max_chars {
        (text.to_string(), false)
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        (truncated + "…", true)
    }
}

/// Run a command to completion, returning error with stderr on failure.
pub fn run_command(command: &mut Command) -> Result<()> {
    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() { stderr } else { stdout };
        if message.is_empty() {
            bail!("command failed with status {:?}", output.status.code());
        }
        bail!("{message}");
    }
    Ok(())
}

/// Run a command and capture its stdout as a string.
pub fn run_command_capture(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Build a `soffice --convert-to` Command with shared boilerplate.
///
/// The returned `Command` has its stdout/stderr piped. Callers may
/// redirect them (e.g. to `Stdio::null()`) before spawning.
pub fn soffice_convert_cmd(profile_dir: &Path, fmt: &str, outdir: &Path, input: &Path) -> Command {
    let mut cmd = Command::new("soffice");
    cmd.arg(format!(
        "-env:UserInstallation=file://{}",
        profile_dir.display()
    ))
    .arg("--invisible")
    .arg("--headless")
    .arg("--norestore")
    .arg("--convert-to")
    .arg(fmt)
    .arg("--outdir")
    .arg(outdir)
    .arg(input)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    cmd
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

    #[test]
    fn test_truncate_text_short() {
        let (text, truncated) = truncate_text("hello", 10);
        assert_eq!(text, "hello");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_text_exact() {
        let (text, truncated) = truncate_text("hello", 5);
        assert_eq!(text, "hello");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_text_long() {
        let (text, truncated) = truncate_text("hello world", 5);
        assert_eq!(text, "hell…");
        assert!(truncated);
    }

    #[test]
    fn test_truncate_text_unicode() {
        let (text, truncated) = truncate_text("你好世界test", 4);
        assert_eq!(text, "你好世…");
        assert!(truncated);
    }
}
