//! Literature verification tool — DOI reachability check, claim coverage analysis.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// Maximum number of elements in any array-type parameter to a research tool.
/// Prevents single-call memory exhaustion via malicious oversized arrays.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

pub(super) fn tool_verification_literature(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "literature verification requires 'check' (doi|claim_coverage)",
        ))?;
    match check {
        "doi" => {
            let doi = arguments
                .get("doi")
                .and_then(Value::as_str)
                .ok_or(FrameworkError::validation(
                    "doi check requires 'doi' (string)",
                ))?;
            // Reuse a single tokio runtime across all DOI check calls rather
            // than creating one per request (which has significant IO driver
            // initialization overhead).
            static DOI_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            let rt = DOI_RUNTIME.get_or_init(|| {
                #[allow(clippy::expect_used)]
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("failed to build tokio runtime for DOI checker")
            });
            let reachable = rt
                .block_on(crate::verification::literature::verify_doi_reachable(doi))
                .map_err(|e| FrameworkError::validation(format!("doi check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "doi", "doi": doi, "reachable": reachable,
            }))
            .map_err(FrameworkError::Json)
        }
        "claim_coverage" => {
            let claims_val = arguments.get("claims").and_then(Value::as_array).ok_or(
                FrameworkError::validation("claim_coverage requires 'claims' array"),
            )?;
            let references_val = arguments
                .get("references")
                .and_then(Value::as_array)
                .ok_or(FrameworkError::validation(
                    "claim_coverage requires 'references' array",
                ))?;

            if claims_val.len() > MAX_ARRAY_ELEMENTS {
                return Err(FrameworkError::validation(format!(
                    "claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
                    claims_val.len()
                )));
            }
            if references_val.len() > MAX_ARRAY_ELEMENTS {
                return Err(FrameworkError::validation(format!(
                    "references array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
                    references_val.len()
                )));
            }
            let claims: Vec<String> = claims_val
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let refs: Vec<String> = references_val
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let coverage =
                crate::verification::literature::verify_claim_coverage(&claims, &refs)
                    .map_err(|e| {
                        FrameworkError::validation(format!("coverage check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "claim_coverage", "coverage": coverage,
                "claims_count": claims.len(), "references_count": refs.len(),
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown literature check: {check}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::super::handle_research_tool;
    use serde_json::json;
}
