use super::text::normalize_text;
use super::types::SkillRecord;

pub(crate) fn framework_alias_entrypoints_from_hints(
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
    if slug_lower == "autopilot" {
        let has_slash_entrypoint = entrypoints.iter().any(|value| value == "/autopilot");
        if has_slash_entrypoint {
            entrypoints.extend([
                "/autopilot-quick".to_string(),
                "/autopilot-deep".to_string(),
                "/autopilot quick".to_string(),
                "/autopilot deep".to_string(),
            ]);
        }
    }
    entrypoints.sort();
    entrypoints.dedup();
    entrypoints
}

pub(crate) fn framework_alias_requires_explicit_call(record: &SkillRecord) -> bool {
    !record.framework_alias_entrypoints.is_empty()
}

pub(crate) fn has_literal_framework_alias_call(query_text: &str, record: &SkillRecord) -> bool {
    record
        .framework_alias_entrypoints
        .iter()
        .any(|entrypoint| has_explicit_entrypoint_term(query_text, entrypoint))
}

pub(crate) fn has_explicit_entrypoint_term(query_text: &str, entrypoint: &str) -> bool {
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

pub(crate) fn has_explicit_framework_alias_call(
    query_text: &str,
    query_token_list: &[String],
    record: &SkillRecord,
) -> bool {
    record.framework_alias_entrypoints.iter().any(|entrypoint| {
        has_explicit_entrypoint_term(query_text, entrypoint)
            || query_token_list.iter().any(|token| token == entrypoint)
    })
}
