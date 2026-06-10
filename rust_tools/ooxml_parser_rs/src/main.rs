use anyhow::Result;
use clap::{Parser, Subcommand};
use ooxml_parser_rs::{
    batch, extract_pptx, inspect_docx, inspect_xlsx, read_docx, read_xlsx, render_docx_cli,
    render_xlsx_cli, RenderDocxArgs, RenderXlsxArgs,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "Fast OOXML parser (XLSX, PPTX)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect XLSX workbook
    Xlsx {
        input: String,
        #[arg(long)]
        json: bool,
    },
    /// Inspect DOCX document structure
    Docx {
        input: String,
        #[arg(long)]
        json: bool,
    },
    /// Read DOCX body content as linear text or JSON
    ReadDocx {
        input: String,
        #[arg(long, default_value_t = true)]
        text: bool,
        #[arg(long, conflicts_with = "text")]
        json: bool,
        #[arg(long, requires = "json")]
        compact: bool,
    },
    /// Read XLSX sheet cell values as markdown tables or JSON
    ReadXlsx {
        input: String,
        #[arg(long, default_value_t = 10000)]
        max_rows: usize,
        #[arg(long = "sheets")]
        sheets: Vec<String>,
        #[arg(long, default_value_t = true)]
        text: bool,
        #[arg(long, conflicts_with = "text")]
        json: bool,
        #[arg(long, requires = "json")]
        compact: bool,
    },
    /// Render an XLSX workbook to PDF and optional PNG pages
    RenderXlsx(RenderXlsxArgs),
    /// Render a DOCX-like document to PNG pages
    RenderDocx(RenderDocxArgs),
    /// Extract PPTX structure
    Pptx {
        input: String,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(long)]
        extract_images: bool,
    },
    /// Batch-read multiple .docx / .xlsx files into a catalog directory
    Batch {
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        stdin_paths: bool,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long, default_value = "auto")]
        jobs: String,
        #[arg(long)]
        resume: bool,
        #[arg(long, default_value = "false")]
        fail_fast: bool,
        #[arg(long, default_value_t = 8000)]
        max_chars: usize,
        #[arg(long, default_value_t = 10000)]
        max_rows: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Xlsx { input, json } => inspect_xlsx(&input, json)?,
        Commands::Docx { input, json } => inspect_docx(&input, json)?,
        Commands::ReadDocx {
            input,
            text: _,
            json,
            compact,
        } => read_docx(&input, json, compact)?,
        Commands::ReadXlsx {
            input,
            max_rows,
            sheets,
            text: _,
            json,
            compact,
        } => read_xlsx(&input, max_rows, &sheets, json, compact)?,
        Commands::RenderXlsx(args) => render_xlsx_cli(args)?,
        Commands::RenderDocx(args) => render_docx_cli(args)?,
        Commands::Pptx {
            input,
            output,
            extract_images: _,
        } => extract_pptx(&input, output)?,
        Commands::Batch {
            manifest,
            stdin_paths,
            out_dir,
            jobs,
            resume,
            fail_fast,
            max_chars,
            max_rows,
        } => {
            let paths = batch::load_paths(manifest.as_deref(), stdin_paths)?;
            let jobs = batch::resolve_jobs(&jobs, &paths);
            let opts = batch::BatchOptions {
                out_dir,
                jobs,
                resume,
                fail_fast,
                max_chars,
                max_rows,
            };
            let summary = batch::run_batch(paths, &opts)?;
            batch::print_catalog_summary(&summary)?;
        }
    }

    Ok(())
}
