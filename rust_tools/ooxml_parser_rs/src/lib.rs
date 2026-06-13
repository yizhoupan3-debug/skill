pub mod batch;
pub mod mcp;
pub mod schema;

use anyhow::{anyhow, bail, Context, Result};
use calamine::{open_workbook, Data, Reader as CalamineReader, Xlsx};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader as XmlReader;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, canonicalize, File};
use std::io::{BufWriter, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;
use zip::result::ZipError;
use zip::ZipArchive;

pub use mcp_stdio_common::util::{expand_path, file_sha256, has_extension};

// ---------------------------------------------------------------------------
// OoxmlKind — lightweight file-kind enum for batch dispatch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OoxmlKind {
    Docx,
    Xlsx,
    Pptx,
    Unsupported,
}

pub fn detect_ooxml_kind(path: &Path) -> OoxmlKind {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| match ext.as_str() {
            "docx" => OoxmlKind::Docx,
            "xlsx" => OoxmlKind::Xlsx,
            "pptx" => OoxmlKind::Pptx,
            _ => OoxmlKind::Unsupported,
        })
        .unwrap_or(OoxmlKind::Unsupported)
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const TWIPS_PER_INCH: f64 = 1440.0;
pub const POINTS_PER_INCH: f64 = 72.0;

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Relationship parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub target: String,
    pub kind: Option<String>,
}

pub fn parse_relationships(xml: &str) -> Result<Vec<Relationship>> {
    let mut reader = XmlReader::from_str(xml);
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

// ---------------------------------------------------------------------------
// XLSX types and functions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DefinedNameEntry {
    pub name: String,
    pub hidden: Option<bool>,
    pub value: Option<String>,
}

pub type WorkbookMetadata = (
    Vec<SheetMeta>,
    Vec<DefinedNameEntry>,
    HashMap<usize, String>,
);

