use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use font8x8::{UnicodeFonts, BASIC_FONTS};
use image::{
    imageops::{self, FilterType},
    DynamicImage, Rgba, RgbaImage,
};
use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const EMU_PER_INCH: f64 = 914_400.0;
const POINTS_PER_INCH: f64 = 72.0;
const DEFAULT_PAD_PX: u32 = 100;
const SOFFICE_PROBE_TIMEOUT: Duration = Duration::from_secs(20);

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

#[derive(ValueEnum, Clone, Debug)]
enum DeckTemplate {
    Dark,
    Light,
    Corporate,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum QualityMode {
    Standard,
    Strict,
}

#[derive(Args)]
struct InitArgs {
    #[arg(default_value = ".")]
    workdir: String,
    #[arg(long, value_enum, default_value_t = DeckTemplate::Dark)]
    template: DeckTemplate,
    #[arg(long, default_value_t = false)]
    force: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct NewArgs {
    #[command(flatten)]
    init: InitArgs,
}

#[derive(Args)]
struct OutlineArgs {
    input: String,
    #[arg(short, long, default_value = "deck.plan.json")]
    output: String,
    #[arg(long, value_enum, default_value_t = DeckTemplate::Dark)]
    template: DeckTemplate,
    #[arg(long, default_value_t = false)]
    bootstrap: bool,
    #[arg(long, default_value_t = false)]
    build: bool,
    #[arg(long, default_value_t = false)]
    qa: bool,
    #[arg(long, value_enum, default_value_t = QualityMode::Standard)]
    quality: QualityMode,
    #[arg(long, default_value = "rendered")]
    rendered_dir: String,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct RenderArgs {
    input_path: String,
    #[arg(long, visible_alias = "output_dir")]
    output_dir: Option<String>,
    #[arg(long, default_value_t = 1600)]
    width: u32,
    #[arg(long, default_value_t = 900)]
    height: u32,
}

#[derive(Args)]
struct ExtractStructureArgs {
    input: String,
    #[arg(short, long)]
    output: Option<String>,
    #[arg(long)]
    extract_images: bool,
    #[arg(long, default_value = "extracted_assets")]
    image_dir: String,
    #[arg(long, default_value_t = true)]
    pretty: bool,
}

#[derive(Args)]
struct EnsureRasterImageArgs {
    #[arg(long, visible_alias = "input_files")]
    input_files: Vec<String>,
    #[arg(long, visible_alias = "input_dir")]
    input_dir: Option<String>,
    #[arg(long, visible_alias = "output_dir")]
    output_dir: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum LabelMode {
    Number,
    Filename,
    None,
}

#[derive(Args)]
struct CreateMontageArgs {
    #[arg(long, visible_alias = "input_files")]
    input_files: Vec<String>,
    #[arg(long, visible_alias = "input_dir")]
    input_dir: Option<String>,
    #[arg(long, visible_alias = "output_file")]
    output_file: String,
    #[arg(long, visible_alias = "num_col", default_value_t = 5)]
    num_col: usize,
    #[arg(long, visible_alias = "cell_width", default_value_t = 400)]
    cell_width: u32,
    #[arg(long, visible_alias = "cell_height", default_value_t = 225)]
    cell_height: u32,
    #[arg(long, default_value_t = 16)]
    gap: u32,
    #[arg(long, visible_alias = "label_mode", value_enum, default_value_t = LabelMode::Number)]
    label_mode: LabelMode,
    #[arg(
        long,
        visible_alias = "retain_converted_files",
        default_value_t = false
    )]
    retain_converted_files: bool,
    #[arg(long, visible_alias = "fail_on_image_error", default_value_t = false)]
    fail_on_image_error: bool,
}

#[derive(Args)]
struct SlidesTestArgs {
    input_path: String,
    #[arg(long, default_value_t = 1600)]
    width: u32,
    #[arg(long, default_value_t = 900)]
    height: u32,
    #[arg(long, visible_alias = "pad_px", default_value_t = DEFAULT_PAD_PX)]
    pad_px: u32,
    #[arg(long, default_value_t = false)]
    fail_on_overflow: bool,
}

#[derive(Args)]
struct DetectFontsArgs {
    input_path: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = true)]
    include_missing: bool,
    #[arg(long, default_value_t = true)]
    include_substituted: bool,
}

#[derive(Args)]
struct SanitizePptxArgs {
    input_path: String,
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Args)]
struct QaArgs {
    deck: String,
    #[arg(long, default_value = "rendered")]
    rendered_dir: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    fail_on_issues: bool,
}

