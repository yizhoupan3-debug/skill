use anyhow::Result;
use clap::{Parser, Subcommand};
use pdf_tool_rs::{batch, info, read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pdf", version, about = "Pure Rust PDF text extraction tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract text from a single PDF (default: plain text to stdout)
    Read {
        input: PathBuf,
        /// Emit JSON instead of plain text
        #[arg(long)]
        json: bool,
        /// Max characters to extract (default 8000)
        #[arg(long, default_value_t = 8000)]
        max_chars: usize,
    },
    /// Show PDF metadata without full batch extraction
    Info {
        input: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 200)]
        preview_chars: usize,
    },
    /// Batch-extract multiple PDFs into an output directory
    Batch {
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        stdin_paths: bool,
        #[arg(long)]
        out_dir: PathBuf,
        /// Parallel workers: number or `auto` (default min(8, cpus); see `PDF_BATCH_JOBS`)
        #[arg(long, default_value = "auto")]
        jobs: String,
        #[arg(long)]
        resume: bool,
        /// Shallow-probe first pages; skip full extract when no text layer (scanned/empty).
        #[arg(long)]
        skip_scanned: bool,
        #[arg(long, default_value = "false")]
        fail_fast: bool,
        #[arg(long, default_value_t = 8000)]
        max_chars: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Read {
            input,
            json,
            max_chars,
        } => {
            let opts = read::ReadOptions {
                max_chars,
                text_out_dir: None,
            };
            let out = read::read_pdf(&input, &opts)?;
            if json {
                let payload = serde_json::json!({
                    "path": input.display().to_string(),
                    "sha256": out.file_sha256,
                    "page_count": out.page_count,
                    "content_class": out.content_class,
                    "char_count": out.text.chars().count(),
                    "truncated": out.truncated,
                    "warnings": out.warnings,
                    "text": out.text,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print!("{}", out.text);
            }
        }
        Commands::Info {
            input,
            json,
            preview_chars,
        } => {
            let meta = info::pdf_info(&input, preview_chars)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&meta)?);
            } else {
                println!("path: {}", meta.path);
                println!("sha256: {}", meta.sha256);
                println!("pages: {}", meta.page_count);
                println!("size_bytes: {}", meta.file_size_bytes);
                println!("content_class: {}", meta.content_class);
                if !meta.warnings.is_empty() {
                    println!("warnings: {:?}", meta.warnings);
                }
            }
        }
        Commands::Batch {
            manifest,
            stdin_paths,
            out_dir,
            jobs,
            resume,
            skip_scanned,
            fail_fast,
            max_chars,
        } => {
            let paths = batch::load_paths(manifest.as_deref(), stdin_paths)?;
            let jobs = batch::resolve_jobs(&jobs, &paths);
            let opts = batch::BatchOptions {
                out_dir,
                jobs,
                resume,
                fail_fast,
                max_chars,
            };
            let summary = batch::run_batch(paths, &opts, skip_scanned)?;
            batch::print_catalog_summary(&summary)?;
        }
    }
    Ok(())
}
