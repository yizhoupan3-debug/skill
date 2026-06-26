use observer_rs::AuditJournalEntry;
use std::collections::HashSet;
use std::sync::LazyLock;

pub static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["the", "and", "for", "with", "this", "help", "how", "give", "can", "you"]
        .iter()
        .copied()
        .collect()
});

pub fn stem(word: &str) -> String {
    let mut s = word.to_string();
    if s.len() <= 2 {
        return s;
    }
    if s.ends_with("ing") {
        s.truncate(s.len() - 3);
        let b = s.as_bytes();
        if b.len() >= 2 {
            let last = b[b.len() - 1];
            let second_last = b[b.len() - 2];
            if last == second_last && last.is_ascii_lowercase() {
                s.pop();
            }
        }
    } else if s.ends_with("ed") {
        s.truncate(s.len() - 2);
    } else if s.ends_with("ment") {
        s.truncate(s.len() - 4);
    } else if s.ends_with("s") && !s.ends_with("ss") {
        s.truncate(s.len() - 1);
    }
    if s.len() < 3 {
        word.to_string()
    } else {
        s
    }
}

pub fn entry_is_recent(entry: &AuditJournalEntry, cutoff: chrono::DateTime<chrono::Utc>) -> bool {
    chrono::DateTime::parse_from_rfc3339(&entry.ts)
        .map(|ts| ts.with_timezone(&chrono::Utc) >= cutoff)
        .unwrap_or(false)
}

pub fn canonical_skill_name(raw: &str, known_skills: &HashSet<String>) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "none" || trimmed == "general" {
        return None;
    }
    if known_skills.contains(trimmed) {
        return Some(trimmed.to_string());
    }

    trimmed
        .split(&['+', ',', '/', '|'][..])
        .map(str::trim)
        .find(|part| known_skills.contains(*part))
        .map(str::to_string)
}

pub fn normalize_token(w: &str) -> String {
    if w.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return w.to_string();
    }
    w.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn calculate_jaccard(s1: &str, s2: &str) -> f32 {
    let t1: HashSet<String> = s1
        .split_whitespace()
        .map(normalize_token)
        .filter(|w| !w.is_empty())
        .collect();
    let t2: HashSet<String> = s2
        .split_whitespace()
        .map(normalize_token)
        .filter(|w| !w.is_empty())
        .collect();
    if t1.is_empty() || t2.is_empty() {
        return 0.0;
    }

    let intersection = t1.iter().filter(|w| t2.contains(*w)).count() as f32;
    let union = (t1.len() + t2.len()) as f32 - intersection;
    intersection / union
}

pub fn row_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::String(text) => text.clone(),
        _ => String::new(),
    }
}

pub fn row_terms(value: &serde_json::Value) -> HashSet<&str> {
    match value {
        serde_json::Value::Array(items) => items.iter().filter_map(|item| item.as_str()).collect(),
        serde_json::Value::String(text) => text.split_whitespace().collect(),
        _ => HashSet::new(),
    }
}

pub struct ManifestColumns<'a> {
    pub skills: &'a Vec<serde_json::Value>,
    pub idx_slug: usize,
    pub idx_trigger_hints: usize,
}

pub fn manifest_skill_columns(manifest: &serde_json::Value) -> Option<ManifestColumns<'_>> {
    let skills = manifest.get("skills")?.as_array()?;
    let keys = manifest.get("keys")?.as_array()?;
    let idx_slug = keys.iter().position(|key| key.as_str() == Some("slug"))?;
    let idx_trigger_hints = keys
        .iter()
        .position(|key| matches!(key.as_str(), Some("trigger_hints" | "triggers")))?;
    Some(ManifestColumns {
        skills,
        idx_slug,
        idx_trigger_hints,
    })
}

pub fn truncate_ts_chars(ts: &str, max_chars: usize) -> String {
    ts.chars().take(max_chars).collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_entries(path: &std::path::Path) -> anyhow::Result<Vec<AuditJournalEntry>> {
    observer_rs::load_audit_journal_entries(path)
}
