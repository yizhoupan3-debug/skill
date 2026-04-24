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

impl DeckTemplate {
    fn asset_file(&self) -> &'static str {
        match self {
            Self::Dark => "deck.template.js",
            Self::Light => "template_light.js",
            Self::Corporate => "template_corporate.js",
        }
    }
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
    skip_npm_install: bool,
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
    #[arg(short, long, default_value = "deck.js")]
    output: String,
    #[arg(long, value_enum, default_value_t = DeckTemplate::Dark)]
    template: DeckTemplate,
    #[arg(long, default_value_t = false)]
    bootstrap: bool,
    #[arg(long, default_value_t = false)]
    build: bool,
    #[arg(long, default_value_t = false)]
    qa: bool,
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
    #[arg(long, default_value = "deck.js")]
    entry: String,
    #[arg(long, default_value = "deck.pptx")]
    deck: String,
    #[arg(long, default_value = "rendered")]
    rendered_dir: String,
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
    npm_install: String,
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
    deck: String,
    render: QaRenderSummary,
    overflow_check: QaOverflowSummary,
    font_check: Value,
    officecli: Value,
}

#[derive(Debug, Serialize)]
struct OfficeProbeSummary {
    available: bool,
    binary: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Serialize)]
struct OfficeDoctorSummary {
    officecli_version: Option<String>,
    file: String,
    outline: Value,
    issues: Value,
    validation: Value,
}

