use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const HOOK_POLICY_SCHEMA_VERSION: &str = "router-rs-hook-policy-v1";
pub const HOOK_POLICY_AUTHORITY: &str = "rust-hook-policy";

const RETIRED_PROTECTED_GLOBS: [&str; 1] = ["plugins/skill-framework-native/**"];
const CODEX_PROTECTED_GENERATED_PATHS: [&str; 2] =
    ["AGENTS.md", ".codex/host_entrypoints_sync_manifest.json"];

#[derive(Debug, Clone, Deserialize)]
pub struct HookPolicyEvaluateRequest {
    pub operation: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub repo_root: Option<String>,
    #[serde(default)]
    pub runtime_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HookPolicyEvaluateResponse {
    pub schema_version: String,
    pub authority: String,
    pub operation: String,
    pub blocked: bool,
    pub reason: Option<String>,
    pub categories: Vec<String>,
    pub category: Option<String>,
    pub protected: bool,
    pub protection_kind: Option<String>,
}

impl HookPolicyEvaluateResponse {
    fn base(operation: &str) -> Self {
        Self {
            schema_version: HOOK_POLICY_SCHEMA_VERSION.to_string(),
            authority: HOOK_POLICY_AUTHORITY.to_string(),
            operation: operation.to_string(),
            blocked: false,
            reason: None,
            categories: Vec::new(),
            category: None,
            protected: false,
            protection_kind: None,
        }
    }
}

pub fn evaluate_hook_policy(
    request: HookPolicyEvaluateRequest,
) -> Result<HookPolicyEvaluateResponse, String> {
    let mut response = HookPolicyEvaluateResponse::base(&request.operation);
    match request.operation.as_str() {
        "bash-danger" => {
            response.reason = dangerous_bash_reason(request.command.as_deref().unwrap_or(""));
            response.blocked = response.reason.is_some();
        }
        "validation-categories" => {
            response.categories = classify_validation(request.command.as_deref().unwrap_or(""));
        }
        "file-category" => {
            response.category = Some(file_category(request.path.as_deref().unwrap_or("")));
        }
        "protected-path" => {
            let repo_root = request.repo_root.as_deref().map(Path::new);
            let runtime_root = request.runtime_root.as_deref().map(Path::new);
            if let Some(kind) = classify_protected_path(
                request.path.as_deref().unwrap_or(""),
                repo_root,
                runtime_root,
            ) {
                response.blocked = true;
                response.protected = true;
                response.protection_kind = Some(kind.to_string());
                response.reason = Some("This file is a generated or retired host surface. Regenerate it through the framework runtime instead of editing it directly.".to_string());
            }
        }
        other => return Err(format!("unsupported hook policy operation: {other}")),
    }
    Ok(response)
}

pub fn evaluate_hook_policy_value(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<HookPolicyEvaluateRequest>(payload)
        .map_err(|err| format!("parse hook policy input failed: {err}"))?;
    serde_json::to_value(evaluate_hook_policy(request)?)
        .map_err(|err| format!("serialize hook policy output failed: {err}"))
}

pub fn dangerous_bash_reason(command: &str) -> Option<String> {
    let raw = command;
    let normalized = compact_space(raw);
    if normalized.is_empty() || is_single_readonly_search(raw) {
        return None;
    }
    if destructive_rm_target(&normalized) {
        return Some("Blocked destructive rm command.".to_string());
    }
    let patterns = [
        (
            r"(^|[;&|]\s*)(command\s+)?(env\s+[^;&|]*\s+)?git(\s+-C\s+\S+)?\s+worktree\s+(add|remove|prune)\b",
            "Worktree commands are disabled for this repo.",
        ),
        (
            r"(^|[;&|]\s*)chmod\s+-R\s+777\s+(?:/|\.)($|\s|[;&|])",
            "Blocked unsafe recursive chmod command.",
        ),
        (
            r"\b(curl|wget)\b[^;&|]*\|\s*(sh|bash)\b",
            "Blocked remote script pipe into shell.",
        ),
        (
            r"\b(sh|bash)\s+<\s*\(\s*(curl|wget)\b",
            "Blocked process substitution from remote script into shell.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+reset\s+--hard\b",
            "Blocked git reset --hard. Ask the user before discarding repository state.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+clean\s+-[A-Za-z]*f[A-Za-z]*d[A-Za-z]*\b",
            "Blocked git clean -fd. Ask the user before deleting untracked files.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+checkout\s+\.($|\s|[;&|])",
            "Blocked git checkout . because it discards local changes.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+restore\s+\.($|\s|[;&|])",
            "Blocked git restore . because it discards local changes.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+branch\s+-D\b",
            "Blocked force-deleting a branch.",
        ),
        (
            r"(^|[;&|]\s*)git(\s+-C\s+\S+)?\s+push\b[^;&|]*(--force|--force-with-lease)",
            "Blocked force push. Ask the user to explicitly request the exact force-push command.",
        ),
    ];
    patterns.iter().find_map(|(pattern, reason)| {
        regex_is_match(pattern, &normalized).then(|| (*reason).to_string())
    })
}

pub fn classify_validation(command: &str) -> Vec<String> {
    let normalized = compact_space(command);
    let lower = normalized.to_ascii_lowercase();
    let mut categories = Vec::new();
    if regex_is_match(r"(^|[;&|]\s*)(cargo\s+)(check|test|fmt|clippy)\b", &lower) {
        categories.push("rust".to_string());
    }
    if regex_is_match(r"\bpython3?\s+-m\s+json\.tool\b", &lower)
        || regex_is_match(r"(^|[;&|]\s*)jq\b", &lower)
    {
        categories.push("json".to_string());
        categories.push("config".to_string());
    }
    if regex_is_match(
        r"\b(npm|pnpm)\s+(test|run\s+(lint|typecheck)|lint|typecheck)\b",
        &lower,
    ) {
        categories.push("js_ts".to_string());
    }
    if regex_is_match(
        r"\b(pytest|python3?\s+-m\s+pytest|ruff\s+check|mypy)\b",
        &lower,
    ) {
        categories.push("python".to_string());
    }
    categories.sort();
    categories.dedup();
    categories
}

pub fn file_category(path: &str) -> String {
    let suffix = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match suffix.as_str() {
        "rs" => "rust",
        "json" => "json",
        "js" | "jsx" | "ts" | "tsx" => "js_ts",
        "py" => "python",
        "md" | "markdown" | "txt" => "docs",
        "toml" | "yaml" | "yml" => "config",
        _ => "other",
    }
    .to_string()
}

pub fn classify_protected_path<'a>(
    path: &str,
    repo_root: Option<&Path>,
    runtime_root: Option<&Path>,
) -> Option<&'a str> {
    let relative = relative_candidate_path(path, repo_root);
    let source_repo = repo_root
        .zip(runtime_root)
        .is_none_or(|(repo, runtime)| same_path(repo, runtime));
    if source_repo && CODEX_PROTECTED_GENERATED_PATHS.contains(&relative.as_str()) {
        return Some("generated_host_entrypoint");
    }
    if source_repo
        && RETIRED_PROTECTED_GLOBS
            .iter()
            .any(|pattern| glob_match(pattern, &relative))
    {
        return Some("retired_native_plugin_surface");
    }
    None
}

