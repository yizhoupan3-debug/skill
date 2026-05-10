//! `SkillRecord` construction helpers.
use super::aliases::framework_alias_entrypoints_from_hints;
use super::gate_hints::gate_hint_phrases;
use super::text::{common_route_stop_tokens, normalize_text, tokenize_query};
use super::types::{RawSkillRecord, SkillRecord};
use std::collections::HashSet;

impl SkillRecord {
    pub(crate) fn from_raw(raw: RawSkillRecord) -> Self {
        let RawSkillRecord {
            slug,
            skill_path,
            layer,
            owner,
            gate,
            priority,
            session_start,
            summary,
            short_description,
            when_to_use,
            do_not_use,
            tags,
            trigger_hints,
        } = raw;
        let slug_lower = normalize_text(&slug);
        let owner_lower = normalize_text(&owner);
        let gate_lower = normalize_text(&gate);
        let session_start_lower = normalize_text(&session_start);
        let alias_tokens = tags
            .iter()
            .flat_map(|tag| tokenize_query(tag))
            .collect::<HashSet<_>>();
        let framework_alias_entrypoints =
            framework_alias_entrypoints_from_hints(&slug_lower, &layer, &trigger_hints);
        let do_not_use_tokens = negative_trigger_tokens([do_not_use.as_str()]);
        let gate_phrases = gate_hint_phrases(&gate);
        let name_tokens = tokenize_query(&slug.replace('-', " "))
            .into_iter()
            .collect::<HashSet<_>>();
        let keyword_tokens = tokenize_query(&format!(
            "{summary} {short_description} {when_to_use} {} {}",
            trigger_hints.join(" "),
            tags.join(" ")
        ))
        .into_iter()
        .filter(|token| {
            !common_route_stop_tokens().contains(&token.as_str())
                && (token.chars().count() > 1 || !token.is_ascii())
        })
        .collect::<HashSet<_>>();

        Self {
            slug,
            skill_path,
            layer,
            owner,
            gate,
            priority,
            session_start,
            summary,
            slug_lower,
            owner_lower,
            gate_lower,
            session_start_lower,
            gate_phrases,
            trigger_hints,
            name_tokens,
            keyword_tokens,
            alias_tokens,
            do_not_use_tokens,
            framework_alias_entrypoints,
        }
    }
}

pub(crate) fn negative_trigger_tokens<'a>(
    phrases: impl IntoIterator<Item = &'a str>,
) -> HashSet<String> {
    phrases
        .into_iter()
        .flat_map(tokenize_query)
        .filter(|token| !common_route_stop_tokens().contains(&token.as_str()) && token.len() > 2)
        .collect::<HashSet<_>>()
}
