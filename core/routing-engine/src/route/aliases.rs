use super::text::normalize_text;
use super::types::SkillRecord;

pub fn framework_alias_entrypoints_from_hints(
    slug_lower: &str,
    layer: &str,
    trigger_hints: &[String],
) -> Vec<String> {
    let mut entrypoints = trigger_hints
        .iter()
        .map(|hint| normalize_text(hint))
        .filter(|hint| {
            if hint == slug_lower {
                return false;
            }
            if let Some(without_prefix) = hint.strip_prefix('/') {
                return without_prefix == slug_lower
                    || without_prefix.starts_with(&format!("{slug_lower}-"))
                    || without_prefix.starts_with(&format!("{slug_lower} "));
            }
            if let Some(without_prefix) = hint.strip_prefix('$') {
                return without_prefix == slug_lower
                    || without_prefix.starts_with(&format!("{slug_lower}-"))
                    || without_prefix.starts_with(&format!("{slug_lower} "));
            }
            false
        })
        .collect::<Vec<_>>();
    if layer == "L0"
        && !entrypoints.is_empty()
        && trigger_hints
            .iter()
            .map(|hint| normalize_text(hint))
            .any(|hint| hint == slug_lower)
    {
        entrypoints.push(slug_lower.to_string());
    }
    entrypoints.sort();
    entrypoints.dedup();
    entrypoints
}

pub fn framework_alias_requires_explicit_call(record: &SkillRecord) -> bool {
    let result = !record.framework_alias_entrypoints.is_empty();
    if result {
    }
    result
}

pub fn has_literal_framework_alias_call(query_text: &str, record: &SkillRecord) -> bool {
    if record
        .framework_alias_entrypoints
        .iter()
        .any(|entrypoint| has_explicit_entrypoint_term(query_text, entrypoint))
    {
        return true;
    }
    // Paper-stack skills advertise `$slug`/`/slug` hints; manuscripts often omit the sigil while
    // still naming the lane token (e.g. `paper-reviewer` vs `$paper-reviewer`).
    framework_alias_plain_paper_slug_token(query_text, record)
}

fn framework_alias_plain_paper_slug_token(query_text: &str, record: &SkillRecord) -> bool {
    !record.framework_alias_entrypoints.is_empty()
        && record.slug.starts_with("paper-")
        && (has_explicit_entrypoint_term(query_text, record.slug.as_str())
            || query_contains_whole_hyphenated_slug(query_text, record.slug.as_str()))
}

/// True when `slug` appears as its own token (handles CJK adjoined text like `用paper-reviewer`).
fn query_contains_whole_hyphenated_slug(query_text: &str, slug: &str) -> bool {
    if !slug.as_bytes().iter().all(|b| b.is_ascii()) {
        return false;
    }
    let mut start = 0usize;
    while let Some(rel) = query_text.get(start..).and_then(|s| s.find(slug)) {
        let pos = start + rel;
        let prev = query_text[..pos].chars().last();
        let next = query_text[pos + slug.len()..].chars().next();
        let prev_ok = prev.is_none_or(|c| !c.is_ascii_alphanumeric());
        let next_ok = next.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        if prev_ok && next_ok {
            return true;
        }
        start = pos + slug.len().max(1);
    }
    false
}

pub fn has_explicit_entrypoint_term(query_text: &str, entrypoint: &str) -> bool {
    query_text.split_whitespace().any(|part| {
        let token = part.trim_matches(|ch: char| {
            matches!(
                ch,
                '(' | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | ','
                    | '.'
                    | '!'
                    | '?'
                    | '，'
                    | '。'
                    | '：'
                    | '；'
                    | '"'
                    | '\''
                    | '`'
            )
        });
        token == entrypoint
            || token.starts_with(&format!("{entrypoint}-"))
            || token.starts_with(&format!("{entrypoint} "))
    })
}

/// Retired framework slash commands must fail-closed to native runtime (no skill owner).
///
/// This is the single maintenance location for retired framework command names.
/// When a framework command is deprecated, add its name here so that it
/// reliably fails closed to the native runtime instead of matching a skill.
/// New framework command constants should be registered in `super::constants`.
const RETIRED_FRAMEWORK_SLASH_COMMANDS: &[&str] = &["/team"];

pub fn query_invokes_retired_framework_slash_command(query_text: &str) -> bool {
    let normalized = normalize_text(query_text);
    RETIRED_FRAMEWORK_SLASH_COMMANDS.iter().any(|cmd| {
        normalized.split_whitespace().any(|part| {
            let token = part.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '(' | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                        | ','
                        | '.'
                        | '!'
                        | '?'
                        | '，'
                        | '。'
                        | '：'
                        | '；'
                        | '"'
                        | '\''
                        | '`'
                )
            });
            token == *cmd
                || token.starts_with(&format!("{cmd}-"))
                || token.starts_with(&format!("{cmd} "))
        })
    })
}

pub fn has_explicit_framework_alias_call(
    query_text: &str,
    query_token_list: &[String],
    record: &SkillRecord,
) -> bool {
    if record.framework_alias_entrypoints.iter().any(|entrypoint| {
        has_explicit_entrypoint_term(query_text, entrypoint)
            || query_token_list.iter().any(|token| token == entrypoint)
    }) {
        return true;
    }
    framework_alias_plain_paper_slug_claims(query_text, query_token_list, record)
}

fn framework_alias_plain_paper_slug_claims(
    query_text: &str,
    query_token_list: &[String],
    record: &SkillRecord,
) -> bool {
    if record.framework_alias_entrypoints.is_empty() || !record.slug.starts_with("paper-") {
        return false;
    }
    has_explicit_entrypoint_term(query_text, record.slug.as_str())
        || query_contains_whole_hyphenated_slug(query_text, record.slug.as_str())
        || query_token_list
            .iter()
            .any(|token| token == record.slug.as_str())
}

/// Map from skill slug to QG Checker ID (Wave 5b).
///
/// When a framework alias matches one of these verification skills, the
/// route decision's `checker_id` is set so the runtime can invoke the
/// QG Checker directly instead of loading a full skill session.
pub fn qg_checker_id_for_slug(slug: &str) -> Option<&'static str> {
    match slug {
        "prose-verification" => Some("prose-qc"),
        "literature-verification" => Some("literature-gate"),
        "statistical-verification" => Some("statistical-gate"),
        "reproducibility-verification" => Some("reproducibility"),
        "structure-verification" => Some("structure-gate"),
        "formal-verification" => Some("formal-gate"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn retired_slash_team_fail_closed_without_skill_owner() {
        assert!(query_invokes_retired_framework_slash_command("/team"));
        assert!(query_invokes_retired_framework_slash_command(
            "please /team orchestrate this"
        ));
        assert!(!query_invokes_retired_framework_slash_command("/workflow"));
    }

    #[test]
    fn hyphenated_slug_rejects_extended_token_with_trailing_hyphen() {
        assert!(!query_contains_whole_hyphenated_slug(
            "paper-reviewer-notes for my draft",
            "paper-reviewer"
        ));
        assert!(query_contains_whole_hyphenated_slug(
            "用paper-reviewer改稿",
            "paper-reviewer"
        ));
        assert!(query_contains_whole_hyphenated_slug(
            "run paper-reviewer on section 2",
            "paper-reviewer"
        ));
    }
}
