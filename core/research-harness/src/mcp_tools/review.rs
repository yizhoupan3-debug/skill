//! Research review loop tools — dimensions query and multi-round adversarial review.
//!
//! # Functions
//! - `tool_research_review_dimensions` — get review dimension prompt/checklist for a round
//! - `tool_research_review_loop` — manage the review loop lifecycle (start/submit_round/status)
//! - `review_loop_start` — operation `start` handler
//! - `review_loop_submit_round` — operation `submit_round` handler
//! - `review_loop_status` — operation `status` handler

use core_errors::FrameworkError;
use serde_json::{Value, json};

/// Get review dimension prompt, checklist, and full reviewer prompt for a given round.
pub(super) fn tool_research_review_dimensions(arguments: &Value) -> Result<String, FrameworkError> {
    let round =
        arguments
            .get("round")
            .and_then(Value::as_u64)
            .ok_or(FrameworkError::validation(
                "research_review_dimensions requires 'round' parameter",
            ))?;
    let manuscript_summary = arguments
        .get("manuscript_summary")
        .and_then(Value::as_str)
        .unwrap_or("(no summary provided)");

    let dim = crate::types::ReviewDimension::for_round(round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);
    let full_prompt =
        crate::review::orchestrator::build_reviewer_prompt(round, &dim, manuscript_summary);

    serde_json::to_string_pretty(&json!({
        "round": round,
        "dimension": dim.display_name(),
        "prompt": prompt,
        "checklist": checklist,
        "full_reviewer_prompt": full_prompt,
    }))
    .map_err(FrameworkError::Json)
}

/// Manage the research review loop lifecycle.
/// Dispatches to `start`, `submit_round`, or `status` based on the `operation` argument.
pub(super) fn tool_research_review_loop(arguments: &Value) -> Result<String, FrameworkError> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("start");

    match operation {
        "start" => review_loop_start(arguments),
        "submit_round" => review_loop_submit_round(arguments),
        "status" => review_loop_status(arguments),
        _ => Err(FrameworkError::validation(format!(
            "research_review_loop: unknown operation '{operation}' — expected start|submit_round|status"
        ))),
    }
}

/// Operation `start`: return config + round-1 dimension. Stateless — no persistence.
fn review_loop_start(arguments: &Value) -> Result<String, FrameworkError> {
    let max_rounds = arguments
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    let min_rounds = arguments
        .get("min_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(5);
    let stable_req = arguments
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);

    let dim = crate::types::ReviewDimension::for_round(1);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);

    serde_json::to_string_pretty(&json!({
        "operation": "started",
        "quality_gate_config": {
            "min_rounds": min_rounds,
            "max_rounds": max_rounds,
            "consecutive_stable_required": stable_req,
        },
        "current_round": {
            "round": 1,
            "dimension": dim.display_name(),
            "prompt": prompt,
            "checklist": checklist,
        },
        "total_dimensions": std::cmp::min(max_rounds, 7),
        "workflow": "1. Call research_review_loop(operation=start, min_rounds=5, max_rounds=10, ...) to init the review loop. 2. Spawn reviewer subagent using current_round. 3. Call research_review_loop(operation=submit_round, round=N, findings=[...]) for next-round. 4. research_review_loop handles convergence tracking.",
    }))
    .map_err(FrameworkError::Json)
}

/// Operation `submit_round`: accept round + findings, return next-round dimension or completion.
/// Stateless — convergence is managed by research_review_loop at the runtime layer.
fn review_loop_submit_round(arguments: &Value) -> Result<String, FrameworkError> {
    let round =
        arguments
            .get("round")
            .and_then(Value::as_u64)
            .ok_or(FrameworkError::validation(
                "research_review_loop submit_round requires 'round' (u64)",
            ))?;

    let max_rounds = arguments.get("max_rounds").and_then(Value::as_u64);

    // Ceiling check: if this round has reached max, signal completion.
    if let Some(ceiling) = max_rounds {
        if round >= ceiling {
            return serde_json::to_string_pretty(&json!({
                "operation": "completed",
                "reason": "max_rounds reached — ceiling exceeded",
                "rounds_completed": round,
            }))
            .map_err(FrameworkError::Json);
        }
    }

    let findings_val = arguments.get("findings").and_then(Value::as_array);
    let findings: Vec<crate::types::Finding> = match findings_val {
        Some(arr) => serde_json::from_value(Value::Array(arr.clone()))
            .map_err(|e| FrameworkError::validation(format!("invalid findings format: {e}")))?,
        None => Vec::new(),
    };

    let has_blocking = findings.iter().any(|f| f.severity.blocks_convergence());

    let next_round = round + 1;
    let dim = crate::types::ReviewDimension::for_round(next_round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);

    serde_json::to_string_pretty(&json!({
        "operation": "continue",
        "round_completed": round,
        "findings_this_round": findings.len(),
        "has_blocking": has_blocking,
        "findings": findings.iter().map(|f| json!({
            "id": f.id,
            "severity": format!("{:?}", f.severity),
            "dimension": f.dimension,
            "location": f.location,
            "description": f.description,
            "suggestion": f.suggestion,
        })).collect::<Vec<_>>(),
        "next_round": {
            "round": next_round,
            "dimension": dim.display_name(),
            "prompt": prompt,
            "checklist": checklist,
        },
        "next_step": "Call research_review_loop(operation=submit_round, ...) to record the round in the runtime loop, then spawn reviewer for next round.",
    }))
    .map_err(FrameworkError::Json)
}

