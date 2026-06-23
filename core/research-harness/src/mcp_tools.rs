//! MCP tool dispatch for research tools.
//!
//! Delegated from host-projection's tool dispatcher (Phase 4 T1).

use serde_json::{Value, json};

/// Handle a research MCP tool call.
/// Delegates to the appropriate research-harness module.
pub fn handle_research_tool(name: &str, arguments: &Value) -> Result<String, String> {
    match name {
        "research_aigc_check" => tool_research_aigc_check(arguments),
        "research_aigc_humanize" => tool_research_aigc_humanize(arguments),
        "research_review_dimensions" => tool_research_review_dimensions(arguments),
        "research_claim_drift" => tool_research_claim_drift(arguments),
        "research_review_loop" => tool_research_review_loop(arguments),
        _ => Err(format!("unknown research tool: {name}")),
    }
}

fn tool_research_aigc_check(arguments: &Value) -> Result<String, String> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or("research_aigc_check requires 'text' parameter")?;
    // TODO: DetectionConfig currently uses Default; wire language param once
    // the detector supports locale-aware thresholds (DetectionConfig has no language field yet).
    let _ = arguments
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("en");

    let config = crate::aigc::detector::DetectionConfig::default();
    let results = crate::aigc::detector::detect(text, &config)
        .map_err(|e| format!("AIGC detection failed: {e}"))?;
    let score = crate::aigc::scorer::score(&results);

    serde_json::to_string_pretty(&json!({
        "score": score,
        "ai_probability": score as f64 / 100.0,
        "segments_analyzed": results.len(),
        "results": results,
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_aigc_humanize(arguments: &Value) -> Result<String, String> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or("research_aigc_humanize requires 'text' parameter")?;
    // TODO: humanize_with_config supports HumanizeConfig (locale, tone); wire these
    // when the MCP contract and HumanizeConfig locale field are finalized.
    let _ = arguments
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("en");
    let _ = arguments
        .get("preserve_academic_tone")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let strategies = vec![
        crate::aigc::humanizer::HumanizeStrategy::VocabularySwap,
        crate::aigc::humanizer::HumanizeStrategy::SyntacticRewrite,
        crate::aigc::humanizer::HumanizeStrategy::SentenceVariation,
    ];
    let result = crate::aigc::humanizer::humanize(text, &strategies)
        .map_err(|e| format!("AIGC humanization failed: {e}"))?;

    serde_json::to_string_pretty(&json!({
        "original_length": text.len(),
        "rewritten_length": result.rewritten.len(),
        "strategies_applied": result.strategies_applied,
        "estimated_score_improvement": result.estimated_score_improvement,
        "rewritten": result.rewritten,
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_review_dimensions(arguments: &Value) -> Result<String, String> {
    let round = arguments
        .get("round")
        .and_then(Value::as_u64)
        .ok_or("research_review_dimensions requires 'round' parameter")?;
    let manuscript_summary = arguments
        .get("manuscript_summary")
        .and_then(Value::as_str)
        .unwrap_or("(no summary provided)");

    let dim = crate::types::ReviewDimension::for_round(round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);
    let full_prompt = crate::review::orchestrator::build_reviewer_prompt(
        round,
        &dim,
        manuscript_summary,
    );

    serde_json::to_string_pretty(&json!({
        "round": round,
        "dimension": dim.display_name(),
        "prompt": prompt,
        "checklist": checklist,
        "full_reviewer_prompt": full_prompt,
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_claim_drift(arguments: &Value) -> Result<String, String> {
    let original_claims = arguments
        .get("original_claims")
        .and_then(Value::as_array)
        .ok_or("research_claim_drift requires 'original_claims' array")?;
    let current_claims = arguments
        .get("current_claims")
        .and_then(Value::as_array)
        .ok_or("research_claim_drift requires 'current_claims' array")?;

    let parse_claims = |arr: &[Value]| -> Result<Vec<crate::types::Claim>, String> {
        arr.iter()
            .map(|v| {
                let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let text = v.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                Ok(crate::types::Claim {
                    id,
                    text,
                    evidence: vec![],
                    ceiling: crate::types::ClaimCeiling::ConferenceReady,
                })
            })
            .collect()
    };

    let orig = parse_claims(original_claims)?;
    let curr = parse_claims(current_claims)?;

    let results = crate::claims::drift::detect_drift(&orig, &curr)
        .map_err(|e| e.to_string())?;

    serde_json::to_string_pretty(&json!({
        "drift_results": results,
        "total_claims_analyzed": results.len(),
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_review_loop(arguments: &Value) -> Result<String, String> {
    let max_rounds = arguments
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    let min_rounds = arguments
        .get("min_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(5);
    let consecutive_stable = arguments
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);

    let state = crate::types::ConvergenceState {
        min_rounds,
        consecutive_stable_required: consecutive_stable,
        consecutive_stable_count: 0,
        max_rounds,
        current_round: 0,
    };

    let dimensions: Vec<Value> = (1..=max_rounds)
        .map(|round| {
            let dim = crate::types::ReviewDimension::for_round(round);
            let prompt = crate::review::dimensions::dimension_prompt(&dim);
            json!({
                "round": round,
                "dimension": dim.display_name(),
                "prompt_preview": &prompt[..200.min(prompt.len())],
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "convergence_config": {
            "min_rounds": state.min_rounds,
            "max_rounds": state.max_rounds,
            "consecutive_stable_required": state.consecutive_stable_required,
        },
        "dimensions": dimensions,
        "workflow": "spawn reviewer subagent per round → fix findings → check convergence → repeat",
    }))
    .map_err(|e| e.to_string())
}