#[derive(Debug)]
struct OfficeBinary {
    path: PathBuf,
    version: Option<String>,
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
    let summary = init_workspace(&workdir, &args.template, args.force, !args.skip_npm_install)?;
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
        init_workspace(&workdir, &args.template, false, false)?;
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
    if args.build || args.qa {
        let tool_bin = std::env::current_exe()
            .context("failed to locate current executable")?
            .display()
            .to_string();
        let entry = output
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| anyhow!("invalid deck source path {}", output.display()))?;
        let status = Command::new("node")
            .arg(entry)
            .current_dir(&workdir)
            .env("PPT_PPTX_RUST_TOOL_BIN", tool_bin)
            .status()
            .with_context(|| format!("failed to run node {}", output.display()))?;
        if !status.success() {
            bail!(
                "node {} failed with status {:?}",
                output.display(),
                status.code()
            );
        }
    }
    if args.qa {
        qa_payload = Some(serde_json::to_value(qa_summary(
            &workdir.join("deck.pptx").display().to_string(),
            &workdir.join(&args.rendered_dir).display().to_string(),
        )?)?);
    }

    emit_value(
        serde_json::to_value(OutlineSummary {
            input: input.display().to_string(),
            output: output.display().to_string(),
            bootstrapped: args.bootstrap,
            built: args.build || args.qa,
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
    let officecli = office_doctor_value(&args.deck)?;
    let payload = json!({
        "deck": args.deck,
        "structure": structure,
        "officecli": officecli,
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
    let status = Command::new("node")
        .arg(&args.entry)
        .current_dir(&workdir)
        .status()
        .with_context(|| format!("failed to run node {}", args.entry))?;
    if !status.success() {
        bail!("node {} failed with status {:?}", args.entry, status.code());
    }
    let deck = workdir.join(&args.deck);
    let rendered = workdir.join(&args.rendered_dir);
    let payload = qa_summary(&deck.display().to_string(), &rendered.display().to_string())?;
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
    let probe = detect_officecli();
    let payload = OfficeProbeSummary {
        available: probe.is_some(),
        binary: probe.as_ref().map(|item| item.path.display().to_string()),
        version: probe.and_then(|item| item.version),
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else if payload.available {
        println!("officecli: {}", payload.binary.clone().unwrap_or_default());
        println!(
            "version: {}",
            payload.version.unwrap_or_else(|| "unknown".to_string())
        );
    } else {
        println!("officecli: missing");
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
    let office = require_officecli()?;
    let mut args = vec![command.to_string(), file.to_string()];
    if let Some(tail) = tail {
        args.push(tail.to_string());
    }
    if json_output {
        let payload = run_office_json(&office.path, &args)?;
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        let status = Command::new(&office.path).args(&args).status()?;
        if !status.success() {
            bail!("officecli command failed with status {:?}", status.code());
        }
    }
    Ok(())
}

fn office_get_command(args: OfficeGetArgs) -> Result<()> {
    let office = require_officecli()?;
    let mut command = Command::new(&office.path);
    command
        .arg("get")
        .arg(&args.file)
        .arg(&args.path)
        .arg("--depth")
        .arg(args.depth.to_string());
    if args.json {
        command.arg("--json");
    }
    let status = command.status()?;
    if !status.success() {
        bail!("officecli get failed with status {:?}", status.code());
    }
    Ok(())
}

fn office_query_command(args: OfficeQueryArgs) -> Result<()> {
    let office = require_officecli()?;
    let mut command = Command::new(&office.path);
    command.arg("query").arg(&args.file).arg(&args.selector);
    if let Some(text) = args.text {
        command.arg("--text").arg(text);
    }
    if args.json {
        command.arg("--json");
    }
    let status = command.status()?;
    if !status.success() {
        bail!("officecli query failed with status {:?}", status.code());
    }
    Ok(())
}

fn office_watch_command(args: OfficeWatchArgs) -> Result<()> {
    let office = require_officecli()?;
    let status = Command::new(&office.path)
        .arg("watch")
        .arg(&args.file)
        .arg("--port")
        .arg(args.port.to_string())
        .status()?;
    if !status.success() {
        bail!("officecli watch failed with status {:?}", status.code());
    }
    if args.browser {
        let status = Command::new("open")
            .arg(format!("http://127.0.0.1:{}", args.port))
            .status()?;
        if !status.success() {
            bail!("failed to open browser with status {:?}", status.code());
        }
    }
    Ok(())
}

fn office_batch_command(args: OfficeBatchArgs) -> Result<()> {
    let office = require_officecli()?;
    let mut command = Command::new(&office.path);
    command.arg("batch").arg(&args.file);
    if let Some(input) = args.input {
        command.arg("--input").arg(input);
    }
    if let Some(commands) = args.commands {
        command.arg("--commands").arg(commands);
    }
    if args.force {
        command.arg("--force");
    }
    if args.json {
        command.arg("--json");
    }
    let status = command.status()?;
    if !status.success() {
        bail!("officecli batch failed with status {:?}", status.code());
    }
    Ok(())
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

fn init_workspace(
    workdir: &Path,
    template: &DeckTemplate,
    force: bool,
    run_install: bool,
) -> Result<InitSummary> {
    let skill_root = resolve_skill_root()?;
    let assets_root = skill_root.join("assets");
    let scripts_root = skill_root.join("scripts");
    let mut created = Vec::new();

    fs::create_dir_all(workdir)?;
    fs::create_dir_all(workdir.join("assets"))?;
    fs::create_dir_all(workdir.join("rendered"))?;
    fs::create_dir_all(workdir.join("scripts"))?;

    copy_file_checked(
        &assets_root.join(template.asset_file()),
        &workdir.join("deck.js"),
        force,
        &mut created,
    )?;
    copy_file_checked(
        &assets_root.join("package.template.json"),
        &workdir.join("package.json"),
        force,
        &mut created,
    )?;
    copy_file_checked(
        &scripts_root.join("outline_to_deck.js"),
        &workdir.join("scripts").join("outline_to_deck.js"),
        force,
        &mut created,
    )?;
    copy_dir_checked(
        &assets_root.join("pptxgenjs_helpers"),
        &workdir.join("pptxgenjs_helpers"),
        force,
        &mut created,
    )?;

    let npm_install = if run_install {
        let status = Command::new("npm")
            .arg("install")
            .current_dir(workdir)
            .status()
            .context("failed to run npm install")?;
        if !status.success() {
            bail!("npm install failed with status {:?}", status.code());
        }
        "ran".to_string()
    } else {
        "skipped".to_string()
    };

    Ok(InitSummary {
        workdir: workdir.display().to_string(),
        template: format!("{:?}", template).to_ascii_lowercase(),
        files: created,
        npm_install,
    })
}

fn read_outline(input: &Path) -> Result<Value> {
    let raw = fs::read_to_string(input)
        .with_context(|| format!("failed to read outline {}", input.display()))?;
    if has_extension(input, "json") {
        return serde_json::from_str(&raw).context("failed to parse JSON outline");
    }
    parse_outline_yaml_subset(&raw)
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
    let slides = reflow_outline_slides(outline.get("slides").and_then(Value::as_array));
    let total_slides = slides.len() + 2;
    let palette = outline
        .get("palette")
        .and_then(Value::as_str)
        .unwrap_or("dark");
    let palette_value = outline_palette(palette);
    let mut code = String::new();
    code.push_str(&format!(
        r#"const fs = require("fs");
const path = require("path");
const {{ spawnSync }} = require("child_process");
const pptxgen = require("pptxgenjs");
const {{
  imageSizingCrop,
  warnIfSlideHasOverlaps,
  warnIfSlideElementsOutOfBounds,
  addStyledChart,
  addGlassPanel,
  getTypography,
  getSmartTypography,
}} = require("./pptxgenjs_helpers");

const pptx = new pptxgen();
pptx.layout = "LAYOUT_WIDE";
pptx.title = {};
pptx.lang = "zh-CN";
pptx.theme = {{
  headFontFace: "Arial",
  bodyFontFace: "Arial",
  lang: "zh-CN",
}};

const palette = {};
const totalSlides = {};

pptx.defineSlideMaster({{
  title: "PPT_RUST_SEMANTIC",
  background: {{ color: palette.stage }},
  objects: [{{ placeholder: {{ options: {{ name: "officecli_title", type: "title" }} }} }}],
}});

function addTopLabel(slide, text) {{
  slide.addText(text, {{ x: 0.9, y: 0.38, w: 2.0, h: 0.12, ...getTypography("overline", {{ color: palette.textMute, charSpace: 1.2 }}) }});
}}

function addBottomGlow(slide) {{
  slide.addShape(pptx.ShapeType.rect, {{ x: 0.86, y: 6.86, w: 11.6, h: 0.018, line: {{ color: palette.glow, transparency: 100 }}, fill: {{ color: palette.glow, transparency: 24 }} }});
}}

function addSectionTitle(slide, title, subtitle, x, y, w) {{
  slide.addText(title || "", {{ x, y, w: Math.min(w * 0.72, 5.0), h: 0.32, placeholder: "officecli_title", ...getSmartTypography("h2", title || "", w, 0.32, {{ color: palette.text }}) }});
  if (subtitle) slide.addText(subtitle, {{ x, y: y + 0.42, w, h: 0.16, ...getSmartTypography("body2", subtitle, w, 0.16, {{ color: palette.textSoft, bold: true }}) }});
}}

function finalizeSlide(slide, opts = {{}}) {{
  if (!opts.skipOverlap) warnIfSlideHasOverlaps(slide, pptx, {{ ignoreDecorativeShapes: true }});
  warnIfSlideElementsOutOfBounds(slide, pptx);
}}

function fileExists(assetPath) {{
  try {{ return !!assetPath && fs.existsSync(assetPath); }} catch (_) {{ return false; }}
}}

function addOptionalImage(slide, assetPath, sizingFactory, fallback = {{}}) {{
  if (fileExists(assetPath)) {{
    slide.addImage({{ path: assetPath, ...sizingFactory(assetPath) }});
    return true;
  }}
  slide.addShape(pptx.ShapeType.rect, {{
    x: fallback.x ?? 0, y: fallback.y ?? 0, w: fallback.w ?? 13.333, h: fallback.h ?? 7.5,
    line: {{ color: fallback.fill || palette.panelSoft, transparency: 100 }},
    fill: {{ color: fallback.fill || palette.panelSoft, transparency: fallback.transparency ?? 0 }},
  }});
  if (fallback.label) {{
    slide.addText(fallback.label, {{ x: (fallback.x ?? 0) + 0.18, y: (fallback.y ?? 0) + 0.18, w: Math.max((fallback.w ?? 4) - 0.36, 1.2), h: 0.14, ...getTypography("caption", {{ color: palette.textMute }}) }});
  }}
  return false;
}}

function sanitizeGeneratedDeck(fileName) {{
  const tool = process.env.PPT_PPTX_RUST_TOOL_BIN || "ppt";
  const completed = spawnSync(tool, ["sanitize-pptx", fileName], {{ stdio: "inherit" }});
  if (completed.status !== 0) throw new Error(`sanitize-pptx failed for ${{fileName}}`);
}}

"#,
        js_string(outline_str(outline, "title", "Untitled Deck"))?,
        serde_json::to_string_pretty(&palette_value)?,
        total_slides
    ));

    code.push_str(&generate_cover_source(outline, total_slides)?);
    for (idx, slide) in slides.iter().enumerate() {
        code.push_str(&generate_content_slide_source(slide, idx, total_slides)?);
    }
    code.push_str(&generate_closing_source(outline, total_slides)?);
    code.push_str(
        r#"
async function writeDeck() {
  await pptx.writeFile({ fileName: "deck.pptx" });
  sanitizeGeneratedDeck("deck.pptx");
}

writeDeck().catch((error) => {
  console.error(error);
  process.exit(1);
});
"#,
    );
    Ok(code)
}

fn outline_palette(name: &str) -> Value {
    match name {
        "light" => json!({
            "stage": "FAFAFA", "panel": "FFFFFF", "panelSoft": "F0F0F0", "line": "E0E0E0",
            "glow": "3B82F6", "text": "1A1A1A", "textSoft": "666666", "textMute": "999999",
            "chip": "1A1A1A", "chipText": "FFFFFF"
        }),
        "academic" => json!({
            "stage": "F5F3EF", "panel": "FFFFFF", "panelSoft": "EDE9E3", "line": "D4CFC7",
            "glow": "2563EB", "text": "1F2937", "textSoft": "4B5563", "textMute": "9CA3AF",
            "chip": "1F2937", "chipText": "FFFFFF"
        }),
        _ => json!({
            "stage": "000000", "panel": "111111", "panelSoft": "171717", "line": "2A2A2A",
            "glow": "7EA9FF", "text": "F2F2EE", "textSoft": "B9B9B2", "textMute": "888883",
            "chip": "F4F4EF", "chipText": "111111"
        }),
    }
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

fn generate_cover_source(outline: &Value, total_slides: usize) -> Result<String> {
    let meta = [outline.get("presenter"), outline.get("date")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" / ");
    let meta_value = json!({ "meta": meta });
    Ok(format!(
        r#"
const cover = pptx.addSlide({{ masterName: "PPT_RUST_SEMANTIC" }});
cover.background = {{ color: palette.stage }};
addOptionalImage(cover, {}, (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {{ x: 0, y: 0, w: 13.333, h: 7.5, fill: palette.panelSoft, transparency: 0, label: "COVER IMAGE OPTIONAL" }});
cover.addShape(pptx.ShapeType.rect, {{ x: 0, y: 0, w: 13.333, h: 7.5, line: {{ color: palette.stage, transparency: 100 }}, fill: {{ color: palette.stage, transparency: 40 }} }});
cover.addShape(pptx.ShapeType.rect, {{ x: 0, y: 0, w: 6.1, h: 7.5, line: {{ color: palette.stage, transparency: 100 }}, fill: {{ color: palette.stage, transparency: 22 }} }});
addTopLabel(cover, "PRESENTATION");
cover.addText({}, {{ x: 0.92, y: 1.76, w: 4.64, h: 1.06, placeholder: "officecli_title", ...getTypography("display", {{ color: palette.text, animate: {{ type: "fade", prop: "in", delay: 0.1 }} }}) }});
{}
{}
cover.addText("01 / " + String(totalSlides).padStart(2, "0"), {{ x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", {{ color: palette.textMute, align: "right" }}) }});
addBottomGlow(cover);
finalizeSlide(cover, {{ skipOverlap: true }});
"#,
        js_string(outline_str(outline, "coverImage", "./assets/cover.jpg"))?,
        js_string(outline_str(outline, "title", "Title"))?,
        optional_add_text("cover", outline, "subtitle", 0.96, 3.02, 4.48, 0.66, "body1")?,
        optional_add_text("cover", &meta_value, "meta", 0.96, 4.48, 3.0, 0.14, "body2")?
    )
    .replace("totalSlides", &total_slides.to_string()))
}

fn generate_content_slide_source(
    slide: &Value,
    index: usize,
    total_slides: usize,
) -> Result<String> {
    let pattern = detect_outline_pattern(slide);
    let mut code = format!(
        r#"
const slide{index} = pptx.addSlide({{ masterName: "PPT_RUST_SEMANTIC" }});
slide{index}.background = {{ color: palette.stage }};
addTopLabel(slide{index}, "SECTION {section}");
addSectionTitle(slide{index}, {title}, {subtitle}, 0.92, 0.96, 5.0);
"#,
        index = index,
        section = format!("{:02}", index + 1),
        title = js_string(outline_str(slide, "title", ""))?,
        subtitle = js_string(outline_str(slide, "subtitle", ""))?
    );
    code.push_str(&match pattern {
        "multi-card" => generate_multi_card_source(slide, index)?,
        "data-panel" => generate_data_panel_source(slide, index)?,
        "comparison" => generate_comparison_source(slide, index)?,
        "image-text-split" => generate_image_text_split_source(slide, index)?,
        "hero-image" => generate_hero_image_source(slide, index)?,
        "timeline" => generate_list_cards_source(slide, index, "timeline", 4)?,
        "process-flow" => generate_list_cards_source(slide, index, "steps", 4)?,
        _ => generate_full_text_source(slide, index)?,
    });
    code.push_str(&format!(
        r#"
slide{index}.addText("{page} / {total}", {{ x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", {{ color: palette.textMute, align: "right" }}) }});
addBottomGlow(slide{index});
finalizeSlide(slide{index});
"#,
        index = index,
        page = format!("{:02}", index + 2),
        total = format!("{:02}", total_slides)
    ));
    Ok(code)
}

fn generate_multi_card_source(slide: &Value, idx: usize) -> Result<String> {
    generate_list_cards_source(slide, idx, "bullets", 4)
}

fn generate_list_cards_source(
    slide: &Value,
    idx: usize,
    key: &str,
    max_count: usize,
) -> Result<String> {
    let items = value_string_array(slide, key);
    let card_count = items.len().min(max_count);
    let card_w = if card_count <= 2 {
        5.48
    } else if card_count == 3 {
        3.56
    } else {
        2.62
    };
    let mut code = String::new();
    for (i, item) in items.iter().take(max_count).enumerate() {
        let x = 0.94 + i as f64 * (card_w + 0.22);
        code.push_str(&format!(
            r#"addGlassPanel(slide{idx}, pptx, {x:.2}, 2.0, {card_w:.2}, 3.8, {{ fill: palette.panelSoft, transparency: 10 }});
slide{idx}.addText("{num:02}", {{ x: {num_x:.2}, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", {{ color: palette.text }}) }});
slide{idx}.addText({text}, {{ x: {text_x:.2}, y: 2.72, w: {text_w:.2}, h: 2.8, ...getSmartTypography("body2", {text}, {text_w:.2}, 2.8, {{ color: palette.textSoft, valign: "top", breakLine: true }}) }});
"#,
            idx = idx,
            x = x,
            card_w = card_w,
            num = i + 1,
            num_x = x + 0.18,
            text = js_string(item)?,
            text_x = x + 0.18,
            text_w = card_w - 0.36
        ));
    }
    Ok(code)
}

fn generate_data_panel_source(slide: &Value, idx: usize) -> Result<String> {
    let metrics = slide
        .get("metrics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let chip_count = metrics.len().min(5);
    let chip_w = if chip_count > 0 {
        (11.4 / chip_count as f64 - 0.2).min(2.2)
    } else {
        2.0
    };
    let mut code = String::new();
    for (i, metric) in metrics.iter().take(5).enumerate() {
        let x = 0.94 + i as f64 * (chip_w + 0.22);
        let value = js_string(outline_str(metric, "value", ""))?;
        let label = js_string(outline_str(metric, "label", ""))?;
        code.push_str(&format!(
            r#"addGlassPanel(slide{idx}, pptx, {x:.2}, 2.3, {chip_w:.2}, 0.94, {{ fill: palette.panelSoft, transparency: 15 }});
slide{idx}.addText({value}, {{ x: {vx:.2}, y: 2.48, w: {vw:.2}, h: 0.18, ...getSmartTypography("metric", {value}, {vw:.2}, 0.18, {{ color: palette.text }}) }});
slide{idx}.addText({label}, {{ x: {vx:.2}, y: 2.84, w: {vw:.2}, h: 0.12, ...getSmartTypography("caption", {label}, {vw:.2}, 0.12, {{ color: palette.textSoft }}) }});
"#,
            idx = idx, x = x, chip_w = chip_w, value = value, label = label, vx = x + 0.14, vw = chip_w - 0.28
        ));
    }
    code.push_str(&format!("addGlassPanel(slide{idx}, pptx, 0.94, 3.56, 11.42, 2.24, {{ fill: palette.panelSoft, transparency: 8 }});\n"));
    if let Some(chart) = slide.get("chart") {
        code.push_str(&format!(
            "addStyledChart(slide{idx}, pptx, {}, {{ series: {}, categories: {}, position: {{ x: 1.1, y: 3.7, w: 11.1, h: 1.96 }} }});\n",
            js_string(outline_str(chart, "type", "bar"))?,
            serde_json::to_string(chart.get("series").unwrap_or(&json!([])))?,
            serde_json::to_string(chart.get("categories").unwrap_or(&json!([])))?
        ));
    }
    Ok(code)
}

fn generate_comparison_source(slide: &Value, idx: usize) -> Result<String> {
    let left = slide.pointer("/comparison/left").unwrap_or(&Value::Null);
    let right = slide.pointer("/comparison/right").unwrap_or(&Value::Null);
    let left_text = numbered_lines(value_string_array(left, "items"));
    let right_text = numbered_lines(value_string_array(right, "items"));
    Ok(format!(
        r#"addGlassPanel(slide{idx}, pptx, 0.94, 1.9, 5.48, 4.4, {{ fill: palette.panelSoft, transparency: 10 }});
addGlassPanel(slide{idx}, pptx, 6.72, 1.9, 5.48, 4.4, {{ fill: palette.panelSoft, transparency: 10 }});
slide{idx}.addText({left_title}, {{ x: 1.18, y: 2.18, w: 1.1, h: 0.14, ...getSmartTypography("body1", {left_title}, 1.1, 0.14, {{ color: palette.text, bold: true }}) }});
slide{idx}.addText({left_text}, {{ x: 1.18, y: 2.54, w: 4.82, h: 3.2, ...getSmartTypography("body2", {left_text}, 4.82, 3.2, {{ color: palette.textSoft, valign: "top", breakLine: true }}) }});
slide{idx}.addText({right_title}, {{ x: 6.96, y: 2.18, w: 1.1, h: 0.14, ...getSmartTypography("body1", {right_title}, 1.1, 0.14, {{ color: palette.text, bold: true }}) }});
slide{idx}.addText({right_text}, {{ x: 6.96, y: 2.54, w: 4.82, h: 3.2, ...getSmartTypography("body2", {right_text}, 4.82, 3.2, {{ color: palette.textSoft, valign: "top", breakLine: true }}) }});
"#,
        idx = idx,
        left_title = js_string(outline_str(left, "title", "A"))?,
        left_text = js_string(&left_text)?,
        right_title = js_string(outline_str(right, "title", "B"))?,
        right_text = js_string(&right_text)?
    ))
}

fn generate_image_text_split_source(slide: &Value, idx: usize) -> Result<String> {
    let text = numbered_lines(value_string_array(slide, "bullets"));
    Ok(format!(
        r#"addGlassPanel(slide{idx}, pptx, 0.94, 1.76, 5.14, 4.44, {{ fill: palette.panelSoft, transparency: 15 }});
const slide{idx}HasImage = addOptionalImage(slide{idx}, {image}, (assetPath) => imageSizingCrop(assetPath, 1.0, 1.82, 5.02, 4.32), {{ x: 1.0, y: 1.82, w: 5.02, h: 4.32, fill: palette.panel, transparency: 0, label: "OPTIONAL IMAGE" }});
if (slide{idx}HasImage) slide{idx}.addShape(pptx.ShapeType.roundRect, {{ x: 1.0, y: 1.82, w: 5.02, h: 4.32, rectRadius: 0.06, line: {{ color: palette.stage, transparency: 100 }}, fill: {{ color: palette.stage, transparency: 26 }} }});
slide{idx}.addText({text}, {{ x: 6.56, y: 2.0, w: 5.58, h: 4.0, ...getSmartTypography("body1", {text}, 5.58, 4.0, {{ color: palette.textSoft, valign: "top", breakLine: true }}) }});
"#,
        idx = idx,
        image = js_string(outline_str(slide, "image", "./assets/placeholder.jpg"))?,
        text = js_string(&text)?
    ))
}

fn generate_hero_image_source(slide: &Value, idx: usize) -> Result<String> {
    Ok(format!(
        r#"const slide{idx}HasHero = addOptionalImage(slide{idx}, {image}, (assetPath) => imageSizingCrop(assetPath, 0, 1.4, 13.333, 6.1), {{ x: 0, y: 1.4, w: 13.333, h: 6.1, fill: palette.panelSoft, transparency: 0, label: "OPTIONAL IMAGE" }});
if (slide{idx}HasHero) slide{idx}.addShape(pptx.ShapeType.rect, {{ x: 0, y: 1.4, w: 13.333, h: 6.1, line: {{ color: palette.stage, transparency: 100 }}, fill: {{ color: palette.stage, transparency: 40 }} }});
{}
"#,
        optional_add_text(
            &format!("slide{idx}"),
            slide,
            "caption",
            0.96,
            5.8,
            6.0,
            0.36,
            "body1"
        )?,
        idx = idx,
        image = js_string(outline_str(slide, "image", "./assets/placeholder.jpg"))?
    ))
}

fn generate_full_text_source(slide: &Value, idx: usize) -> Result<String> {
    let text = numbered_lines(value_string_array(slide, "bullets"));
    Ok(format!(
        r#"addGlassPanel(slide{idx}, pptx, 0.94, 1.76, 11.42, 4.44, {{ fill: palette.panelSoft, transparency: 10 }});
slide{idx}.addText({text}, {{ x: 1.18, y: 2.04, w: 10.9, h: 3.88, ...getSmartTypography("body1", {text}, 10.9, 3.88, {{ color: palette.textSoft, valign: "top", breakLine: true }}) }});
"#,
        idx = idx,
        text = js_string(&text)?
    ))
}

fn generate_closing_source(outline: &Value, total_slides: usize) -> Result<String> {
    Ok(format!(
        r#"
const closing = pptx.addSlide({{ masterName: "PPT_RUST_SEMANTIC" }});
closing.background = {{ color: palette.stage }};
addOptionalImage(closing, {}, (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {{ x: 0, y: 0, w: 13.333, h: 7.5, fill: palette.panelSoft, transparency: 0, label: "COVER IMAGE OPTIONAL" }});
closing.addShape(pptx.ShapeType.rect, {{ x: 0, y: 0, w: 13.333, h: 7.5, line: {{ color: palette.stage, transparency: 100 }}, fill: {{ color: palette.stage, transparency: 52 }} }});
addTopLabel(closing, "FINAL SLIDE");
closing.addText("THANK YOU", {{ x: 4.18, y: 2.1, w: 4.98, h: 0.42, placeholder: "officecli_title", ...getTypography("display", {{ color: palette.text, align: "center" }}) }});
closing.addText("{total:02} / {total:02}", {{ x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", {{ color: palette.textMute, align: "right" }}) }});
addBottomGlow(closing);
finalizeSlide(closing, {{ skipOverlap: true }});
"#,
        js_string(outline_str(outline, "coverImage", "./assets/cover.jpg"))?,
        total = total_slides
    ))
}

fn optional_add_text(
    target: &str,
    value: &Value,
    key: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    typo: &str,
) -> Result<String> {
    let text = outline_str(value, key, "");
    if text.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        r#"{target}.addText({}, {{ x: {x}, y: {y}, w: {w}, h: {h}, ...getTypography("{typo}", {{ color: palette.textSoft }}) }});"#,
        js_string(text)?
    ))
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

fn numbered_lines(items: Vec<String>) -> String {
    items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| format!("{}. {}", idx + 1, item))
        .collect::<Vec<_>>()
        .join("\n")
}

fn outline_str<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn js_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn resolve_skill_root() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("PPT_PPTX_SKILL_ROOT") {
        let root = expand_path(&path);
        if root.join("assets").exists() && root.join("scripts").exists() {
            return Ok(root);
        }
    }

    let current = std::env::current_exe().context("failed to locate current executable")?;
    for ancestor in current.ancestors() {
        let candidate = ancestor.join("skills").join("ppt-pptx");
        if candidate.join("assets").exists() && candidate.join("scripts").exists() {
            return Ok(candidate);
        }
        let candidate = ancestor.join("ppt-pptx");
        if candidate.join("assets").exists() && candidate.join("scripts").exists() {
            return Ok(candidate);
        }
    }

    let cwd = std::env::current_dir().context("failed to read current directory")?;
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join("skills").join("ppt-pptx");
        if candidate.join("assets").exists() && candidate.join("scripts").exists() {
            return Ok(candidate);
        }
    }

    bail!("cannot locate skills/ppt-pptx; set PPT_PPTX_SKILL_ROOT")
}

fn copy_file_checked(
    src: &Path,
    dest: &Path,
    force: bool,
    created: &mut Vec<String>,
) -> Result<()> {
    if dest.exists() && !force {
        created.push(format!("kept:{}", dest.display()));
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dest)
        .with_context(|| format!("failed to copy {} to {}", src.display(), dest.display()))?;
    created.push(dest.display().to_string());
    Ok(())
}

fn copy_dir_checked(src: &Path, dest: &Path, force: bool, created: &mut Vec<String>) -> Result<()> {
    if dest.exists() {
        if !force {
            created.push(format!("kept:{}", dest.display()));
            return Ok(());
        }
        fs::remove_dir_all(dest)
            .with_context(|| format!("failed to replace {}", dest.display()))?;
    }
    copy_dir_recursive(src, dest)?;
    created.push(dest.display().to_string());
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn qa_summary(deck_path: &str, rendered_dir: &str) -> Result<QaSummary> {
    let deck = expand_path(deck_path);
    let rendered_dir_path = expand_path(rendered_dir);
    let rendered = render_paths(&deck, &rendered_dir_path, 1600, 900)?;
    let overflow = slide_overflow_summary(&deck)?;
    let font_check = detect_fonts_payload(&deck)?;
    let officecli = office_doctor_value(&deck.display().to_string())?;
    Ok(QaSummary {
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
        officecli,
    })
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

    Ok(json!({
        "font_missing_overall": missing_overall.into_iter().collect::<Vec<_>>(),
        "font_missing_by_slide": missing_by_slide,
        "font_substituted_overall": substituted_overall.into_iter().collect::<Vec<_>>(),
        "font_substituted_by_slide": substituted_by_slide,
    }))
}

fn extract_structure_payload(input_path: &str) -> Result<Value> {
    let input = expand_path(input_path);
    let bundle = ZipBundle::from_path(&input)?;
    extract_pptx_structure(&bundle, &input, false, None)
}

fn detect_officecli() -> Option<OfficeBinary> {
    let path = which_path("officecli")?;
    let version = Command::new(&path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty());
    Some(OfficeBinary { path, version })
}

fn require_officecli() -> Result<OfficeBinary> {
    detect_officecli()
        .ok_or_else(|| anyhow!("officecli not found. Install it first, then rerun this command."))
}

fn run_office_json(binary: &Path, args: &[String]) -> Result<Value> {
    let output = Command::new(binary)
        .args(args)
        .arg("--json")
        .output()
        .with_context(|| format!("failed to run {}", binary.display()))?;
    if !output.status.success() {
        bail!(
            "officecli command failed: {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("officecli did not return valid JSON for args={args:?}"))
}

fn office_doctor_value(file: &str) -> Result<Value> {
    Ok(serde_json::to_value(office_doctor_summary(file)?)?)
}

fn office_doctor_summary(file: &str) -> Result<OfficeDoctorSummary> {
    let office = require_officecli()?;
    let outline_payload = run_office_json(
        &office.path,
        &["view".to_string(), file.to_string(), "outline".to_string()],
    )?;
    let issues_payload = run_office_json(
        &office.path,
        &["view".to_string(), file.to_string(), "issues".to_string()],
    )?;
    let validate_payload =
        run_office_json(&office.path, &["validate".to_string(), file.to_string()])?;
    summarize_office_doctor(
        file,
        outline_payload,
        issues_payload,
        validate_payload,
        office.version,
    )
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
        officecli_version: version,
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
        "officecli: {}",
        summary
            .officecli_version
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

fn emit_value(value: Value, _format: EmitFormat) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn which_path(binary: &str) -> Option<PathBuf> {
    let output = Command::new("which").arg(binary).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
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
        .arg(input);
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
        .arg(input);
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
        .arg(&odp_path);
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
}
