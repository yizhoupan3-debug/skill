//! Structure verification tool — LaTeX compilation check, figure reference consistency.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};

pub(super) fn tool_verification_structure(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "structure verification requires 'check' (latex|figures)",
        ))?;
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "structure verification requires 'path' (string)",
        ))?;
    let tex_path = std::path::Path::new(path);
    match check {
        "latex" => {
            let compilable =
                crate::verification::structure::check_latex_compilable(tex_path).map_err(|e| {
                    FrameworkError::validation(format!("latex check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "latex", "path": path, "compilable": compilable,
            }))
            .map_err(FrameworkError::Json)
        }
        "figures" => {
            let missing =
                crate::verification::structure::check_figure_references(tex_path).map_err(|e| {
                    FrameworkError::validation(format!("figure check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "figures", "path": path, "missing_refs": missing,
                "has_missing": !missing.is_empty(),
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown structure check: {check}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::super::handle_research_tool;
    use serde_json::json;
}
