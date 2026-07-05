//! Prose QC verification tool — terminology consistency, slop detection, hedging analysis.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};

pub(super) fn tool_verification_prose(arguments: &Value) -> Result<String, FrameworkError> {
    let check =
        arguments
            .get("check")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "prose verification requires 'check' (terminology|slop|hedging)",
            ))?;
    match check {
        "terminology" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "terminology check requires 'text' (string)",
                    ))?;
            let glossary = arguments
                .get("glossary")
                .and_then(Value::as_object)
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let violations =
                crate::verification::prose_qc::check_terminology_consistency(text, &glossary)
                    .map_err(|e| {
                        FrameworkError::validation(format!("terminology check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "terminology_consistency", "violations": violations,
                "has_violations": !violations.is_empty(),
            }))
            .map_err(FrameworkError::Json)
        }
        "slop" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "slop check requires 'text' (string)",
                    ))?;
            let language = arguments
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("en");
            let hits = match language {
                "zh" | "chinese" => crate::verification::prose_qc::detect_zh_slop(text),
                _ => crate::verification::prose_qc::detect_en_slop(text),
            };
            serde_json::to_string_pretty(&json!({
                "check": "slop_detection", "language": language,
                "hits_found": hits.len(), "hits": hits,
            }))
            .map_err(FrameworkError::Json)
        }
        "hedging" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "hedging check requires 'text' (string)",
                    ))?;
            let count = crate::verification::prose_qc::count_hedging_words(text);
            serde_json::to_string_pretty(&json!({
                "check": "hedging_analysis", "hedging_word_count": count,
                "suggestion": if count > 5 { "High hedging density — consider firming up language".to_string() }
                    else if count > 2 { "Moderate hedging — review for unnecessary qualifiers".to_string() }
                    else { "Hedging count is acceptable".to_string() },
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown prose check: {check}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::super::handle_research_tool;
    use serde_json::json;
}
