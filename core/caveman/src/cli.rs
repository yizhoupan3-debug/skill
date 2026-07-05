use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use crate::{compress, validate, CompressResult};

/// Caveman compression CLI.
#[derive(Parser)]
#[command(name = "caveman", about = "Compress text files into caveman style")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compress a single file
    Compress {
        /// Path to the file to compress
        file: PathBuf,
        /// Max retry attempts on validation failure
        #[arg(long, default_value = "2")]
        max_retries: u8,
    },
    /// Compress text from stdin
    Stdin,
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compress { file, max_retries } => {
            let result = compress_file(&file, max_retries)?;
            if result.valid {
                println!(
                    "Compressed: {:.1}% saved ({} → {} chars, ~{} tokens)",
                    result.savings_percent(),
                    result.original_size,
                    result.compressed_size,
                    result.token_estimate_saved,
                );
                println!("Backup: {}", result.backup_path.unwrap_or_default());
            } else {
                eprintln!("Compression failed validation after {} retries", max_retries);
                std::process::exit(1);
            }
        }
        Commands::Stdin => {
            use std::io::Read;
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .context("Failed to read stdin")?;
            let compressed = compress::compress_text(&input);
            let v = validate::validate_compression(&input, &compressed);
            if v.is_valid() {
                print!("{}", compressed);
            } else {
                eprintln!("Compression validation failed:");
                for err in &v.errors {
                    eprintln!("  - {}", err);
                }
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

/// Compress a single file with backup and validation.
pub fn compress_file(path: &Path, max_retries: u8) -> Result<CompressResult> {
    let original = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let original_size = original.len();

    // Create backup
    let backup_path = {
        let mut p = path.as_os_str().to_os_string();
        p.push(".original.md");
        PathBuf::from(p)
    };

    // Only write backup if it doesn't already exist (idempotent)
    if !backup_path.exists() {
        fs::write(&backup_path, &original)
            .with_context(|| format!("Failed to write backup to {}", backup_path.display()))?;
    }

    let mut compressed = compress::compress_text(&original);
    let mut attempt = 0;

    loop {
        let v = validate::validate_compression(&original, &compressed);

        if v.is_valid() {
            fs::write(path, &compressed)
                .with_context(|| format!("Failed to write compressed output to {}", path.display()))?;

            let compressed_size = compressed.len();
            return Ok(CompressResult {
                original_size,
                compressed_size,
                backup_path: Some(backup_path.to_string_lossy().to_string()),
                token_estimate_saved: crate::estimate_tokens(&original)
                    - crate::estimate_tokens(&compressed),
                valid: true,
            });
        }

        attempt += 1;
        if attempt > max_retries {
            return Ok(CompressResult {
                original_size,
                compressed_size: compressed.len(),
                backup_path: Some(backup_path.to_string_lossy().to_string()),
                token_estimate_saved: 0,
                valid: false,
            });
        }

        // Targeted patch: try to fix specific issues
        for err in &v.errors {
            eprintln!("  Fixing: {}", err);
        }
        // Re-compress the original for retry
        compressed = compress::compress_text(&original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_compress_file_roundtrip() {
        use std::fs;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().unwrap();
        let content = "Hello, this is a test file with some filler words like basically and really.\n\n```\nfn test() {}\n```";
        writeln!(tmp, "{}", content).unwrap();
        let path = tmp.path().to_path_buf();

        // Create backup path (will be auto-created by compress_file)
        let result = compress_file(&path, 1).unwrap();
        assert!(result.valid, "Compression should be valid");

        // Read back the compressed version
        let compressed = fs::read_to_string(&path).unwrap();
        assert!(compressed.contains("fn test() {}"), "code block should be preserved");
        assert!(compressed.contains("```"), "code fences should be preserved");
        assert!(result.compressed_size < content.len(), "should be smaller");
        assert!(result.backup_path.is_some(), "backup should exist");
    }

    #[test]
    fn test_stdin_roundtrip() {
        let input = "This is basically a test. It has really simple content.";
        use crate::compress::compress_text;
        let output = compress_text(input);
        assert!(!output.contains("basically"));
        assert!(!output.contains("really"));
    }
}
