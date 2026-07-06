//! Ablation analysis MCP tool.
//! Runs baseline-vs-ablated component comparison and returns a contribution matrix.

use core_errors::FrameworkError;
use serde_json::Value;
use std::path::PathBuf;

/// Resolve the framework project root by walking up from CWD.
fn resolve_repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("templates").exists() || d.join(".git").exists() {
                return d.to_path_buf();
            }
            dir = d.parent();
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Run component-level ablation analysis.
///
/// Accepts a template, baseline params, and a list of components to remove individually.
/// Returns a structured matrix comparing baseline metrics vs each ablated run.
pub(super) fn tool_research_ablation(arguments: &Value) -> Result<String, FrameworkError> {
    let template = arguments
        .get("template")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "research_ablation requires 'template' (string): executable filename in templates/",
            )
        })?;

    if template.is_empty() || template.contains('/') || template.contains('\\') || template.contains("..") {
        return Err(FrameworkError::validation(format!(
            "template name must not contain path separators: {template:?}"
        )));
    }

    let baseline_params_obj = arguments
        .get("baseline_params")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            FrameworkError::validation(
                "research_ablation requires 'baseline_params' (object of {key: value, ...})",
            )
        })?;

    let components = arguments
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            FrameworkError::validation(
                "research_ablation requires 'components' (array of {name, description, ...})",
            )
        })?;

    if components.is_empty() {
        return Err(FrameworkError::validation(
            "components must not be empty — at least one component is required for ablation",
        ));
    }

    let metrics: Vec<String> = arguments
        .get("metrics")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let concurrency = arguments
        .get("concurrency")
        .and_then(Value::as_u64)
        .map(|c| c as usize)
        .unwrap_or(4)
        .clamp(1, 32);

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(60_000);

    let no_cache = arguments
        .get("no_cache")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // Build component specs
    let mut component_specs = Vec::with_capacity(components.len());
    for comp in components {
        let name = comp
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FrameworkError::validation("each component must have a 'name' (string)")
            })?;
        let description = comp
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let ablation_params = comp.get("ablation_params").and_then(Value::as_object).map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    let s = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => serde_json::to_string(v).unwrap_or_default(),
                    };
                    (k.clone(), s)
                })
                .collect()
        });

        component_specs.push(crate::ablation::ComponentSpec {
            name: name.to_string(),
            description,
            ablation_params,
        });
    }

    // Build baseline params
    let baseline_params: std::collections::HashMap<String, String> = baseline_params_obj
        .iter()
        .map(|(k, v)| {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            };
            (k.clone(), s)
        })
        .collect();

    let config = crate::ablation::AblationConfig {
        template: template.to_string(),
        baseline_params,
        components: component_specs,
        metrics,
        concurrency,
        timeout_ms,
        no_cache,
    };

    let repo_root = resolve_repo_root();
    let result = crate::ablation::run_ablation(&repo_root, &config)?;

    // Serialize the matrix as the response
    let response = serde_json::to_string(&result.matrix).map_err(FrameworkError::Json)?;

    // Size guard (reuse existing MAX_MCP_RESPONSE_BYTES constant)
    if response.len() > crate::smoke::MAX_MCP_RESPONSE_BYTES {
        return Ok(serde_json::to_string(&serde_json::json!({
            "truncated": true,
            "total_components": result.components.len(),
            "summary": result.matrix["summary"],
            "note": format!("Response too large ({} bytes). Check artifacts/research-log/smoke/ for full data.", response.len()),
        })).map_err(FrameworkError::Json)?);
    }

    Ok(response)
}
