//! AIGC detection and humanization tools.
//!
//! # Functions
//! - `tool_research_aigc_check` — detect whether text is AI-generated
//! - `tool_research_aigc_humanize` — rewrite text to reduce AIGC signals
//! - `parse_language` — parse language parameter into `crate::aigc::Language`

use core_errors::FrameworkError;
use serde_json::{Value, json};

/// Parse a language parameter from arguments into `crate::aigc::Language`.
fn parse_language(value: &Value, key: &str) -> crate::aigc::Language {
    match value.get(key).and_then(Value::as_str) {
        Some("zh") | Some("zh-CN") | Some("chinese") => crate::aigc::Language::Chinese,
        _ => crate::aigc::Language::English,
    }
}

/// Detect whether text is AI-generated using multi-strategy analysis.
pub(super) fn tool_research_aigc_check(arguments: &Value) -> Result<String, FrameworkError> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "research_aigc_check requires 'text' parameter",
        ))?;
    let language = parse_language(arguments, "language");

    let config = crate::aigc::detector::DetectionConfig {
        language,
        ..Default::default()
    };
    let results = crate::aigc::detector::detect(text, &config)
        .map_err(|e| FrameworkError::validation(format!("AIGC detection failed: {e}")))?;
    let score = crate::aigc::scorer::score(&results);

    serde_json::to_string_pretty(&json!({
        "score": score,
        "ai_probability": score as f64 / 100.0,
        "segments_analyzed": results.len(),
        "results": results,
    }))
    .map_err(FrameworkError::Json)
}

/// AIGC humanization — reduce AIGC signals through lexical/syntactic rewriting.
pub(super) fn tool_research_aigc_humanize(arguments: &Value) -> Result<String, FrameworkError> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "research_aigc_humanize requires 'text' parameter",
        ))?;
    let language = match arguments.get("language").and_then(Value::as_str) {
        Some("zh") | Some("zh-CN") | Some("chinese") => crate::aigc::Language::Chinese,
        _ => crate::aigc::Language::English,
    };
    let preserve_academic = arguments
        .get("preserve_academic")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let config = crate::aigc::humanizer::HumanizeConfig {
        language,
        preserve_academic_tone: preserve_academic,
        ..Default::default()
    };
    let result = crate::aigc::humanizer::humanize_with_config(text, &config)
        .map_err(|e| FrameworkError::validation(format!("humanization failed: {e}")))?;

    serde_json::to_string_pretty(&json!({
        "original": result.original,
        "rewritten": result.rewritten,
        "strategies_applied": result.strategies_applied,
        "estimated_score_improvement": result.estimated_score_improvement,
    }))
    .map_err(FrameworkError::Json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

    #[test]
    fn research_aigc_check_missing_text() {
        let result = handle_research_tool("research_aigc_check", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'text'"));
    }

    #[test]
    fn research_aigc_check_with_language_en() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "This is a test sentence for AIGC detection.", "language": "en"}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("ai_probability").is_some());
        assert!(parsed.get("segments_analyzed").is_some());
    }

    #[test]
    fn research_aigc_check_with_language_zh() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "这是一个用于 AIGC 检测的测试句子。", "language": "zh"}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_aigc_check_default_language() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "Some default language test text."}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn parse_language_defaults_to_english() {
        let value = json!({});
        assert_eq!(
            super::parse_language(&value, "language"),
            crate::aigc::Language::English
        );
    }

    #[test]
    fn parse_language_zh() {
        let value = json!({"language": "zh"});
        assert_eq!(
            super::parse_language(&value, "language"),
            crate::aigc::Language::Chinese
        );
    }

    #[test]
    fn parse_language_en() {
        let value = json!({"language": "en"});
        assert_eq!(
            super::parse_language(&value, "language"),
            crate::aigc::Language::English
        );
    }
}