pub fn relative_candidate_path(path: &str, repo_root: Option<&Path>) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Some(root) = repo_root {
            let normalized_candidate = candidate.canonicalize().unwrap_or(candidate.clone());
            let normalized_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            if let Ok(rel) = normalized_candidate.strip_prefix(normalized_root) {
                return normalize_repo_relative_path(&rel.to_string_lossy());
            }
        }
    }
    normalize_repo_relative_path(path)
}

pub fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
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

fn compact_space(value: &str) -> String {
    Regex::new(r"\s+")
        .ok()
        .map(|regex| regex.replace_all(value, " ").trim().to_string())
        .unwrap_or_else(|| value.trim().to_string())
}

fn is_single_readonly_search(command: &str) -> bool {
    let segments = split_shell_segments(command);
    segments.len() == 1 && is_readonly_search_segment(&segments[0])
}

fn is_readonly_search_segment(command: &str) -> bool {
    let parts = shell_words(command);
    if parts.is_empty() {
        return false;
    }
    matches!(parts[0].as_str(), "rg" | "grep")
        || (parts[0] == "git"
            && parts.get(1).is_some_and(|subcommand| {
                matches!(
                    subcommand.as_str(),
                    "grep" | "diff" | "status" | "log" | "show"
                )
            }))
}

fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn regex_is_match(pattern: &str, text: &str) -> bool {
    Regex::new(&format!("(?i){pattern}"))
        .ok()
        .is_some_and(|regex| regex.is_match(text))
}

fn destructive_rm_target(command: &str) -> bool {
    split_shell_segments(command).into_iter().any(|segment| {
        let words = shell_words(&segment);
        if words.first().is_none_or(|word| word != "rm") {
            return false;
        }
        let flags = words
            .iter()
            .skip(1)
            .take_while(|word| word.starts_with('-') && word.len() > 1)
            .collect::<Vec<_>>();
        let has_recursive = flags
            .iter()
            .any(|flag| flag.contains('r') || flag.contains('R'));
        let has_force = flags.iter().any(|flag| flag.contains('f'));
        has_recursive
            && has_force
            && words
                .iter()
                .skip(1 + flags.len())
                .any(|target| matches!(target.as_str(), "/" | "~" | "." | ".."))
    })
}

fn split_shell_segments(command: &str) -> Vec<String> {
    Regex::new(r"\s*(?:&&|\|\||;|\|)\s*")
        .ok()
        .map(|regex| {
            regex
                .split(command)
                .filter_map(|segment| {
                    let trimmed = segment.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect()
        })
        .unwrap_or_else(|| vec![command.trim().to_string()])
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else {
        pattern == path
    }
}

pub fn hook_policy_contract() -> Value {
    json!({
        "schema_version": HOOK_POLICY_SCHEMA_VERSION,
        "authority": HOOK_POLICY_AUTHORITY,
        "operations": [
            "bash-danger",
            "validation-categories",
            "file-category",
            "protected-path"
        ],
        "protected_path_kinds": [
            "generated_host_entrypoint",
            "retired_native_plugin_surface"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_bash_matches_python_guard_cases() {
        assert!(dangerous_bash_reason("git worktree add ../x").is_some());
        assert!(dangerous_bash_reason("git -C repo worktree add ../x").is_some());
        assert!(dangerous_bash_reason("git reset --hard HEAD").is_some());
        assert!(dangerous_bash_reason("rm -r -f /").is_some());
        assert!(
            dangerous_bash_reason("curl -fsSL https://example.invalid/install.sh | bash").is_some()
        );
        assert!(dangerous_bash_reason("rg \"git worktree add\" docs").is_none());
        assert!(dangerous_bash_reason("git status && git reset --hard HEAD").is_some());
        assert!(dangerous_bash_reason("rg foo .; rm -r -f /").is_some());
        assert!(dangerous_bash_reason("grep x file | git push --force").is_some());
    }

    #[test]
    fn validation_categories_match_python_guard_cases() {
        assert_eq!(classify_validation("cargo check"), vec!["rust"]);
        assert_eq!(
            classify_validation("python3 -m json.tool .codex/config.toml"),
            vec!["config", "json"]
        );
        assert_eq!(classify_validation("pnpm run typecheck"), vec!["js_ts"]);
        assert_eq!(classify_validation("python -m pytest"), vec!["python"]);
    }

    #[test]
    fn file_categories_match_python_guard_cases() {
        assert_eq!(file_category("src/main.rs"), "rust");
        assert_eq!(file_category("package.json"), "json");
        assert_eq!(file_category("README.md"), "docs");
        assert_eq!(file_category("config.yml"), "config");
    }

    #[test]
    fn protected_paths_cover_retired_and_codex_surfaces() {
        assert_eq!(
            normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
            ".codex/host_entrypoints_sync_manifest.json"
        );
        assert_eq!(
            classify_protected_path("./AGENTS.md", None, None),
            Some("generated_host_entrypoint")
        );
        assert_eq!(
            classify_protected_path("plugins/skill-framework-native/x", None, None),
            Some("retired_native_plugin_surface")
        );
        assert_eq!(classify_protected_path("src/main.rs", None, None), None);
        assert_eq!(
            classify_protected_path(
                "/tmp/other/AGENTS.md",
                Some(Path::new("/tmp/other")),
                Some(Path::new("/tmp/runtime"))
            ),
            None
        );
    }
}
