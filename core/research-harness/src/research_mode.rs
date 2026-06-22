//! Research mode inference — moved from framework-runtime to L5 (ADR-010 §7.4).
//!
//! L4 uses this module via the `host_projection::hooks::research_mode_for_request`
//! function pointer. L5 registers the inference callback at bootstrap.

use framework_kernel::stdio_payload_types::ExecuteRequestPayload;

/// Research depth classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchMode {
    Quick,
    Deep,
}

impl ResearchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchMode::Quick => "quick",
            ResearchMode::Deep => "deep",
        }
    }
}

/// `external research` alone matches many integration/API strings; require a second research cue.
fn external_research_phrase_signals_deep(lower: &str) -> bool {
    if !lower.contains("external research") {
        return false;
    }
    lower.contains("调研")
        || lower.contains("文献")
        || lower.contains("审计")
        || lower.contains("ledger")
        || lower.contains("quality_gate")
        || lower.contains("rfv")
        || lower.contains("外研")
        || lower.contains("literature")
        || lower.contains("unknowns")
        || lower.contains("contradiction")
        || lower.contains("auditable")
        || lower.contains("research-grade")
        || lower.contains("research grade")
        || lower.contains("科研级")
        || lower.contains("deep dive")
}

/// Narrow host-neutral cues for Execute deep shaping (substring match; ASCII segments may be lowercased).
pub fn payload_text_signals_deep_research(text: &str) -> bool {
    text.contains("深度调研")
        || text.contains("深度研究")
        || text.contains("deep research")
        || text.contains("deep dive")
        || text.contains("literature review")
        || text.contains("literature-review")
        || text.contains("文献调研")
        || external_research_phrase_signals_deep(text)
        || text.contains("research-grade")
        || text.contains("research grade")
        || text.contains("科研级调研")
}

fn normalize_research_mode_token(value: &str) -> Option<ResearchMode> {
    let lowered = value.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    match lowered.as_str() {
        "quick" | "fast" | "lite" | "shallow" => Some(ResearchMode::Quick),
        "deep" | "deep_research" | "deep-research" => Some(ResearchMode::Deep),
        _ => None,
    }
}

/// Infer the research mode from a live-execute request payload.
pub fn infer_research_mode(payload: &ExecuteRequestPayload) -> ResearchMode {
    if let Some(mode) = payload
        .research_mode
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    if let Some(mode) = payload
        .execution_protocol
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    let task = payload.task.trim().to_ascii_lowercase();
    if payload_text_signals_deep_research(&task) {
        return ResearchMode::Deep;
    }
    if task.contains("快查") || task.contains("快速调研") {
        return ResearchMode::Quick;
    }
    for reason in &payload.reasons {
        if let Some(mode) = normalize_research_mode_token(reason) {
            return mode;
        }
        let lowered = reason.to_ascii_lowercase();
        if payload_text_signals_deep_research(&lowered) {
            return ResearchMode::Deep;
        }
    }
    if payload.selected_skill == "implementx" {
        return ResearchMode::Quick;
    }
    ResearchMode::Quick
}

/// Register the function pointer so L4 can call research mode inference.
///
/// Should be called during L5 bootstrap (e.g., from research-harness init).
pub fn register_research_mode_inference() {
    host_projection::hooks::register_research_mode_inference(|payload_json: &serde_json::Value| {
        // Deserialize the JSON back into ExecuteRequestPayload
        let payload: ExecuteRequestPayload =
            serde_json::from_value(payload_json.clone()).unwrap_or_else(|_| {
                ExecuteRequestPayload {
                    schema_version: String::new(),
                    task: String::new(),
                    session_id: String::new(),
                    user_id: String::new(),
                    selected_skill: String::new(),
                    overlay_skill: None,
                    layer: String::new(),
                    route_engine: None,
                    diagnostic_route_mode: None,
                    reasons: Vec::new(),
                    prompt_preview: None,
                    dry_run: false,
                    trace_event_count: 0,
                    trace_output_path: None,
                    default_output_tokens: 0,
                    research_mode: None,
                    execution_protocol: None,
                    verification_required: None,
                    evidence_required: None,
                    model_id: String::new(),
                    aggregator_base_url: String::new(),
                    aggregator_api_key: String::new(),
                }
            });
        infer_research_mode(&payload).as_str().to_string()
    });
}
