use crate::types::{LoopAction, LoopRegistryEntry, SafetyLevel};
use std::collections::BTreeMap;

/// Parse a safety level string (`L1`, `L1-report-only`, `L2`, `L2-assisted-fix`, `L3`, `L3-unattended`)
/// into the corresponding `SafetyLevel` enum variant.
/// Returns `None` for unrecognized strings; callers should fall back to `L1ReportOnly`
/// (fail-safe: default to the most restrictive action-observation-only level).
pub fn parse_safety_level(raw: &str) -> Option<SafetyLevel> {
    match raw {
        "L1" | "L1-report-only" => Some(SafetyLevel::L1ReportOnly),
        "L2" | "L2-assisted-fix" => Some(SafetyLevel::L2AssistedFix),
        "L3" | "L3-unattended" => Some(SafetyLevel::L3Unattended),
        _ => None,
    }
}

/// Assign a safety level to a file path based on glob-pattern scope rules from the registry.
/// Falls back to the default level when no rule matches.
pub fn assign_safety_for_file(
    file_path: &str,
    scope_rules: &BTreeMap<String, String>,
    default: &str,
) -> SafetyLevel {
    let path = std::path::Path::new(file_path);
    for (pattern, level_str) in scope_rules {
        if path_matches(path, pattern) {
            return parse_safety_level_or_warn(level_str, "scope_rule");
        }
    }
    parse_safety_level_or_warn(default, "default")
}

/// Parse safety level with warning on unknown values instead of silent downgrade.
fn parse_safety_level_or_warn(raw: &str, context: &str) -> SafetyLevel {
    match parse_safety_level(raw) {
        Some(level) => level,
        None => {
            tracing::warn!(
                "[safety] unknown safety level {:?} in {}, falling back to L1ReportOnly",
                raw,
                context
            );
            SafetyLevel::L1ReportOnly
        }
    }
}

/// Assign a safety level to an action by evaluating its scope paths against the registry's
/// scope-based safety rules. Returns the highest matching level across all scope paths.
pub fn assign_safety_for_action(action: &LoopAction, entry: &LoopRegistryEntry) -> SafetyLevel {
    if action.scope_paths.is_empty() {
        return parse_safety_level_or_warn(
            entry.default_safety.as_deref().unwrap_or("L1"),
            "action_default",
        );
    }

    let scope_rules = entry.scope_based_safety.as_ref();
    let default = entry.default_safety.as_deref().unwrap_or("L1");

    let mut highest = parse_safety_level_or_warn(default, "entry_default");

    if let Some(rules) = scope_rules {
        for path in &action.scope_paths {
            let file_level = assign_safety_for_file(path, rules, default);
            if safety_rank(&file_level) > safety_rank(&highest) {
                highest = file_level;
            }
        }
    }

    highest
}

/// Resolve a conflict between multiple safety levels using the given strategy
/// (`strictest` picks the highest level, `report` always returns L1ReportOnly).
///
/// # Design: fail-safe fallback to L1ReportOnly
/// When no strategy matches or the level list is empty, this function returns
/// `L1ReportOnly` — the most conservative safety level. This is intentional
/// fail-safe behavior: an unrecognized strategy should *restrict* action rather
/// than *permit* it (which would be fail-open). L1ReportOnly means "observe and
/// report only, no file modifications", so the worst outcome of an unknown
/// strategy is a missed opportunity to fix, not an uncontrolled change.
pub fn resolve_conflict(levels: &[SafetyLevel], strategy: &str) -> SafetyLevel {
    if levels.is_empty() {
        return SafetyLevel::L1ReportOnly;
    }
    match strategy {
        "strictest" => levels
            .iter()
            .max_by_key(|l| safety_rank(l))
            .cloned()
            .unwrap_or(SafetyLevel::L1ReportOnly),
        "report" => SafetyLevel::L1ReportOnly,
        unknown => {
            tracing::warn!(
                "[safety] unknown conflict resolution strategy {:?}, using strictest as fail-safe",
                unknown
            );
            levels
                .iter()
                .max_by_key(|l| safety_rank(l))
                .cloned()
                .unwrap_or(SafetyLevel::L1ReportOnly)
        }
    }
}

fn safety_rank(level: &SafetyLevel) -> u8 {
    match level {
        SafetyLevel::L1ReportOnly => 1,
        SafetyLevel::L2AssistedFix => 2,
        SafetyLevel::L3Unattended => 3,
    }
}