#[derive(Args)]
struct IntakeArgs {
    deck: String,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct BuildQaArgs {
    #[arg(long, default_value = ".")]
    workdir: String,
    #[arg(long, default_value = "deck.plan.json")]
    entry: String,
    #[arg(long, default_value = "deck.pptx")]
    deck: String,
    #[arg(long, default_value = "rendered")]
    rendered_dir: String,
    #[arg(long, value_enum, default_value_t = QualityMode::Standard)]
    quality: QualityMode,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct OfficeArgs {
    #[command(subcommand)]
    command: OfficeCommands,
}

#[derive(Subcommand)]
enum OfficeCommands {
    Probe(OfficeProbeArgs),
    Doctor(OfficeDoctorArgs),
    Outline(OfficeFileArgs),
    Issues(OfficeFileArgs),
    Validate(OfficeFileArgs),
    Get(OfficeGetArgs),
    Query(OfficeQueryArgs),
    Watch(OfficeWatchArgs),
    Batch(OfficeBatchArgs),
}

#[derive(Args)]
struct OfficeProbeArgs {
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct OfficeDoctorArgs {
    file: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    fail_on_issues: bool,
    #[arg(long, default_value_t = false)]
    fail_on_validation: bool,
}

#[derive(Args)]
struct OfficeFileArgs {
    file: String,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct OfficeGetArgs {
    file: String,
    #[arg(default_value = "/")]
    path: String,
    #[arg(long, default_value_t = 1)]
    depth: i32,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct OfficeQueryArgs {
    file: String,
    selector: String,
    #[arg(long)]
    text: Option<String>,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
struct OfficeWatchArgs {
    file: String,
    #[arg(long, default_value_t = 18080)]
    port: u16,
    #[arg(long, default_value_t = false)]
    browser: bool,
}

#[derive(Args)]
struct OfficeBatchArgs {
    file: String,
    #[arg(long)]
    input: Option<String>,
    #[arg(long)]
    commands: Option<String>,
    #[arg(long, default_value_t = false)]
    force: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Serialize)]
struct InitSummary {
    workdir: String,
    template: String,
    files: Vec<String>,
    rust_only: bool,
    command_manifest: String,
}

#[derive(Debug, Serialize)]
struct OutlineSummary {
    input: String,
    output: String,
    bootstrapped: bool,
    built: bool,
    qa: Option<Value>,
}

#[derive(Debug, Clone)]
struct ZipBundle {
    files: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
struct Position {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ParagraphInfo {
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct TextInfo {
    #[serde(rename = "fullText")]
    full_text: String,
    paragraphs: Vec<ParagraphInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct ImageInfo {
    content_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    extracted_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TableInfo {
    rows: usize,
    cols: usize,
    data: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct ChartInfo {
    chart_type: String,
    has_legend: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct ElementInfo {
    index: usize,
    name: String,
    #[serde(rename = "type")]
    element_type: String,
    position: Position,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<TableInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chart: Option<ChartInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<ElementInfo>>,
}

#[derive(Debug, Clone, Serialize)]
struct LayoutPlaceholder {
    idx: Option<String>,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct LayoutInfo {
    name: String,
    placeholders: Vec<LayoutPlaceholder>,
}

#[derive(Debug, Serialize)]
struct QaRenderSummary {
    rendered_dir: String,
    png_count: usize,
    paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct QaOverflowSummary {
    ok: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
struct QaSummary {
    ok: bool,
    deck: String,
    render: QaRenderSummary,
    overflow_check: QaOverflowSummary,
    font_check: Value,
    inspector: Value,
}

#[derive(Debug, Serialize)]
struct OfficeProbeSummary {
    available: bool,
    engine: String,
    version: Option<String>,
}

#[derive(Debug, Serialize)]
struct OfficeDoctorSummary {
    inspector_version: Option<String>,
    file: String,
    outline: Value,
    issues: Value,
    validation: Value,
}

#[derive(Debug, PartialEq, Eq)]
enum EmitFormat {
    Json,
    Text,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init(args) => init_command(args)?,
        Commands::New(args) => init_command(args.init)?,
        Commands::Outline(args) => outline_command(args)?,
        Commands::Render(args) => render_command(args)?,
        Commands::ExtractStructure(args) => extract_structure_command(args)?,
        Commands::EnsureRasterImage(args) => ensure_raster_image_command(args)?,
        Commands::CreateMontage(args) => create_montage_command(args)?,
        Commands::SlidesTest(args) => slides_test_command(args)?,
        Commands::DetectFonts(args) => detect_fonts_command(args)?,
        Commands::SanitizePptx(args) => sanitize_pptx_command(args)?,
        Commands::Qa(args) => qa_command(args)?,
        Commands::Intake(args) => intake_command(args)?,
        Commands::BuildQa(args) => build_qa_command(args)?,
        Commands::Office(args) => office_command(args)?,
    }
    Ok(())
}

fn init_command(args: InitArgs) -> Result<()> {
    let workdir = expand_path(&args.workdir);
    let summary = init_workspace(&workdir, &args.template, args.force)?;
    emit_value(
        serde_json::to_value(summary)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn outline_command(args: OutlineArgs) -> Result<()> {
    let input = expand_path(&args.input);
    let workdir = input
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    if args.bootstrap {
        init_workspace(&workdir, &args.template, false)?;
    }

    let output = expand_path(&args.output);
    let output = if output.is_absolute() {
        output
    } else {
        workdir.join(output)
    };

    let outline = read_outline(&input)?;
    let generated = generate_outline_deck_source(&outline, &args.template)?;
    fs::write(&output, generated)
        .with_context(|| format!("failed to write {}", output.display()))?;

    let mut qa_payload = None;
    let strict_quality = args.quality == QualityMode::Strict;
    if args.build || args.qa || strict_quality {
        let deck = workdir.join("deck.pptx");
        write_outline_deck_pptx(&outline, &deck, &args.template)?;
        sanitize_pptx_command(SanitizePptxArgs {
            input_path: deck.display().to_string(),
            output: None,
        })?;
    }
    if args.qa || strict_quality {
        qa_payload = Some(serde_json::to_value(qa_summary(
            &workdir.join("deck.pptx").display().to_string(),
            &workdir.join(&args.rendered_dir).display().to_string(),
        )?)?);
        if strict_quality {
            strict_quality_gate(qa_payload.as_ref().unwrap())?;
        }
    }

    emit_value(
        serde_json::to_value(OutlineSummary {
            input: input.display().to_string(),
            output: output.display().to_string(),
            bootstrapped: args.bootstrap,
            built: args.build || args.qa || strict_quality,
            qa: qa_payload,
        })?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn qa_command(args: QaArgs) -> Result<()> {
    let payload = qa_summary(&args.deck, &args.rendered_dir)?;
    if args.fail_on_issues && !payload.ok {
        bail!("qa failed: overflow, font, or Rust inspector issue detected");
    }
    emit_value(
        serde_json::to_value(payload)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn intake_command(args: IntakeArgs) -> Result<()> {
    let structure = extract_structure_payload(&args.deck)?;
    let inspector = office_doctor_value(&args.deck)?;
    let payload = json!({
        "deck": args.deck,
        "structure": structure,
        "inspector": inspector,
    });
    emit_value(
        payload,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn build_qa_command(args: BuildQaArgs) -> Result<()> {
    let workdir = expand_path(&args.workdir);
    let entry = expand_path(&args.entry);
    let entry = if entry.is_absolute() {
        entry
    } else {
        workdir.join(entry)
    };
    let outline = read_outline(&entry)?;
    let deck = workdir.join(&args.deck);
    write_outline_deck_pptx(&outline, &deck, &DeckTemplate::Dark)?;
    sanitize_pptx_command(SanitizePptxArgs {
        input_path: deck.display().to_string(),
        output: None,
    })?;
    let rendered = workdir.join(&args.rendered_dir);
    let payload = qa_summary(&deck.display().to_string(), &rendered.display().to_string())?;
    if args.quality == QualityMode::Strict {
        strict_quality_gate(&serde_json::to_value(&payload)?)?;
    }
    emit_value(
        serde_json::to_value(payload)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn office_command(args: OfficeArgs) -> Result<()> {
    match args.command {
        OfficeCommands::Probe(args) => office_probe_command(args),
        OfficeCommands::Doctor(args) => office_doctor_command(args),
        OfficeCommands::Outline(args) => {
            office_file_passthrough("view", &args.file, Some("outline"), args.json)
        }
        OfficeCommands::Issues(args) => {
            office_file_passthrough("view", &args.file, Some("issues"), args.json)
        }
        OfficeCommands::Validate(args) => {
            office_file_passthrough("validate", &args.file, None, args.json)
        }
        OfficeCommands::Get(args) => office_get_command(args),
        OfficeCommands::Query(args) => office_query_command(args),
        OfficeCommands::Watch(args) => office_watch_command(args),
        OfficeCommands::Batch(args) => office_batch_command(args),
    }
}

fn office_probe_command(args: OfficeProbeArgs) -> Result<()> {
    let payload = OfficeProbeSummary {
        available: true,
        engine: "rust-pptx-inspector".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("inspector: {}", payload.engine);
        println!(
            "version: {}",
            payload.version.unwrap_or_else(|| "unknown".to_string())
        );
    }
    Ok(())
}

fn office_doctor_command(args: OfficeDoctorArgs) -> Result<()> {
    let payload = office_doctor_summary(&args.file)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        print_office_doctor_summary(&payload);
    }
    if (args.fail_on_issues && payload.issues["count"].as_u64().unwrap_or(0) > 0)
        || (args.fail_on_validation && !payload.validation["ok"].as_bool().unwrap_or(false))
    {
        bail!("office doctor checks failed")
    }
    Ok(())
}

fn office_file_passthrough(
    command: &str,
    file: &str,
    tail: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let payload = match (command, tail) {
        ("view", Some("outline")) => rust_office_outline_value(file)?,
        ("view", Some("issues")) => rust_office_issues_value(file)?,
        ("validate", None) => rust_office_validate_value(file)?,
        _ => bail!("unsupported Rust inspector command: {command} {tail:?}"),
    };
    emit_value(
        payload,
        if json_output {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )?;
    Ok(())
}

fn office_get_command(args: OfficeGetArgs) -> Result<()> {
    let payload = rust_office_get_value(&args.file, &args.path, args.depth)?;
    emit_value(
        payload,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn office_query_command(args: OfficeQueryArgs) -> Result<()> {
    let payload = rust_office_query_value(&args.file, &args.selector, args.text.as_deref())?;
    emit_value(
        payload,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn office_watch_command(args: OfficeWatchArgs) -> Result<()> {
    let preview = write_rust_office_preview(&args.file, args.port)?;
    if args.browser {
        let status = Command::new("open").arg(&preview).status()?;
        if !status.success() {
            bail!("failed to open browser with status {:?}", status.code());
        }
    }
    println!("preview: {}", preview.display());
    Ok(())
}

fn office_batch_command(args: OfficeBatchArgs) -> Result<()> {
    let payload = rust_office_batch_value(
        &args.file,
        args.input.as_deref(),
        args.commands.as_deref(),
        args.force,
    )?;
    emit_value(
        payload,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

fn render_command(args: RenderArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    let output_dir = args
        .output_dir
        .as_deref()
        .map(expand_path)
        .unwrap_or_else(|| default_render_dir(&input));
    let rendered = render_paths(&input, &output_dir, args.width, args.height)?;
    for path in rendered {
        println!("{}", path.display());
    }
    Ok(())
}

fn init_workspace(workdir: &Path, template: &DeckTemplate, force: bool) -> Result<InitSummary> {
    let mut created = Vec::new();

    fs::create_dir_all(workdir)?;
    fs::create_dir_all(workdir.join("assets"))?;
    fs::create_dir_all(workdir.join("rendered"))?;

    let starter_outline = workdir.join("outline.json");
    if !starter_outline.exists() || force {
        fs::write(
            &starter_outline,
            serde_json::to_string_pretty(&starter_outline_value(template))?,
        )
        .with_context(|| format!("failed to write {}", starter_outline.display()))?;
        created.push(starter_outline.display().to_string());
    } else {
        created.push(format!("kept:{}", starter_outline.display()));
    }

    let plan = workdir.join("deck.plan.json");
    if !plan.exists() || force {
        fs::write(
            &plan,
            generate_outline_deck_source(&starter_outline_value(template), template)?,
        )
        .with_context(|| format!("failed to write {}", plan.display()))?;
        created.push(plan.display().to_string());
    } else {
        created.push(format!("kept:{}", plan.display()));
    }

    let sources = workdir.join("sources.md");
    if !sources.exists() || force {
        fs::write(&sources, starter_sources_markdown(template))
            .with_context(|| format!("failed to write {}", sources.display()))?;
        created.push(sources.display().to_string());
    } else {
        created.push(format!("kept:{}", sources.display()));
    }

    let command_manifest = workdir.join("ppt.commands.json");
    if !command_manifest.exists() || force {
        fs::write(
            &command_manifest,
            serde_json::to_string_pretty(&rust_command_manifest_value())?,
        )
        .with_context(|| format!("failed to write {}", command_manifest.display()))?;
        created.push(command_manifest.display().to_string());
    } else {
        created.push(format!("kept:{}", command_manifest.display()));
    }

    Ok(InitSummary {
        workdir: workdir.display().to_string(),
        template: format!("{:?}", template).to_ascii_lowercase(),
        files: created,
        rust_only: true,
        command_manifest: command_manifest.display().to_string(),
    })
}

fn starter_sources_markdown(template: &DeckTemplate) -> String {
    format!(
        "# Sources\n\n- Deck source plan: `deck.plan.json`\n- Editable output: `deck.pptx`\n- Runtime: Rust `ppt` CLI\n- Template: `{}`\n\n## Workflow Notes\n\n- Text pass: use `$humanizer` for ordinary prose, `$copywriting` for pitch / sales / product-message decks, and `$paper-writing` for academic prose.\n- Design pass: use `$design-md` for source-material design extraction, `$frontend-design` for a fresh premium direction, `$visual-review` for rendered PNG evidence, and `$design-output-auditor` for drift acceptance.\n\nAdd source URLs, local asset paths, and review notes here before final delivery.\n",
        format!("{:?}", template).to_ascii_lowercase()
    )
}

fn rust_command_manifest_value() -> Value {
    json!({
        "name": "ppt-pptx-rust-commands",
        "runtime": "ppt",
        "commands": {
            "build": "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered",
            "render": "ppt render deck.pptx --output_dir rendered",
            "check_overflow": "ppt slides-test deck.pptx",
            "check_fonts": "ppt detect-fonts deck.pptx --json",
            "check_inspector": "ppt office doctor deck.pptx --json",
            "check_rust": "ppt qa deck.pptx --rendered-dir rendered --fail-on-issues --json",
            "build_rust": "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --json",
            "build_strict": "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered --quality strict --json",
            "intake_rust": "ppt intake deck.pptx --json",
            "inspect_outline": "ppt office outline deck.pptx --json",
            "watch_rust": "ppt office watch deck.pptx --browser"
        }
    })
}

fn read_outline(input: &Path) -> Result<Value> {
    let raw = fs::read_to_string(input)
        .with_context(|| format!("failed to read outline {}", input.display()))?;
    if has_extension(input, "json") {
        let value: Value = serde_json::from_str(&raw).context("failed to parse JSON outline")?;
        return Ok(unwrap_outline_plan(value));
    }
    parse_outline_yaml_subset(&raw)
}

fn unwrap_outline_plan(value: Value) -> Value {
    value
        .get("outline")
        .filter(|_| {
            value
                .get("format")
                .and_then(Value::as_str)
                .is_some_and(|format| format == "ppt-rust-outline-plan")
        })
        .cloned()
        .unwrap_or(value)
}

fn starter_outline_value(template: &DeckTemplate) -> Value {
    let palette = match template {
        DeckTemplate::Light => "light",
        DeckTemplate::Corporate => "academic",
        DeckTemplate::Dark => "dark",
    };
    json!({
        "title": "Rust Authored Deck",
        "subtitle": "Editable PPTX generated by Rust",
        "presenter": "Presenter",
        "date": "2026",
        "palette": palette,
        "slides": [
            {
                "title": "One Rust path",
                "subtitle": "Outline, PPTX writing, QA and inspection stay in the ppt CLI",
                "bullets": [
                    "Write outline JSON or YAML",
                    "Run ppt outline outline.json --build",
                    "Review rendered PNGs and QA output"
                ]
            }
        ],
        "closingText": "Thank you"
    })
}

fn parse_outline_yaml_subset(raw: &str) -> Result<Value> {
    let mut root = serde_json::Map::new();
    let mut slides = Vec::new();
    let mut current_slide: Option<serde_json::Map<String, Value>> = None;
    let mut current_list: Option<String> = None;
    let mut current_object: Option<String> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if !line.starts_with(' ') && trimmed.ends_with(':') {
            current_list = None;
            current_object = None;
            continue;
        }

        if !line.starts_with(' ') {
            if let Some((key, value)) = trimmed.split_once(':') {
                root.insert(key.trim().to_string(), parse_yaml_scalar(value.trim()));
            }
            continue;
        }

        if trimmed.starts_with("- ") && line.starts_with("  - ") {
            if let Some(slide) = current_slide.take() {
                slides.push(Value::Object(slide));
            }
            let mut slide = serde_json::Map::new();
            let rest = trimmed.trim_start_matches("- ").trim();
            if let Some((key, value)) = rest.split_once(':') {
                slide.insert(key.trim().to_string(), parse_yaml_scalar(value.trim()));
            }
            current_slide = Some(slide);
            current_list = None;
            current_object = None;
            continue;
        }

        let Some(slide) = current_slide.as_mut() else {
            continue;
        };

        if line.starts_with("        - ") {
            let Some(object_key) = current_object.clone() else {
                continue;
            };
            let item = trimmed.trim_start_matches("- ").trim();
            if let Some(object) = slide.get_mut(&object_key).and_then(Value::as_object_mut) {
                let array = object
                    .entry("series".to_string())
                    .or_insert_with(|| json!([]));
                if let Some(items) = array.as_array_mut() {
                    items.push(parse_yaml_inline_value(item));
                }
            }
            continue;
        }

        if line.starts_with("        ") {
            let object_key = current_object
                .clone()
                .or_else(|| current_list.clone())
                .unwrap_or_default();
            if object_key.is_empty() {
                continue;
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                if let Some(object) = slide.get_mut(&object_key).and_then(Value::as_object_mut) {
                    object.insert(key.trim().to_string(), parse_yaml_scalar(value.trim()));
                }
            }
            continue;
        }

        if line.starts_with("    ") && !line.starts_with("      ") {
            if let Some((key, value)) = trimmed.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim();
                current_list = None;
                current_object = None;
                if value.is_empty() {
                    if matches!(key.as_str(), "bullets" | "metrics" | "steps" | "timeline") {
                        slide.insert(key.clone(), Value::Array(Vec::new()));
                        current_list = Some(key);
                    } else {
                        slide.insert(key.clone(), json!({}));
                        current_object = Some(key);
                    }
                } else if let Some(object_key) = current_object.clone() {
                    if let Some(object) = slide.get_mut(&object_key).and_then(Value::as_object_mut)
                    {
                        object.insert(key, parse_yaml_scalar(value));
                    }
                } else {
                    slide.insert(key, parse_yaml_scalar(value));
                    current_object = None;
                }
            }
            continue;
        }

        if line.starts_with("      - ") {
            let Some(list_key) = current_list.clone() else {
                continue;
            };
            let item = trimmed.trim_start_matches("- ").trim();
            if let Some(array) = slide.get_mut(&list_key).and_then(Value::as_array_mut) {
                array.push(parse_yaml_inline_value(item));
            }
            continue;
        }

        if line.starts_with("      ") && !line.starts_with("        ") {
            if let Some((raw_key, raw_value)) = trimmed.split_once(':') {
                let key = raw_key.trim().to_string();
                let value = raw_value.trim();
                if let Some(object_key) = current_object.clone() {
                    if let Some(object) = slide.get_mut(&object_key).and_then(Value::as_object_mut)
                    {
                        if value.is_empty() {
                            object.insert(key, json!([]));
                        } else {
                            object.insert(key, parse_yaml_scalar(value));
                        }
                    }
                } else if value.is_empty() {
                    slide.insert(key.clone(), json!({}));
                    current_object = Some(key);
                } else {
                    slide.insert(key, parse_yaml_scalar(value));
                    current_object = None;
                }
            }
            continue;
        }
    }

    if let Some(slide) = current_slide.take() {
        slides.push(Value::Object(slide));
    }
    root.insert("slides".to_string(), Value::Array(slides));
    Ok(Value::Object(root))
}

fn parse_yaml_inline_value(value: &str) -> Value {
    let value = value.trim();
    if value.starts_with('{') || value.starts_with('[') {
        parse_yaml_jsonish(value).unwrap_or_else(|| Value::String(unquote_yaml(value).to_string()))
    } else {
        parse_yaml_scalar(value)
    }
}

fn parse_yaml_scalar(value: &str) -> Value {
    let value = value.trim();
    if value.is_empty() {
        Value::Null
    } else if value.starts_with('[') {
        parse_yaml_jsonish(value).unwrap_or_else(|| Value::String(unquote_yaml(value).to_string()))
    } else {
        Value::String(unquote_yaml(value).to_string())
    }
}

fn parse_yaml_jsonish(value: &str) -> Option<Value> {
    serde_json::from_str(value)
        .ok()
        .or_else(|| serde_json::from_str(&yaml_inline_to_jsonish(value)).ok())
}

fn yaml_inline_to_jsonish(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    let mut in_string = false;
    let mut quote = '\0';
    let mut key_start = true;
    let mut reading_key = false;
    for ch in value.chars() {
        if in_string {
            if ch == quote {
                in_string = false;
            }
            out.push(ch);
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_string = true;
                quote = ch;
                out.push('"');
            }
            '{' | ',' => {
                key_start = ch == '{';
                reading_key = false;
                out.push(ch);
            }
            ':' if reading_key => {
                key_start = false;
                reading_key = false;
                out.push_str("\":");
            }
            ch if key_start && !ch.is_whitespace() => {
                key_start = false;
                reading_key = true;
                out.push('"');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn unquote_yaml(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|item| item.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|item| item.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn generate_outline_deck_source(outline: &Value, _template: &DeckTemplate) -> Result<String> {
    let outline = naturalize_outline_value(outline);
    serde_json::to_string_pretty(&json!({
        "format": "ppt-rust-outline-plan",
        "design_brief": {
            "source": "DESIGN.md visual contract when source materials or brand examples exist; otherwise choose a frontend-design direction before styling",
            "copy": "run a text pass before layout: $humanizer for natural prose, $copywriting for persuasive decks, $paper-writing for academic decks; keep direct claims and remove generic AI filler",
            "layout": "one visual lead, readable type, no equal-weight card farm; encode the chosen design roles in deck.plan.json",
            "audit": "render evidence, visual-review findings, design-output-auditor drift verdict, then strict Rust QA"
        },
        "outline": outline
    }))
    .context("failed to serialize Rust outline plan")
}

fn naturalize_outline_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(naturalize_copy_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(naturalize_outline_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), naturalize_outline_value(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn naturalize_copy_text(input: &str) -> String {
    let mut text = clean_copy_spacing(input);
    let replacements = [
        ("核心观点如下：", ""),
        ("核心观点如下:", ""),
        ("请重点关注", "重点看"),
        ("保持叙事连贯性", "让转场自然"),
        ("结合实际选取最优方案", "回到现场约束再取舍"),
        ("具有重要意义", "会影响具体决策"),
        ("多维度", "几个角度"),
        ("赋能", "支持"),
        ("打造", "做"),
        ("显著提升", "提高"),
        ("持续优化", "继续改"),
        ("综上所述，", ""),
        ("综上所述,", ""),
        ("值得关注的是，", ""),
        ("值得关注的是,", ""),
        ("This slide presents ", ""),
        ("This slide shows ", ""),
        ("This slide introduces ", ""),
        ("It is important to note that ", ""),
    ];
    for (from, to) in replacements {
        text = text.replace(from, to);
    }

    for prefix in [
        "本页主要展示了",
        "本页主要展示",
        "本页重点展示了",
        "本页重点展示",
        "本页展示了",
        "本页展示",
        "本页呈现了",
        "本页呈现",
        "本页介绍了",
        "本页介绍",
        "本页说明了",
        "本页说明",
        "本页从多个维度展开分析，",
        "本页从多个维度展开分析,",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_string();
            break;
        }
    }

    clean_copy_spacing(&text)
}

fn clean_copy_spacing(input: &str) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, '：' | ':' | '，' | ',' | '。'))
        .to_string()
}

fn reflow_outline_slides(slides: Option<&Vec<Value>>) -> Vec<Value> {
    let mut out = Vec::new();
    for slide in slides.into_iter().flatten() {
        let pattern = detect_outline_pattern(slide);
        if pattern == "multi-card" && value_array_len(slide, "bullets") > 4 {
            push_chunked_slide(&mut out, slide, "bullets", 4);
        } else if pattern == "process-flow" && value_array_len(slide, "steps") > 5 {
            push_chunked_slide(&mut out, slide, "steps", 4);
        } else if pattern == "timeline" && value_array_len(slide, "timeline") > 5 {
            push_chunked_slide(&mut out, slide, "timeline", 4);
        } else if pattern == "image-text-split" && joined_array_chars(slide, "bullets") > 150 {
            push_split_slide(&mut out, slide, "bullets");
        } else {
            out.push(slide.clone());
        }
    }
    out
}

fn push_chunked_slide(out: &mut Vec<Value>, slide: &Value, key: &str, chunk_size: usize) {
    let Some(items) = slide.get(key).and_then(Value::as_array) else {
        out.push(slide.clone());
        return;
    };
    let chunks: Vec<&[Value]> = items.chunks(chunk_size).collect();
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut cloned = slide.as_object().cloned().unwrap_or_default();
        cloned.insert(
            "title".to_string(),
            Value::String(format!(
                "{} ({}/{})",
                outline_str(slide, "title", "Untitled"),
                idx + 1,
                chunks.len()
            )),
        );
        cloned.insert(key.to_string(), Value::Array(chunk.to_vec()));
        out.push(Value::Object(cloned));
    }
}

fn push_split_slide(out: &mut Vec<Value>, slide: &Value, key: &str) {
    let Some(items) = slide.get(key).and_then(Value::as_array) else {
        out.push(slide.clone());
        return;
    };
    let mid = items.len().div_ceil(2);
    for (idx, chunk) in [&items[..mid], &items[mid..]].iter().enumerate() {
        let mut cloned = slide.as_object().cloned().unwrap_or_default();
        cloned.insert(
            "title".to_string(),
            Value::String(format!(
                "{} ({}/2)",
                outline_str(slide, "title", "Untitled"),
                idx + 1
            )),
        );
        cloned.insert(key.to_string(), Value::Array(chunk.to_vec()));
        out.push(Value::Object(cloned));
    }
}

fn detect_outline_pattern(slide: &Value) -> &'static str {
    if value_array_len(slide, "timeline") > 0 {
        "timeline"
    } else if value_array_len(slide, "steps") > 0 {
        "process-flow"
    } else if slide.get("comparison").is_some() {
        "comparison"
    } else if slide.get("chart").is_some() || value_array_len(slide, "metrics") >= 3 {
        "data-panel"
    } else if slide.get("image").is_some() && value_array_len(slide, "bullets") <= 2 {
        "hero-image"
    } else if slide.get("image").is_some() {
        "image-text-split"
    } else if value_array_len(slide, "bullets") >= 3 {
        "multi-card"
    } else {
        "full-text"
    }
}

#[derive(Clone, Debug)]
struct PptxSlideSpec {
    title: String,
    subtitle: String,
    label: String,
    body: Vec<String>,
    notes: String,
    layout: &'static str,
}

fn write_outline_deck_pptx(outline: &Value, output: &Path, template: &DeckTemplate) -> Result<()> {
    let outline = naturalize_outline_value(outline);
    let slides = build_pptx_slide_specs(&outline);
    let palette = ppt_palette(outline.get("palette").and_then(Value::as_str).unwrap_or(
        match template {
            DeckTemplate::Light => "light",
            DeckTemplate::Corporate => "academic",
            DeckTemplate::Dark => "dark",
        },
    ));
    write_pptx_package(
        output,
        &slides,
        &palette,
        outline_str(&outline, "title", "Deck"),
    )
}

fn build_pptx_slide_specs(outline: &Value) -> Vec<PptxSlideSpec> {
    let content_slides = reflow_outline_slides(outline.get("slides").and_then(Value::as_array));
    let mut slides = Vec::new();
    slides.push(PptxSlideSpec {
        title: outline_str(outline, "title", "Untitled Deck").to_string(),
        subtitle: outline_str(outline, "subtitle", "").to_string(),
        label: "OPENING".to_string(),
        body: [
            outline_str(outline, "presenter", ""),
            outline_str(outline, "date", ""),
        ]
        .into_iter()
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect(),
        notes: "Cover slide generated by the Rust ppt CLI.".to_string(),
        layout: "cover",
    });

    for (idx, slide) in content_slides.iter().enumerate() {
        let mut body = value_string_array(slide, "bullets");
        if body.is_empty() {
            body = value_string_array(slide, "steps");
        }
        if body.is_empty() {
            body = value_string_array(slide, "timeline");
        }
        if body.is_empty() {
            body = comparison_body(slide);
        }
        if body.is_empty() {
            body = metrics_body(slide);
        }
        if body.is_empty() {
            body.push(outline_str(slide, "body", "").to_string());
        }
        body.retain(|item| !item.trim().is_empty());
        slides.push(PptxSlideSpec {
            title: outline_str(slide, "title", "").to_string(),
            subtitle: outline_str(slide, "subtitle", "").to_string(),
            label: format!("SECTION {:02}", idx + 1),
            notes: format!(
                "Slide {}: {}",
                idx + 2,
                outline_str(slide, "title", "Untitled")
            ),
            body,
            layout: detect_outline_pattern(slide),
        });
    }

    slides.push(PptxSlideSpec {
        title: outline_str(outline, "closingText", "Thank you").to_string(),
        subtitle: String::new(),
        label: "FINAL SLIDE".to_string(),
        body: Vec::new(),
        notes: "Closing slide generated by the Rust ppt CLI.".to_string(),
        layout: "closing",
    });
    slides
}

fn comparison_body(slide: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for side in ["left", "right"] {
        let value = slide
            .get("comparison")
            .and_then(|comparison| comparison.get(side))
            .unwrap_or(&Value::Null);
        let title = outline_str(value, "title", side);
        let items = value_string_array(value, "items").join("; ");
        if !items.is_empty() {
            out.push(format!("{title}: {items}"));
        }
    }
    out
}

fn metrics_body(slide: &Value) -> Vec<String> {
    slide
        .get("metrics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|metric| {
            format!(
                "{} {}",
                outline_str(metric, "value", ""),
                outline_str(metric, "label", "")
            )
            .trim()
            .to_string()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

#[derive(Clone, Debug)]
struct PptPalette {
    stage: &'static str,
    panel: &'static str,
    panel_soft: &'static str,
    line: &'static str,
    glow: &'static str,
    text: &'static str,
    text_soft: &'static str,
    text_mute: &'static str,
}

fn ppt_palette(name: &str) -> PptPalette {
    match name {
        "light" => PptPalette {
            stage: "FAFAFA",
            panel: "FFFFFF",
            panel_soft: "F0F0F0",
            line: "E0E0E0",
            glow: "3B82F6",
            text: "1A1A1A",
            text_soft: "555555",
            text_mute: "777777",
        },
        "academic" => PptPalette {
            stage: "F5F3EF",
            panel: "FFFFFF",
            panel_soft: "EDE9E3",
            line: "D4CFC7",
            glow: "2563EB",
            text: "1F2937",
            text_soft: "4B5563",
            text_mute: "6B7280",
        },
        _ => PptPalette {
            stage: "000000",
            panel: "111111",
            panel_soft: "171717",
            line: "2A2A2A",
            glow: "7EA9FF",
            text: "F2F2EE",
            text_soft: "B9B9B2",
            text_mute: "888883",
        },
    }
}

fn value_array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn joined_array_chars(value: &Value, key: &str) -> usize {
    value_string_array(value, key).join("").chars().count()
}

fn value_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| item.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write_pptx_package(
    output: &Path,
    slides: &[PptxSlideSpec],
    palette: &PptPalette,
    title: &str,
) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file =
        File::create(output).with_context(|| format!("failed to create {}", output.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut write = |path: &str, content: String| -> Result<()> {
        zip.start_file(path, options)?;
        zip.write_all(content.as_bytes())?;
        Ok(())
    };

    write("[Content_Types].xml", content_types_xml(slides.len()))?;
    write("_rels/.rels", root_rels_xml())?;
    write("docProps/app.xml", app_xml(slides.len()))?;
    write("docProps/core.xml", core_xml(title))?;
    write("ppt/presentation.xml", presentation_xml(slides.len()))?;
    write(
        "ppt/_rels/presentation.xml.rels",
        presentation_rels_xml(slides.len()),
    )?;
    write("ppt/theme/theme1.xml", theme_xml(palette))?;
    write(
        "ppt/slideMasters/slideMaster1.xml",
        slide_master_xml(palette),
    )?;
    write(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        slide_master_rels_xml(),
    )?;
    write("ppt/slideLayouts/slideLayout1.xml", slide_layout_xml())?;
    write(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        slide_layout_rels_xml(),
    )?;
    write("ppt/notesMasters/notesMaster1.xml", notes_master_xml())?;
    write(
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        notes_master_rels_xml(),
    )?;

    for (idx, slide) in slides.iter().enumerate() {
        let slide_no = idx + 1;
        write(
            &format!("ppt/slides/slide{slide_no}.xml"),
            slide_xml(slide, slide_no, slides.len(), palette)?,
        )?;
        write(
            &format!("ppt/slides/_rels/slide{slide_no}.xml.rels"),
            slide_rels_xml(slide_no),
        )?;
        write(
            &format!("ppt/notesSlides/notesSlide{slide_no}.xml"),
            notes_slide_xml(slide, slide_no),
        )?;
        write(
            &format!("ppt/notesSlides/_rels/notesSlide{slide_no}.xml.rels"),
            notes_slide_rels_xml(slide_no),
        )?;
    }

    zip.finish()?;
    Ok(())
}

fn content_types_xml(slide_count: usize) -> String {
    let mut overrides = String::new();
    for idx in 1..=slide_count {
        overrides.push_str(&format!(r#"<Override PartName="/ppt/slides/slide{idx}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/><Override PartName="/ppt/notesSlides/notesSlide{idx}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>{overrides}</Types>"#
    )
}

fn root_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#.to_string()
}

fn app_xml(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ppt Rust CLI</Application><PresentationFormat>Widescreen</PresentationFormat><Slides>{slide_count}</Slides><Notes>{slide_count}</Notes></Properties>"#
    )
}

fn core_xml(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>ppt Rust CLI</dc:creator><cp:lastModifiedBy>ppt Rust CLI</cp:lastModifiedBy></cp:coreProperties>"#,
        xml_escape(title)
    )
}

fn presentation_xml(slide_count: usize) -> String {
    let slide_ids = (1..=slide_count)
        .map(|idx| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + idx, idx + 1))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id="rId{}"/></p:notesMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000" type="wide"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle/></p:presentation>"#,
        slide_count + 2
    )
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for idx in 1..=slide_count {
        rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{idx}.xml"/>"#, idx + 1));
    }
    rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/><Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#, slide_count + 2, slide_count + 3));
    rels
}

fn slide_rels_xml(slide_no: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide{slide_no}.xml"/></Relationships>"#
    )
}

fn slide_master_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#.to_string()
}

fn slide_layout_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#.to_string()
}

fn notes_master_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#.to_string()
}

fn notes_slide_rels_xml(slide_no: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide{slide_no}.xml"/></Relationships>"#
    )
}

fn theme_xml(palette: &PptPalette) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ppt Rust Theme"><a:themeElements><a:clrScheme name="RustDeck"><a:dk1><a:srgbClr val="{}"/></a:dk1><a:lt1><a:srgbClr val="{}"/></a:lt1><a:dk2><a:srgbClr val="{}"/></a:dk2><a:lt2><a:srgbClr val="{}"/></a:lt2><a:accent1><a:srgbClr val="{}"/></a:accent1><a:accent2><a:srgbClr val="{}"/></a:accent2><a:accent3><a:srgbClr val="{}"/></a:accent3><a:accent4><a:srgbClr val="{}"/></a:accent4><a:accent5><a:srgbClr val="{}"/></a:accent5><a:accent6><a:srgbClr val="{}"/></a:accent6><a:hlink><a:srgbClr val="{}"/></a:hlink><a:folHlink><a:srgbClr val="{}"/></a:folHlink></a:clrScheme><a:fontScheme name="RustDeckFonts"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface="Arial"/><a:cs typeface="Arial"/></a:majorFont><a:minorFont><a:latin typeface="Arial"/><a:ea typeface="Arial"/><a:cs typeface="Arial"/></a:minorFont></a:fontScheme><a:fmtScheme name="RustDeckFormat"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"/><a:gradFill rotWithShape="1"/></a:fillStyleLst><a:lnStyleLst><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="25400" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="38100" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle/><a:effectStyle/><a:effectStyle/></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#,
        palette.stage,
        palette.text,
        palette.panel,
        palette.panel_soft,
        palette.glow,
        palette.line,
        palette.text_soft,
        palette.text_mute,
        palette.glow,
        palette.panel_soft,
        palette.glow,
        palette.glow
    )
}

fn slide_master_xml(palette: &PptPalette) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#,
        palette.stage
    )
}

fn slide_layout_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Rust Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#.to_string()
}

fn notes_master_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/><p:notesStyle/></p:notesMaster>"#.to_string()
}

fn slide_xml(
    slide: &PptxSlideSpec,
    slide_no: usize,
    total: usize,
    palette: &PptPalette,
) -> Result<String> {
    let bg = rect_shape(
        2,
        "Background",
        0.0,
        0.0,
        13.333,
        7.5,
        palette.stage,
        None,
        0,
    );
    let rail = rect_shape(
        3,
        "Glow Rail",
        0.86,
        6.86,
        11.6,
        0.018,
        palette.glow,
        None,
        25000,
    );
    let panel = if slide.layout == "cover" || slide.layout == "closing" {
        rect_shape(
            4,
            "Hero Panel",
            0.72,
            0.72,
            5.8,
            6.0,
            palette.panel,
            Some(palette.line),
            16000,
        )
    } else {
        rect_shape(
            4,
            "Content Panel",
            0.86,
            1.62,
            11.6,
            4.82,
            palette.panel_soft,
            Some(palette.line),
            10000,
        )
    };
    let mut shapes = vec![bg, rail, panel];
    shapes.push(text_shape(
        10,
        "Slide Label",
        &slide.label,
        0.9,
        0.38,
        2.7,
        0.22,
        9,
        palette.text_mute,
        false,
        false,
    ));
    let title_box = if slide.layout == "cover" {
        (0.96, 2.02, 4.9, 1.1, 31)
    } else if slide.layout == "closing" {
        (3.2, 2.48, 6.9, 0.9, 34)
    } else {
        (0.92, 0.92, 6.3, 0.6, 24)
    };
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        title_box.0,
        title_box.1,
        title_box.2,
        title_box.3,
        title_box.4,
        palette.text,
        true,
        true,
    ));
    if !slide.subtitle.is_empty() {
        shapes.push(text_shape(
            12,
            "Subtitle",
            &slide.subtitle,
            0.96,
            if slide.layout == "cover" { 3.2 } else { 1.28 },
            6.7,
            0.38,
            13,
            palette.text_soft,
            false,
            false,
        ));
    }
    for (idx, item) in slide.body.iter().take(6).enumerate() {
        let y = if slide.layout == "cover" { 4.35 } else { 2.0 } + idx as f64 * 0.58;
        let text = if slide.layout == "cover" {
            item.clone()
        } else {
            format!("{}. {}", idx + 1, item)
        };
        shapes.push(text_shape(
            20 + idx as u32,
            "Body",
            &text,
            1.18,
            y,
            10.9,
            0.42,
            15,
            palette.text_soft,
            false,
            false,
        ));
    }
    shapes.push(text_shape(
        40,
        "Page",
        &format!("{slide_no:02} / {total:02}"),
        11.82,
        7.0,
        0.8,
        0.22,
        9,
        palette.text_mute,
        false,
        false,
    ));
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld name="{}"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
        xml_escape(&slide.title),
        shapes.join("")
    ))
}

fn notes_slide_xml(slide: &PptxSlideSpec, slide_no: usize) -> String {
    let notes = if slide.notes.is_empty() {
        format!("Slide {slide_no}: {}", slide.title)
    } else {
        slide.notes.clone()
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#,
        text_shape(2, "Notes", &notes, 0.7, 5.0, 5.5, 2.0, 12, "222222", false, false)
    )
}

fn rect_shape(
    id: u32,
    name: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    line: Option<&str>,
    transparency: u32,
) -> String {
    let alpha = 100000 - transparency.min(100000);
    let line_xml = line
        .map(|color| {
            format!(r#"<a:ln><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></a:ln>"#)
        })
        .unwrap_or_else(|| "<a:ln><a:noFill/></a:ln>".to_string());
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{fill}"><a:alpha val="{alpha}"/></a:srgbClr></a:solidFill>{line_xml}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>"#,
        inch_emu(x),
        inch_emu(y),
        inch_emu(w),
        inch_emu(h)
    )
}

fn text_shape(
    id: u32,
    name: &str,
    text: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    size_pt: u32,
    color: &str,
    bold: bool,
    title_placeholder: bool,
) -> String {
    let bold_attr = if bold { r#" b="1""# } else { "" };
    let placeholder = if title_placeholder {
        r#"<p:nvPr><p:ph type="title"/></p:nvPr>"#
    } else {
        "<p:nvPr/>"
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/>{placeholder}</p:nvSpPr><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square" anchor="t"/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="{}"{}><a:solidFill><a:srgbClr val="{color}"/></a:solidFill><a:latin typeface="Arial"/><a:ea typeface="Arial"/><a:cs typeface="Arial"/></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN" sz="{}"/></a:p></p:txBody></p:sp>"#,
        inch_emu(x),
        inch_emu(y),
        inch_emu(w),
        inch_emu(h),
        size_pt * 100,
        bold_attr,
        xml_escape(text),
        size_pt * 100
    )
}

fn inch_emu(value: f64) -> i64 {
    (value * EMU_PER_INCH).round() as i64
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn outline_str<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn qa_summary(deck_path: &str, rendered_dir: &str) -> Result<QaSummary> {
    let deck = expand_path(deck_path);
    let rendered_dir_path = expand_path(rendered_dir);
    let rendered = render_paths(&deck, &rendered_dir_path, 1600, 900)?;
    let overflow = slide_overflow_summary(&deck)?;
    let font_check = detect_fonts_payload(&deck)?;
    let inspector = office_doctor_value(&deck.display().to_string())?;
    let ok = overflow.ok && font_check_ok(&font_check) && inspector_ok(&inspector);
    Ok(QaSummary {
        ok,
        deck: deck.display().to_string(),
        render: QaRenderSummary {
            rendered_dir: rendered_dir_path.display().to_string(),
            png_count: rendered.len(),
            paths: rendered
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
        },
        overflow_check: overflow,
        font_check,
        inspector,
    })
}

fn strict_quality_gate(payload: &Value) -> Result<()> {
    if payload.pointer("/ok").and_then(Value::as_bool) == Some(false) {
        bail!("strict quality failed: combined QA status is false");
    }
    if payload
        .pointer("/overflow_check/ok")
        .and_then(Value::as_bool)
        == Some(false)
    {
        bail!("strict quality failed: slide overflow detected");
    }
    if payload.pointer("/font_check/ok").and_then(Value::as_bool) == Some(false) {
        bail!("strict quality failed: font check reported issues");
    }
    if payload
        .pointer("/inspector/validation/ok")
        .and_then(Value::as_bool)
        == Some(false)
    {
        bail!("strict quality failed: Rust inspector validation failed");
    }
    if payload
        .pointer("/inspector/issues/count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        bail!("strict quality failed: Rust inspector reported deck issues");
    }
    Ok(())
}

fn font_check_ok(payload: &Value) -> bool {
    payload
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            payload
                .get("font_missing_overall")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
                && payload
                    .get("font_substituted_overall")
                    .and_then(Value::as_array)
                    .is_none_or(Vec::is_empty)
        })
}

fn inspector_ok(payload: &Value) -> bool {
    payload
        .pointer("/validation/ok")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && payload
            .pointer("/issues/count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            == 0
}

fn render_paths(input: &Path, output_dir: &Path, width: u32, height: u32) -> Result<Vec<PathBuf>> {
    let dpi = if has_extension(input, "pdf") {
        calc_dpi_via_pdf(input, width, height)?
    } else {
        calc_dpi_via_ooxml(input, width, height)?
    };
    rasterize_to_pngs(input, output_dir, dpi)
}

fn slide_overflow_summary(input: &Path) -> Result<QaOverflowSummary> {
    let bundle = ZipBundle::from_path(input)?;
    let structure = extract_pptx_structure(&bundle, input, false, None)?;
    let slide_w = structure
        .get("slide_width")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_width"))?;
    let slide_h = structure
        .get("slide_height")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_height"))?;
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing slides"))?;
    let mut failing = Vec::new();
    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize + 1;
        let mut overflow = false;
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            overflow = elements
                .iter()
                .any(|item| element_overflows(item, slide_w, slide_h));
        }
        if overflow {
            failing.push(index);
        }
    }
    if failing.is_empty() {
        return Ok(QaOverflowSummary {
            ok: true,
            stdout: "Test passed. No overflow detected.".to_string(),
            stderr: String::new(),
        });
    }
    Ok(QaOverflowSummary {
        ok: false,
        stdout: format!(
            "ERROR: Slides with content overflowing original canvas (1-based indexing): {}",
            failing
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        stderr: String::new(),
    })
}

fn detect_fonts_payload(input: &Path) -> Result<Value> {
    let bundle = ZipBundle::from_path(input)?;
    let requested = extract_requested_fonts_by_slide(&bundle)?;
    let installed = build_font_synonym_map()?;
    let resolved = match extract_resolved_fonts_from_odp(input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("warning: resolved-font probe skipped: {err:#}");
            BTreeSet::new()
        }
    };

    let mut missing_overall = BTreeSet::new();
    let mut substituted_overall = BTreeSet::new();
    let mut missing_by_slide: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut substituted_by_slide: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (slide_no, families) in &requested {
        let mut slide_missing = BTreeSet::new();
        let mut slide_substituted = BTreeSet::new();
        for family in families {
            let normalized = normalize_font_family_name(family);
            if normalized.is_empty() {
                continue;
            }
            let acceptable = expand_font_family_aliases(&installed, &normalized);
            let is_installed = acceptable.iter().any(|alias| installed.contains_key(alias));
            if !is_installed {
                slide_missing.insert(family.clone());
                missing_overall.insert(family.clone());
                continue;
            }
            if !resolved.is_empty() && !acceptable.iter().any(|alias| resolved.contains(alias)) {
                slide_substituted.insert(family.clone());
                substituted_overall.insert(family.clone());
            }
        }
        if !slide_missing.is_empty() {
            missing_by_slide.insert(slide_no.to_string(), slide_missing.into_iter().collect());
        }
        if !slide_substituted.is_empty() {
            substituted_by_slide.insert(
                slide_no.to_string(),
                slide_substituted.into_iter().collect(),
            );
        }
    }

    let missing = missing_overall.into_iter().collect::<Vec<_>>();
    let substituted = substituted_overall.into_iter().collect::<Vec<_>>();
    Ok(json!({
        "ok": missing.is_empty() && substituted.is_empty(),
        "font_missing_overall": missing,
        "font_missing_by_slide": missing_by_slide,
        "font_substituted_overall": substituted,
        "font_substituted_by_slide": substituted_by_slide,
    }))
}

fn extract_structure_payload(input_path: &str) -> Result<Value> {
    let input = expand_path(input_path);
    let bundle = ZipBundle::from_path(&input)?;
    extract_pptx_structure(&bundle, &input, false, None)
}

fn office_doctor_value(file: &str) -> Result<Value> {
    Ok(serde_json::to_value(office_doctor_summary(file)?)?)
}

fn office_doctor_summary(file: &str) -> Result<OfficeDoctorSummary> {
    let outline_payload = rust_office_outline_value(file)?;
    let issues_payload = rust_office_issues_value(file)?;
    let validate_payload = rust_office_validate_value(file)?;
    summarize_office_doctor(
        file,
        outline_payload,
        issues_payload,
        validate_payload,
        Some(env!("CARGO_PKG_VERSION").to_string()),
    )
}

fn rust_office_outline_value(file: &str) -> Result<Value> {
    let structure = extract_structure_payload(file)?;
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|slide| {
            let title = first_slide_title(&slide).unwrap_or_else(|| "Untitled".to_string());
            let elements = slide
                .get("elements")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let text_boxes = elements
                .iter()
                .filter(|element| {
                    element
                        .get("text")
                        .and_then(|text| text.get("fullText"))
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
                })
                .count();
            let images = elements
                .iter()
                .filter(|element| element.get("image").is_some())
                .count();
            json!({
                "index": slide.get("index").and_then(Value::as_u64).unwrap_or(0) + 1,
                "title": title,
                "layout": slide.get("layout").cloned().unwrap_or(Value::Null),
                "elementCount": elements.len(),
                "textBoxCount": text_boxes,
                "imageCount": images,
                "notes": slide.get("notes").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "success": true,
        "data": {
            "engine": "rust-pptx-inspector",
            "totalSlides": structure.get("slide_count").cloned().unwrap_or(Value::Null),
            "slides": slides,
        }
    }))
}

fn rust_office_issues_value(file: &str) -> Result<Value> {
    let structure = extract_structure_payload(file)?;
    let slide_w = structure
        .get("slide_width")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_width"))?;
    let slide_h = structure
        .get("slide_height")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_height"))?;
    let mut issues = Vec::new();
    for slide in structure
        .get("slides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let slide_no = slide.get("index").and_then(Value::as_u64).unwrap_or(0) + 1;
        let elements = slide
            .get("elements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if first_slide_title(slide).is_none() {
            issues.push(json!({
                "Slide": slide_no,
                "Severity": "warning",
                "Message": "No title text found",
            }));
        }
        for element in &elements {
            if element_overflows(element, slide_w, slide_h) {
                issues.push(json!({
                    "Slide": slide_no,
                    "Shape": element.get("name").cloned().unwrap_or(Value::Null),
                    "Severity": "error",
                    "Message": "Shape overflow outside slide canvas",
                }));
            }
        }
    }
    Ok(json!({
        "success": true,
        "data": {
            "Engine": "rust-pptx-inspector",
            "Count": issues.len(),
            "Issues": issues,
        }
    }))
}

fn rust_office_validate_value(file: &str) -> Result<Value> {
    let input = expand_path(file);
    let bundle = ZipBundle::from_path(&input)?;
    let required = [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ];
    let mut errors = Vec::new();
    for path in required {
        if !bundle.files.contains_key(path) {
            errors.push(format!("missing {path}"));
        }
    }
    let structure = extract_pptx_structure(&bundle, &input, false, None)?;
    if structure
        .get("slide_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        errors.push("presentation contains no slides".to_string());
    }
    let ok = errors.is_empty();
    let message = if ok {
        "0 validation errors from Rust inspector".to_string()
    } else {
        format!("{} validation errors: {}", errors.len(), errors.join("; "))
    };
    Ok(json!({
        "success": ok,
        "message": message,
        "data": {
            "engine": "rust-pptx-inspector",
            "errors": errors,
        }
    }))
}

fn rust_office_get_value(file: &str, selector: &str, depth: i32) -> Result<Value> {
    let structure = extract_structure_payload(file)?;
    let selected = select_structure_path(&structure, selector)?;
    Ok(json!({
        "success": true,
        "selector": selector,
        "depth": depth,
        "data": trim_json_depth(selected, depth.max(0) as usize),
    }))
}

fn rust_office_query_value(file: &str, selector: &str, text: Option<&str>) -> Result<Value> {
    let structure = extract_structure_payload(file)?;
    let matches = query_structure(&structure, selector, text);
    Ok(json!({
        "success": true,
        "selector": selector,
        "text": text,
        "count": matches.len(),
        "data": matches,
    }))
}

fn write_rust_office_preview(file: &str, _port: u16) -> Result<PathBuf> {
    let input = expand_path(file);
    let structure = extract_structure_payload(file)?;
    let preview = input.with_extension("preview.html");
    let mut html = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>PPTX Preview</title><style>body{font-family:Arial,sans-serif;background:#111;color:#eee;margin:24px}.slide{border:1px solid #444;border-radius:12px;padding:18px;margin:0 0 16px;background:#1b1b1b}.meta{color:#aaa;font-size:12px}pre{white-space:pre-wrap}</style>",
    );
    for slide in structure
        .get("slides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let slide_no = slide.get("index").and_then(Value::as_u64).unwrap_or(0) + 1;
        html.push_str(&format!(
            "<section class=\"slide\"><div class=\"meta\">Slide {slide_no}</div><h2>{}</h2>",
            xml_escape(&first_slide_title(slide).unwrap_or_else(|| "Untitled".to_string()))
        ));
        for text in slide_texts(slide) {
            html.push_str(&format!("<pre>{}</pre>", xml_escape(&text)));
        }
        html.push_str("</section>");
    }
    fs::write(&preview, html)?;
    Ok(preview)
}

fn rust_office_batch_value(
    file: &str,
    input: Option<&str>,
    commands: Option<&str>,
    force: bool,
) -> Result<Value> {
    let source = if commands.is_some() {
        "inline --commands".to_string()
    } else if let Some(path) = input {
        fs::read_to_string(expand_path(path))
            .with_context(|| format!("failed to read batch input {}", path))?;
        format!("--input {}", path)
    } else {
        "no batch commands".to_string()
    };
    bail!(
        "ppt office batch is not supported by the read-only Rust inspector \
         (file: {file}, force: {force}, source: {source}); rebuild editable changes through deck.plan.json"
    )
}

fn first_slide_title(slide: &Value) -> Option<String> {
    slide_texts(slide)
        .into_iter()
        .map(|text| text.trim().to_string())
        .find(|text| !text.is_empty())
}

fn slide_texts(slide: &Value) -> Vec<String> {
    slide
        .get("elements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|element| {
            element
                .get("text")
                .and_then(|text| text.get("fullText"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|text| !text.trim().is_empty())
        .collect()
}

fn select_structure_path(root: &Value, selector: &str) -> Result<Value> {
    if selector == "/" {
        return Ok(root.clone());
    }
    let slide_re = Regex::new(r"^/slide\[(\d+)\]$")?;
    if let Some(caps) = slide_re.captures(selector) {
        let index = caps[1].parse::<usize>()?.saturating_sub(1);
        return root
            .get("slides")
            .and_then(Value::as_array)
            .and_then(|slides| slides.get(index))
            .cloned()
            .ok_or_else(|| anyhow!("slide selector out of range: {selector}"));
    }
    let shape_re = Regex::new(r"^/slide\[(\d+)\]/shape\[(\d+)\]$")?;
    if let Some(caps) = shape_re.captures(selector) {
        let slide_index = caps[1].parse::<usize>()?.saturating_sub(1);
        let shape_index = caps[2].parse::<usize>()?.saturating_sub(1);
        return root
            .get("slides")
            .and_then(Value::as_array)
            .and_then(|slides| slides.get(slide_index))
            .and_then(|slide| slide.get("elements"))
            .and_then(Value::as_array)
            .and_then(|elements| elements.get(shape_index))
            .cloned()
            .ok_or_else(|| anyhow!("shape selector out of range: {selector}"));
    }
    bail!("unsupported selector: {selector}. Use /, /slide[N], or /slide[N]/shape[N].")
}

fn trim_json_depth(value: Value, depth: usize) -> Value {
    if depth == 0 {
        return match value {
            Value::Array(items) => json!({"type": "array", "len": items.len()}),
            Value::Object(map) => json!({"type": "object", "keys": map.keys().collect::<Vec<_>>()}),
            other => other,
        };
    }
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| trim_json_depth(item, depth - 1))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, trim_json_depth(value, depth - 1)))
                .collect(),
        ),
        other => other,
    }
}

fn query_structure(structure: &Value, selector: &str, text: Option<&str>) -> Vec<Value> {
    let needle = text.map(|value| value.to_lowercase());
    let font_filter = selector
        .strip_prefix("shape[font=")
        .and_then(|rest| rest.strip_suffix(']'))
        .map(|font| font.trim_matches('"').trim_matches('\'').to_lowercase());
    let wants_shape = selector == "shape" || selector.starts_with("shape[");
    if !wants_shape {
        return Vec::new();
    }
    let mut out = Vec::new();
    for slide in structure
        .get("slides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let slide_no = slide.get("index").and_then(Value::as_u64).unwrap_or(0) + 1;
        for element in slide
            .get("elements")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let full_text = element
                .get("text")
                .and_then(|text| text.get("fullText"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(needle) = &needle {
                if !full_text.to_lowercase().contains(needle) {
                    continue;
                }
            }
            if let Some(font) = &font_filter {
                let shape_text = serde_json::to_string(element)
                    .unwrap_or_default()
                    .to_lowercase();
                if !shape_text.contains(font) {
                    continue;
                }
            }
            let mut cloned = element.clone();
            if let Some(object) = cloned.as_object_mut() {
                object.insert("slide".to_string(), json!(slide_no));
            }
            out.push(cloned);
        }
    }
    out
}

fn summarize_office_doctor(
    file: &str,
    outline_payload: Value,
    issues_payload: Value,
    validate_payload: Value,
    version: Option<String>,
) -> Result<OfficeDoctorSummary> {
    let outline_data = outline_payload
        .get("data")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let issues_data = issues_payload
        .get("data")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let issue_list = issues_data
        .get("Issues")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let validate_message = validate_payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let validation_ok = validate_message
        .to_lowercase()
        .contains("0 validation error")
        || (validate_payload.get("success").and_then(Value::as_bool) == Some(true)
            && !validate_message.to_lowercase().contains("validation error"));
    let overflow_count = issue_list
        .iter()
        .filter(|item| {
            item.get("Message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase()
                .contains("overflow")
        })
        .count();
    let title_count = issue_list
        .iter()
        .filter(|item| {
            item.get("Message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase()
                .contains("no title")
        })
        .count();
    Ok(OfficeDoctorSummary {
        inspector_version: version,
        file: file.to_string(),
        outline: json!({
            "total_slides": outline_data.get("totalSlides").cloned().unwrap_or(Value::Null),
            "slides": outline_data.get("slides").cloned().unwrap_or_else(|| json!([])),
        }),
        issues: json!({
            "count": issues_data
                .get("Count")
                .and_then(Value::as_u64)
                .unwrap_or(issue_list.len() as u64),
            "overflow_count": overflow_count,
            "title_count": title_count,
            "items": issue_list,
        }),
        validation: json!({
            "ok": validation_ok,
            "message": validate_message,
        }),
    })
}

fn print_office_doctor_summary(summary: &OfficeDoctorSummary) {
    println!(
        "inspector: {}",
        summary
            .inspector_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("file: {}", summary.file);
    println!(
        "slides: {}",
        summary
            .outline
            .get("total_slides")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );
    println!(
        "issues: total={} overflow={} missing_title={}",
        summary
            .issues
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .issues
            .get("overflow_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .issues
            .get("title_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );
    println!(
        "validation_ok: {}",
        summary
            .validation
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    );
    if let Some(message) = summary.validation.get("message").and_then(Value::as_str) {
        if !message.is_empty() {
            println!("validation_message: {}", message);
        }
    }
}

fn emit_value(value: Value, format: EmitFormat) -> Result<()> {
    match format {
        EmitFormat::Json => println!("{}", serde_json::to_string_pretty(&value)?),
        EmitFormat::Text => print_text_value(&value)?,
    }
    Ok(())
}

fn print_text_value(value: &Value) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                println!("{}: {}", key, text_value(item)?);
            }
        }
        Value::Array(items) => {
            for item in items {
                println!("{}", text_value(item)?);
            }
        }
        other => println!("{}", text_value(other)?),
    }
    Ok(())
}

fn text_value(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)?,
    })
}

fn extract_structure_command(args: ExtractStructureArgs) -> Result<()> {
    let input = expand_path(&args.input);
    let bundle = ZipBundle::from_path(&input)?;
    let structure = extract_pptx_structure(
        &bundle,
        &input,
        args.extract_images,
        if args.extract_images {
            Some(expand_path(&args.image_dir))
        } else {
            None
        },
    )?;
    let json_str = if args.pretty {
        serde_json::to_string_pretty(&structure)?
    } else {
        serde_json::to_string(&structure)?
    };
    if let Some(output) = args.output {
        fs::write(expand_path(&output), json_str)?;
        eprintln!("Structure extracted to {}", output);
    } else {
        println!("{}", json_str);
    }
    Ok(())
}

fn ensure_raster_image_command(args: EnsureRasterImageArgs) -> Result<()> {
    let paths = resolve_input_paths(&args.input_files, args.input_dir.as_deref())?;
    let out_dir = args.output_dir.as_deref().map(expand_path);
    let mut converted = Vec::new();
    for path in &paths {
        let output = ensure_raster_image(path, out_dir.as_deref())?;
        if output != *path {
            converted.push(path.display().to_string());
        }
    }
    if !converted.is_empty() {
        println!("Converted the following files to PNG:");
        for item in converted {
            println!("{}", item);
        }
    }
    Ok(())
}

fn create_montage_command(args: CreateMontageArgs) -> Result<()> {
    let inputs = resolve_input_paths(&args.input_files, args.input_dir.as_deref())?;
    if inputs.is_empty() {
        bail!("No input images found");
    }
    let output = expand_path(&args.output_file);
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temp_dir = if args.retain_converted_files {
        None
    } else {
        Some(TempDir::new().context("failed to create temp dir for montage conversions")?)
    };
    let converted_root = temp_dir.as_ref().map(|dir| dir.path().to_path_buf());
    let mut items = Vec::new();
    for input in &inputs {
        match ensure_raster_image(input, converted_root.as_deref()) {
            Ok(raster_path) => match image::open(&raster_path) {
                Ok(img) => items.push((input.clone(), Some(img))),
                Err(err) if args.fail_on_image_error => {
                    return Err(err)
                        .with_context(|| format!("failed to open {}", raster_path.display()))
                }
                Err(_) => items.push((input.clone(), None)),
            },
            Err(err) if args.fail_on_image_error => return Err(err),
            Err(_) => items.push((input.clone(), None)),
        }
    }
    let montage = build_montage(
        &items,
        args.num_col,
        args.cell_width,
        args.cell_height,
        args.gap,
        args.label_mode,
    )?;
    montage.save(&output)?;
    println!("Montage saved to {}", output.display());
    Ok(())
}

fn slides_test_command(args: SlidesTestArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    let bundle = ZipBundle::from_path(&input)?;
    let structure = extract_pptx_structure(&bundle, &input, false, None)?;
    let slide_w = structure
        .get("slide_width")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_width"))?;
    let slide_h = structure
        .get("slide_height")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_height"))?;
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing slides"))?;
    let mut failing = Vec::new();
    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize + 1;
        let mut overflow = false;
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            overflow = elements
                .iter()
                .any(|item| element_overflows(item, slide_w, slide_h));
        }
        if overflow {
            failing.push(index);
        }
    }
    if failing.is_empty() {
        println!("Test passed. No overflow detected.");
        return Ok(());
    }
    print!("ERROR: Slides with content overflowing original canvas (1-based indexing): ");
    for (i, slide_no) in failing.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("{}", slide_no);
    }
    println!();
    if args.fail_on_overflow {
        bail!("slides-test failed: content overflow detected");
    }
    Ok(())
}

fn detect_fonts_command(args: DetectFontsArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    let bundle = ZipBundle::from_path(&input)?;
    let requested = extract_requested_fonts_by_slide(&bundle)?;
    let installed = build_font_synonym_map()?;
    let resolved = match extract_resolved_fonts_from_odp(&input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("warning: resolved-font probe skipped: {err:#}");
            BTreeSet::new()
        }
    };

    let mut missing_overall = BTreeSet::new();
    let mut substituted_overall = BTreeSet::new();
    let mut missing_by_slide: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut substituted_by_slide: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (slide_no, families) in &requested {
        let mut slide_missing = BTreeSet::new();
        let mut slide_substituted = BTreeSet::new();
        for family in families {
            let normalized = normalize_font_family_name(family);
            if normalized.is_empty() {
                continue;
            }
            let acceptable = expand_font_family_aliases(&installed, &normalized);
            let is_installed = acceptable.iter().any(|alias| installed.contains_key(alias));
            if !is_installed {
                slide_missing.insert(family.clone());
                missing_overall.insert(family.clone());
                continue;
            }
            if !resolved.is_empty() && !acceptable.iter().any(|alias| resolved.contains(alias)) {
                slide_substituted.insert(family.clone());
                substituted_overall.insert(family.clone());
            }
        }
        if !slide_missing.is_empty() {
            missing_by_slide.insert(slide_no.to_string(), slide_missing.into_iter().collect());
        }
        if !slide_substituted.is_empty() {
            substituted_by_slide.insert(
                slide_no.to_string(),
                slide_substituted.into_iter().collect(),
            );
        }
    }

    let payload = json!({
        "ok": missing_overall.is_empty() && substituted_overall.is_empty(),
        "font_missing_overall": missing_overall.into_iter().collect::<Vec<_>>(),
        "font_missing_by_slide": missing_by_slide,
        "font_substituted_overall": substituted_overall.into_iter().collect::<Vec<_>>(),
        "font_substituted_by_slide": substituted_by_slide,
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        if args.include_missing {
            println!(
                "font_missing_overall: {}",
                join_display_list(payload["font_missing_overall"].as_array())
            );
            println!(
                "font_missing_by_slide: {}",
                serde_json::to_string(&payload["font_missing_by_slide"])?
            );
        }
        if args.include_substituted {
            println!(
                "font_substituted_overall: {}",
                join_display_list(payload["font_substituted_overall"].as_array())
            );
            println!(
                "font_substituted_by_slide: {}",
                serde_json::to_string(&payload["font_substituted_by_slide"])?
            );
        }
    }
    Ok(())
}

fn sanitize_pptx_command(args: SanitizePptxArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    let output = args
        .output
        .as_deref()
        .map(expand_path)
        .unwrap_or_else(|| input.clone());
    let temp_output = if output == input {
        input.with_extension("sanitized.tmp.pptx")
    } else {
        output.clone()
    };

    let file = File::open(&input).with_context(|| format!("failed to open {}", input.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read zip archive")?;
    let writer = File::create(&temp_output)
        .with_context(|| format!("failed to create {}", temp_output.display()))?;
    let mut zip = ZipWriter::new(writer);

    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx)?;
        let name = entry.name().to_string();
        let options = SimpleFileOptions::default().compression_method(entry.compression());

        if entry.is_dir() {
            zip.add_directory(name, options)?;
            continue;
        }

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let data = if name == "ppt/presentation.xml" {
            sanitize_presentation_xml(std::str::from_utf8(&buf)?)?.into_bytes()
        } else {
            buf
        };
        zip.start_file(name, options)?;
        zip.write_all(&data)?;
    }

    zip.finish()?;
    if output == input {
        fs::rename(&temp_output, &input).with_context(|| {
            format!(
                "failed to replace {} with sanitized output",
                input.display()
            )
        })?;
    }
    Ok(())
}

fn expand_path(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(input)
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|value| value.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

fn default_render_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("rendered");
    input.parent().unwrap_or_else(|| Path::new(".")).join(stem)
}

impl ZipBundle {
    fn from_path(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let mut archive = ZipArchive::new(file).context("failed to read zip archive")?;
        let mut files = HashMap::new();
        for idx in 0..archive.len() {
            let mut entry = archive.by_index(idx)?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            files.insert(normalize_zip_path(entry.name()), buf);
        }
        Ok(Self { files })
    }

    fn text(&self, path: &str) -> Result<String> {
        let data = self
            .files
            .get(&normalize_zip_path(path))
            .ok_or_else(|| anyhow!("missing zip entry {}", path))?;
        Ok(String::from_utf8(data.clone())
            .with_context(|| format!("{} is not valid utf-8 xml", path))?)
    }

    fn bytes(&self, path: &str) -> Option<&[u8]> {
        self.files
            .get(&normalize_zip_path(path))
            .map(|value| value.as_slice())
    }

    fn names(&self) -> impl Iterator<Item = &String> {
        self.files.keys()
    }
}

fn normalize_zip_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_path_like_zip(path: &Path) -> String {
    let mut parts = Vec::<String>::new();
    for component in path.components() {
        let part = component.as_os_str().to_string_lossy();
        match part.as_ref() {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_string()),
        }
    }
    parts.join("/")
}

fn calc_dpi_via_ooxml(input: &Path, max_w_px: u32, max_h_px: u32) -> Result<u32> {
    let bundle = ZipBundle::from_path(input)?;
    let xml = bundle.text("ppt/presentation.xml")?;
    let doc = Document::parse(&xml)?;
    let sld_sz = doc
        .descendants()
        .find(|node| node.tag_name().name() == "sldSz")
        .ok_or_else(|| anyhow!("Slide size not found in presentation.xml"))?;
    let cx = attr_f64(&sld_sz, "cx").ok_or_else(|| anyhow!("missing slide width"))?;
    let cy = attr_f64(&sld_sz, "cy").ok_or_else(|| anyhow!("missing slide height"))?;
    let width_in = cx / EMU_PER_INCH;
    let height_in = cy / EMU_PER_INCH;
    if width_in <= 0.0 || height_in <= 0.0 {
        bail!("Invalid slide size values in presentation.xml");
    }
    Ok(((max_w_px as f64 / width_in).min(max_h_px as f64 / height_in)).round() as u32)
}

fn calc_dpi_via_pdf(input: &Path, max_w_px: u32, max_h_px: u32) -> Result<u32> {
    let output = run_command_capture(
        Command::new("pdfinfo")
            .arg(input)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .context("pdfinfo failed")?;
    let page_size = output
        .lines()
        .find_map(|line| line.strip_prefix("Page size:"))
        .map(str::trim)
        .ok_or_else(|| anyhow!("failed to read PDF page size"))?;
    let (w_pts, h_pts) = parse_pdf_page_size(page_size)?;
    let width_in = w_pts / POINTS_PER_INCH;
    let height_in = h_pts / POINTS_PER_INCH;
    if width_in <= 0.0 || height_in <= 0.0 {
        bail!("Invalid PDF page size values");
    }
    Ok(((max_w_px as f64 / width_in).min(max_h_px as f64 / height_in)).round() as u32)
}

fn parse_pdf_page_size(value: &str) -> Result<(f64, f64)> {
    let pts = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)\s*pts\b")?;
    if let Some(caps) = pts.captures(value) {
        return Ok((caps[1].parse()?, caps[2].parse()?));
    }
    let inch = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)\s*in\b")?;
    if let Some(caps) = inch.captures(value) {
        return Ok((
            caps[1].parse::<f64>()? * POINTS_PER_INCH,
            caps[2].parse::<f64>()? * POINTS_PER_INCH,
        ));
    }
    let bare = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)")?;
    if let Some(caps) = bare.captures(value) {
        return Ok((caps[1].parse()?, caps[2].parse()?));
    }
    bail!("Unrecognized PDF page size format: {}", value);
}

fn rasterize_to_pngs(input: &Path, out_dir: &Path, dpi: u32) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(out_dir)?;
    let temp_profile = TempDir::new().context("failed to create soffice profile")?;
    let temp_convert = TempDir::new().context("failed to create convert dir")?;
    let pdf_path = if has_extension(input, "pdf") {
        input.to_path_buf()
    } else {
        convert_to_pdf(input, temp_profile.path(), temp_convert.path())?
    };
    let prefix = out_dir.join("slide");
    run_command(
        Command::new("pdftoppm")
            .arg("-r")
            .arg(dpi.to_string())
            .arg("-png")
            .arg(&pdf_path)
            .arg(&prefix),
    )
    .context("pdftoppm failed")?;
    let mut generated = collect_prefixed_pngs(out_dir, "slide")?;
    generated.sort();
    let mut final_paths = Vec::new();
    for (index, src) in generated.iter().enumerate() {
        let dest = out_dir.join(format!("slide-{}.png", index + 1));
        if *src != dest {
            fs::rename(src, &dest)?;
        }
        final_paths.push(dest);
    }
    Ok(final_paths)
}

fn collect_prefixed_pngs(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(OsStr::to_str) != Some("png") {
            continue;
        }
        let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if file_name.starts_with(prefix) {
            files.push(path);
        }
    }
    Ok(files)
}

fn convert_to_pdf(input: &Path, profile_dir: &Path, convert_dir: &Path) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("invalid input stem"))?;
    let pdf_path = convert_dir.join(format!("{}.pdf", stem));
    let profile = format!("file://{}", profile_dir.display());
    let mut direct = Command::new("soffice");
    direct
        .arg(format!("-env:UserInstallation={}", profile))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(convert_dir)
        .arg(input)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = run_command(&mut direct);
    if pdf_path.exists() {
        return Ok(pdf_path);
    }
    let odp_path = convert_dir.join(format!("{}.odp", stem));
    let mut to_odp = Command::new("soffice");
    to_odp
        .arg(format!("-env:UserInstallation={}", profile))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("odp")
        .arg("--outdir")
        .arg(convert_dir)
        .arg(input)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = run_command(&mut to_odp);
    if !odp_path.exists() {
        bail!("Failed to convert {} to ODP", input.display());
    }
    let mut odp_to_pdf = Command::new("soffice");
    odp_to_pdf
        .arg(format!("-env:UserInstallation={}", profile))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(convert_dir)
        .arg(&odp_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = run_command(&mut odp_to_pdf);
    if pdf_path.exists() {
        return Ok(pdf_path);
    }
    bail!("Failed to produce PDF for {}", input.display())
}

