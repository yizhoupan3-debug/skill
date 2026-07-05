//! Reproducibility verification tool — seed, deterministic, environment, data_versioned,
//! checkpoint, and full_audit checks.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};

pub(super) fn tool_verification_reproducibility(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "reproducibility verification requires 'check' \
             (seed|deterministic|environment|data_versioned|checkpoint|full_audit)",
        ))?;
    match check {
        "seed" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("seed check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_seed_set(std::path::Path::new(path))
                    .map_err(|e| FrameworkError::validation(format!("seed check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "seed", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "deterministic" => {
            let run_paths_val = arguments.get("run_paths").and_then(Value::as_array).ok_or(
                FrameworkError::validation("deterministic check requires 'run_paths' array"),
            )?;
            let run_paths: Vec<&std::path::Path> = run_paths_val
                .iter()
                .filter_map(|v| v.as_str())
                .map(std::path::Path::new)
                .collect();
            if run_paths.len() < 2 {
                return Err(FrameworkError::validation(
                    "deterministic check requires at least 2 run paths",
                ));
            }
            let result =
                crate::verification::reproducibility::check_deterministic_rerun(&run_paths)
                    .map_err(|e| {
                        FrameworkError::validation(format!("deterministic check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "deterministic", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "environment" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("environment check requires 'path' (string)"),
            )?;
            let result = crate::verification::reproducibility::check_environment_reproducible(
                std::path::Path::new(path),
            )
            .map_err(|e| FrameworkError::validation(format!("environment check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "environment", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "data_versioned" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("data_versioned check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_data_versioned(std::path::Path::new(
                    path,
                ))
                .map_err(|e| {
                    FrameworkError::validation(format!("data_versioned check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "data_versioned", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "checkpoint" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("checkpoint check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_checkpoint_recoverable(
                    std::path::Path::new(path),
                )
                .map_err(|e| {
                    FrameworkError::validation(format!("checkpoint check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "checkpoint", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "full_audit" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("full_audit requires 'path' (string)"),
            )?;
            let run_paths_val = arguments.get("run_paths").and_then(Value::as_array);
            let run_paths: Option<Vec<&std::path::Path>> = run_paths_val.map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(std::path::Path::new)
                    .collect()
            });
            let report = crate::verification::reproducibility::run_reproducibility_audit(
                std::path::Path::new(path),
                run_paths.as_deref(),
            )
            .map_err(|e| FrameworkError::validation(format!("full audit failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "full_audit", "checks": report.checks,
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown reproducibility check: {check}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::super::handle_research_tool;
    use serde_json::json;
}
