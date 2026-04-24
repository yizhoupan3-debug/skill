use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, canonicalize, File};
use std::io::{BufWriter, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;
use zip::result::ZipError;
use zip::ZipArchive;

const TWIPS_PER_INCH: f64 = 1440.0;
const POINTS_PER_INCH: f64 = 72.0;

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
}

#[derive(Parser)]
struct RenderXlsxArgs {
    workbook: String,
    #[arg(long, default_value = "rendered")]
    outdir: String,
    #[arg(long)]
    png: bool,
    #[arg(long, default_value_t = 144)]
    dpi: u32,
}

#[derive(Parser)]
struct RenderDocxArgs {
    input_path: String,
    #[arg(long, visible_alias = "output_dir")]
    output_dir: Option<String>,
    #[arg(long, default_value_t = 1600)]
    width: u32,
    #[arg(long, default_value_t = 2000)]
    height: u32,
    #[arg(long)]
    dpi: Option<u32>,
}

#[derive(Debug, Clone)]
struct Relationship {
    id: String,
    target: String,
    kind: Option<String>,
}

#[derive(Debug, Clone)]
struct DefinedNameEntry {
    name: String,
    hidden: Option<bool>,
    value: Option<String>,
}

#[derive(Debug, Clone)]
struct SheetMeta {
    name: String,
    state: String,
    rel_id: String,
    index: usize,
}

#[derive(Debug, Clone, Copy)]
struct SheetBounds {
    min_row: usize,
    max_row: usize,
    min_col: usize,
    max_col: usize,
}

#[derive(Debug, Clone)]
struct SheetParseData {
    dimension: String,
    bounds: SheetBounds,
    merged_ranges: usize,
    freeze_panes: Option<String>,
    auto_filter: Option<String>,
    tables: Vec<TableSummary>,
    formula_count: usize,
    data_validation_rules: usize,
    conditional_format_regions: usize,
    chart_count: usize,
    image_count: usize,
}

