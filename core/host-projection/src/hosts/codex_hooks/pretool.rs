//! PreToolUse protection: blocks direct edits to generated Codex agent surfaces.
//!
//! Contains path classification, bash write-target detection, and the PreToolUse
//! handler that prevents accidental modification of protected files.

use super::install::protected_generated_paths;
use super::{HOST_ENTRYPOINT_SYNC_HINT, PROTECTED_GENERATED_PREFIXES};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use regex::Regex;

// ---------------------------------------------------------------------------
// PreToolUse handler
// ---------------------------------------------------------------------------

pub(super) fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(&path).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(block_codex_pre_tool_use(message));
        }
    }
    if let Some(path) = bash_generated_write_target(payload) {
        let message = pre_tool_use_message(&path);
        return Ok(block_codex_pre_tool_use(message));
    }
    Ok(None)
}

pub(super) fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

fn block_codex_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex agent surface {path}; rerun `{}` instead.",
        HOST_ENTRYPOINT_SYNC_HINT
    )
}

// ---------------------------------------------------------------------------
// Path classification
// ---------------------------------------------------------------------------

pub(super) fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    let normalized = normalize_repo_relative_path(path);
    if protected_generated_paths().contains(&normalized.as_str()) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Ok(rel) = candidate
            .canonicalize()
            .unwrap_or(candidate.clone())
            .strip_prefix(
                repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf()),
            )
        {
            return normalize_repo_relative_path(&rel.to_string_lossy());
        }
    }
    normalize_repo_relative_path(path)
}

pub(super) fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

// ---------------------------------------------------------------------------
// Payload path extraction
// ---------------------------------------------------------------------------

fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
    }
    candidates
}

// ---------------------------------------------------------------------------
// Bash write-target detection
// ---------------------------------------------------------------------------

fn bash_generated_write_target(payload: &Value) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in protected_generated_paths() {
            if bash_segment_mentions_generated_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    let chars = command.chars().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut separator_len = 0usize;

        if current == ';' {
            separator_len = 1;
        } else if next == Some(current) && matches!(current, '&' | '|') {
            separator_len = 2;
        } else if current == '|' && prev != Some('>') {
            separator_len = 1;
        }

        if separator_len > 0 {
            let segment = chars[start..idx].iter().collect::<String>();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += separator_len;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let tail = chars[start..].iter().collect::<String>();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    if segments.is_empty() {
        vec![command.trim().to_string()]
    } else {
        segments
    }
}

static MUTATING_COMMAND_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

fn bash_command_looks_mutating(command: &str) -> bool {
    MUTATING_COMMAND_PATTERNS.iter().any(|re| re.is_match(command))
}

fn bash_segment_mentions_generated_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    std::thread_local! {
        static HINT_RE_CACHE: std::cell::RefCell<HashMap<String, [Regex; 3]>> =
            std::cell::RefCell::new(HashMap::new());
    }
    HINT_RE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        let regexes = map.entry(hint.to_string()).or_insert_with(|| {
            let escaped = regex::escape(hint);
            let p1 = format!(r#"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p2 = format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p3 = format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            [
                Regex::new(&p1).unwrap(),
                Regex::new(&p2).unwrap(),
                Regex::new(&p3).unwrap(),
            ]
        });
        regexes.iter().any(|re| re.is_match(segment))
    })
}
