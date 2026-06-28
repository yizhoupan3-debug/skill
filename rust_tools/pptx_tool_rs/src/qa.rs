use crate::{
    EmitFormat, ZipBundle, detect_fonts_payload, element_overflows, emit_value, expand_path,
    extract_pptx_structure, has_ai_copy_slop_risk, has_decorative_title_underline_risk,
    has_dense_text_density_risk, has_dense_text_overlap_risk, has_text_bbox_overlap,
    layout_rhythm_failing_slides, render_paths,
};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

use crate::QaArgs;

#[derive(Debug, Serialize)]
pub struct QaRenderSummary {
    pub rendered_dir: String,
    pub png_count: usize,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct QaOverflowSummary {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct QaAestheticSummary {
    pub ok: bool,
    pub failing_slides: Vec<usize>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct QaSummary {
    pub ok: bool,
    pub deck: String,
    pub render: QaRenderSummary,
    pub overflow_check: QaOverflowSummary,
    pub overlap_check: QaOverflowSummary,
    pub aesthetic_check: QaAestheticSummary,
    pub font_check: Value,
    pub inspector: Value,
}

pub fn qa_command(args: QaArgs) -> Result<()> {
    let payload = qa_summary(&args.deck, &args.rendered_dir)?;
    emit_value(
        serde_json::to_value(&payload)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )?;
    if args.fail_on_issues && !payload.ok {
        bail!("qa failed: overflow, overlap, aesthetic, font, or Rust inspector issue detected");
    }
    Ok(())
}

pub fn qa_summary(deck_path: &str, rendered_dir: &str) -> Result<QaSummary> {
    let deck = expand_path(deck_path);
    let rendered_dir_path = expand_path(rendered_dir);
    let rendered = render_paths(&deck, &rendered_dir_path, 1600, 900)?;
    let overflow = slide_overflow_summary(&deck)?;
    let overlap = slide_overlap_summary(&deck)?;
    let aesthetic = slide_aesthetic_summary(&deck)?;
    let font_check = detect_fonts_payload(&deck)?;
    let inspector = crate::office::office_doctor_value(&deck.display().to_string())?;
    let ok = overflow.ok
        && overlap.ok
        && aesthetic.ok
        && font_check_ok(&font_check)
        && inspector_ok(&inspector);
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
        overlap_check: overlap,
        aesthetic_check: aesthetic,
        font_check,
        inspector,
    })
}

pub fn strict_quality_gate(payload: &Value) -> Result<()> {
    if payload.pointer("/ok").and_then(Value::as_bool) == Some(false) {
        bail!("strict quality failed: combined QA status is false");
    }
    let overflow_ok = payload
        .pointer("/overflow_check/ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("strict quality failed: overflow check status missing"))?;
    if !overflow_ok {
        bail!("strict quality failed: slide overflow detected");
    }
    let overlap_ok = payload
        .pointer("/overlap_check/ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("strict quality failed: overlap check status missing"))?;
    if !overlap_ok {
        bail!("strict quality failed: slide overlap detected");
    }
    let aesthetic_ok = payload
        .pointer("/aesthetic_check/ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("strict quality failed: aesthetic check status missing"))?;
    if !aesthetic_ok {
        let failing = payload
            .pointer("/aesthetic_check/failing_slides")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if failing.is_empty() {
            bail!("strict quality failed: aesthetic check reported dense text overlap risk");
        }
        bail!(
            "strict quality failed: aesthetic check reported dense text overlap risk on slides {}",
            failing
        );
    }
    let font_ok = payload
        .pointer("/font_check/ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("strict quality failed: font check status missing"))?;
    if !font_ok {
        bail!("strict quality failed: font check reported issues");
    }
    let inspector_validation_ok = payload
        .pointer("/inspector/validation/ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("strict quality failed: inspector validation status missing"))?;
    if !inspector_validation_ok {
        bail!("strict quality failed: Rust inspector validation failed");
    }
    let inspector_issue_count = payload
        .pointer("/inspector/issues/count")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("strict quality failed: inspector issue count missing"))?;
    if inspector_issue_count > 0 {
        bail!("strict quality failed: Rust inspector reported deck issues");
    }
    Ok(())
}

pub fn font_check_ok(payload: &Value) -> bool {
    payload.get("ok").and_then(Value::as_bool).unwrap_or(false)
}

pub fn inspector_ok(payload: &Value) -> bool {
    let validation_ok = payload
        .pointer("/validation/ok")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let issue_count = payload
        .pointer("/issues/count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    validation_ok && issue_count == 0
}

pub fn slide_overflow_summary(input: &Path) -> Result<QaOverflowSummary> {
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

pub fn slide_overlap_summary(input: &Path) -> Result<QaOverflowSummary> {
    let bundle = ZipBundle::from_path(input)?;
    let structure = extract_pptx_structure(&bundle, input, false, None)?;
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing slides"))?;
    let mut failing = Vec::new();
    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize + 1;
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            if has_text_bbox_overlap(elements) {
                failing.push(index);
            }
        }
    }
    if failing.is_empty() {
        return Ok(QaOverflowSummary {
            ok: true,
            stdout: "Test passed. No overlap detected.".to_string(),
            stderr: String::new(),
        });
    }
    Ok(QaOverflowSummary {
        ok: false,
        stdout: format!(
            "ERROR: Slides with overlapping text shape bounding boxes (1-based indexing): {}",
            failing
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        stderr: String::new(),
    })
}

pub fn slide_aesthetic_summary(input: &Path) -> Result<QaAestheticSummary> {
    let bundle = ZipBundle::from_path(input)?;
    let structure = extract_pptx_structure(&bundle, input, false, None)?;
    let slides = structure
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing slides"))?;
    let slide_w = structure
        .get("slide_width")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_width"))?;
    let slide_h = structure
        .get("slide_height")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("missing slide_height"))?;

    let mut failing: BTreeSet<usize> = BTreeSet::new();
    failing.extend(layout_rhythm_failing_slides(slides));

    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize + 1;
        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            if has_dense_text_overlap_risk(elements)
                || has_dense_text_density_risk(elements)
                || has_decorative_title_underline_risk(elements, slide_w, slide_h)
                || has_ai_copy_slop_risk(elements)
            {
                failing.insert(index);
            }
        }
    }
    let failing_list: Vec<usize> = failing.into_iter().collect();
    if failing_list.is_empty() {
        return Ok(QaAestheticSummary {
            ok: true,
            failing_slides: Vec::new(),
            stdout: "Test passed. Slide aesthetic check ok.".to_string(),
            stderr: String::new(),
        });
    }
    Ok(QaAestheticSummary {
        ok: false,
        failing_slides: failing_list.clone(),
        stdout: format!(
            "ERROR: Slides reporting layout density, slop words or alignment risk (1-based): {}",
            failing_list
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        stderr: String::new(),
    })
}
