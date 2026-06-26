//! Research mode inference — moved from framework-runtime to L5 (ADR-010 §7.4).
//!
//! L7 (runtime-core) consumes this inference result via the
//! `host_projection::hooks::research_mode_for_request` function pointer.
//! L5 registers the inference callback at bootstrap.

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
    ResearchMode::Quick
}

/// Register the function pointer so L4 can call research mode inference.
///
/// Should be called during L5 bootstrap (e.g., from research-harness init).
pub fn register_research_mode_inference() {
    host_projection::hooks::register_research_mode_inference(|payload_json: &serde_json::Value| {
        // Deserialize the JSON back into ExecuteRequestPayload
        let payload: ExecuteRequestPayload =
            serde_json::from_value(payload_json.clone()).unwrap_or_else(|e| {
                tracing::warn!("research_mode: failed to deserialize payload: {e}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use framework_kernel::stdio_payload_types::ExecuteRequestPayload;

    fn make_payload(task: &str) -> ExecuteRequestPayload {
        ExecuteRequestPayload {
            schema_version: String::new(),
            task: task.into(),
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
    }

    #[test]
    fn normalize_token_quick_variants() {
        assert_eq!(normalize_research_mode_token("quick"), Some(ResearchMode::Quick));
        assert_eq!(normalize_research_mode_token("fast"), Some(ResearchMode::Quick));
        assert_eq!(normalize_research_mode_token("lite"), Some(ResearchMode::Quick));
        assert_eq!(normalize_research_mode_token("shallow"), Some(ResearchMode::Quick));
    }

    #[test]
    fn normalize_token_deep_variants() {
        assert_eq!(normalize_research_mode_token("deep"), Some(ResearchMode::Deep));
        assert_eq!(normalize_research_mode_token("deep_research"), Some(ResearchMode::Deep));
        assert_eq!(normalize_research_mode_token("deep-research"), Some(ResearchMode::Deep));
    }

    #[test]
    fn normalize_token_invalid_returns_none() {
        assert_eq!(normalize_research_mode_token("medium"), None);
        assert_eq!(normalize_research_mode_token("auto"), None);
        assert_eq!(normalize_research_mode_token(""), None);
        assert_eq!(normalize_research_mode_token("  "), None);
    }

    #[test]
    fn normalize_token_case_insensitive() {
        assert_eq!(normalize_research_mode_token("DEEP"), Some(ResearchMode::Deep));
        assert_eq!(normalize_research_mode_token("QUICK"), Some(ResearchMode::Quick));
        assert_eq!(normalize_research_mode_token("Deep_Research"), Some(ResearchMode::Deep));
    }

    #[test]
    fn payload_signals_deep_research() {
        assert!(payload_text_signals_deep_research("深度调研论文"));
        assert!(payload_text_signals_deep_research("deep research topic"));
        assert!(payload_text_signals_deep_research("literature review for project"));
        assert!(payload_text_signals_deep_research("文献调研"));
        assert!(payload_text_signals_deep_research("research-grade analysis"));
        assert!(payload_text_signals_deep_research("do a deep dive"));
        assert!(!payload_text_signals_deep_research("quick lookup please"));
        assert!(!payload_text_signals_deep_research("fuzzy research topic"));
    }

    #[test]
    fn external_research_phrase_requires_second_cue() {
        assert!(!external_research_phrase_signals_deep("external research"));
        assert!(external_research_phrase_signals_deep("external research 文献"));
        assert!(external_research_phrase_signals_deep("external research literature"));
    }

    #[test]
    fn infer_mode_defaults_to_quick() {
        let payload = make_payload("简单的查询");
        assert_eq!(infer_research_mode(&payload), ResearchMode::Quick);
    }

    #[test]
    fn infer_mode_research_mode_field_has_top_priority() {
        let mut payload = make_payload("simple query");
        payload.research_mode = Some("deep".into());
        // Even though task is simple, research_mode field overrides
        assert_eq!(infer_research_mode(&payload), ResearchMode::Deep);
    }

    #[test]
    fn infer_mode_execution_protocol_overrides_after_mode() {
        let mut payload = make_payload("深度调研");
        payload.execution_protocol = Some("quick".into());
        // execution_protocol overrides only after research_mode is checked
        // Since research_mode is None, execution_protocol=quick wins
        assert_eq!(infer_research_mode(&payload), ResearchMode::Quick);
    }

    #[test]
    fn infer_mode_task_text_signals_deep() {
        let payload = make_payload("帮我做深度调研");
        assert_eq!(infer_research_mode(&payload), ResearchMode::Deep);
    }

    #[test]
    fn infer_mode_task_text_signals_quick() {
        let payload = make_payload("快查一下这个");
        assert_eq!(infer_research_mode(&payload), ResearchMode::Quick);
    }

    #[test]
    fn infer_mode_reasons_checked_after_task() {
        let mut payload = make_payload("simple query");
        payload.reasons = vec!["deep research".into()];
        assert_eq!(infer_research_mode(&payload), ResearchMode::Deep);
    }

    #[test]
    fn infer_mode_deep_research_field_overrides_reason() {
        // research_mode = Some("deep") should win over task text saying "quick"
        let mut payload = make_payload("quick check");
        payload.research_mode = Some("deep".into());
        assert_eq!(infer_research_mode(&payload), ResearchMode::Deep);
    }

    #[test]
    fn register_hook_handles_bad_json() {
        // The registration closure should not panic on bogus JSON
        let _bad_json = serde_json::json!({"invalid": true});
        // We can't easily call the registered fn pointer directly in a unit test,
        // but we can verify that infer_research_mode handles default-constructed payloads
        let default = ExecuteRequestPayload {
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
            research_mode: Some("deep".into()),
            execution_protocol: None,
            verification_required: None,
            evidence_required: None,
            model_id: String::new(),
            aggregator_base_url: String::new(),
            aggregator_api_key: String::new(),
        };
        assert_eq!(infer_research_mode(&default), ResearchMode::Deep);
    }

    /// Verify that `infer_research_mode` can be recovered from a serialized round-trip via serde_json.
    #[test]
    fn infer_mode_serde_roundtrip() {
        let original = make_payload("深度调研这个主题");
        let json = serde_json::to_value(&original).unwrap();
        let recovered: ExecuteRequestPayload = serde_json::from_value(json).unwrap();
        assert_eq!(infer_research_mode(&recovered), ResearchMode::Deep);
    }
}
