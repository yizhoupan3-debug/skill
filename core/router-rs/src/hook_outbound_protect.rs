//! Shared outbound hook context protection (truncation must not drop gate / paper hooks).

const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

/// Lines that must survive UTF-8 byte budget clipping on hook outbound context.
pub(crate) fn hook_outbound_line_is_framework_protected(line: &str) -> bool {
    let t = line.trim_start();
    t.contains("router-rs REVIEW_GATE")
        || t.starts_with(REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX)
        || t.contains("continuity_suppressed=")
        || t.contains("**PAPER_PROSE_QUALITY_HOOK**")
        || t.contains("PAPER_PROSE_QUALITY_HOOK")
        || t.contains("**PAPER_ADVERSARIAL_HOOK**")
        || t.contains("PAPER_ADVERSARIAL_HOOK")
}

fn truncate_hook_outbound_bytes(combined: &str, max_bytes: usize, suffix: &str) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let suf_len = suffix.len();
    if max_bytes <= suf_len {
        let mut cut = max_bytes.min(combined.len());
        while cut > 0 && !combined.is_char_boundary(cut) {
            cut -= 1;
        }
        return combined[..cut].to_string();
    }
    let budget = max_bytes.saturating_sub(suf_len);
    let mut cut = budget.min(combined.len());
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    if let Some(pos) = combined[..cut].rfind('\n') {
        if pos > 0 {
            cut = pos;
        }
    }
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &combined[..cut], suffix)
}

/// Preserve protected lines first, then truncate the remainder to fit `max_bytes`.
pub(crate) fn truncate_hook_outbound_lines_preserving(
    combined: &str,
    max_bytes: usize,
    suffix: &str,
) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let mut protected: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for line in combined.lines() {
        if hook_outbound_line_is_framework_protected(line) {
            protected.push(line);
        } else {
            rest.push(line);
        }
    }
    let protected_body = protected.join("\n");
    if protected_body.len() >= max_bytes {
        return truncate_hook_outbound_bytes(&protected_body, max_bytes, suffix);
    }
    let rest_body = rest.join("\n");
    if rest_body.is_empty() {
        return protected_body;
    }
    let sep_len = if protected_body.is_empty() { 0 } else { 1 };
    let rest_budget = max_bytes.saturating_sub(protected_body.len() + sep_len);
    let truncated_rest = truncate_hook_outbound_bytes(&rest_body, rest_budget, suffix);
    if protected_body.is_empty() {
        truncated_rest
    } else if truncated_rest.is_empty() {
        protected_body
    } else {
        format!("{protected_body}\n{truncated_rest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_paper_prose_survives_truncation() {
        let filler = "x".repeat(900);
        let combined = format!(
            "{filler}\n**PAPER_PROSE_QUALITY_HOOK**\nprose chain body must remain"
        );
        let out = truncate_hook_outbound_lines_preserving(&combined, 640, "...");
        assert!(out.contains("PAPER_PROSE_QUALITY_HOOK"));
    }

    #[test]
    fn unprotected_only_truncates_normally() {
        let combined = "a".repeat(900);
        let out = truncate_hook_outbound_lines_preserving(&combined, 100, "...");
        assert!(out.len() <= 100);
        assert!(!out.contains("PAPER_PROSE"));
    }
}
