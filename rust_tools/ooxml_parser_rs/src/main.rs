use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{canonicalize, File};
use std::io::{BufWriter, Read, Seek};
use std::path::Path;
use zip::result::ZipError;
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
    let mut segments: Vec<&str> = base_part.split('/').filter(|segment| !segment.is_empty()).collect();
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

fn read_zip_entry_optional<R: Read + Seek>(archive: &mut ZipArchive<R>, path: &str) -> Result<Option<String>> {
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
            Event::Start(event) | Event::Empty(event) if local_name(event.name().as_ref()) == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr_value(&event, b"Id"), attr_value(&event, b"Target")) {
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
) -> Result<(Vec<SheetMeta>, Vec<DefinedNameEntry>, HashMap<usize, String>)> {
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
            Event::Start(event) | Event::Empty(event) if local_name(event.name().as_ref()) == b"sheet" => {
                if let (Some(name), Some(rel_id)) = (attr_value(&event, b"name"), attr_value(&event, b"id")) {
                    sheets.push(SheetMeta {
                        name,
                        state: attr_value(&event, b"state").unwrap_or_else(|| "visible".to_string()),
                        rel_id,
                        index: sheets.len(),
                    });
                }
            }
            Event::Start(event) if local_name(event.name().as_ref()) == b"definedName" => {
                pending_defined_name = Some(PendingDefinedName {
                    name: attr_value(&event, b"name").unwrap_or_default(),
                    hidden: attr_value(&event, b"hidden").map(|value| parse_bool_flag(&value)),
                    local_sheet_id: attr_value(&event, b"localSheetId").and_then(|value| value.parse().ok()),
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
                    entry.value.push_str(&String::from_utf8_lossy(text.as_ref()));
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
                        if let (Some(local_sheet_id), Some(print_area)) = (entry.local_sheet_id, value.clone()) {
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

fn parse_table_summary<R: Read + Seek>(archive: &mut ZipArchive<R>, part_path: &str) -> Result<TableSummary> {
    let xml = read_zip_entry(archive, part_path)?;
    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) | Event::Empty(event) if local_name(event.name().as_ref()) == b"table" => {
                let name = attr_value(&event, b"name").or_else(|| attr_value(&event, b"displayName"));
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

fn parse_drawing_counts<R: Read + Seek>(archive: &mut ZipArchive<R>, part_path: &str) -> Result<(usize, usize)> {
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
    let start = format!("{}{}", column_index_to_label(bounds.min_col), bounds.min_row);
    let end = format!("{}{}", column_index_to_label(bounds.max_col), bounds.max_row);
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

fn parse_sheet_data<R: Read + Seek>(archive: &mut ZipArchive<R>, part_path: &str) -> Result<SheetParseData> {
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
                    current_row = attr_value(&event, b"r").and_then(|value| value.parse::<usize>().ok());
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
            Event::Empty(event) => {
                match local_name(event.name().as_ref()) {
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
                }
            }
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
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::io::Write;
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
        assert_eq!(summary.sheets[0].print_area.as_deref(), Some("'Visible'!$A$1:$C$4"));
        assert_eq!(summary.sheets[0].formula_count, 2);
        assert_eq!(summary.sheets[0].merged_ranges, 1);
        assert_eq!(summary.sheets[0].data_validation_rules, 1);
        assert_eq!(summary.sheets[0].conditional_format_regions, 1);
        assert_eq!(summary.sheets[0].chart_count, 1);
        assert_eq!(summary.sheets[0].image_count, 1);
        assert_eq!(summary.sheets[0].tables.len(), 1);
        assert_eq!(summary.sheets[0].tables[0].name.as_deref(), Some("Table1"));
        assert_eq!(summary.sheets[0].tables[0].reference.as_deref(), Some("A1:C4"));
        assert_eq!(summary.sheets[1].state, "hidden");
        assert_eq!(summary.sheets[1].dimensions, "A1:A1");
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
