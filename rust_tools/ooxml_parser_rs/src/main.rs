use anyhow::Result;
use calamine::{open_workbook, Reader, Xlsx};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::fs::File;
use std::io::BufWriter;
use zip::ZipArchive;

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
    /// Extract PPTX structure
    Pptx {
        input: String,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(long)]
        extract_images: bool,
    },
}

fn inspect_xlsx(input: &str, as_json: bool) -> Result<()> {
    let mut workbook: Xlsx<_> = open_workbook(input)?;
    let sheet_names = workbook.sheet_names();
    let mut sheets_info = Vec::new();

    for name in &sheet_names {
        if let Some(Ok(range)) = workbook.worksheet_range(name) {
            sheets_info.push(json!({
                "title": name,
                "max_row": range.height(),
                "max_column": range.width(),
                "bounds": format!("{}x{}", range.width(), range.height()),
            }));
        }
    }

    let summary = json!({
        "path": input,
        "sheet_count": sheet_names.len(),
        "sheet_names": sheet_names,
        "sheets": sheets_info,
    });

    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Workbook: {}", input);
        println!("Sheets ({}): {}", sheet_names.len(), sheet_names.join(", "));
        for sheet in sheets_info {
            println!(
                "[{}] bounds={} max_row={} max_col={}",
                sheet["title"].as_str().unwrap(),
                sheet["bounds"].as_str().unwrap(),
                sheet["max_row"].as_u64().unwrap(),
                sheet["max_column"].as_u64().unwrap()
            );
        }
    }

    Ok(())
}

fn extract_pptx(input: &str, output: Option<String>) -> Result<()> {
    let file = File::open(input)?;
    let mut archive = ZipArchive::new(file)?;

    let mut slide_count = 0;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().starts_with("ppt/slides/slide") && file.name().ends_with(".xml") {
            slide_count += 1;
        }
    }

    let summary = json!({
        "file": input,
        "slide_count": slide_count,
        "status": "basic_extraction_complete_pending_loop_optimizations"
    });

    if let Some(out_path) = output {
        let out_file = File::create(&out_path)?;
        let writer = BufWriter::new(out_file);
        serde_json::to_writer_pretty(writer, &summary)?;
    } else {
        let json_out = serde_json::to_string_pretty(&summary)?;
        println!("{}", json_out);
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Xlsx { input, json } => inspect_xlsx(&input, json)?,
        Commands::Pptx { input, output, extract_images: _ } => {
            extract_pptx(&input, output)?;
        }
    }

    Ok(())
}