#[derive(Debug, Serialize, Clone)]
struct TableSummary {
    name: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct DefinedNameSummary {
    name: String,
    hidden: Option<bool>,
    value: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct SheetSummary {
    title: String,
    state: String,
    dimensions: String,
    size_index: String,
    max_row: usize,
    max_column: usize,
    merged_ranges: usize,
    freeze_panes: Option<String>,
    auto_filter: Option<String>,
    tables: Vec<TableSummary>,
    formula_count: usize,
    data_validation_rules: usize,
    conditional_format_regions: usize,
    chart_count: usize,
    image_count: usize,
    print_area: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct WorkbookSummary {
    path: String,
    sheet_count: usize,
    sheet_names: Vec<String>,
    defined_names: Vec<DefinedNameSummary>,
    external_link_count: usize,
    sheets: Vec<SheetSummary>,
}

#[derive(Debug, Serialize, Clone)]
struct DocxPageSize {
    width_inches: f64,
    height_inches: f64,
}

#[derive(Debug, Serialize, Clone)]
struct DocxHeadingSummary {
    level: Option<u8>,
    text: String,
}

#[derive(Debug, Serialize, Clone)]
struct DocxSummary {
    path: String,
    paragraph_count: usize,
    heading_count: usize,
    headings: Vec<DocxHeadingSummary>,
    table_count: usize,
    section_count: usize,
    page_size: Option<DocxPageSize>,
    image_count: usize,
    hyperlink_count: usize,
    footnote_count: usize,
    endnote_count: usize,
    comment_count: usize,
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attr_value(start: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for attr in start.attributes().with_checks(false).flatten() {
        if local_name(attr.key.as_ref()) == key {
            return Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
        }
    }
    None
}

fn parse_bool_flag(value: &str) -> bool {
    matches!(value, "1" | "true" | "TRUE")
}

fn resolve_zip_path(base_part: &str, target: &str) -> String {
    let mut segments: Vec<&str> = base_part
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if !base_part.ends_with('/') && !segments.is_empty() {
        segments.pop();
    }

    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            _ => segments.push(segment),
        }
    }

    segments.join("/")
}

fn rels_path_for(part_path: &str) -> String {
    match part_path.rsplit_once('/') {
        Some((dir, file_name)) => format!("{dir}/_rels/{file_name}.rels"),
        None => format!("_rels/{part_path}.rels"),
    }
}

fn read_zip_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, path: &str) -> Result<String> {
    let mut file = archive
        .by_name(path)
        .with_context(|| format!("Missing OOXML part: {path}"))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn read_zip_entry_optional<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Option<String>> {
    match archive.by_name(path) {
        Ok(mut file) => {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            Ok(Some(content))
        }
        Err(ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn parse_relationships(xml: &str) -> Result<Vec<Relationship>> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut relationships = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) =
                    (attr_value(&event, b"Id"), attr_value(&event, b"Target"))
                {
                    relationships.push(Relationship {
                        id,
                        target,
                        kind: attr_value(&event, b"Type"),
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(relationships)
}

fn parse_workbook_metadata(
    workbook_xml: &str,
) -> Result<(
    Vec<SheetMeta>,
    Vec<DefinedNameEntry>,
    HashMap<usize, String>,
)> {
    struct PendingDefinedName {
        name: String,
        hidden: Option<bool>,
        local_sheet_id: Option<usize>,
        value: String,
    }

    let mut reader = Reader::from_str(workbook_xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut sheets = Vec::new();
    let mut defined_names = Vec::new();
    let mut print_areas = HashMap::new();
    let mut pending_defined_name: Option<PendingDefinedName> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()) == b"sheet" =>
            {
                if let (Some(name), Some(rel_id)) =
                    (attr_value(&event, b"name"), attr_value(&event, b"id"))
                {
                    sheets.push(SheetMeta {
                        name,
                        state: attr_value(&event, b"state")
                            .unwrap_or_else(|| "visible".to_string()),
                        rel_id,
                        index: sheets.len(),
                    });
                }
            }
            Event::Start(event) if local_name(event.name().as_ref()) == b"definedName" => {
                pending_defined_name = Some(PendingDefinedName {
                    name: attr_value(&event, b"name").unwrap_or_default(),
                    hidden: attr_value(&event, b"hidden").map(|value| parse_bool_flag(&value)),
                    local_sheet_id: attr_value(&event, b"localSheetId")
                        .and_then(|value| value.parse().ok()),
                    value: String::new(),
                });
            }
            Event::Text(text) => {
                if let Some(entry) = pending_defined_name.as_mut() {
                    entry.value.push_str(&text.unescape()?.into_owned());
                }
            }
            Event::CData(text) => {
                if let Some(entry) = pending_defined_name.as_mut() {
                    entry
                        .value
                        .push_str(&String::from_utf8_lossy(text.as_ref()));
                }
            }
            Event::End(event) if local_name(event.name().as_ref()) == b"definedName" => {
                if let Some(entry) = pending_defined_name.take() {
                    let value = if entry.value.is_empty() {
                        None
                    } else {
                        Some(entry.value)
                    };

                    if entry.name == "_xlnm.Print_Area" {
                        if let (Some(local_sheet_id), Some(print_area)) =
                            (entry.local_sheet_id, value.clone())
                        {
                            print_areas.insert(local_sheet_id, print_area);
                        }
                    }

                    defined_names.push(DefinedNameEntry {
                        name: entry.name,
                        hidden: entry.hidden,
                        value,
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok((sheets, defined_names, print_areas))
}

fn parse_table_summary<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    part_path: &str,
) -> Result<TableSummary> {
    let xml = read_zip_entry(archive, part_path)?;
    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()) == b"table" =>
            {
                let name =
                    attr_value(&event, b"name").or_else(|| attr_value(&event, b"displayName"));
                let reference = attr_value(&event, b"ref");
                return Ok(TableSummary { name, reference });
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(TableSummary {
        name: None,
        reference: None,
    })
}

fn parse_drawing_counts<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    part_path: &str,
) -> Result<(usize, usize)> {
    let rels_path = rels_path_for(part_path);
    let Some(rels_xml) = read_zip_entry_optional(archive, &rels_path)? else {
        return Ok((0, 0));
    };

    let mut chart_count = 0;
    let mut image_count = 0;
    for relationship in parse_relationships(&rels_xml)? {
        if let Some(kind) = relationship.kind.as_deref() {
            if kind.ends_with("/chart") {
                chart_count += 1;
            } else if kind.ends_with("/image") {
                image_count += 1;
            }
        }
    }

    Ok((chart_count, image_count))
}

fn column_label_to_index(label: &str) -> Option<usize> {
    if label.is_empty() {
        return None;
    }

    let mut value = 0usize;
    for byte in label.bytes() {
        if !byte.is_ascii_alphabetic() {
            return None;
        }
        value = value * 26 + usize::from(byte.to_ascii_uppercase() - b'A' + 1);
    }
    Some(value)
}

fn column_index_to_label(mut index: usize) -> String {
    if index == 0 {
        return "A".to_string();
    }

    let mut label = String::new();
    while index > 0 {
        let rem = (index - 1) % 26;
        label.insert(0, char::from(b'A' + rem as u8));
        index = (index - 1) / 26;
    }
    label
}

fn parse_cell_ref(reference: &str) -> Option<(usize, usize)> {
    let mut column = String::new();
    let mut row = String::new();

    for ch in reference.chars() {
        if ch == '$' {
            continue;
        }
        if ch.is_ascii_alphabetic() && row.is_empty() {
            column.push(ch);
        } else if ch.is_ascii_digit() {
            row.push(ch);
        } else {
            return None;
        }
    }

    Some((row.parse().ok()?, column_label_to_index(&column)?))
}

fn parse_dimension(reference: &str) -> Option<SheetBounds> {
    let (start, end) = match reference.split_once(':') {
        Some((start, end)) => (start, end),
        None => (reference, reference),
    };

    let (min_row, min_col) = parse_cell_ref(start)?;
    let (max_row, max_col) = parse_cell_ref(end)?;

    Some(SheetBounds {
        min_row,
        max_row,
        min_col,
        max_col,
    })
}

fn format_dimension(bounds: SheetBounds) -> String {
    let start = format!(
        "{}{}",
        column_index_to_label(bounds.min_col),
        bounds.min_row
    );
    let end = format!(
        "{}{}",
        column_index_to_label(bounds.max_col),
        bounds.max_row
    );
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn format_size_index(bounds: SheetBounds) -> String {
    format!(
        "{}:{} x {}:{}",
        bounds.min_row, bounds.max_row, bounds.min_col, bounds.max_col
    )
}

fn update_bounds(bounds: &mut Option<SheetBounds>, row: usize, col: usize) {
    match bounds {
        Some(existing) => {
            existing.min_row = existing.min_row.min(row);
            existing.max_row = existing.max_row.max(row);
            existing.min_col = existing.min_col.min(col);
            existing.max_col = existing.max_col.max(col);
        }
        None => {
            *bounds = Some(SheetBounds {
                min_row: row,
                max_row: row,
                min_col: col,
                max_col: col,
            });
        }
    }
}

fn parse_sheet_data<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    part_path: &str,
) -> Result<SheetParseData> {
    let sheet_xml = read_zip_entry(archive, part_path)?;
    let rels_path = rels_path_for(part_path);
    let rels = read_zip_entry_optional(archive, &rels_path)?
        .map(|xml| parse_relationships(&xml))
        .transpose()?
        .unwrap_or_default();
    let rel_map: HashMap<String, String> = rels
        .iter()
        .map(|relationship| (relationship.id.clone(), relationship.target.clone()))
        .collect();

    let mut reader = Reader::from_str(&sheet_xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut dimension = None;
    let mut observed_bounds = None;
    let mut current_row = None;
    let mut merged_ranges = 0usize;
    let mut freeze_panes = None;
    let mut auto_filter = None;
    let mut formula_count = 0usize;
    let mut data_validation_rules = 0usize;
    let mut conditional_format_regions = 0usize;
    let mut table_targets = Vec::new();
    let mut drawing_targets = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => match local_name(event.name().as_ref()) {
                b"dimension" => {
                    if dimension.is_none() {
                        dimension = attr_value(&event, b"ref");
                    }
                }
                b"row" => {
                    current_row =
                        attr_value(&event, b"r").and_then(|value| value.parse::<usize>().ok());
                }
                b"c" => {
                    if let Some(reference) = attr_value(&event, b"ref") {
                        if let Some((row, col)) = parse_cell_ref(&reference) {
                            update_bounds(&mut observed_bounds, row, col);
                        }
                    } else if let Some(row) = current_row {
                        update_bounds(&mut observed_bounds, row, 1);
                    }
                }
                b"f" => {
                    formula_count += 1;
                }
                b"mergeCell" => {
                    merged_ranges += 1;
                }
                b"pane" => {
                    if freeze_panes.is_none() {
                        freeze_panes = attr_value(&event, b"topLeftCell");
                    }
                }
                b"autoFilter" => {
                    auto_filter = attr_value(&event, b"ref");
                }
                b"dataValidation" => {
                    data_validation_rules += 1;
                }
                b"conditionalFormatting" => {
                    conditional_format_regions += 1;
                }
                b"tablePart" => {
                    if let Some(rel_id) = attr_value(&event, b"id") {
                        if let Some(target) = rel_map.get(&rel_id) {
                            table_targets.push(resolve_zip_path(part_path, target));
                        }
                    }
                }
                b"drawing" => {
                    if let Some(rel_id) = attr_value(&event, b"id") {
                        if let Some(target) = rel_map.get(&rel_id) {
                            drawing_targets.push(resolve_zip_path(part_path, target));
                        }
                    }
                }
                _ => {}
            },
            Event::Empty(event) => match local_name(event.name().as_ref()) {
                b"dimension" => {
                    if dimension.is_none() {
                        dimension = attr_value(&event, b"ref");
                    }
                }
                b"row" => {
                    current_row = None;
                }
                b"c" => {
                    if let Some(reference) = attr_value(&event, b"ref") {
                        if let Some((row, col)) = parse_cell_ref(&reference) {
                            update_bounds(&mut observed_bounds, row, col);
                        }
                    } else if let Some(row) = current_row {
                        update_bounds(&mut observed_bounds, row, 1);
                    }
                }
                b"f" => {
                    formula_count += 1;
                }
                b"mergeCell" => {
                    merged_ranges += 1;
                }
                b"pane" => {
                    if freeze_panes.is_none() {
                        freeze_panes = attr_value(&event, b"topLeftCell");
                    }
                }
                b"autoFilter" => {
                    auto_filter = attr_value(&event, b"ref");
                }
                b"dataValidation" => {
                    data_validation_rules += 1;
                }
                b"conditionalFormatting" => {
                    conditional_format_regions += 1;
                }
                b"tablePart" => {
                    if let Some(rel_id) = attr_value(&event, b"id") {
                        if let Some(target) = rel_map.get(&rel_id) {
                            table_targets.push(resolve_zip_path(part_path, target));
                        }
                    }
                }
                b"drawing" => {
                    if let Some(rel_id) = attr_value(&event, b"id") {
                        if let Some(target) = rel_map.get(&rel_id) {
                            drawing_targets.push(resolve_zip_path(part_path, target));
                        }
                    }
                }
                _ => {}
            },
            Event::End(event) if local_name(event.name().as_ref()) == b"row" => {
                current_row = None;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let bounds = dimension
        .as_deref()
        .and_then(parse_dimension)
        .or(observed_bounds)
        .unwrap_or(SheetBounds {
            min_row: 1,
            max_row: 1,
            min_col: 1,
            max_col: 1,
        });
    let dimension = dimension.unwrap_or_else(|| format_dimension(bounds));

    let mut tables = Vec::new();
    for target in table_targets {
        tables.push(parse_table_summary(archive, &target)?);
    }

    let mut chart_count = 0usize;
    let mut image_count = 0usize;
    for target in drawing_targets {
        let (charts, images) = parse_drawing_counts(archive, &target)?;
        chart_count += charts;
        image_count += images;
    }

    Ok(SheetParseData {
        dimension,
        bounds,
        merged_ranges,
        freeze_panes,
        auto_filter,
        tables,
        formula_count,
        data_validation_rules,
        conditional_format_regions,
        chart_count,
        image_count,
    })
}

fn inspect_xlsx_summary(path: &Path) -> Result<WorkbookSummary> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let workbook_xml = read_zip_entry(&mut archive, "xl/workbook.xml")?;
    let workbook_rels_xml = read_zip_entry(&mut archive, "xl/_rels/workbook.xml.rels")?;
    let workbook_rels = parse_relationships(&workbook_rels_xml)?;
    let rel_map: HashMap<String, String> = workbook_rels
        .iter()
        .map(|relationship| (relationship.id.clone(), relationship.target.clone()))
        .collect();

    let (sheet_meta, defined_names, print_areas) = parse_workbook_metadata(&workbook_xml)?;
    let mut sheets = Vec::new();
    let mut sheet_names = Vec::new();

    for meta in &sheet_meta {
        sheet_names.push(meta.name.clone());
        let target = rel_map
            .get(&meta.rel_id)
            .ok_or_else(|| anyhow!("Missing workbook relationship for {}", meta.rel_id))?;
        let sheet_part = resolve_zip_path("xl/workbook.xml", target);
        let parsed = parse_sheet_data(&mut archive, &sheet_part)?;
        sheets.push(SheetSummary {
            title: meta.name.clone(),
            state: meta.state.clone(),
            dimensions: parsed.dimension,
            size_index: format_size_index(parsed.bounds),
            max_row: parsed.bounds.max_row,
            max_column: parsed.bounds.max_col,
            merged_ranges: parsed.merged_ranges,
            freeze_panes: parsed.freeze_panes,
            auto_filter: parsed.auto_filter,
            tables: parsed.tables,
            formula_count: parsed.formula_count,
            data_validation_rules: parsed.data_validation_rules,
            conditional_format_regions: parsed.conditional_format_regions,
            chart_count: parsed.chart_count,
            image_count: parsed.image_count,
            print_area: print_areas.get(&meta.index).cloned(),
        });
    }

    let defined_names = defined_names
        .into_iter()
        .map(|entry| DefinedNameSummary {
            name: entry.name,
            hidden: entry.hidden,
            value: entry.value,
        })
        .collect();

    let external_link_count = archive
        .file_names()
        .filter(|name| name.starts_with("xl/externalLinks/") && name.ends_with(".xml"))
        .count();

    Ok(WorkbookSummary {
        path: canonicalize(path)?.to_string_lossy().into_owned(),
        sheet_count: sheet_names.len(),
        sheet_names,
        defined_names,
        external_link_count,
        sheets,
    })
}

fn print_xlsx_text(summary: &WorkbookSummary) {
    println!("Workbook: {}", summary.path);
    println!(
        "Sheets ({}): {}",
        summary.sheet_count,
        summary.sheet_names.join(", ")
    );
    println!("Defined names: {}", summary.defined_names.len());
    println!("External links: {}", summary.external_link_count);
    println!();

    for sheet in &summary.sheets {
        println!(
            "[{}] state={} range={} formulas={}",
            sheet.title, sheet.state, sheet.dimensions, sheet.formula_count
        );
        println!(
            "  merged={} tables={} validations={} conditional={}",
            sheet.merged_ranges,
            sheet.tables.len(),
            sheet.data_validation_rules,
            sheet.conditional_format_regions
        );
        println!(
            "  freeze_panes={} auto_filter={} print_area={}",
            sheet.freeze_panes.as_deref().unwrap_or("None"),
            sheet.auto_filter.as_deref().unwrap_or("None"),
            sheet.print_area.as_deref().unwrap_or("None")
        );
        if !sheet.tables.is_empty() {
            for table in &sheet.tables {
                println!(
                    "  table: {} {}",
                    table.name.as_deref().unwrap_or("None"),
                    table.reference.as_deref().unwrap_or("None")
                );
            }
        }
        println!();
    }
}

fn inspect_xlsx(input: &str, as_json: bool) -> Result<()> {
    let summary = inspect_xlsx_summary(Path::new(input))?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_xlsx_text(&summary);
    }
    Ok(())
}

fn parse_docx_heading_level(style_id: &str) -> Option<u8> {
    let compact = style_id
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .collect::<String>()
        .to_ascii_lowercase();
    compact
        .strip_prefix("heading")
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .filter(|level| *level > 0)
}

fn inspect_docx_summary(path: &Path) -> Result<DocxSummary> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("failed to read docx zip archive")?;
    let document_xml = read_zip_entry(&mut archive, "word/document.xml")?;
    let rels = read_zip_entry_optional(&mut archive, "word/_rels/document.xml.rels")?
        .map(|xml| parse_relationships(&xml))
        .transpose()?
        .unwrap_or_default();

    let mut reader = Reader::from_str(&document_xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut paragraph_count = 0usize;
    let mut table_count = 0usize;
    let mut section_count = 0usize;
    let mut hyperlink_count = 0usize;
    let mut image_count = 0usize;
    let mut page_size = None;
    let mut headings = Vec::new();
    let mut current_paragraph_style: Option<String> = None;
    let mut current_paragraph_text = String::new();
    let mut in_paragraph = false;
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => match local_name(event.name().as_ref()) {
                b"p" => {
                    paragraph_count += 1;
                    in_paragraph = true;
                    current_paragraph_style = None;
                    current_paragraph_text.clear();
                }
                b"tbl" => {
                    table_count += 1;
                }
                b"hyperlink" => {
                    hyperlink_count += 1;
                }
                b"drawing" | b"pict" => {
                    image_count += 1;
                }
                b"pStyle" if in_paragraph => {
                    current_paragraph_style = attr_value(&event, b"val");
                }
                b"t" if in_paragraph => {
                    in_text = true;
                }
                b"pgSz" => {
                    if page_size.is_none() {
                        if let (Some(width), Some(height)) =
                            (attr_value(&event, b"w"), attr_value(&event, b"h"))
                        {
                            let width = width.parse::<f64>()?;
                            let height = height.parse::<f64>()?;
                            if width > 0.0 && height > 0.0 {
                                page_size = Some(DocxPageSize {
                                    width_inches: width / TWIPS_PER_INCH,
                                    height_inches: height / TWIPS_PER_INCH,
                                });
                            }
                        }
                    }
                }
                b"sectPr" => {
                    section_count += 1;
                }
                _ => {}
            },
            Event::Empty(event) => match local_name(event.name().as_ref()) {
                b"tbl" => table_count += 1,
                b"hyperlink" => hyperlink_count += 1,
                b"drawing" | b"pict" => image_count += 1,
                b"pStyle" if in_paragraph => {
                    current_paragraph_style = attr_value(&event, b"val");
                }
                b"pgSz" => {
                    if page_size.is_none() {
                        if let (Some(width), Some(height)) =
                            (attr_value(&event, b"w"), attr_value(&event, b"h"))
                        {
                            let width = width.parse::<f64>()?;
                            let height = height.parse::<f64>()?;
                            if width > 0.0 && height > 0.0 {
                                page_size = Some(DocxPageSize {
                                    width_inches: width / TWIPS_PER_INCH,
                                    height_inches: height / TWIPS_PER_INCH,
                                });
                            }
                        }
                    }
                }
                b"sectPr" => section_count += 1,
                _ => {}
            },
            Event::Text(text) if in_text => {
                current_paragraph_text.push_str(&text.unescape()?.into_owned());
            }
            Event::CData(text) if in_text => {
                current_paragraph_text.push_str(&String::from_utf8_lossy(text.as_ref()));
            }
            Event::End(event) => match local_name(event.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => {
                    if let Some(style) = current_paragraph_style.as_deref() {
                        if let Some(level) = parse_docx_heading_level(style) {
                            let text = current_paragraph_text.trim();
                            if !text.is_empty() {
                                headings.push(DocxHeadingSummary {
                                    level: Some(level),
                                    text: text.to_string(),
                                });
                            }
                        }
                    }
                    in_paragraph = false;
                    in_text = false;
                    current_paragraph_style = None;
                    current_paragraph_text.clear();
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let footnote_count = read_zip_entry_optional(&mut archive, "word/footnotes.xml")?
        .as_deref()
        .map(count_docx_note_items)
        .transpose()?
        .unwrap_or(0);
    let endnote_count = read_zip_entry_optional(&mut archive, "word/endnotes.xml")?
        .as_deref()
        .map(count_docx_note_items)
        .transpose()?
        .unwrap_or(0);
    let comment_count = read_zip_entry_optional(&mut archive, "word/comments.xml")?
        .as_deref()
        .map(count_docx_comment_items)
        .transpose()?
        .unwrap_or(0);
    let relationship_image_count = rels
        .iter()
        .filter(|relationship| {
            relationship
                .kind
                .as_deref()
                .map(|kind| kind.ends_with("/image"))
                .unwrap_or(false)
        })
        .count();

    Ok(DocxSummary {
        path: canonicalize(path)?.to_string_lossy().into_owned(),
        paragraph_count,
        heading_count: headings.len(),
        headings,
        table_count,
        section_count,
        page_size,
        image_count: image_count.max(relationship_image_count),
        hyperlink_count,
        footnote_count,
        endnote_count,
        comment_count,
    })
}

fn count_docx_note_items(xml: &str) -> Result<usize> {
    count_xml_elements(xml, b"footnote").or_else(|_| count_xml_elements(xml, b"endnote"))
}

fn count_docx_comment_items(xml: &str) -> Result<usize> {
    count_xml_elements(xml, b"comment")
}

fn count_xml_elements(xml: &str, element_name: &[u8]) -> Result<usize> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut count = 0usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()) == element_name =>
            {
                count += 1;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(count)
}

fn print_docx_text(summary: &DocxSummary) {
    println!("Document: {}", summary.path);
    println!("Paragraphs: {}", summary.paragraph_count);
    println!("Headings: {}", summary.heading_count);
    println!("Tables: {}", summary.table_count);
    println!("Sections: {}", summary.section_count);
    println!("Images: {}", summary.image_count);
    println!("Hyperlinks: {}", summary.hyperlink_count);
    println!(
        "Footnotes: {} Endnotes: {} Comments: {}",
        summary.footnote_count, summary.endnote_count, summary.comment_count
    );
    if let Some(page_size) = &summary.page_size {
        println!(
            "Page size: {:.2} x {:.2} in",
            page_size.width_inches, page_size.height_inches
        );
    }
    if !summary.headings.is_empty() {
        println!();
        println!("Heading outline:");
        for heading in &summary.headings {
            let level = heading
                .level
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            println!("  H{} {}", level, heading.text);
        }
    }
}

fn inspect_docx(input: &str, as_json: bool) -> Result<()> {
    let summary = inspect_docx_summary(Path::new(input))?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_docx_text(&summary);
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

fn default_render_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rendered");
    input.parent().unwrap_or_else(|| Path::new(".")).join(stem)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

fn run_command(command: &mut Command) -> Result<()> {
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

fn run_command_capture(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn convert_to_pdf(input: &Path, profile_dir: &Path, convert_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(convert_dir)?;
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid input stem"))?;
    let pdf_path = convert_dir.join(format!("{stem}.pdf"));
    let profile = format!("file://{}", profile_dir.display());

    let mut direct = Command::new("soffice");
    direct
        .arg(format!("-env:UserInstallation={profile}"))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(convert_dir)
        .arg(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let _ = run_command(&mut direct);
    if pdf_path.exists() {
        return Ok(pdf_path);
    }

    if has_extension(input, "docx")
        || has_extension(input, "docm")
        || has_extension(input, "dotx")
        || has_extension(input, "dotm")
    {
        let odt_path = convert_dir.join(format!("{stem}.odt"));
        let mut to_odt = Command::new("soffice");
        to_odt
            .arg(format!("-env:UserInstallation={profile}"))
            .arg("--invisible")
            .arg("--headless")
            .arg("--norestore")
            .arg("--convert-to")
            .arg("odt")
            .arg("--outdir")
            .arg(convert_dir)
            .arg(input)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let _ = run_command(&mut to_odt);

        if odt_path.exists() {
            let mut odt_to_pdf = Command::new("soffice");
            odt_to_pdf
                .arg(format!("-env:UserInstallation={profile}"))
                .arg("--invisible")
                .arg("--headless")
                .arg("--norestore")
                .arg("--convert-to")
                .arg("pdf")
                .arg("--outdir")
                .arg(convert_dir)
                .arg(&odt_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let _ = run_command(&mut odt_to_pdf);
            if pdf_path.exists() {
                return Ok(pdf_path);
            }
        }
    }

    bail!("Failed to produce PDF for {}", input.display())
}

fn render_xlsx_pdf(workbook: &Path, outdir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(outdir)?;
    let profile = TempDir::new().context("failed to create soffice profile")?;
    let temp_input_dir = TempDir::new().context("failed to create render input dir")?;
    let temp_input = temp_input_dir.path().join(
        workbook
            .file_name()
            .ok_or_else(|| anyhow!("invalid workbook filename"))?,
    );
    fs::copy(workbook, &temp_input).with_context(|| {
        format!(
            "failed to copy {} to {}",
            workbook.display(),
            temp_input.display()
        )
    })?;
    let pdf = convert_to_pdf(&temp_input, profile.path(), outdir)?;
    Ok(pdf)
}

fn render_pdf_to_pngs(pdf: &Path, outdir: &Path, dpi: u32, prefix: &str) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(outdir)?;
    let output_prefix = outdir.join(prefix);
    run_command(
        Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg(dpi.to_string())
            .arg(pdf)
            .arg(&output_prefix)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .context("pdftoppm failed")?;
    let mut files = collect_prefixed_pngs(outdir, prefix)?;
    files.sort();
    Ok(files)
}

fn collect_prefixed_pngs(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("png") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if file_name.starts_with(prefix) {
            files.push(path);
        }
    }
    Ok(files)
}

fn render_xlsx(args: RenderXlsxArgs) -> Result<()> {
    let workbook = expand_path(&args.workbook);
    if !workbook.is_file() {
        bail!("Workbook not found: {}", workbook.display());
    }
    let outdir = expand_path(&args.outdir);
    let pdf = render_xlsx_pdf(&workbook, &outdir)?;
    println!("PDF: {}", pdf.display());
    if args.png {
        let prefix = pdf
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("page");
        render_pdf_to_pngs(&pdf, &outdir, args.dpi, prefix)?;
        println!("PNG prefix: {}-*.png", outdir.join(prefix).display());
    }
    Ok(())
}

fn docx_page_size(input: &Path) -> Result<(f64, f64)> {
    let file = File::open(input).with_context(|| format!("failed to open {}", input.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read docx zip archive")?;
    let xml = read_zip_entry(&mut archive, "word/document.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut in_section = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) if local_name(event.name().as_ref()) == b"sectPr" => {
                in_section = true;
            }
            Event::End(event) if local_name(event.name().as_ref()) == b"sectPr" => {
                in_section = false;
            }
            Event::Start(event) | Event::Empty(event)
                if in_section && local_name(event.name().as_ref()) == b"pgSz" =>
            {
                let width = attr_value(&event, b"w")
                    .ok_or_else(|| anyhow!("page width missing in document.xml"))?
                    .parse::<f64>()?;
                let height = attr_value(&event, b"h")
                    .ok_or_else(|| anyhow!("page height missing in document.xml"))?
                    .parse::<f64>()?;
                if width <= 0.0 || height <= 0.0 {
                    bail!("invalid page size values in document.xml");
                }
                return Ok((width / TWIPS_PER_INCH, height / TWIPS_PER_INCH));
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    bail!("Page size not found in document.xml")
}

fn pdf_page_size(pdf: &Path) -> Result<(f64, f64)> {
    let output = run_command_capture(
        Command::new("pdfinfo")
            .arg(pdf)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .context("pdfinfo failed")?;
    let line = output
        .lines()
        .find_map(|line| line.strip_prefix("Page size:"))
        .map(str::trim)
        .ok_or_else(|| anyhow!("failed to read PDF page size"))?;
    parse_pdf_page_size(line)
}

fn parse_pdf_page_size(value: &str) -> Result<(f64, f64)> {
    let mut numbers = Vec::new();
    for token in value.split_whitespace() {
        let token = token.trim_matches(|ch: char| ch == ',' || ch == ';');
        if let Ok(number) = token.parse::<f64>() {
            numbers.push(number);
            if numbers.len() == 2 {
                break;
            }
        }
    }
    if numbers.len() != 2 {
        bail!("Unrecognized PDF page size format: {value}");
    }
    let mut width = numbers[0];
    let mut height = numbers[1];
    if value.contains(" pts") || value.contains(" pt") {
        width /= POINTS_PER_INCH;
        height /= POINTS_PER_INCH;
    }
    if width <= 0.0 || height <= 0.0 {
        bail!("Invalid PDF page size values");
    }
    Ok((width, height))
}

fn docx_render_dpi(input: &Path, width: u32, height: u32) -> Result<u32> {
    let page_size = if has_extension(input, "pdf") {
        pdf_page_size(input)
    } else if has_extension(input, "docx")
        || has_extension(input, "docm")
        || has_extension(input, "dotx")
        || has_extension(input, "dotm")
    {
        docx_page_size(input)
    } else {
        bail!("not a DOCX container")
    };
    let (width_in, height_in) = match page_size {
        Ok(value) => value,
        Err(_) => {
            let profile = TempDir::new().context("failed to create soffice profile")?;
            let convert_dir = TempDir::new().context("failed to create convert dir")?;
            let pdf = convert_to_pdf(input, profile.path(), convert_dir.path())?;
            pdf_page_size(&pdf)?
        }
    };
    Ok(((width as f64 / width_in).min(height as f64 / height_in)).round() as u32)
}

fn rasterize_docx(input: &Path, outdir: &Path, dpi: u32) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(outdir)?;
    let profile = TempDir::new().context("failed to create soffice profile")?;
    let convert_dir = TempDir::new().context("failed to create convert dir")?;
    let pdf = if has_extension(input, "pdf") {
        input.to_path_buf()
    } else {
        convert_to_pdf(input, profile.path(), convert_dir.path())?
    };
    let generated = render_pdf_to_pngs(&pdf, outdir, dpi, "page")?;
    let mut final_paths = Vec::new();
    for (index, src) in generated.iter().enumerate() {
        let dest = outdir.join(format!("page-{}.png", index + 1));
        if *src != dest {
            fs::rename(src, &dest)?;
        }
        final_paths.push(dest);
    }
    Ok(final_paths)
}

fn render_docx(args: RenderDocxArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    if !input.is_file() {
        bail!("Input not found: {}", input.display());
    }
    let outdir = args
        .output_dir
        .as_deref()
        .map(expand_path)
        .unwrap_or_else(|| default_render_dir(&input));
    let dpi = args
        .dpi
        .map(Ok)
        .unwrap_or_else(|| docx_render_dpi(&input, args.width, args.height))?;
    rasterize_docx(&input, &outdir, dpi)?;
    println!("Pages rendered to {}", outdir.display());
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
        println!("{json_out}");
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Xlsx { input, json } => inspect_xlsx(&input, json)?,
        Commands::Docx { input, json } => inspect_docx(&input, json)?,
        Commands::RenderXlsx(args) => render_xlsx(args)?,
        Commands::RenderDocx(args) => render_docx(args)?,
        Commands::Pptx {
            input,
            output,
            extract_images: _,
        } => extract_pptx(&input, output)?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::FileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    fn temp_xlsx_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}.xlsx", name, unique));
        path
    }

    fn write_zip_entry<W: Write + Seek>(zip: &mut ZipWriter<W>, path: &str, content: &[u8]) {
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file(path, options).unwrap();
        zip.write_all(content).unwrap();
    }

    fn build_test_workbook(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);

        write_zip_entry(
            &mut zip,
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Visible" sheetId="1" r:id="rId1"/>
    <sheet name="Hidden" sheetId="2" state="hidden" r:id="rId2"/>
  </sheets>
  <definedNames>
    <definedName name="_xlnm.Print_Area" localSheetId="0">'Visible'!$A$1:$C$4</definedName>
    <definedName name="LocalRange" hidden="1">Visible!$A$2:$A$3</definedName>
  </definedNames>
</workbook>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/sheet1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
           xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C4"/>
  <sheetViews>
    <sheetView workbookViewId="0">
      <pane xSplit="1" ySplit="1" topLeftCell="B2" state="frozen"/>
    </sheetView>
  </sheetViews>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1"><f>SUM(A2:A3)</f></c>
    </row>
    <row r="2">
      <c r="A2"><v>1</v></c>
    </row>
    <row r="3">
      <c r="A3"><f>A2*2</f></c>
    </row>
  </sheetData>
  <autoFilter ref="A1:C4"/>
  <mergeCells count="1">
    <mergeCell ref="A1:A2"/>
  </mergeCells>
  <conditionalFormatting sqref="B2:B4">
    <cfRule type="expression" priority="1"><formula>1</formula></cfRule>
  </conditionalFormatting>
  <dataValidations count="1">
    <dataValidation type="whole" sqref="C2:C4"/>
  </dataValidations>
  <drawing r:id="rId2"/>
  <tableParts count="1">
    <tablePart r:id="rId1"/>
  </tableParts>
</worksheet>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/_rels/sheet1.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/sheet2.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A1"/>
  <sheetData>
    <row r="1"><c r="A1"><v>42</v></c></row>
  </sheetData>
</worksheet>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/tables/table1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
       id="1"
       name="Table1"
       displayName="Table1"
       ref="A1:C4"/>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/drawings/drawing1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/drawings/_rels/drawing1.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/charts/chart1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><chartSpace xmlns="http://schemas.openxmlformats.org/drawingml/2006/chart"/>"#,
        );
        write_zip_entry(&mut zip, "xl/media/image1.png", b"not-a-real-png");
        write_zip_entry(
            &mut zip,
            "xl/externalLinks/externalLink1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><externalLink xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>"#,
        );

        zip.finish().unwrap();
    }

    fn temp_docx_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}.docx", name, unique));
        path
    }

    fn build_test_document(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);

        write_zip_entry(
            &mut zip,
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/document.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Executive Summary</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text</w:t></w:r></w:p>
    <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
    <w:p><w:hyperlink r:id="rIdHyper"><w:r><w:t>Link</w:t></w:r></w:hyperlink></w:p>
    <w:p><w:r><w:drawing/></w:r></w:p>
    <w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr>
  </w:body>
</w:document>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdHyper" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/footnotes.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="1"><w:p/></w:footnote>
</w:footnotes>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/comments.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="0"><w:p/></w:comment>
</w:comments>"#,
        );
        write_zip_entry(&mut zip, "word/media/image1.png", b"not-a-real-png");

        zip.finish().unwrap();
    }

    #[test]
    fn inspect_xlsx_summary_preserves_workbook_structure() {
        let path = temp_xlsx_path("ooxml_parser_rs_fixture");
        build_test_workbook(&path);

        let summary = inspect_xlsx_summary(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(summary.sheet_count, 2);
        assert_eq!(summary.sheet_names, vec!["Visible", "Hidden"]);
        assert_eq!(summary.external_link_count, 1);
        assert_eq!(summary.defined_names.len(), 2);
        assert_eq!(summary.sheets[0].state, "visible");
        assert_eq!(summary.sheets[0].dimensions, "A1:C4");
        assert_eq!(summary.sheets[0].size_index, "1:4 x 1:3");
        assert_eq!(summary.sheets[0].freeze_panes.as_deref(), Some("B2"));
        assert_eq!(summary.sheets[0].auto_filter.as_deref(), Some("A1:C4"));
        assert_eq!(
            summary.sheets[0].print_area.as_deref(),
            Some("'Visible'!$A$1:$C$4")
        );
        assert_eq!(summary.sheets[0].formula_count, 2);
        assert_eq!(summary.sheets[0].merged_ranges, 1);
        assert_eq!(summary.sheets[0].data_validation_rules, 1);
        assert_eq!(summary.sheets[0].conditional_format_regions, 1);
        assert_eq!(summary.sheets[0].chart_count, 1);
        assert_eq!(summary.sheets[0].image_count, 1);
        assert_eq!(summary.sheets[0].tables.len(), 1);
        assert_eq!(summary.sheets[0].tables[0].name.as_deref(), Some("Table1"));
        assert_eq!(
            summary.sheets[0].tables[0].reference.as_deref(),
            Some("A1:C4")
        );
        assert_eq!(summary.sheets[1].state, "hidden");
        assert_eq!(summary.sheets[1].dimensions, "A1:A1");
    }

    #[test]
    fn inspect_docx_summary_reports_document_structure() {
        let path = temp_docx_path("ooxml_parser_rs_docx_fixture");
        build_test_document(&path);

        let summary = inspect_docx_summary(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(summary.paragraph_count, 5);
        assert_eq!(summary.heading_count, 1);
        assert_eq!(summary.headings[0].level, Some(1));
        assert_eq!(summary.headings[0].text, "Executive Summary");
        assert_eq!(summary.table_count, 1);
        assert_eq!(summary.section_count, 1);
        assert_eq!(summary.image_count, 1);
        assert_eq!(summary.hyperlink_count, 1);
        assert_eq!(summary.footnote_count, 1);
        assert_eq!(summary.comment_count, 1);
        let page_size = summary.page_size.unwrap();
        assert_eq!(page_size.width_inches, 8.5);
        assert_eq!(page_size.height_inches, 11.0);
    }

    #[test]
    fn parse_dimension_handles_single_cells_and_ranges() {
        let single = parse_dimension("B3").unwrap();
        assert_eq!(single.min_row, 3);
        assert_eq!(single.max_col, 2);

        let range = parse_dimension("$C$2:$F$9").unwrap();
        assert_eq!(range.min_col, 3);
        assert_eq!(range.max_row, 9);
        assert_eq!(format_dimension(range), "C2:F9");
    }
}