fn run_command(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        bail!("command failed with status {:?}", status.code());
    }
    Ok(())
}

fn run_command_timeout(command: &mut Command, timeout: Duration) -> Result<()> {
    let mut child = command.spawn()?;
    let started_at = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("command failed with status {:?}", status.code());
            }
            return Ok(());
        }
        if started_at.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("command timed out after {} seconds", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn run_command_capture(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn resolve_input_paths(input_files: &[String], input_dir: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !input_files.is_empty() {
        for item in input_files {
            paths.push(expand_path(item));
        }
        return Ok(paths);
    }
    let dir = input_dir.ok_or_else(|| anyhow!("provide --input-files or --input-dir"))?;
    let root = expand_path(dir);
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if path.is_file() && supported_image_extension(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        bail!("No files with supported extensions in input_dir");
    }
    Ok(paths)
}

fn supported_image_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str).map(|value| value.to_ascii_lowercase()),
        Some(ext)
            if matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tif" | "tiff" | "webp" | "emf"
                    | "wmf" | "emz" | "wmz" | "svg" | "svgz" | "wdp" | "jxr" | "heic"
                    | "heif" | "pdf" | "eps" | "ps"
            )
    )
}

fn ensure_raster_image(path: &Path, out_dir: Option<&Path>) -> Result<PathBuf> {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let out_dir = out_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });
    fs::create_dir_all(&out_dir)?;
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("image");
    let out_path = out_dir.join(format!("{}.png", stem));
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tif" | "tiff" | "webp" => Ok(path.to_path_buf()),
        "emf" | "wmf" | "svg" | "svgz" => {
            run_command(Command::new("inkscape").arg(path).arg("-o").arg(&out_path))
                .context("inkscape conversion failed")?;
            Ok(out_path)
        }
        "emz" | "wmz" => {
            let decompressed = out_dir.join(format!(
                "{}.{}",
                stem,
                if ext == "emz" { "emf" } else { "wmf" }
            ));
            let bytes = fs::read(path)?;
            let mut decoder = flate_like_gzip_decoder(&bytes)?;
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf)?;
            fs::write(&decompressed, buf)?;
            run_command(
                Command::new("inkscape")
                    .arg(&decompressed)
                    .arg("-o")
                    .arg(&out_path),
            )
            .context("inkscape conversion failed")?;
            Ok(out_path)
        }
        "wdp" | "jxr" => {
            let tiff_path = out_dir.join(format!("{}.tiff", stem));
            run_command(
                Command::new("JxrDecApp")
                    .arg("-i")
                    .arg(path)
                    .arg("-o")
                    .arg(&tiff_path),
            )
            .context("JxrDecApp failed")?;
            let binary = if which("magick") { "magick" } else { "convert" };
            run_command(Command::new(binary).arg(&tiff_path).arg(&out_path))
                .context("imagemagick conversion failed")?;
            Ok(out_path)
        }
        "heic" | "heif" => {
            let binary = if which("heif-convert") {
                "heif-convert"
            } else {
                bail!("heif-convert not found");
            };
            run_command(Command::new(binary).arg(path).arg(&out_path))
                .context("heif-convert failed")?;
            Ok(out_path)
        }
        "pdf" | "eps" | "ps" => {
            run_command(
                Command::new("gs")
                    .arg("-dSAFER")
                    .arg("-dBATCH")
                    .arg("-dNOPAUSE")
                    .arg("-sDEVICE=pngalpha")
                    .arg("-dFirstPage=1")
                    .arg("-dLastPage=1")
                    .arg("-r200")
                    .arg("-o")
                    .arg(&out_path)
                    .arg(path),
            )
            .context("ghostscript failed")?;
            Ok(out_path)
        }
        _ => bail!("Unsupported image format for montage: {}", path.display()),
    }
}

