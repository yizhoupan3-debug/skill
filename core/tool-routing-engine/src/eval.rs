//! Tool routing eval: load golden dataset and evaluate accuracy.
//!
//! Parallel to `routing-engine/src/route/eval.rs`.

use core_errors::FrameworkError;
use core_state_utils::json_io::read_json_strict;

use std::collections::HashSet;
use std::path::Path;

use crate::routing::route_tool_from_records;
use crate::types::{
    EvaluatedToolRoutingCase, ToolRoutingEvalCasesPayload, ToolRoutingEvalMetricsPayload,
    ToolRoutingEvalReportPayload, ToolRoutingEvalResultPayload,
};
use mcp_tool_registry::McpToolRecord;

const TOOL_ROUTING_EVAL_CASES_SCHEMA_VERSION: &str = "tool-routing-eval-cases-v1";
const TOOL_ROUTING_EVAL_REPORT_SCHEMA_VERSION: &str = "tool-routing-eval-v1";

/// Load tool routing eval cases from a JSON file.
pub fn load_tool_routing_eval_cases(path: &Path) -> Result<ToolRoutingEvalCasesPayload, FrameworkError> {
    let payload = read_json_strict(path)
        .map_err(|e| FrameworkError::registry(format!("failed reading {}: {e}", path.display())))?;
    let cases = serde_json::from_value::<ToolRoutingEvalCasesPayload>(payload)
        .map_err(|err| FrameworkError::validation(format!("failed parsing {}: {err}", path.display())))?;
    if cases.schema_version != TOOL_ROUTING_EVAL_CASES_SCHEMA_VERSION {
        return Err(FrameworkError::validation(format!(
            "tool routing eval case file returned an unknown schema: {:?}",
            cases.schema_version
        )));
    }
    Ok(cases)
}

/// Evaluate tool routing accuracy against a golden dataset.
///
/// Runs every case through `route_tool_from_records` and computes aggregate
/// metrics: trigger hit/miss, overtrigger, and overall tool correctness.
pub fn evaluate_tool_routing_cases(
    records: &[McpToolRecord],
    cases_payload: ToolRoutingEvalCasesPayload,
) -> Result<ToolRoutingEvalReportPayload, FrameworkError> {
    let mut metrics = ToolRoutingEvalMetricsPayload::default();
    let cases = cases_payload.cases;

    let mut evaluated: Vec<EvaluatedToolRoutingCase> = cases
        .into_iter()
        .enumerate()
        .filter_map(|(input_index, case)| {
            let task = case.task.trim().to_string();
            if task.is_empty() {
                return None;
            }

            let host_id = case.host_id.as_deref();

            let decision = route_tool_from_records(&task, records, host_id);
            let selected_tool = decision.as_ref().map(|d| d.selected_tool.clone());

            let category = case.category.trim().to_string();
            let expected_tool = case.expected_tool.filter(|v| !v.trim().is_empty());
            let forbidden_tools: HashSet<String> = case
                .forbidden_tools
                .into_iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();

            let mut trigger_hit = false;
            let mut overtrigger = false;
            let tool_correct_raw = expected_tool
                .as_ref()
                .map(|expected| selected_tool.as_deref() == Some(expected.as_str()))
                .unwrap_or(false);

            // For should-not-trigger with no explicit expected_tool, correctness
            // means the query did not hit a forbidden tool.
            let tool_correct = if !tool_correct_raw
                && expected_tool.is_none()
                && category == "should-not-trigger"
            {
                true
            } else {
                tool_correct_raw
            };

            match category.as_str() {
                "should-trigger" | "fuzzy-rescue" => {
                    trigger_hit = tool_correct;
                    overtrigger = selected_tool
                        .as_deref()
                        .map(|st| forbidden_tools.contains(st))
                        .unwrap_or(false);
                }
                "should-not-trigger" => {
                    overtrigger = selected_tool
                        .as_deref()
                        .map(|st| forbidden_tools.contains(st))
                        .unwrap_or(false);
                }
                _ => {}
            }

            Some(EvaluatedToolRoutingCase {
                input_index,
                result: ToolRoutingEvalResultPayload {
                    id: case.id,
                    category,
                    task,
                    expected_tool,
                    selected_tool,
                    trigger_hit,
                    overtrigger,
                    tool_correct,
                },
            })
        })
        .collect();

    evaluated.sort_by_key(|row| row.input_index);

    let mut results = Vec::with_capacity(evaluated.len());
    for row in evaluated {
        metrics.case_count += 1;
        match row.result.category.as_str() {
            "should-trigger" | "fuzzy-rescue" => {
                if row.result.trigger_hit {
                    metrics.trigger_hit += 1;
                } else {
                    metrics.trigger_miss += 1;
                }
            }
            _ => {}
        }
        if row.result.overtrigger {
            metrics.overtrigger += 1;
        }
        if row.result.tool_correct {
            metrics.tool_correct += 1;
        }
        results.push(row.result);
    }

    Ok(ToolRoutingEvalReportPayload {
        schema_version: TOOL_ROUTING_EVAL_REPORT_SCHEMA_VERSION.to_string(),
        metrics,
        results,
    })
}