/// Operation `status`: return current round's dimension. Stateless — pure function of round.
fn review_loop_status(arguments: &Value) -> Result<String, FrameworkError> {
    let round = arguments.get("round").and_then(Value::as_u64).unwrap_or(1);
    let dim = crate::types::ReviewDimension::for_round(round);

    serde_json::to_string_pretty(&json!({
        "round": round,
        "dimension": dim.display_name(),
        "note": "Runtime loop state (convergence, rounds) is managed by research_review_loop at the runtime layer.",
        "next_step": "Call research_review_loop(operation=status) for convergence state.",
    }))
    .map_err(FrameworkError::Json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

    #[test]
    fn research_review_dimensions_missing_round() {
        let result = handle_research_tool("research_review_dimensions", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'round'"));
    }

    #[test]
    fn research_review_dimensions_round_1() {
        let result = handle_research_tool("research_review_dimensions", &json!({"round": 1}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("逻辑与证据")
        );
    }

    #[test]
    fn research_review_loop_defaults() {
        let result = handle_research_tool("research_review_loop", &json!({}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("started")
        );
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(5));
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(10));
        assert_eq!(
            config
                .get("consecutive_stable_required")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert!(parsed.get("current_round").is_some());
    }

    #[test]
    fn research_review_loop_custom_params() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"max_rounds": 3, "min_rounds": 1, "consecutive_stable_required": 1}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("started")
        );
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(3));
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(1));
        assert!(parsed.get("current_round").is_some());
    }

    #[test]
    fn research_review_loop_status_round_1() {
        let result = handle_research_tool("research_review_loop", &json!({"operation": "status"}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("逻辑与证据")
        );
    }

    #[test]
    fn research_review_loop_status_round_3() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "status", "round": 3}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(3));
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("数学与符号")
        );
    }

    #[test]
    fn research_review_loop_submit_missing_round() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round"}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'round'"));
    }

    #[test]
    fn research_review_loop_submit_empty_findings() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 1, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("continue")
        );
        assert_eq!(
            parsed.get("round_completed").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            parsed.get("has_blocking").and_then(Value::as_bool),
            Some(false)
        );
        let next = parsed.get("next_round").unwrap();
        assert_eq!(next.get("round").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn research_review_loop_submit_with_blocking() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({
                "operation": "submit_round",
                "round": 1,
                "findings": [{"id": "f1", "severity": "P0", "dimension": "逻辑与证据", "location": "§2", "description": "data integrity"}]
            }),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("has_blocking").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn research_review_loop_unknown_operation() {
        let result = handle_research_tool("research_review_loop", &json!({"operation": "unknown"}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown operation")
        );
    }

    #[test]
    fn research_review_loop_submit_advances_round() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 3, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("round_completed").and_then(Value::as_u64),
            Some(3)
        );
        let next = parsed.get("next_round").unwrap();
        assert_eq!(next.get("round").and_then(Value::as_u64), Some(4));
        assert_eq!(
            next.get("dimension").and_then(Value::as_str),
            Some("图表与可读性")
        );
    }

    #[test]
    fn research_review_loop_submit_at_ceiling() {
        // round == max_rounds → ceiling hit, operation="completed"
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 10, "max_rounds": 10, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("completed")
        );
        assert!(
            parsed
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("max_rounds")
        );

        // round > max_rounds → also completed
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 999, "max_rounds": 10, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("completed")
        );

        // no max_rounds → proceeds normally (backward compat)
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 99, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("continue")
        );
    }
}
