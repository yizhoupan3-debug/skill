//! Solution evaluation MCP tool.
//! Compares baseline vs candidate solutions across dimensions, coverage, and cost.

use core_errors::FrameworkError;
use serde_json::Value;

/// Run solution evaluation: compare baseline vs candidate across dimensions.
pub(super) fn tool_research_evaluate(arguments: &Value) -> Result<String, FrameworkError> {
    let baseline = arguments.get("baseline").and_then(Value::as_object).ok_or_else(|| {
        FrameworkError::validation("research_evaluate requires 'baseline' object")
    })?;
    let candidate = arguments.get("candidate").and_then(Value::as_object).ok_or_else(|| {
        FrameworkError::validation("research_evaluate requires 'candidate' object")
    })?;
    let dims_arr = arguments.get("dimensions").and_then(Value::as_array).ok_or_else(|| {
        FrameworkError::validation("research_evaluate requires 'dimensions' array")
    })?;

    let get_str = |obj: &serde_json::Map<_, _>, key: &str| -> Result<String, FrameworkError> {
        obj.get(key).and_then(Value::as_str).map(String::from)
            .ok_or_else(|| FrameworkError::validation(format!("missing '{key}' in solution spec")))
    };

    let check_template = |name: &str| -> Result<(), FrameworkError> {
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(FrameworkError::validation(format!(
                "template name must not contain path separators: {name:?}"
            )));
        }
        Ok(())
    };

    let get_params = |obj: &serde_json::Map<_, _>| -> std::collections::HashMap<String, String> {
        obj.get("params").and_then(Value::as_object).map(|o| {
            o.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_default())).collect()
        }).unwrap_or_default()
    };

    let get_caps = |obj: &serde_json::Map<_, _>| -> Vec<String> {
        obj.get("capabilities").and_then(Value::as_array).map(|a| {
            a.iter().filter_map(Value::as_str).map(String::from).collect()
        }).unwrap_or_default()
    };

    let dims: Vec<crate::evaluation::EvalDimension> = dims_arr.iter().map(|d| {
        crate::evaluation::EvalDimension {
            name: d.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            higher_is_better: d.get("higher_is_better").and_then(Value::as_bool).unwrap_or(true),
            weight: d.get("weight").and_then(Value::as_f64).unwrap_or(1.0),
        }
    }).collect();

    let concurrency = arguments.get("concurrency").and_then(Value::as_u64).map(|c| c as usize).unwrap_or(4).clamp(1, 32);
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64).unwrap_or(60_000);
    let no_cache = arguments.get("no_cache").and_then(Value::as_bool)
        // Default to true: each evaluation has unique template/params, cache never hits
        .unwrap_or(true);

    let baseline_template = get_str(baseline, "template")?;
    let candidate_template = get_str(candidate, "template")?;
    check_template(&baseline_template)?;
    check_template(&candidate_template)?;

    let config = crate::evaluation::EvaluationConfig {
        baseline: crate::evaluation::SolutionSpec {
            name: get_str(baseline, "name")?,
            template: baseline_template,
            params: get_params(baseline),
            capabilities: get_caps(baseline),
        },
        candidate: crate::evaluation::SolutionSpec {
            name: get_str(candidate, "name")?,
            template: get_str(candidate, "template")?,
            params: get_params(candidate),
            capabilities: get_caps(candidate),
        },
        dimensions: dims,
        concurrency,
        timeout_ms,
        no_cache,
    };

    let repo_root = crate::mcp_tools::resolve_repo_root();
    let result = crate::evaluation::run_evaluation(&repo_root, &config)?;

    let json = crate::evaluation::evaluation_to_json(&result);
    serde_json::to_string(&json).map_err(FrameworkError::Json)
}
