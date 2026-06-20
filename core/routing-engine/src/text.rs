//! Normalization, tokenization, and JSON string helpers for routing.
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

pub fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))
}

pub fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(raw) => raw.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn value_to_string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .filter(|item| !item.trim().is_empty())
            .collect(),
        Value::Null => Vec::new(),
        _ => split_phrases(&value_to_string(value)),
    }
}

pub fn normalize_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_whitespace = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_whitespace {
                result.push(' ');
            }
            prev_whitespace = true;
        } else {
            result.extend(ch.to_lowercase());
            prev_whitespace = false;
        }
    }
    result
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let lowered = normalize_text(text);
    let mut tokens = Vec::new();
    for capture in token_regex().find_iter(&lowered) {
        let token = capture.as_str().to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
    tokens
}

fn token_regex() -> &'static Regex {
    static TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    TOKEN_REGEX.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9.+#/-]+|[\u{4e00}-\u{9fff}]{2,}").expect("token regex")
    })
}

pub fn phrase_split_regex() -> &'static Regex {
    static PHRASE_SPLIT_REGEX: OnceLock<Regex> = OnceLock::new();
    PHRASE_SPLIT_REGEX.get_or_init(|| Regex::new(r"[,\n/|，]+").expect("phrase split regex"))
}

pub fn common_route_stop_tokens() -> &'static [&'static str] {
    &[
        "一个",
        "帮我",
        "帮我看",
        "我看",
        "先给",
        "给我",
        "给我一",
        "我一个",
        "写一",
        "写一个",
        "写",
        "做",
        "做一个",
        "看这",
        "这张",
        "然后",
        "输出",
        "a",
        "an",
        "and",
        "are",
        "as",
        "for",
        "in",
        "is",
        "of",
        "or",
        "the",
        "to",
        "with",
        "skill",
        "路由",
    ]
}

fn wordlike_token_regex() -> &'static Regex {
    static WORDLIKE_TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    WORDLIKE_TOKEN_REGEX
        .get_or_init(|| Regex::new(r"^[a-z0-9.+#/_-]+$").expect("wordlike token regex"))
}

pub fn tokenize_route_text(text: &str) -> Vec<String> {
    token_regex()
        .find_iter(&normalize_text(text))
        .map(|capture| capture.as_str().to_string())
        .collect()
}

pub fn split_phrases(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut phrases = Vec::new();
    for raw in phrase_split_regex().split(text) {
        let normalized = normalize_text(raw);
        if normalized.is_empty() || normalized == "none" {
            continue;
        }
        if seen.insert(normalized.clone()) {
            phrases.push(normalized);
        }
    }
    phrases
}

pub fn phrase_token_matches(task_token: &str, phrase_token: &str) -> bool {
    if wordlike_token_regex().is_match(phrase_token) {
        task_token == phrase_token
    } else {
        task_token.contains(phrase_token)
    }
}

pub fn text_matches_phrase(task_tokens: &[String], phrase: &str) -> bool {
    let normalized = normalize_text(phrase);
    let phrase_tokens: Vec<&str> = token_regex()
        .find_iter(&normalized)
        .map(|m| m.as_str())
        .collect();
    if phrase_tokens.is_empty() {
        return false;
    }
    // Single-token fast path: zero extra heap allocation beyond normalize_text.
    if phrase_tokens.len() == 1 {
        return task_tokens
            .iter()
            .any(|task_token| phrase_token_matches(task_token, phrase_tokens[0]));
    }
    if phrase_tokens.len() > task_tokens.len() {
        return false;
    }
    for start in 0..=(task_tokens.len() - phrase_tokens.len()) {
        if phrase_tokens
            .iter()
            .enumerate()
            .all(|(offset, &phrase_token)| {
                phrase_token_matches(&task_tokens[start + offset], phrase_token)
            })
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tokenize_query_is_send_safe() {
        let text = "hello world 你好世界".to_string();
        let result = tokio::task::spawn_blocking(move || tokenize_query(&text))
            .await
            .expect("spawn_blocking");
        assert!(!result.is_empty());
        assert!(result.contains(&"hello".to_string()));
        assert!(result.contains(&"你好世界".to_string()));
    }

    #[tokio::test]
    async fn token_regex_is_send_safe() {
        let result = tokio::task::spawn_blocking(|| {
            token_regex().is_match("hello")
        })
            .await
            .expect("spawn_blocking");
        assert!(result);
    }

    #[tokio::test]
    async fn normalize_text_is_send_safe() {
        let text = "Hello   World".to_string();
        let result = tokio::task::spawn_blocking(move || normalize_text(&text))
            .await
            .expect("spawn_blocking");
        assert_eq!(result, "hello world");
    }
}
