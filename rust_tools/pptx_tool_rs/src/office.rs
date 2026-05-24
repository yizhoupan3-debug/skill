use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use regex::Regex;
use crate::{
    ZipBundle, extract_pptx_structure, extract_structure_payload, expand_path,
    EmitFormat, emit_value, xml_escape, element_overflows,
    OfficeArgs, OfficeCommands, OfficeProbeArgs, OfficeDoctorArgs,
    OfficeGetArgs, OfficeQueryArgs, OfficeWatchArgs, OfficeBatchArgs,
};

#[derive(Debug, Serialize)]
pub struct OfficeProbeSummary {
    pub available: bool,
    pub engine: String,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OfficeDoctorSummary {
    pub inspector_version: Option<String>,
    pub file: String,
    pub outline: Value,
    pub issues: Value,
    pub validation: Value,
}

pub fn office_command(args: OfficeArgs) -> Result<()> {
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

pub fn office_probe_command(args: OfficeProbeArgs) -> Result<()> {
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

pub fn office_doctor_command(args: OfficeDoctorArgs) -> Result<()> {
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

pub fn office_file_passthrough(
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

pub fn office_get_command(args: OfficeGetArgs) -> Result<()> {
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

pub fn office_query_command(args: OfficeQueryArgs) -> Result<()> {
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

pub fn office_watch_command(args: OfficeWatchArgs) -> Result<()> {
    let html_path = write_rust_office_preview(&args.file, args.port)?;
    println!(
        "Rust PPTX Live inspector preview generated at: {}",
        html_path.display()
    );
    println!("Watching directory for changes... press Ctrl+C to stop.");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

pub fn office_batch_command(args: OfficeBatchArgs) -> Result<()> {
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

pub fn office_doctor_value(file: &str) -> Result<Value> {
    Ok(serde_json::to_value(office_doctor_summary(file)?)?)
}

pub fn office_doctor_summary(file: &str) -> Result<OfficeDoctorSummary> {
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

pub fn rust_office_outline_value(file: &str) -> Result<Value> {
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

pub fn rust_office_issues_value(file: &str) -> Result<Value> {
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

pub fn rust_office_validate_value(file: &str) -> Result<Value> {
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

pub fn rust_office_get_value(file: &str, selector: &str, depth: i32) -> Result<Value> {
    let structure = extract_structure_payload(file)?;
    let selected = select_structure_path(&structure, selector)?;
    Ok(json!({
        "success": true,
        "selector": selector,
        "depth": depth,
        "data": trim_json_depth(selected, depth.max(0) as usize),
    }))
}

pub fn rust_office_query_value(file: &str, selector: &str, text: Option<&str>) -> Result<Value> {
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

pub fn write_rust_office_preview(file: &str, _port: u16) -> Result<std::path::PathBuf> {
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

pub fn rust_office_batch_value(
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

pub fn first_slide_title(slide: &Value) -> Option<String> {
    slide_texts(slide)
        .into_iter()
        .map(|text| text.trim().to_string())
        .find(|text| !text.is_empty())
}

pub fn slide_texts(slide: &Value) -> Vec<String> {
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

pub fn select_structure_path(root: &Value, selector: &str) -> Result<Value> {
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

pub fn trim_json_depth(value: Value, depth: usize) -> Value {
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

pub fn query_structure(structure: &Value, selector: &str, text: Option<&str>) -> Vec<Value> {
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

pub fn summarize_office_doctor(
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

pub fn print_office_doctor_summary(summary: &OfficeDoctorSummary) {
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
        "validation: ok={} message=\"{}\"",
        summary
            .validation
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        summary
            .validation
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
    );
}