fn which(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn flate_like_gzip_decoder<'a>(bytes: &'a [u8]) -> Result<Box<dyn Read + 'a>> {
    let cursor = Cursor::new(bytes);
    Ok(Box::new(flate2::read::GzDecoder::new(cursor)))
}

fn build_montage(
    items: &[(PathBuf, Option<DynamicImage>)],
    num_col: usize,
    cell_width: u32,
    cell_height: u32,
    gap: u32,
    label_mode: LabelMode,
) -> Result<RgbaImage> {
    if num_col == 0 {
        bail!("num_col must be positive");
    }
    if items.is_empty() {
        bail!("No valid images to render.");
    }
    let label_height = if matches!(label_mode, LabelMode::None) {
        0
    } else {
        20
    };
    let row_height = cell_height + label_height;
    let rows = (items.len() + num_col - 1) / num_col;
    let canvas_w = num_col as u32 * cell_width + (num_col as u32 + 1) * gap;
    let canvas_h = rows as u32 * row_height + (rows as u32 + 1) * gap;
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([242, 242, 242, 255]));
    let placeholder = make_placeholder(
        (cell_width as f32 * 0.6) as u32,
        (cell_height as f32 * 0.6) as u32,
    );

    for (index, (path, image_opt)) in items.iter().enumerate() {
        let col = index % num_col;
        let row = index / num_col;
        let x0 = gap + col as u32 * (cell_width + gap);
        let y0 = gap + row as u32 * (row_height + gap);
        let rendered = image_opt
            .as_ref()
            .map(|img| resize_to_fit(img, cell_width, cell_height))
            .unwrap_or_else(|| placeholder.clone());
        let paste_x = x0 + (cell_width - rendered.width()) / 2;
        let paste_y = y0 + (cell_height - rendered.height()) / 2;
        imageops::overlay(&mut canvas, &rendered, paste_x.into(), paste_y.into());
        draw_rect_outline(
            &mut canvas,
            paste_x.saturating_sub(1),
            paste_y.saturating_sub(1),
            rendered.width() + 1,
            rendered.height() + 1,
            Rgba([160, 160, 160, 255]),
        );
        let label = match label_mode {
            LabelMode::Number => Some((index + 1).to_string()),
            LabelMode::Filename => path
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s.to_string()),
            LabelMode::None => None,
        };
        if let Some(label) = label {
            draw_text_bitmap(
                &mut canvas,
                x0 + 4,
                y0 + cell_height + 4,
                &label,
                Rgba([0, 0, 0, 255]),
            );
        }
    }
    Ok(canvas)
}

