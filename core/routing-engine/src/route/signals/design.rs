use super::has_signal_by_name;

pub fn has_design_reference_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("design_reference", query_text, query_token_list)
}

pub fn has_visual_evidence_review_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("visual_evidence_review", query_text, query_token_list)
}

pub fn has_design_contract_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("design_contract", query_text, query_token_list)
}

pub fn has_design_contract_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("design_contract_negation", query_text, query_token_list)
}

pub fn has_design_output_audit_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("design_output_audit", query_text, query_token_list)
}

pub fn has_design_workflow_protocol_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("design_workflow_protocol", query_text, query_token_list)
}

pub fn has_quick_artifact_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("quick_artifact", query_text, query_token_list)
}

pub fn has_beamer_slide_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("beamer_slide", query_text, query_token_list)
}

pub fn has_source_slide_format_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("source_slide_format", query_text, query_token_list)
}

pub fn has_diagramming_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("diagramming", query_text, query_token_list)
}
