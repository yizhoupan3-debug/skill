//! Structured tool routing audit logging.
//!
//! Logs every `route_tool_from_records()` decision as a JSON line to
//! `logs/tool-routing/tool_routing_audit.ndjson`.  Auto-initializes on
//! first use (creates directory + file).  Thread-safe via shared
//! `routing_core::audit_log::AuditLog`.
//!
//! Parallel to `routing-engine/src/route/routing.rs::log_decision`.

use crate::types::McpToolDecision;

/// Write a tool routing decision to the structured audit log.
/// Auto-initializes on first use — creates `logs/tool-routing/tool_routing_audit.ndjson`
/// relative to `FRAMEWORK_ROOT` or `CARGO_MANIFEST_DIR`.
pub fn log_tool_decision(decision: &McpToolDecision, query: &str) {
    static LOG: routing_core::audit_log::AuditLog = routing_core::audit_log::AuditLog::new();

    let entry = serde_json::json!({
        "ts": routing_core::audit_log::iso_timestamp_now(),
        "query": query,
        "selected_tool": decision.selected_tool,
        "score": decision.score,
        "fuzzy_match": decision.fuzzy_match,
        "matched_token_count": decision.matched_token_count,
        "dispatch_domain": decision.dispatch_domain,
        "mcp_server": decision.mcp_server,
        "top_3_reasons": &decision.reasons.iter().take(3).cloned().collect::<Vec<_>>(),
    });

    LOG.write_entry_with_rotation("logs/tool-routing/tool_routing_audit.ndjson", &entry);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    #[test]
    fn fuzzy_match_handles_typo() {
        use crate::fuzzy::best_fuzzy_score;
        let hints = vec!["screenshot".to_string(), "浏览器截图".to_string()];
        let score = best_fuzzy_score("screeenshot", &hints);
        assert!(score.is_some(), "typo should fuzzy-match via n-gram");
        assert!(score.unwrap() > 50.0);
    }
}