fn resize_to_fit(img: &DynamicImage, max_w: u32, max_h: u32) -> RgbaImage {
    let resized = img.resize(max_w, max_h, FilterType::Lanczos3);
    resized.to_rgba8()
}

fn make_placeholder(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width.max(1), height.max(1), Rgba([220, 220, 220, 255]));
    let red = Rgba([180, 0, 0, 255]);
    let max_x = img.width().saturating_sub(1);
    let max_y = img.height().saturating_sub(1);
    let diag = max_x.min(max_y);
    for i in 0..=diag {
        img.put_pixel(i, i, red);
        img.put_pixel(max_x.saturating_sub(i), i, red);
    }
    img
}

fn draw_rect_outline(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    if w == 0 || h == 0 {
        return;
    }
    let x2 = x
        .saturating_add(w.saturating_sub(1))
        .min(img.width().saturating_sub(1));
    let y2 = y
        .saturating_add(h.saturating_sub(1))
        .min(img.height().saturating_sub(1));
    for xx in x..=x2 {
        img.put_pixel(xx, y, color);
        img.put_pixel(xx, y2, color);
    }
    for yy in y..=y2 {
        img.put_pixel(x, yy, color);
        img.put_pixel(x2, yy, color);
    }
}

fn draw_text_bitmap(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = x;
            continue;
        }
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if (bits >> col) & 1 == 1 {
                        let px = cursor_x + (7 - col as u32);
                        let py = y + row as u32;
                        if px < img.width() && py < img.height() {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
            cursor_x += 8;
        } else {
            cursor_x += 8;
        }
    }
}

