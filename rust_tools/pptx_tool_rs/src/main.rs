use anyhow::Result;
use clap::{Parser, Subcommand};

// Re-export everything from lib for bin/ppt.rs compatibility
pub use pptx_tool_rs::*;

#[derive(Parser)]
#[command(author, version, about = "Rust-first CLI for skills/ppt-pptx")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    New(NewArgs),
    Outline(OutlineArgs),
    Render(RenderArgs),
    ExtractStructure(ExtractStructureArgs),
    ReadFull(ReadFullArgs),
    EnsureRasterImage(EnsureRasterImageArgs),
    CreateMontage(CreateMontageArgs),
    SlidesTest(SlidesTestArgs),
    DetectFonts(DetectFontsArgs),
    SanitizePptx(SanitizePptxArgs),
    Qa(QaArgs),
    Intake(IntakeArgs),
    BuildQa(BuildQaArgs),
    Office(OfficeArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init(args) => init_command(args),
        Commands::New(args) => init_command(args.init),
        Commands::Outline(args) => outline_command(args),
        Commands::Render(args) => render_command(args),
        Commands::ExtractStructure(args) => extract_structure_command(args),
        Commands::ReadFull(args) => read_full_command(args),
        Commands::EnsureRasterImage(args) => ensure_raster_image_command(args),
        Commands::CreateMontage(args) => create_montage_command(args),
        Commands::SlidesTest(args) => slides_test_command(args),
        Commands::DetectFonts(args) => detect_fonts_command(args),
        Commands::SanitizePptx(args) => sanitize_pptx_command(args),
        Commands::Qa(args) => qa_command(args),
        Commands::Intake(args) => intake_command(args),
        Commands::BuildQa(args) => build_qa_command(args),
        Commands::Office(args) => office_command(args),
    }
}
