//! Shared PreToolUse path protection for all 4 hosts.
//!
//! Moves Codex's pretool logic into a shared module. Protected paths and
//! error messages are configurable via parameters — not hardcoded to Codex.
//! All hosts can use this module for path protection.

use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// PreToolUse handler
// ---------------------------------------------------------------------------

/// Run PreToolUse path protection with host-specific config.
/// `protected_paths`: set of repo-relative paths to protect.
/// `protected_prefixes`: path prefixes to protect.
/// `entrypoint_hint`: shell command hint for error message (e.g. "codex install").
pub fn run_pre_tool_use(
    repo_root: &Path,
    payload: &Value,
    protected_paths: &HashSet<String>,
    protected_prefixes: &[&str],
    entrypoint_hint: &str,
) -> Result<Option<Value>, String> {
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_path(&path, protected_paths, protected_prefixes).is_some() {
            let message = pre_tool_use_message(&path, entrypoint_hint);
            return Ok(Some(block_pre_tool_use(message)));
        }
    }
    if let Some(path) = bash_write_target(payload, protected_paths, protected_prefixes) {
        let message = pre_tool_use_message(&path, entrypoint_hint);
        return Ok(Some(block_pre_tool_use(message)));
    }
    Ok(None)
}

/// Build a deny response for PreToolUse.
pub fn block_pre_tool_use(reason: String) -> Value {
    json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    })
}

fn pre_tool_use_message(path: &str, entrypoint_hint: &str) -> String {
    format!(
        "[pre-tool-use] blocked direct edits to protected path {path}; rerun `{entrypoint_hint}` instead."
    )
}

// ---------------------------------------------------------------------------
// Path classification
// ---------------------------------------------------------------------------

/// Check if a path matches any protected path or prefix.
pub fn classify_protected_path(
    path: &str,
    protected_paths: &HashSet<String>,
    protected_prefixes: &[&str],
) -> Option<&'static str> {
    let normalized = normalize_repo_relative_path(path);
    if protected_paths.contains(&normalized) {
        return Some("protected_file");
    }
    if protected_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return Some("protected_prefix");
    }
    None
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute()
        && let Ok(rel) = candidate
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
    normalize_repo_relative_path(path)
}

/// Normalize a repo-relative path to a canonical form.
pub fn normalize_repo_relative_path(path: &str) -> String {
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
    for key in &[
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

/// Detect if a Bash command attempts to write to a protected path.
pub fn bash_write_target(
    payload: &Value,
    protected_paths: &HashSet<String>,
    protected_prefixes: &[&str],
) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|ti| ti.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        let all_hints: Vec<String> = protected_paths
            .iter()
            .cloned()
            .chain(protected_prefixes.iter().map(|s| s.to_string()))
            .collect();
        for hint in &all_hints {
            if bash_segment_mentions_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;
    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut sep_len = 0usize;
        if current == ';' {
            sep_len = 1;
        } else if next == Some(current) && matches!(current, '&' | '|') {
            sep_len = 2;
        } else if current == '|' && prev != Some('>') {
            sep_len = 1;
        }
        if sep_len > 0 {
            let seg: String = chars[start..idx].iter().collect();
            let trimmed = seg.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += sep_len;
            start = idx;
            continue;
        }
        idx += 1;
    }
    let tail: String = chars[start..].iter().collect();
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
    MUTATING_COMMAND_PATTERNS
        .iter()
        .any(|re| re.is_match(command))
}

fn bash_segment_mentions_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

#[allow(clippy::unwrap_used)]
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
            let p2 =
                format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p3 =
                format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            [
                Regex::new(&p1).unwrap(),
                Regex::new(&p2).unwrap(),
                Regex::new(&p3).unwrap(),
            ]
        });
        regexes.iter().any(|re| re.is_match(segment))
    })
}