fn extract_pptx_structure(
    bundle: &ZipBundle,
    input: &Path,
    extract_images: bool,
    image_dir: Option<PathBuf>,
) -> Result<Value> {
    if let Some(dir) = &image_dir {
        if extract_images {
            fs::create_dir_all(dir)?;
        }
    }
    let presentation_xml = bundle.text("ppt/presentation.xml")?;
    let presentation_doc = Document::parse(&presentation_xml)?;
    let (slide_width, slide_height) = presentation_doc
        .descendants()
        .find(|node| node.tag_name().name() == "sldSz")
        .map(|node| {
            (
                attr_f64(&node, "cx").unwrap_or_default() / EMU_PER_INCH,
                attr_f64(&node, "cy").unwrap_or_default() / EMU_PER_INCH,
            )
        })
        .ok_or_else(|| anyhow!("missing slide size in presentation.xml"))?;

    let presentation_rels = parse_relationships(&bundle.text("ppt/_rels/presentation.xml.rels")?)?;
    let slide_refs = presentation_doc
        .descendants()
        .filter(|node| node.tag_name().name() == "sldId")
        .filter_map(|node| rel_attr_value(&node, "id").map(str::to_string))
        .collect::<Vec<_>>();

    let mut slides = Vec::new();
    for (idx, rel_id) in slide_refs.iter().enumerate() {
        let rel_target = presentation_rels
            .get(rel_id)
            .ok_or_else(|| anyhow!("missing relationship {} in presentation rels", rel_id))?;
        let slide_path = normalize_zip_path(&format!("ppt/{}", rel_target));
        let slide_xml = bundle.text(&slide_path)?;
        let slide_doc = Document::parse(&slide_xml)?;
        let rel_path = slide_rel_path(&slide_path);
        let slide_rels = bundle
            .bytes(&rel_path)
            .map(|bytes| String::from_utf8(bytes.to_vec()))
            .transpose()?
            .map(|text| parse_relationships(&text))
            .transpose()?
            .unwrap_or_default();
        let layout_name = slide_rels
            .iter()
            .find(|(_, target)| target.contains("slideLayouts"))
            .and_then(|(_, target)| extract_layout_name(bundle, target).ok());
        let notes = slide_rels
            .iter()
            .find(|(_, target)| target.contains("notesSlides"))
            .and_then(|(_, target)| extract_notes(bundle, target).ok())
            .filter(|text| !text.trim().is_empty());
        let elements = extract_slide_elements(
            bundle,
            &slide_doc,
            &slide_rels,
            idx,
            extract_images,
            image_dir.as_deref(),
        )?;
        slides.push(json!({
            "index": idx,
            "layout": layout_name,
            "elements": elements,
            "notes": notes,
        }));
    }

    let available_layouts = bundle
        .names()
        .filter(|name| name.starts_with("ppt/slideLayouts/slideLayout") && name.ends_with(".xml"))
        .filter_map(|name| extract_layout_info(bundle, name).ok())
        .collect::<Vec<_>>();

    Ok(json!({
        "file": input.file_name().and_then(OsStr::to_str).unwrap_or_default(),
        "slide_width": round4(slide_width),
        "slide_height": round4(slide_height),
        "slide_count": slides.len(),
        "slides": slides,
        "available_layouts": available_layouts,
    }))
}