fn path_matches(path: &std::path::Path, pattern: &str) -> bool {
    let path_str = path.to_string_lossy();
    let path_str = path_str.trim_start_matches("./");

    let Some(star_pos) = pattern.find('*') else {
        return path_str == pattern
            || path_str.trim_start_matches('/') == pattern.trim_start_matches('/');
    };

    let prefix = &pattern[..star_pos];
    let rest = &pattern[star_pos..];

    if let Some(after_double_star) = rest.strip_prefix("**/") {
        return match_after_double_star(path_str, prefix, after_double_star);
    }

    if let Some(ext) = rest.strip_prefix('*')
        && ext.starts_with('.')
    {
        return match_single_star_ext(path, path_str, prefix, ext);
    }

    let prefix_trimmed = prefix.trim_end_matches('/');
    path_str.starts_with(prefix_trimmed)
}

fn match_after_double_star(path_str: &str, prefix: &str, after_double_star: &str) -> bool {
    if let Some(ext) = after_double_star.strip_prefix('*')
        && ext.starts_with('.')
    {
        if !path_str.starts_with(prefix.trim_end_matches('/')) {
            return false;
        }
        let remainder = &path_str[prefix.trim_end_matches('/').len()..];
        let remainder = remainder.trim_start_matches('/');
        return remainder.ends_with(ext)
            && (!remainder.contains('/')
                || remainder
                    .split('/')
                    .next_back()
                    .map(|f| f.ends_with(ext))
                    .unwrap_or(false));
    }
    if after_double_star.is_empty() {
        return path_str.starts_with(prefix.trim_end_matches('/'));
    }
    path_str.starts_with(prefix.trim_end_matches('/')) && path_str.ends_with(after_double_star)
}

fn match_single_star_ext(path: &std::path::Path, path_str: &str, prefix: &str, ext: &str) -> bool {
    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let prefix_trimmed = prefix.trim_end_matches('/');
    if prefix_trimmed.is_empty() || prefix_trimmed == "." {
        return file_name.ends_with(ext);
    }
    path_str.starts_with(prefix_trimmed) && file_name.ends_with(ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_parse_safety_level() {
        assert_eq!(parse_safety_level("L1"), Some(SafetyLevel::L1ReportOnly));
        assert_eq!(
            parse_safety_level("L2-assisted-fix"),
            Some(SafetyLevel::L2AssistedFix)
        );
        assert_eq!(
            parse_safety_level("L3-unattended"),
            Some(SafetyLevel::L3Unattended)
        );
        assert_eq!(parse_safety_level("L4"), None);
    }

    #[test]
    fn test_assign_safety_for_file() {
        let mut rules = BTreeMap::new();
        rules.insert("src/**/*.rs".to_string(), "L2-assisted-fix".to_string());
        rules.insert("*.md".to_string(), "L3-unattended".to_string());

        assert_eq!(
            assign_safety_for_file("src/main.rs", &rules, "L1"),
            SafetyLevel::L2AssistedFix
        );
        assert_eq!(
            assign_safety_for_file("README.md", &rules, "L1"),
            SafetyLevel::L3Unattended
        );
        assert_eq!(
            assign_safety_for_file("Cargo.toml", &rules, "L1"),
            SafetyLevel::L1ReportOnly
        );
    }

    #[test]
    fn test_safety_rank_ordering() {
        assert!(safety_rank(&SafetyLevel::L3Unattended) > safety_rank(&SafetyLevel::L2AssistedFix));
        assert!(safety_rank(&SafetyLevel::L2AssistedFix) > safety_rank(&SafetyLevel::L1ReportOnly));
    }

    #[test]
    fn test_resolve_conflict_strictest() {
        let levels = vec![SafetyLevel::L1ReportOnly, SafetyLevel::L3Unattended];
        assert_eq!(
            resolve_conflict(&levels, "strictest"),
            SafetyLevel::L3Unattended
        );
    }

    #[test]
    fn test_resolve_conflict_report() {
        let levels = vec![SafetyLevel::L2AssistedFix, SafetyLevel::L3Unattended];
        assert_eq!(
            resolve_conflict(&levels, "report"),
            SafetyLevel::L1ReportOnly
        );
    }

    #[test]
    fn test_resolve_conflict_empty() {
        assert_eq!(resolve_conflict(&[], "split"), SafetyLevel::L1ReportOnly);
    }

    #[test]
    fn test_path_matches_glob() {
        assert!(path_matches(
            std::path::Path::new("src/main.rs"),
            "src/**/*.rs"
        ));
        assert!(path_matches(std::path::Path::new("README.md"), "*.md"));
        assert!(!path_matches(
            std::path::Path::new("Cargo.toml"),
            "src/**/*.rs"
        ));
    }
}
