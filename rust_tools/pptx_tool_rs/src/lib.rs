use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use font8x8::{UnicodeFonts, BASIC_FONTS};
use image::{
    imageops::{self, FilterType},
    DynamicImage, Rgba, RgbaImage,
};
use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::RefCell;
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

pub mod office;
pub mod qa;
pub mod mcp;

pub const EMU_PER_INCH: f64 = 914_400.0;
pub const POINTS_PER_INCH: f64 = 72.0;
pub const DEFAULT_PAD_PX: u32 = 100;
pub const SOFFICE_PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(ValueEnum, Clone, Debug)]
pub enum DeckTemplate {
    Dark,
    Light,
    Corporate,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum QualityMode {
    Standard,
    Strict,
}

#[derive(Args)]
pub struct InitArgs {
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
pub struct NewArgs {
    #[command(flatten)]
    pub init: InitArgs,
}

#[derive(Args)]
pub struct OutlineArgs {
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
pub struct RenderArgs {
    input_path: String,
    #[arg(long, visible_alias = "output_dir")]
    output_dir: Option<String>,
    #[arg(long, default_value_t = 1600)]
    width: u32,
    #[arg(long, default_value_t = 900)]
    height: u32,
}

#[derive(Args)]
pub struct ExtractStructureArgs {
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
pub struct ReadFullArgs {
    input: String,
    /// Linear text output (default).
    #[arg(long, default_value_t = true, conflicts_with = "json")]
    text: bool,
    /// JSON structure output (same schema as extract-structure).
    #[arg(long, default_value_t = false)]
    json: bool,
    /// Compact JSON (no pretty-print); only applies with --json.
    #[arg(long, default_value_t = false)]
    compact: bool,
    /// 1-based slide filter, e.g. `1-10` or `3`.
    #[arg(long)]
    slides: Option<String>,
}

#[derive(Args)]
pub struct EnsureRasterImageArgs {
    #[arg(long, visible_alias = "input_files")]
    input_files: Vec<String>,
    #[arg(long, visible_alias = "input_dir")]
    input_dir: Option<String>,
    #[arg(long, visible_alias = "output_dir")]
    output_dir: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum LabelMode {
    Number,
    Filename,
    None,
}

#[derive(Args)]
pub struct CreateMontageArgs {
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
pub struct SlidesTestArgs {
    input_path: String,
    #[arg(long, default_value_t = 1600)]
    width: u32,
    #[arg(long, default_value_t = 900)]
    height: u32,
    #[arg(long, visible_alias = "pad_px", default_value_t = DEFAULT_PAD_PX)]
    pad_px: u32,
    #[arg(long, default_value_t = false)]
    fail_on_overflow: bool,
    #[arg(long, default_value_t = false)]
    fail_on_overlap: bool,
    #[arg(long, default_value_t = false)]
    fail_on_aesthetic: bool,
    #[arg(long, default_value_t = false)]
    fail_on_any: bool,
}

#[derive(Args)]
pub struct DetectFontsArgs {
    input_path: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = true)]
    include_missing: bool,
    #[arg(long, default_value_t = true)]
    include_substituted: bool,
}

#[derive(Args)]
pub struct SanitizePptxArgs {
    input_path: String,
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Args)]
pub struct QaArgs {
    deck: String,
    #[arg(long, default_value = "rendered")]
    rendered_dir: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    fail_on_issues: bool,
}

#[derive(Args)]
pub struct IntakeArgs {
    deck: String,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
pub struct BuildQaArgs {
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
pub struct OfficeArgs {
    #[command(subcommand)]
    command: OfficeCommands,
}

#[derive(Subcommand)]
pub enum OfficeCommands {
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
pub struct OfficeProbeArgs {
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
pub struct OfficeDoctorArgs {
    file: String,
    #[arg(long, default_value_t = false)]
    json: bool,
    #[arg(long, default_value_t = false)]
    fail_on_issues: bool,
    #[arg(long, default_value_t = false)]
    fail_on_validation: bool,
}

#[derive(Args)]
pub struct OfficeFileArgs {
    file: String,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
pub struct OfficeGetArgs {
    file: String,
    #[arg(default_value = "/")]
    path: String,
    #[arg(long, default_value_t = 1)]
    depth: i32,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
pub struct OfficeQueryArgs {
    file: String,
    selector: String,
    #[arg(long)]
    text: Option<String>,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Args)]
pub struct OfficeWatchArgs {
    file: String,
    #[arg(long, default_value_t = 18080)]
    port: u16,
    #[arg(long, default_value_t = false)]
    browser: bool,
}

#[derive(Args)]
pub struct OfficeBatchArgs {
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
pub struct InitSummary {
    workdir: String,
    template: String,
    files: Vec<String>,
    rust_only: bool,
    command_manifest: String,
}

#[derive(Debug, Serialize)]
pub struct OutlineSummary {
    input: String,
    output: String,
    bootstrapped: bool,
    built: bool,
    qa: Option<Value>,
}

#[derive(Debug)]
pub struct ZipBundle {
    archive: RefCell<ZipArchive<File>>,
    index_by_name: HashMap<String, usize>,
    cache: RefCell<HashMap<String, Vec<u8>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParagraphInfo {
    text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextInfo {
    #[serde(rename = "fullText")]
    full_text: String,
    paragraphs: Vec<ParagraphInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    content_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    extracted_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    rows: usize,
    cols: usize,
    data: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartInfo {
    chart_type: String,
    has_legend: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
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
pub struct LayoutPlaceholder {
    idx: Option<String>,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutInfo {
    name: String,
    placeholders: Vec<LayoutPlaceholder>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EmitFormat {
    Json,
    Text,
}


include!("commands.rs");

pub fn init_workspace(workdir: &Path, template: &DeckTemplate, force: bool) -> Result<InitSummary> {
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

pub fn starter_sources_markdown(template: &DeckTemplate) -> String {
    format!(
        "# Sources\n\n- Deck source plan: `deck.plan.json`\n- Editable output: `deck.pptx`\n- Runtime: Rust `ppt` CLI\n- Template: `{}`\n\n## Workflow Notes\n\n- Text pass: use built-in Rust copy naturalization for ordinary prose, `$copywriting` for pitch / sales / product-message decks, and `$paper-writing` for academic prose.\n- Design pass: use `$design-md` for source-material design extraction, `$frontend-design` for a fresh premium direction, and `$visual-review` for rendered PNG evidence before the `$design-md` drift verdict.\n\nAdd source URLs, local asset paths, and review notes here before final delivery.\n",
        format!("{:?}", template).to_ascii_lowercase()
    )
}

pub fn rust_command_manifest_value() -> Value {
    json!({
        "name": "ppt-pptx-rust-commands",
        "runtime": "ppt",
        "commands": {
            "build": "ppt build-qa --workdir . --entry deck.plan.json --deck deck.pptx --rendered-dir rendered",
            "render": "ppt render deck.pptx --output_dir rendered",
            "check_layout": "ppt slides-test deck.pptx --fail-on-any",
            "check_overflow": "ppt slides-test deck.pptx --fail-on-any",
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

include!("yaml_parse.rs");

include!("text_processing.rs");

pub fn detect_outline_pattern(slide: &Value) -> &'static str {
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
pub struct PptxSlideSpec {
    title: String,
    subtitle: String,
    label: String,
    body: Vec<String>,
    notes: String,
    layout: &'static str,
}

pub fn write_outline_deck_pptx(outline: &Value, output: &Path, template: &DeckTemplate) -> Result<()> {
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

include!("slide_specs.rs");

include!("value_helpers.rs");

pub fn write_pptx_package(
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

include!("pptx_xml_templates.rs");

pub fn outline_str<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(default)
}

pub fn qa_summary(deck_path: &str, rendered_dir: &str) -> Result<qa::QaSummary> {
    qa::qa_summary(deck_path, rendered_dir)
}

pub fn strict_quality_gate(payload: &Value) -> Result<()> {
    qa::strict_quality_gate(payload)
}

pub fn render_paths(input: &Path, output_dir: &Path, width: u32, height: u32) -> Result<Vec<PathBuf>> {
    let dpi = if has_extension(input, "pdf") {
        calc_dpi_via_pdf(input, width, height)?
    } else {
        calc_dpi_via_ooxml(input, width, height)?
    };
    rasterize_to_pngs(input, output_dir, dpi)
}

pub fn detect_fonts_payload(input: &Path) -> Result<Value> {
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

pub fn extract_structure_payload(input_path: &str) -> Result<Value> {
    let input = expand_path(input_path);
    let bundle = ZipBundle::from_path(&input)?;
    extract_pptx_structure(&bundle, &input, false, None)
}

pub fn emit_value(value: Value, format: EmitFormat) -> Result<()> {
    match format {
        EmitFormat::Json => println!("{}", serde_json::to_string_pretty(&value)?),
        EmitFormat::Text => print_text_value(&value)?,
    }
    Ok(())
}

pub fn print_text_value(value: &Value) -> Result<()> {
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

pub fn text_value(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)?,
    })
}

pub fn extract_structure_command(args: ExtractStructureArgs) -> Result<()> {
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

pub fn read_full_command(args: ReadFullArgs) -> Result<()> {
    let input = expand_path(&args.input);
    let bundle = ZipBundle::from_path(&input)?;
    let mut structure = extract_pptx_structure(&bundle, &input, false, None)?;
    if let Some(spec) = &args.slides {
        let slide_count = structure
            .get("slide_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let selected = parse_slide_range(spec, slide_count)?;
        structure = filter_structure_slides(structure, &selected)?;
    }
    if args.json {
        let json_str = if args.compact {
            serde_json::to_string(&structure)?
        } else {
            serde_json::to_string_pretty(&structure)?
        };
        println!("{}", json_str);
    } else {
        print!("{}", format_read_full_text(&structure));
    }
    Ok(())
}

pub fn parse_slide_range(spec: &str, max: usize) -> Result<Vec<usize>> {
    let spec = spec.trim();
    if max == 0 {
        bail!("presentation contains no slides");
    }
    if spec.contains('-') {
        let parts: Vec<&str> = spec.splitn(2, '-').collect();
        let start: usize = parts[0]
            .trim()
            .parse()
            .with_context(|| format!("invalid slide range start in {spec}"))?;
        let end: usize = parts[1]
            .trim()
            .parse()
            .with_context(|| format!("invalid slide range end in {spec}"))?;
        if start == 0 || end == 0 || start > end || end > max {
            bail!("slide range {spec} out of bounds (1-{max})");
        }
        Ok((start..=end).collect())
    } else {
        let slide_no: usize = spec
            .parse()
            .with_context(|| format!("invalid slide number in {spec}"))?;
        if slide_no == 0 || slide_no > max {
            bail!("slide {slide_no} out of bounds (1-{max})");
        }
        Ok(vec![slide_no])
    }
}

pub fn filter_structure_slides(structure: Value, selected: &[usize]) -> Result<Value> {
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing slides in structure"))?;
    let selected_set: BTreeSet<usize> = selected.iter().copied().collect();
    let filtered = slides
        .iter()
        .filter(|slide| {
            slide
                .get("index")
                .and_then(Value::as_u64)
                .map(|idx| selected_set.contains(&(idx as usize + 1)))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut out = structure;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("slide_count".to_string(), json!(filtered.len()));
        obj.insert("slides".to_string(), json!(filtered));
    }
    Ok(out)
}

pub fn format_read_full_text(structure: &Value) -> String {
    let file = structure
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("deck.pptx");
    let slide_count = structure
        .get("slide_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut out = format!("FILE: {file}\nSLIDES: {slide_count}\n\n");
    let Some(slides) = structure.get("slides").and_then(Value::as_array) else {
        return out;
    };
    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let slide_no = index + 1;
        out.push_str(&format!("=== Slide {slide_no} ===\n"));
        if let Some(layout) = slide.get("layout").and_then(Value::as_str) {
            if !layout.is_empty() {
                out.push_str(&format!("LAYOUT: {layout}\n"));
            }
        }
        let mut title_lines = Vec::new();
        let mut body_lines = Vec::new();
        let mut warnings = Vec::new();
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            collect_read_full_elements(elements, &mut title_lines, &mut body_lines, &mut warnings);
        }
        out.push_str("TITLE:\n");
        if title_lines.is_empty() {
            out.push_str("(none)\n");
        } else {
            for line in &title_lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str("\nBODY:\n");
        if body_lines.is_empty() {
            out.push_str("(none)\n");
        } else {
            for line in &body_lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str("\nNOTES:\n");
        match slide.get("notes").and_then(Value::as_str) {
            Some(notes) if !notes.trim().is_empty() => {
                out.push_str(notes.trim());
                out.push('\n');
            }
            _ => out.push_str("(none)\n"),
        }
        out.push_str("\nWARNINGS:\n");
        if warnings.is_empty() {
            out.push_str("(none)\n");
        } else {
            for warning in &warnings {
                out.push_str("- ");
                out.push_str(warning);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

pub fn collect_read_full_elements(
    elements: &[Value],
    title_lines: &mut Vec<String>,
    body_lines: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    for element in elements {
        let name = element
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("shape");
        let element_type = element
            .get("type")
            .or_else(|| element.get("element_type"))
            .and_then(Value::as_str)
            .unwrap_or("shape");
        if let Some(text) = element.get("text").and_then(|t| t.get("fullText")).and_then(Value::as_str)
        {
            if !text.trim().is_empty() {
                let lower = name.to_lowercase();
                if lower.contains("title") || lower.contains("subtitle") {
                    title_lines.push(text.trim().to_string());
                } else {
                    body_lines.push(text.trim().to_string());
                }
            }
        }
        if let Some(table) = element.get("table") {
            if let Some(data) = table.get("data").and_then(Value::as_array) {
                let rows = table.get("rows").and_then(Value::as_u64).unwrap_or(0);
                let cols = table.get("cols").and_then(Value::as_u64).unwrap_or(0);
                body_lines.push(format!("TABLE ({rows}x{cols}):"));
                for row in data {
                    if let Some(cells) = row.as_array() {
                        let line = cells
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" | ");
                        if !line.is_empty() {
                            body_lines.push(line);
                        }
                    }
                }
            }
        }
        if element_type == "image" {
            warnings.push(format!("image \"{name}\" (no extractable text)"));
        } else if element_type == "chart" {
            let chart_type = element
                .get("chart")
                .and_then(|c| c.get("chart_type"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            warnings.push(format!("chart \"{name}\" (type: {chart_type})"));
        } else if element_type == "group" {
            if let Some(children) = element.get("children").and_then(Value::as_array) {
                collect_read_full_elements(children, title_lines, body_lines, warnings);
            }
        }
    }
}

pub fn ensure_raster_image_command(args: EnsureRasterImageArgs) -> Result<()> {
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

pub fn create_montage_command(args: CreateMontageArgs) -> Result<()> {
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

pub fn slides_test_command(args: SlidesTestArgs) -> Result<()> {
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
    let mut overflow_failing = Vec::new();
    let mut overlap_failing = Vec::new();
    let mut aesthetic_failing: BTreeSet<usize> = BTreeSet::new();

    for slide_no in layout_rhythm_failing_slides(slides) {
        aesthetic_failing.insert(slide_no);
    }
    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize + 1;
        let mut overflow = false;
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            overflow = elements
                .iter()
                .any(|item| element_overflows(item, slide_w, slide_h));
            if has_text_bbox_overlap(elements) {
                overlap_failing.push(index);
            }
            if has_dense_text_overlap_risk(elements) || has_dense_text_density_risk(elements) {
                aesthetic_failing.insert(index);
            }
            if has_decorative_title_underline_risk(elements, slide_w, slide_h) {
                aesthetic_failing.insert(index);
            }
            if has_ai_copy_slop_risk(elements) {
                aesthetic_failing.insert(index);
            }
        }
        if overflow {
            overflow_failing.push(index);
        }
    }
    if overflow_failing.is_empty() && overlap_failing.is_empty() && aesthetic_failing.is_empty() {
        println!("Test passed. No overflow/overlap/aesthetic risks detected.");
        return Ok(());
    }
    if !overflow_failing.is_empty() {
        print!("ISSUE: Overflow detected on slides (1-based indexing): ");
        for (i, slide_no) in overflow_failing.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{}", slide_no);
        }
        println!();
    }
    if !overlap_failing.is_empty() {
        print!("ISSUE: Overlap detected on slides (1-based indexing): ");
        for (i, slide_no) in overlap_failing.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{}", slide_no);
        }
        println!();
    }
    if !aesthetic_failing.is_empty() {
        print!("ISSUE: Dense text aesthetic risk detected on slides (1-based indexing): ");
        for (i, slide_no) in aesthetic_failing.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{}", slide_no);
        }
        println!();
    }
    if args.fail_on_overflow && !overflow_failing.is_empty() {
        bail!("slides-test failed: content overflow detected");
    }
    if args.fail_on_overlap && !overlap_failing.is_empty() {
        bail!("slides-test failed: content overlap detected");
    }
    if args.fail_on_aesthetic && !aesthetic_failing.is_empty() {
        bail!("slides-test failed: dense text aesthetic risk detected");
    }
    if args.fail_on_any
        && (!overflow_failing.is_empty() || !overlap_failing.is_empty() || !aesthetic_failing.is_empty())
    {
        bail!("slides-test failed: one or more checks reported issues");
    }
    Ok(())
}

pub fn detect_fonts_command(args: DetectFontsArgs) -> Result<()> {
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

pub fn sanitize_pptx_command(args: SanitizePptxArgs) -> Result<()> {
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

    // Security: refuse to write to a symlink (TOCTOU mitigation)
    if let Ok(meta) = std::fs::symlink_metadata(&temp_output) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("refusing to write to symlink: {}", temp_output.display());
        }
    }

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
        // Security: refuse to replace a symlink (TOCTOU mitigation)
        if let Ok(meta) = std::fs::symlink_metadata(&input) {
            if meta.file_type().is_symlink() {
                anyhow::bail!("refusing to replace symlink: {}", input.display());
            }
        }
        fs::rename(&temp_output, &input).with_context(|| {
            format!(
                "failed to replace {} with sanitized output",
                input.display()
            )
        })?;
    }
    Ok(())
}

pub use mcp_stdio_common::util::{expand_path, has_extension};

pub fn default_render_dir(input: &Path) -> PathBuf {
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
        let mut index_by_name = HashMap::new();
        for idx in 0..archive.len() {
            let entry = archive.by_index(idx)?;
            index_by_name.insert(normalize_zip_path(entry.name()), idx);
        }
        Ok(Self {
            archive: RefCell::new(archive),
            index_by_name,
            cache: RefCell::new(HashMap::new()),
        })
    }

    fn contains(&self, path: &str) -> bool {
        self.index_by_name.contains_key(&normalize_zip_path(path))
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let key = normalize_zip_path(path);
        if let Some(cached) = self.cache.borrow().get(&key) {
            return Ok(cached.clone());
        }
        let idx = self
            .index_by_name
            .get(&key)
            .copied()
            .ok_or_else(|| anyhow!("missing zip entry {}", path))?;
        let mut archive = self.archive.borrow_mut();
        let mut entry = archive.by_index(idx)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        self.cache.borrow_mut().insert(key, buf.clone());
        Ok(buf)
    }

    fn text(&self, path: &str) -> Result<String> {
        let data = self.read_bytes(path)?;
        String::from_utf8(data).with_context(|| format!("{} is not valid utf-8 xml", path))
    }

    fn names(&self) -> impl Iterator<Item = &String> {
        self.index_by_name.keys()
    }
}

pub fn normalize_zip_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn normalize_path_like_zip(path: &Path) -> String {
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

pub fn calc_dpi_via_ooxml(input: &Path, max_w_px: u32, max_h_px: u32) -> Result<u32> {
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

pub fn calc_dpi_via_pdf(input: &Path, max_w_px: u32, max_h_px: u32) -> Result<u32> {
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

pub fn parse_pdf_page_size(value: &str) -> Result<(f64, f64)> {
    fn re_pts() -> &'static Regex {
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)\s*pts\b").unwrap())
    }
    fn re_in() -> &'static Regex {
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)\s*in\b").unwrap())
    }
    fn re_bare() -> &'static Regex {
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*x\s*([0-9]+(?:\.[0-9]+)?)").unwrap())
    }
    if let Some(caps) = re_pts().captures(value) {
        return Ok((caps[1].parse()?, caps[2].parse()?));
    }
    if let Some(caps) = re_in().captures(value) {
        return Ok((
            caps[1].parse::<f64>()? * POINTS_PER_INCH,
            caps[2].parse::<f64>()? * POINTS_PER_INCH,
        ));
    }
    if let Some(caps) = re_bare().captures(value) {
        return Ok((caps[1].parse()?, caps[2].parse()?));
    }
    bail!("Unrecognized PDF page size format: {}", value);
}

pub fn rasterize_to_pngs(input: &Path, out_dir: &Path, dpi: u32) -> Result<Vec<PathBuf>> {
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

pub fn collect_prefixed_pngs(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
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

pub fn convert_to_pdf(input: &Path, profile_dir: &Path, convert_dir: &Path) -> Result<PathBuf> {
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

pub fn run_command(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        bail!("command failed with status {:?}", status.code());
    }
    Ok(())
}

pub fn run_command_timeout(command: &mut Command, timeout: Duration) -> Result<()> {
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

pub fn run_command_capture(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn resolve_input_paths(input_files: &[String], input_dir: Option<&str>) -> Result<Vec<PathBuf>> {
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

pub fn supported_image_extension(path: &Path) -> bool {
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

pub fn ensure_raster_image(path: &Path, out_dir: Option<&Path>) -> Result<PathBuf> {
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

pub fn which(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn flate_like_gzip_decoder<'a>(bytes: &'a [u8]) -> Result<Box<dyn Read + 'a>> {
    let cursor = Cursor::new(bytes);
    Ok(Box::new(flate2::read::GzDecoder::new(cursor)))
}

pub fn build_montage(
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
    let rows = items.len().div_ceil(num_col);
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

pub fn resize_to_fit(img: &DynamicImage, max_w: u32, max_h: u32) -> RgbaImage {
    let resized = img.resize(max_w, max_h, FilterType::Lanczos3);
    resized.to_rgba8()
}

pub fn make_placeholder(width: u32, height: u32) -> RgbaImage {
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

pub fn draw_rect_outline(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
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

pub fn draw_text_bitmap(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
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

include!("pptx_extract_qa.rs");

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