fn slide_rel_path(slide_path: &str) -> String {
    let path = Path::new(slide_path);
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("slide1.xml");
    let parent = path.parent().unwrap_or_else(|| Path::new("ppt/slides"));
    normalize_zip_path(
        &parent
            .join("_rels")
            .join(format!("{}.rels", file_name))
            .display()
            .to_string(),
    )
}

fn parse_relationships(xml: &str) -> Result<HashMap<String, String>> {
    let doc = Document::parse(xml)?;
    let mut rels = HashMap::new();
    for node in doc
        .descendants()
        .filter(|node| node.tag_name().name() == "Relationship")
    {
        if let (Some(id), Some(target)) = (attr_value(&node, "Id"), attr_value(&node, "Target")) {
            rels.insert(id.to_string(), target.to_string());
        }
    }
    Ok(rels)
}

fn resolve_target(base: &str, target: &str) -> String {
    let base_path = Path::new(base);
    let joined = if target.starts_with("../") || target.starts_with("./") {
        base_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    } else if target.starts_with('/') {
        PathBuf::from(target.trim_start_matches('/'))
    } else {
        base_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    normalize_path_like_zip(&joined)
}

fn extract_layout_name(bundle: &ZipBundle, rel_target: &str) -> Result<String> {
    let path = resolve_target("ppt/slides/slide.xml", rel_target);
    let xml = bundle.text(&path)?;
    let doc = Document::parse(&xml)?;
    let name = doc
        .descendants()
        .find(|node| node.tag_name().name() == "cSld")
        .and_then(|node| attr_value(&node, "name").map(str::to_string))
        .unwrap_or_else(|| "Unknown".to_string());
    Ok(name)
}

fn extract_layout_info(bundle: &ZipBundle, path: &str) -> Result<LayoutInfo> {
    let xml = bundle.text(path)?;
    let doc = Document::parse(&xml)?;
    let name = doc
        .descendants()
        .find(|node| node.tag_name().name() == "cSld")
        .and_then(|node| attr_value(&node, "name").map(str::to_string))
        .unwrap_or_else(|| "Unknown".to_string());
    let placeholders = doc
        .descendants()
        .filter(|node| node.tag_name().name() == "ph")
        .map(|node| LayoutPlaceholder {
            idx: attr_value(&node, "idx").map(str::to_string),
            name: node
                .ancestors()
                .find(|ancestor| ancestor.tag_name().name() == "sp")
                .and_then(|shape| {
                    shape
                        .children()
                        .find(|child| child.tag_name().name() == "nvSpPr")
                })
                .and_then(|nv| {
                    nv.descendants()
                        .find(|child| child.tag_name().name() == "cNvPr")
                })
                .and_then(|nv| attr_value(&nv, "name").map(str::to_string))
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    Ok(LayoutInfo { name, placeholders })
}

fn extract_notes(bundle: &ZipBundle, rel_target: &str) -> Result<String> {
    let path = resolve_target("ppt/slides/slide.xml", rel_target);
    let xml = bundle.text(&path)?;
    let doc = Document::parse(&xml)?;
    Ok(collect_text(&doc.root_element()))
}

fn extract_slide_elements(
    bundle: &ZipBundle,
    slide_doc: &Document<'_>,
    slide_rels: &HashMap<String, String>,
    slide_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<Vec<ElementInfo>> {
    let sp_tree = slide_doc
        .descendants()
        .find(|node| node.tag_name().name() == "spTree")
        .ok_or_else(|| anyhow!("slide missing spTree"))?;
    let mut elements = Vec::new();
    let mut element_index = 0;
    for child in sp_tree.children().filter(|node| node.is_element()) {
        let local = child.tag_name().name();
        if !matches!(local, "sp" | "pic" | "graphicFrame" | "grpSp") {
            continue;
        }
        element_index += 1;
        elements.push(extract_element(
            bundle,
            &child,
            slide_rels,
            slide_index,
            element_index,
            extract_images,
            image_dir,
        )?);
    }
    Ok(elements)
}

fn extract_element(
    bundle: &ZipBundle,
    node: &Node<'_, '_>,
    slide_rels: &HashMap<String, String>,
    slide_index: usize,
    shape_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<ElementInfo> {
    let name = node
        .descendants()
        .find(|child| child.tag_name().name() == "cNvPr")
        .and_then(|child| attr_value(&child, "name").map(str::to_string))
        .unwrap_or_default();
    let mut element = ElementInfo {
        index: shape_index,
        name,
        element_type: match node.tag_name().name() {
            "sp" => "shape",
            "pic" => "image",
            "graphicFrame" => "graphicFrame",
            "grpSp" => "group",
            other => other,
        }
        .to_string(),
        position: extract_position(node),
        rotation: extract_rotation(node),
        text: extract_text_info(node),
        image: None,
        table: None,
        chart: None,
        children: None,
    };

    match node.tag_name().name() {
        "pic" => {
            let embed_id = node
                .descendants()
                .find(|child| child.tag_name().name() == "blip")
                .and_then(|child| rel_attr_value(&child, "embed").map(str::to_string));
            if let Some(embed_id) = embed_id {
                let info = extract_image_info(
                    bundle,
                    slide_rels,
                    &embed_id,
                    slide_index,
                    shape_index,
                    extract_images,
                    image_dir,
                )?;
                element.image = Some(info);
            }
        }
        "graphicFrame" => {
            if let Some(tbl) = node
                .descendants()
                .find(|child| child.tag_name().name() == "tbl")
            {
                element.element_type = "table".to_string();
                element.table = Some(extract_table_info(&tbl));
            } else if let Some(chart) = node
                .descendants()
                .find(|child| child.tag_name().name() == "chart")
            {
                element.element_type = "chart".to_string();
                let rel_id = rel_attr_value(&chart, "id").unwrap_or("chart");
                element.chart = Some(ChartInfo {
                    chart_type: rel_id.to_string(),
                    has_legend: None,
                });
            }
        }
        "grpSp" => {
            element.element_type = "group".to_string();
            let mut children = Vec::new();
            let mut child_index = 0;
            for child in node.children().filter(|child| child.is_element()) {
                if !matches!(
                    child.tag_name().name(),
                    "sp" | "pic" | "graphicFrame" | "grpSp"
                ) {
                    continue;
                }
                child_index += 1;
                children.push(extract_element(
                    bundle,
                    &child,
                    slide_rels,
                    slide_index,
                    child_index,
                    extract_images,
                    image_dir,
                )?);
            }
            element.children = Some(children);
        }
        _ => {}
    }
    Ok(element)
}

fn extract_image_info(
    bundle: &ZipBundle,
    slide_rels: &HashMap<String, String>,
    embed_id: &str,
    slide_index: usize,
    shape_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<ImageInfo> {
    let target = slide_rels
        .get(embed_id)
        .ok_or_else(|| anyhow!("missing image relationship {}", embed_id))?;
    let media_path = resolve_target("ppt/slides/slide.xml", target);
    let bytes = bundle
        .bytes(&media_path)
        .ok_or_else(|| anyhow!("missing media {}", media_path))?;
    let image = image::load_from_memory(bytes).ok();
    let content_type = media_path
        .rsplit('.')
        .next()
        .map(|ext| format!("image/{}", ext));
    let extracted_path = if extract_images {
        if let Some(dir) = image_dir {
            fs::create_dir_all(dir)?;
            let ext = media_path.rsplit('.').next().unwrap_or("bin");
            let path = dir.join(format!(
                "slide{}_shape{}.{}",
                slide_index + 1,
                shape_index,
                ext
            ));
            fs::write(&path, bytes)?;
            Some(path.display().to_string())
        } else {
            None
        }
    } else {
        None
    };
    Ok(ImageInfo {
        content_type,
        width: image.as_ref().map(DynamicImage::width),
        height: image.as_ref().map(DynamicImage::height),
        extracted_path,
    })
}

fn extract_table_info(node: &Node<'_, '_>) -> TableInfo {
    let rows = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "tr")
        .map(|row| {
            row.children()
                .filter(|cell| cell.is_element() && cell.tag_name().name() == "tc")
                .map(|cell| collect_text(&cell))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    TableInfo {
        rows: rows.len(),
        cols,
        data: rows,
    }
}

fn extract_position(node: &Node<'_, '_>) -> Position {
    let xfrm = node
        .descendants()
        .find(|child| matches!(child.tag_name().name(), "xfrm" | "off" | "ext"));
    let (x, y, w, h) = if let Some(xfrm) = xfrm {
        if xfrm.tag_name().name() == "xfrm" {
            let off = xfrm
                .children()
                .find(|child| child.tag_name().name() == "off");
            let ext = xfrm
                .children()
                .find(|child| child.tag_name().name() == "ext");
            (
                off.and_then(|node| attr_f64(&node, "x"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                off.and_then(|node| attr_f64(&node, "y"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                ext.and_then(|node| attr_f64(&node, "cx"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                ext.and_then(|node| attr_f64(&node, "cy"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    Position {
        x: round4(x),
        y: round4(y),
        w: round4(w),
        h: round4(h),
    }
}

fn extract_rotation(node: &Node<'_, '_>) -> Option<f64> {
    node.descendants()
        .find(|child| child.tag_name().name() == "xfrm")
        .and_then(|xfrm| attr_f64(&xfrm, "rot"))
        .map(|rot| rot / 60_000.0)
        .filter(|rot| *rot != 0.0)
}

fn extract_text_info(node: &Node<'_, '_>) -> Option<TextInfo> {
    let text_node = node
        .descendants()
        .find(|child| child.tag_name().name() == "txBody")?;
    let paragraphs = text_node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "p")
        .map(|paragraph| ParagraphInfo {
            text: collect_text(&paragraph),
        })
        .collect::<Vec<_>>();
    let full_text = paragraphs
        .iter()
        .map(|item| item.text.clone())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Some(TextInfo {
        full_text,
        paragraphs,
    })
}

fn collect_text(node: &Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_element() && child.tag_name().name() == "t")
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join("")
}

fn attr_value<'a>(node: &'a Node<'a, 'a>, key: &str) -> Option<&'a str> {
    node.attribute(key).or_else(|| {
        key.split_once(':')
            .and_then(|(_, local)| node.attribute(local))
    })
}

fn rel_attr_value<'a>(node: &'a Node<'a, 'a>, local: &str) -> Option<&'a str> {
    node.attribute((
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        local,
    ))
    .or_else(|| node.attribute(local))
}

fn attr_f64(node: &Node<'_, '_>, key: &str) -> Option<f64> {
    attr_value(node, key).and_then(|value| value.parse::<f64>().ok())
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn element_overflows(element: &Value, slide_w: f64, slide_h: f64) -> bool {
    let position = element.get("position");
    let x = position
        .and_then(|pos| pos.get("x"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let y = position
        .and_then(|pos| pos.get("y"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let w = position
        .and_then(|pos| pos.get("w"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let h = position
        .and_then(|pos| pos.get("h"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let over = x < -0.01 || y < -0.01 || x + w > slide_w + 0.01 || y + h > slide_h + 0.01;
    if over {
        return true;
    }
    element
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .any(|child| element_overflows(child, slide_w, slide_h))
        })
        .unwrap_or(false)
}

fn extract_requested_fonts_by_slide(
    bundle: &ZipBundle,
) -> Result<BTreeMap<usize, BTreeSet<String>>> {
    let defaults = extract_theme_fonts(bundle)?;
    let mut by_slide = BTreeMap::new();
    let mut slide_names = bundle
        .names()
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .cloned()
        .collect::<Vec<_>>();
    slide_names.sort();
    for (index, slide_name) in slide_names.iter().enumerate() {
        let xml = bundle.text(slide_name)?;
        let doc = Document::parse(&xml)?;
        let mut fonts = BTreeSet::new();
        for node in doc.descendants() {
            match node.tag_name().name() {
                "latin" | "ea" | "cs" | "sym" | "font" => {
                    if let Some(face) = attr_value(&node, "typeface") {
                        if !face.trim().is_empty() && face != "+mn-lt" && face != "+mj-lt" {
                            fonts.insert(face.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        if fonts.is_empty() {
            fonts.extend(defaults.iter().cloned());
        }
        by_slide.insert(index + 1, fonts);
    }
    Ok(by_slide)
}

fn extract_theme_fonts(bundle: &ZipBundle) -> Result<BTreeSet<String>> {
    let theme_name = bundle
        .names()
        .find(|name| name.starts_with("ppt/theme/theme") && name.ends_with(".xml"))
        .cloned()
        .ok_or_else(|| anyhow!("missing theme xml"))?;
    let xml = bundle.text(&theme_name)?;
    let doc = Document::parse(&xml)?;
    let mut fonts = BTreeSet::new();
    for node in doc
        .descendants()
        .filter(|node| matches!(node.tag_name().name(), "latin" | "ea" | "cs"))
    {
        if let Some(face) = attr_value(&node, "typeface") {
            if !face.trim().is_empty() {
                fonts.insert(face.to_string());
            }
        }
    }
    Ok(fonts)
}

fn normalize_font_family_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let no_paren = Regex::new(r"\([^)]*\)").unwrap().replace_all(&lower, " ");
    let cleaned = Regex::new(r#"[\s\-\_\.,/\'\"]+"#)
        .unwrap()
        .replace_all(&no_paren, " ");
    cleaned.trim().to_string()
}

fn build_font_synonym_map() -> Result<HashMap<String, BTreeSet<String>>> {
    let output = run_command_capture(
        Command::new("fc-list")
            .arg("--format")
            .arg("%{family}\t%{fullname}\t%{postscriptname}\n"),
    )
    .context("fc-list failed")?;
    let mut syn = HashMap::<String, BTreeSet<String>>::new();
    for line in output.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let mut names = BTreeSet::new();
        for field in parts {
            for item in field.split(',') {
                let normalized = normalize_font_family_name(item);
                if !normalized.is_empty() {
                    names.insert(normalized.clone());
                    names.insert(normalized.replace(' ', ""));
                }
            }
        }
        for name in names.clone() {
            syn.entry(name).or_default().extend(names.clone());
        }
    }
    Ok(syn)
}

fn expand_font_family_aliases(
    synonyms: &HashMap<String, BTreeSet<String>>,
    family: &str,
) -> BTreeSet<String> {
    let mut acceptable = BTreeSet::from([family.to_string(), family.replace(' ', "")]);
    if let Some(items) = synonyms.get(family) {
        acceptable.extend(items.iter().cloned());
    }
    let compact = family.replace(' ', "");
    if let Some(items) = synonyms.get(&compact) {
        acceptable.extend(items.iter().cloned());
    }
    acceptable
}

fn extract_resolved_fonts_from_odp(input: &Path) -> Result<BTreeSet<String>> {
    let profile = TempDir::new()?;
    let convert_dir = TempDir::new()?;
    let profile_flag = format!("file://{}", profile.path().display());
    let mut convert = Command::new("soffice");
    convert
        .arg(format!("-env:UserInstallation={}", profile_flag))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("odp")
        .arg("--outdir")
        .arg(convert_dir.path())
        .arg(input)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command_timeout(&mut convert, SOFFICE_PROBE_TIMEOUT)?;
    let stem = input.file_stem().and_then(OsStr::to_str).unwrap_or("deck");
    let odp_path = convert_dir.path().join(format!("{}.odp", stem));
    let bundle = ZipBundle::from_path(&odp_path)?;
    let mut fonts = BTreeSet::new();
    let font_re = Regex::new(r#"font-family[^=]*=\"([^\"]+)\""#)?;
    for target in ["content.xml", "styles.xml"] {
        let text = match bundle.text(target) {
            Ok(text) => text,
            Err(_) => continue,
        };
        for caps in font_re.captures_iter(&text) {
            for value in caps[1].split(',') {
                let normalized = normalize_font_family_name(value.trim_matches('"').trim());
                if !normalized.is_empty() {
                    fonts.insert(normalized);
                }
            }
        }
    }
    Ok(fonts)
}

fn sanitize_presentation_xml(xml: &str) -> Result<String> {
    let notes_master_re =
        Regex::new(r#"(?s)<p:notesMasterIdLst(?:\s*/>|>.*?</p:notesMasterIdLst>)"#)?;
    let sld_master_re = Regex::new(r#"(?s)<p:sldMasterIdLst(?:\s*/>|>.*?</p:sldMasterIdLst>)"#)?;

    let notes_master = match notes_master_re.find(xml) {
        Some(value) => value.as_str().to_string(),
        None => return Ok(xml.to_string()),
    };
    let without_notes_master = notes_master_re.replace(xml, "").to_string();
    if let Some(sld_master) = sld_master_re.find(&without_notes_master) {
        let mut rebuilt = String::with_capacity(without_notes_master.len() + notes_master.len());
        rebuilt.push_str(&without_notes_master[..sld_master.end()]);
        rebuilt.push_str(&notes_master);
        rebuilt.push_str(&without_notes_master[sld_master.end()..]);
        return Ok(rebuilt);
    }
    Ok(without_notes_master)
}

fn join_display_list(value: Option<&Vec<Value>>) -> String {
    value
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pdf_page_size_points() {
        let (w, h) = parse_pdf_page_size("612 x 792 pts (letter)").unwrap();
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    #[test]
    fn normalize_font_names() {
        assert_eq!(
            normalize_font_family_name("Helvetica Neue (Body)"),
            "helvetica neue"
        );
        assert_eq!(normalize_font_family_name("PingFang-SC"), "pingfang sc");
    }

    #[test]
    fn sanitize_presentation_xml_reorders_notes_master_after_slide_master() {
        let input = r#"<p:presentation><p:sldMasterIdLst/><p:sldIdLst/><p:notesMasterIdLst><p:notesMasterId r:id="rId4"/></p:notesMasterIdLst><p:sldSz cx="1" cy="2"/><p:notesSz cx="2" cy="1"/><p:defaultTextStyle/></p:presentation>"#;
        let output = sanitize_presentation_xml(input).unwrap();
        assert!(
            output.find("<p:sldMasterIdLst/>").unwrap()
                < output.find("<p:notesMasterIdLst>").unwrap()
        );
        assert!(
            output.find("<p:notesMasterIdLst>").unwrap() < output.find("<p:sldIdLst/>").unwrap()
        );
    }

    #[test]
    fn outline_source_embeds_design_brief() {
        let outline = json!({
            "title": "测试汇报",
            "slides": [
                {"title": "本页展示增长路径", "bullets": ["赋能业务", "具有重要意义"]}
            ]
        });
        let source = generate_outline_deck_source(&outline, &DeckTemplate::Dark).unwrap();
        assert!(source.contains("ppt-rust-outline-plan"));
        assert!(source.contains("$humanizer"));
        assert!(source.contains("$copywriting"));
        assert!(source.contains("$paper-writing"));
        assert!(source.contains("design-output-auditor drift verdict"));
        assert!(!source.contains("本页展示增长路径"));
        assert!(source.contains("增长路径"));
        assert!(source.contains("支持业务"));
        assert!(source.contains("会影响具体决策"));
    }

    #[test]
    fn strict_quality_gate_accepts_rust_inspector_and_rejects_overflow() {
        let clean = json!({
            "overflow_check": {"ok": true},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        strict_quality_gate(&clean).unwrap();

        let overflow = json!({
            "overflow_check": {"ok": false},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        assert!(strict_quality_gate(&overflow).is_err());
    }
}