#[derive(Debug, Clone)]
pub struct SheetMeta {
    pub name: String,
    pub state: String,
    pub rel_id: String,
    pub index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SheetBounds {
    pub min_row: usize,
    pub max_row: usize,
    pub min_col: usize,
    pub max_col: usize,
}

#[derive(Debug, Clone)]
pub struct SheetParseData {
    pub dimension: String,
    pub bounds: SheetBounds,
    pub merged_ranges: usize,
    pub freeze_panes: Option<String>,
    pub auto_filter: Option<String>,
    pub tables: Vec<TableSummary>,
    pub formula_count: usize,
    pub data_validation_rules: usize,
    pub conditional_format_regions: usize,
    pub chart_count: usize,
    pub image_count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct TableSummary {
    pub name: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DefinedNameSummary {
    pub name: String,
    pub hidden: Option<bool>,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SheetSummary {
    pub title: String,
    pub state: String,
    pub dimensions: String,
    pub size_index: String,
    pub max_row: usize,
    pub max_column: usize,
    pub merged_ranges: usize,
    pub freeze_panes: Option<String>,
    pub auto_filter: Option<String>,
    pub tables: Vec<TableSummary>,
    pub formula_count: usize,
    pub data_validation_rules: usize,
    pub conditional_format_regions: usize,
    pub chart_count: usize,
    pub image_count: usize,
    pub print_area: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkbookSummary {
    pub path: String,
    pub sheet_count: usize,
    pub sheet_names: Vec<String>,
    pub defined_names: Vec<DefinedNameSummary>,
    pub external_link_count: usize,
    pub sheets: Vec<SheetSummary>,
}

pub fn column_label_to_index(label: &str) -> Option<usize> {
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

pub fn column_index_to_label(mut index: usize) -> String {
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

pub fn parse_cell_ref(reference: &str) -> Option<(usize, usize)> {
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

pub fn parse_dimension(reference: &str) -> Option<SheetBounds> {
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

pub fn format_dimension(bounds: SheetBounds) -> String {
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

pub fn format_size_index(bounds: SheetBounds) -> String {
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

pub fn parse_workbook_metadata(workbook_xml: &str) -> Result<WorkbookMetadata> {
    struct PendingDefinedName {
        name: String,
        hidden: Option<bool>,
        local_sheet_id: Option<usize>,
        value: String,
    }

    let mut reader = XmlReader::from_str(workbook_xml);
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
                    entry.value.push_str(&text.unescape()?);
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

pub fn parse_table_summary<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    part_path: &str,
) -> Result<TableSummary> {
    let xml = read_zip_entry(archive, part_path)?;
    let mut reader = XmlReader::from_str(&xml);
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

pub fn parse_drawing_counts<R: Read + Seek>(
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

pub fn parse_sheet_data<R: Read + Seek>(
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

    let mut reader = XmlReader::from_str(&sheet_xml);
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

pub fn inspect_xlsx_summary(path: &Path) -> Result<WorkbookSummary> {
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

pub fn inspect_xlsx(input: &str, as_json: bool) -> Result<()> {
    let summary = inspect_xlsx_summary(Path::new(input))?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_xlsx_text(&summary);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DOCX types and functions
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct DocxPageSize {
    pub width_inches: f64,
    pub height_inches: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DocxHeadingSummary {
    pub level: Option<u8>,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DocxSummary {
    pub path: String,
    pub paragraph_count: usize,
    pub heading_count: usize,
    pub headings: Vec<DocxHeadingSummary>,
    pub table_count: usize,
    pub section_count: usize,
    pub page_size: Option<DocxPageSize>,
    pub image_count: usize,
    pub hyperlink_count: usize,
    pub footnote_count: usize,
    pub endnote_count: usize,
    pub comment_count: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocxBlock {
    Paragraph {
        text: String,
        heading_level: Option<u8>,
    },
    Table {
        rows: Vec<Vec<String>>,
    },
    Image,
}

#[derive(Debug, Serialize, Clone)]
pub struct DocxNote {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct DocxReadOutput {
    pub path: String,
    pub blocks: Vec<DocxBlock>,
    pub footnotes: Vec<DocxNote>,
    pub endnotes: Vec<DocxNote>,
    pub comments: Vec<DocxNote>,
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

pub fn inspect_docx_summary(path: &Path) -> Result<DocxSummary> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("failed to read docx zip archive")?;
    let document_xml = read_zip_entry(&mut archive, "word/document.xml")?;
    let rels = read_zip_entry_optional(&mut archive, "word/_rels/document.xml.rels")?
        .map(|xml| parse_relationships(&xml))
        .transpose()?
        .unwrap_or_default();

    let mut reader = XmlReader::from_str(&document_xml);
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
                current_paragraph_text.push_str(&text.unescape()?);
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
    let mut reader = XmlReader::from_str(xml);
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

pub fn inspect_docx(input: &str, as_json: bool) -> Result<()> {
    let summary = inspect_docx_summary(Path::new(input))?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_docx_text(&summary);
    }
    Ok(())
}

pub fn parse_docx_notes(xml: &str, element_name: &[u8]) -> Result<Vec<DocxNote>> {
    let mut reader = XmlReader::from_str(xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut notes = Vec::new();
    let mut current: Option<(String, String)> = None;
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => {
                if local_name(event.name().as_ref()) == element_name {
                    if let Some(id) = attr_value(&event, b"id") {
                        current = Some((id, String::new()));
                    }
                } else if local_name(event.name().as_ref()) == b"t" && current.is_some() {
                    in_text = true;
                }
            }
            Event::Empty(event) if local_name(event.name().as_ref()) == element_name => {
                if let Some(id) = attr_value(&event, b"id") {
                    notes.push(DocxNote {
                        id,
                        text: String::new(),
                    });
                }
            }
            Event::Text(text) if in_text => {
                if let Some((_, ref mut text_buf)) = current {
                    text_buf.push_str(&text.unescape()?);
                }
            }
            Event::CData(text) if in_text => {
                if let Some((_, ref mut text_buf)) = current {
                    text_buf.push_str(&String::from_utf8_lossy(text.as_ref()));
                }
            }
            Event::End(event) => match local_name(event.name().as_ref()) {
                b"t" => in_text = false,
                name if name == element_name => {
                    if let Some((id, text)) = current.take() {
                        notes.push(DocxNote {
                            id,
                            text: text.trim().to_string(),
                        });
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    notes.retain(|note| note.id != "-1" && !note.text.is_empty());
    Ok(notes)
}

pub fn read_docx_content(path: &Path) -> Result<DocxReadOutput> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("failed to read docx zip archive")?;
    let document_xml = read_zip_entry(&mut archive, "word/document.xml")?;

    let mut reader = XmlReader::from_str(&document_xml);
    reader.trim_text(false);
    let mut buf = Vec::new();
    let mut blocks = Vec::new();

    let mut in_paragraph = false;
    let mut in_text = false;
    let mut in_table = false;
    let mut in_row = false;
    let mut in_cell = false;
    let mut current_paragraph_style: Option<String> = None;
    let mut current_paragraph_text = String::new();
    let mut current_table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => match local_name(event.name().as_ref()) {
                b"p" if !in_table => {
                    in_paragraph = true;
                    current_paragraph_style = None;
                    current_paragraph_text.clear();
                }
                b"tbl" => {
                    in_table = true;
                    current_table_rows.clear();
                }
                b"tr" if in_table => {
                    in_row = true;
                    current_row.clear();
                }
                b"tc" if in_table => {
                    in_cell = true;
                    current_cell_text.clear();
                }
                b"pStyle" if in_paragraph && !in_cell => {
                    current_paragraph_style = attr_value(&event, b"val");
                }
                b"t" if (in_paragraph && !in_table) || in_cell => in_text = true,
                b"drawing" | b"pict" if in_paragraph && !in_table => {
                    blocks.push(DocxBlock::Image);
                }
                _ => {}
            },
            Event::Empty(event) => match local_name(event.name().as_ref()) {
                b"pStyle" if in_paragraph && !in_cell => {
                    current_paragraph_style = attr_value(&event, b"val");
                }
                b"drawing" | b"pict" if in_paragraph && !in_table => {
                    blocks.push(DocxBlock::Image);
                }
                _ => {}
            },
            Event::Text(text) if in_text => {
                let chunk = text.unescape()?;
                if in_cell {
                    current_cell_text.push_str(&chunk);
                } else {
                    current_paragraph_text.push_str(&chunk);
                }
            }
            Event::CData(text) if in_text => {
                let chunk = String::from_utf8_lossy(text.as_ref());
                if in_cell {
                    current_cell_text.push_str(&chunk);
                } else {
                    current_paragraph_text.push_str(&chunk);
                }
            }
            Event::End(event) => match local_name(event.name().as_ref()) {
                b"t" => in_text = false,
                b"p" if in_paragraph && !in_table => {
                    let text = current_paragraph_text.trim().to_string();
                    if !text.is_empty() {
                        let heading_level = current_paragraph_style
                            .as_deref()
                            .and_then(parse_docx_heading_level);
                        blocks.push(DocxBlock::Paragraph {
                            text,
                            heading_level,
                        });
                    }
                    in_paragraph = false;
                    in_text = false;
                    current_paragraph_style = None;
                    current_paragraph_text.clear();
                }
                b"tc" if in_cell => {
                    current_row.push(current_cell_text.trim().to_string());
                    current_cell_text.clear();
                    in_cell = false;
                }
                b"tr" if in_row => {
                    if !current_row.is_empty() {
                        current_table_rows.push(std::mem::take(&mut current_row));
                    }
                    in_row = false;
                }
                b"tbl" if in_table => {
                    if !current_table_rows.is_empty() {
                        blocks.push(DocxBlock::Table {
                            rows: std::mem::take(&mut current_table_rows),
                        });
                    }
                    in_table = false;
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let footnotes = read_zip_entry_optional(&mut archive, "word/footnotes.xml")?
        .as_deref()
        .map(|xml| parse_docx_notes(xml, b"footnote"))
        .transpose()?
        .unwrap_or_default();
    let endnotes = read_zip_entry_optional(&mut archive, "word/endnotes.xml")?
        .as_deref()
        .map(|xml| parse_docx_notes(xml, b"endnote"))
        .transpose()?
        .unwrap_or_default();
    let comments = read_zip_entry_optional(&mut archive, "word/comments.xml")?
        .as_deref()
        .map(|xml| parse_docx_notes(xml, b"comment"))
        .transpose()?
        .unwrap_or_default();

    Ok(DocxReadOutput {
        path: canonicalize(path)?.to_string_lossy().into_owned(),
        blocks,
        footnotes,
        endnotes,
        comments,
    })
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

pub fn format_markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let col_count = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if col_count == 0 {
        return String::new();
    }
    let mut lines = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let mut cells = row
            .iter()
            .map(|cell| escape_markdown_cell(cell))
            .collect::<Vec<_>>();
        cells.resize(col_count, String::new());
        lines.push(format!("| {} |", cells.join(" | ")));
        if index == 0 {
            lines.push(format!("|{}|", " --- |".repeat(col_count)));
        }
    }
    lines.join("\n")
}

pub fn docx_read_text_string(output: &DocxReadOutput) -> String {
    let mut out = String::new();
    for block in &output.blocks {
        match block {
            DocxBlock::Paragraph {
                text,
                heading_level,
            } => {
                if let Some(level) = heading_level {
                    let hashes = "#".repeat(*level as usize);
                    writeln!(out, "{hashes} {text}").ok();
                } else {
                    writeln!(out, "{text}").ok();
                }
                writeln!(out).ok();
            }
            DocxBlock::Table { rows } => {
                writeln!(out, "{}", format_markdown_table(rows)).ok();
                writeln!(out).ok();
            }
            DocxBlock::Image => {
                writeln!(out, "[image]").ok();
                writeln!(out).ok();
            }
        }
    }
    if !output.footnotes.is_empty() {
        writeln!(out, "## Footnotes").ok();
        for note in &output.footnotes {
            if !note.text.is_empty() {
                writeln!(out, "[^{}]: {}", note.id, note.text).ok();
            }
        }
        writeln!(out).ok();
    }
    if !output.endnotes.is_empty() {
        writeln!(out, "## Endnotes").ok();
        for note in &output.endnotes {
            if !note.text.is_empty() {
                writeln!(out, "[^{}]: {}", note.id, note.text).ok();
            }
        }
        writeln!(out).ok();
    }
    if !output.comments.is_empty() {
        writeln!(out, "## Comments").ok();
        for note in &output.comments {
            if !note.text.is_empty() {
                writeln!(out, "- [{}] {}", note.id, note.text).ok();
            }
        }
    }
    out
}

fn print_docx_read_text(output: &DocxReadOutput) {
    print!("{}", docx_read_text_string(output));
}

pub fn read_docx(input: &str, as_json: bool, compact: bool) -> Result<()> {
    let output = read_docx_content(Path::new(input))?;
    if as_json {
        let payload = if compact {
            serde_json::to_string(&output)?
        } else {
            serde_json::to_string_pretty(&output)?
        };
        println!("{payload}");
    } else {
        print_docx_read_text(&output);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// XLSX content reading
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct XlsxSheetRead {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct XlsxReadOutput {
    pub path: String,
    pub sheets: Vec<XlsxSheetRead>,
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("#{value:?}"),
    }
}

pub fn read_xlsx_content(
    path: &Path,
    max_rows: usize,
    sheet_filter: &[String],
) -> Result<XlsxReadOutput> {
    let mut workbook: Xlsx<_> = open_workbook(path).context("failed to open xlsx workbook")?;
    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets = Vec::new();

    for sheet_name in sheet_names {
        if !sheet_filter.is_empty() && !sheet_filter.iter().any(|name| name == &sheet_name) {
            continue;
        }
        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("failed to read sheet {sheet_name}"))?;
        let mut rows = Vec::new();
        let mut truncated = false;
        for (index, row) in range.rows().enumerate() {
            if index >= max_rows {
                truncated = true;
                break;
            }
            rows.push(row.iter().map(cell_to_string).collect());
        }
        sheets.push(XlsxSheetRead {
            name: sheet_name,
            rows,
            truncated,
        });
    }

    Ok(XlsxReadOutput {
        path: canonicalize(path)?.to_string_lossy().into_owned(),
        sheets,
    })
}

pub fn xlsx_read_text_string(output: &XlsxReadOutput) -> String {
    let mut out = String::new();
    for sheet in &output.sheets {
        writeln!(out, "## {}", sheet.name).ok();
        if sheet.rows.is_empty() {
            writeln!(out).ok();
            continue;
        }
        writeln!(out, "{}", format_markdown_table(&sheet.rows)).ok();
        if sheet.truncated {
            writeln!(out).ok();
            writeln!(out, "(truncated)").ok();
        }
        writeln!(out).ok();
    }
    out
}

fn print_xlsx_read_text(output: &XlsxReadOutput) {
    print!("{}", xlsx_read_text_string(output));
}

pub fn read_xlsx(input: &str, max_rows: usize, sheets: &[String], as_json: bool, compact: bool) -> Result<()> {
    let output = read_xlsx_content(Path::new(input), max_rows, sheets)?;
    if as_json {
        let payload = if compact {
            serde_json::to_string(&output)?
        } else {
            serde_json::to_string_pretty(&output)?
        };
        println!("{payload}");
    } else {
        print_xlsx_read_text(&output);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// PPTX content reading (for batch delegation)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct PptxReadOutput {
    pub path: String,
    pub slide_count: u32,
    pub slides: Vec<PptxSlideText>,
}

#[derive(Debug)]
pub struct PptxSlideText {
    pub slide_number: u32,
    pub text: String,
}

/// Read PPTX slide text content by scanning slide XML parts in the zip archive.
pub fn read_pptx_content(path: &Path) -> Result<PptxReadOutput> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut slides = Vec::new();

    // Collect slide part names in order
    let mut slide_parts: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_parts.push(name);
        }
    }
    slide_parts.sort();

    for (idx, part_name) in slide_parts.iter().enumerate() {
        let xml = read_zip_entry(&mut archive, part_name)?;
        let mut text_parts = Vec::new();
        let mut reader = XmlReader::from_str(&xml);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let mut in_text = false;

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(event) if local_name(event.name().as_ref()) == b"t" => {
                    in_text = true;
                }
                Event::Text(text) if in_text => {
                    if let Ok(s) = text.unescape() {
                        text_parts.push(s.to_string());
                    }
                }
                Event::End(event) if local_name(event.name().as_ref()) == b"t" => {
                    in_text = false;
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        slides.push(PptxSlideText {
            slide_number: (idx + 1) as u32,
            text: text_parts.join(" "),
        });
    }

    Ok(PptxReadOutput {
        path: canonicalize(path)?.to_string_lossy().into_owned(),
        slide_count: slides.len() as u32,
        slides,
    })
}

/// Format PPTX read output as a linear text string.
pub fn pptx_read_text_string(output: &PptxReadOutput) -> String {
    let mut out = String::new();
    writeln!(out, "FILE: {}", output.path).ok();
    writeln!(out, "SLIDES: {}", output.slide_count).ok();
    writeln!(out).ok();
    for slide in &output.slides {
        writeln!(out, "=== Slide {} ===", slide.slide_number).ok();
        if slide.text.is_empty() {
            writeln!(out, "(no text)").ok();
        } else {
            writeln!(out, "{}", slide.text).ok();
        }
        writeln!(out).ok();
    }
    out
}

// ---------------------------------------------------------------------------
// Rendering helpers (used by CLI render subcommands)
// ---------------------------------------------------------------------------

fn default_render_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rendered");
    input.parent().unwrap_or_else(|| Path::new(".")).join(stem)
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

pub fn render_xlsx_pdf(workbook: &Path, outdir: &Path) -> Result<PathBuf> {
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

// ---------------------------------------------------------------------------
// PPTX extract (CLI subcommand)
// ---------------------------------------------------------------------------

pub fn extract_pptx(input: &str, output: Option<String>) -> Result<()> {
    let file = File::open(input)?;
    let mut archive = ZipArchive::new(file)?;

    let mut slide_count = 0;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().starts_with("ppt/slides/slide") && file.name().ends_with(".xml") {
            slide_count += 1;
        }
    }

    let summary = serde_json::json!({
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

// ---------------------------------------------------------------------------
// Rendering CLI functions
// ---------------------------------------------------------------------------

pub fn render_xlsx_cli(args: RenderXlsxArgs) -> Result<()> {
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
    let mut reader = XmlReader::from_str(&xml);
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

pub fn render_docx_cli(args: RenderDocxArgs) -> Result<()> {
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

// ---------------------------------------------------------------------------
// CLI argument structs (used by main.rs)
// ---------------------------------------------------------------------------

#[derive(clap::Parser)]
pub struct RenderXlsxArgs {
    pub workbook: String,
    #[arg(long, default_value = "rendered")]
    pub outdir: String,
    #[arg(long)]
    pub png: bool,
    #[arg(long, default_value_t = 144)]
    pub dpi: u32,
}

#[derive(clap::Parser)]
pub struct RenderDocxArgs {
    pub input_path: String,
    #[arg(long, visible_alias = "output_dir")]
    pub output_dir: Option<String>,
    #[arg(long, default_value_t = 1600)]
    pub width: u32,
    #[arg(long, default_value_t = 2000)]
    pub height: u32,
    #[arg(long)]
    pub dpi: Option<u32>,
}

// ---------------------------------------------------------------------------
// Tests (from original main.rs)
// ---------------------------------------------------------------------------


#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
