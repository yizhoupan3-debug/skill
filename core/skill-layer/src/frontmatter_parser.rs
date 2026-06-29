//! SKILL.md frontmatter extraction, parsing, and validation.
//!
//! Extracts the YAML block between `---` delimiters and deserialises it into
//! [`SkillFrontmatter`].  Returns typed errors for missing/invalid fields.

use crate::frontmatter::{
    RecordKind, RoutingGate, RoutingLayer, RoutingOwner, RoutingPriority, SessionStart,
};
use core_errors::FrameworkError;
use serde::Deserialize;
use std::fmt;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract the raw YAML frontmatter block from a SKILL.md text.
///
/// Returns the content between the first pair of `---` delimiters, or `None`
/// if the file doesn't start with `---`.  Handles BOM and `\r\n` line endings.
pub fn extract_frontmatter_block(text: &str) -> Option<&str> {
    // Strip BOM if present
    let trimmed = text.trim_start_matches('\u{FEFF}').trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Skip opening ---
    let rest = &trimmed[3..];
    // Find closing --- (handle both \n--- and \r\n---)
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---"))?;
    // Skip the newline+--- and any trailing newline
    let content = &rest[..end];
    Some(content)
}

/// Extract the body text after the frontmatter block.
///
/// Returns everything after the closing `---` delimiter, or the entire text
/// if no frontmatter block is found.
pub fn extract_body(text: &str) -> Option<&str> {
    let trimmed = text.trim_start_matches('\u{FEFF}').trim_start();
    if !trimmed.starts_with("---") {
        return Some(trimmed);
    }
    let rest = &trimmed[3..];
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---"))?;
    // Skip past the closing --- and the newline(s) after it
    // Check for CRLF delimiter (\r\n---) vs LF (\n---)
    let skip = if rest[end..].starts_with("\r\n---") { 5 } else { 4 };
    let after_close = &rest[end + skip..];
    let after_close = after_close
        .trim_start_matches('\n')
        .trim_start_matches('\r');
    if after_close.is_empty() {
        None
    } else {
        Some(after_close)
    }
}

/// Error type for frontmatter parsing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterError {
    /// YAML syntax or validation error.
    ParseError(String),
}

impl fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(msg) => write!(f, "frontmatter error: {msg}"),
        }
    }
}

impl std::error::Error for FrontmatterError {}

impl From<FrontmatterError> for FrameworkError {
    fn from(e: FrontmatterError) -> Self {
        FrameworkError::validation(e.to_string())
    }
}

/// Warning emitted when a frontmatter field passes parse but triggers a
/// soft constraint (e.g. `trigger_hints` is empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterWarning {
    /// The `trigger_hints` array is empty — routing will rely on NL fallback only.
    EmptyTriggerHints,
    /// `short_description` is missing (recommended for search display).
    MissingShortDescription,
}

impl fmt::Display for FrontmatterWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTriggerHints => write!(f, "trigger_hints is empty"),
            Self::MissingShortDescription => {
                write!(f, "short_description is missing (recommended)")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Minimal YAML frontmatter struct used for initial parse, then validated
/// field-by-field.  We use `serde_json::Value` for trigger_hints to handle
/// both `["a", "b"]` and multiline YAML lists.
#[derive(Debug, Deserialize)]
struct RawFrontmatter {
    name: Option<String>,
    description: Option<String>,
    routing_layer: Option<String>,
    routing_owner: Option<String>,
    routing_gate: Option<String>,
    routing_priority: Option<String>,
    session_start: Option<String>,
    trigger_hints: Option<serde_json::Value>,
    short_description: Option<String>,
    risk: Option<String>,
    source: Option<String>,
    metadata: Option<serde_json::Value>,
    allowed_tools: Option<Vec<String>>,
    runtime_requirements: Option<serde_json::Value>,
    network_access: Option<String>,
    approval_required_tools: Option<Vec<String>>,
    kind: Option<String>,
    scene: Option<String>,
    sub_scene: Option<String>,
}

fn parse_enum<T: std::str::FromStr + fmt::Debug>(
    field: &str,
    raw: Option<&str>,
) -> std::result::Result<T, FrontmatterError>
where
    <T as std::str::FromStr>::Err: fmt::Display,
{
    let val = raw
        .ok_or_else(|| FrontmatterError::ParseError(format!("missing required field: {field}")))?;
    val.parse::<T>().map_err(|e| {
        FrontmatterError::ParseError(format!("invalid value `{}` for field `{field}`: {e}", val,))
    })
}

/// Parse a serde_json::Value into a Vec<String> of trigger hints.
fn parse_trigger_hints(val: &Option<serde_json::Value>) -> Result<Vec<String>, FrontmatterError> {
    match val {
        None => Ok(Vec::new()),
        Some(serde_json::Value::Array(arr)) => Ok(arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()),
        Some(other) => Err(FrontmatterError::ParseError(format!(
            "trigger_hints must be an array, got: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Public parse + validate
// ---------------------------------------------------------------------------

/// Parse the YAML frontmatter block from SKILL.md text into a strongly-typed
/// [`crate::frontmatter::SkillFrontmatter`].
///
/// Returns `Err` if no frontmatter is found, YAML is malformed, required
/// fields are missing, or enum values are invalid.
pub fn parse_frontmatter(
    text: &str,
) -> std::result::Result<crate::frontmatter::SkillFrontmatter, FrontmatterError> {
    let block = extract_frontmatter_block(text).ok_or(FrontmatterError::ParseError(
        "no YAML frontmatter block found".into(),
    ))?;

    let raw: RawFrontmatter =
        serde_yml::from_str(block).map_err(|e| FrontmatterError::ParseError(e.to_string()))?;

    // Validate required fields
    let _name = raw
        .name
        .as_deref()
        .ok_or_else(|| FrontmatterError::ParseError("missing required field: name".into()))?;
    let _description = raw.description.as_deref().ok_or_else(|| {
        FrontmatterError::ParseError("missing required field: description".into())
    })?;

    let routing_layer: RoutingLayer = parse_enum("routing_layer", raw.routing_layer.as_deref())?;
    let routing_owner: RoutingOwner = parse_enum("routing_owner", raw.routing_owner.as_deref())?;
    let routing_gate: RoutingGate = parse_enum("routing_gate", raw.routing_gate.as_deref())?;
    let routing_priority: RoutingPriority =
        parse_enum("routing_priority", raw.routing_priority.as_deref())?;
    let session_start: SessionStart = parse_enum("session_start", raw.session_start.as_deref())?;
    let trigger_hints = parse_trigger_hints(&raw.trigger_hints)?;

    Ok(crate::frontmatter::SkillFrontmatter {
        name: raw.name.unwrap_or_default(),
        description: raw.description.unwrap_or_default(),
        routing_layer,
        routing_owner,
        routing_gate,
        routing_priority,
        session_start,
        trigger_hints,
        short_description: raw.short_description,
        risk: raw.risk,
        source: raw.source,
        metadata: raw.metadata,
        allowed_tools: raw.allowed_tools,
        runtime_requirements: raw.runtime_requirements,
        network_access: raw.network_access,
        approval_required_tools: raw.approval_required_tools,
        kind: match raw.kind {
            Some(v) => Some(parse_enum("kind", Some(&v))?),
            None => None,
        },
        scene: raw.scene,
        sub_scene: raw.sub_scene,
    })
}

/// Validate soft constraints on a parsed frontmatter.
///
/// Hard constraints (missing required fields, invalid enum values) are caught
/// by [`parse_frontmatter`].  This function returns warnings for best-practice
/// violations that don't prevent routing.
pub fn validate_frontmatter(fm: &crate::frontmatter::SkillFrontmatter) -> Vec<FrontmatterWarning> {
    let mut warnings = Vec::new();
    if fm.trigger_hints.is_empty() {
        warnings.push(FrontmatterWarning::EmptyTriggerHints);
    }
    if fm.short_description.is_none() {
        warnings.push(FrontmatterWarning::MissingShortDescription);
    }
    warnings
}

/// Parse + validate in one step.  Returns the parsed frontmatter and any
/// soft-constraint warnings.
pub fn parse_and_validate(
    text: &str,
) -> std::result::Result<
    (
        crate::frontmatter::SkillFrontmatter,
        Vec<FrontmatterWarning>,
    ),
    FrontmatterError,
> {
    let fm = parse_frontmatter(text)?;
    let warnings = validate_frontmatter(&fm);
    Ok((fm, warnings))
}

// ---------------------------------------------------------------------------
// FromStr impls for enum deserialization from raw YAML strings
// ---------------------------------------------------------------------------

impl std::str::FromStr for RoutingLayer {
    type Err = RoutingLayerParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "L0" => Ok(Self::L0),
            "L1" => Ok(Self::L1),
            "L2" => Ok(Self::L2),
            "L3" => Ok(Self::L3),
            "L4" => Ok(Self::L4),
            _ => Err(RoutingLayerParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct RoutingLayerParseError(String);
impl fmt::Display for RoutingLayerParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid routing_layer `{}`; allowed: L0, L1, L2, L3, L4",
            self.0
        )
    }
}
impl std::error::Error for RoutingLayerParseError {}

impl std::str::FromStr for RoutingOwner {
    type Err = RoutingOwnerParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "gate" => Ok(Self::Gate),
            "user" => Ok(Self::User),
            _ => Err(RoutingOwnerParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct RoutingOwnerParseError(String);
impl fmt::Display for RoutingOwnerParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid routing_owner `{}`; allowed: owner, gate, user",
            self.0
        )
    }
}
impl std::error::Error for RoutingOwnerParseError {}

impl std::str::FromStr for RoutingGate {
    type Err = RoutingGateParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "artifact" => Ok(Self::Artifact),
            "source" => Ok(Self::Source),
            "evidence" => Ok(Self::Evidence),
            "delegation" => Ok(Self::Delegation),
            "approve" => Ok(Self::Approve),
            _ => Err(RoutingGateParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct RoutingGateParseError(String);
impl fmt::Display for RoutingGateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid routing_gate `{}`; allowed: none, artifact, source, evidence, delegation, approve",
            self.0
        )
    }
}
impl std::error::Error for RoutingGateParseError {}

impl std::str::FromStr for RoutingPriority {
    type Err = RoutingPriorityParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "P1" => Ok(Self::P1),
            "P2" => Ok(Self::P2),
            "P3" => Ok(Self::P3),
            _ => Err(RoutingPriorityParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct RoutingPriorityParseError(String);
impl fmt::Display for RoutingPriorityParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid routing_priority `{}`; allowed: P1, P2, P3",
            self.0
        )
    }
}
impl std::error::Error for RoutingPriorityParseError {}

impl std::str::FromStr for SessionStart {
    type Err = SessionStartParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "required" => Ok(Self::Required),
            "preferred" => Ok(Self::Preferred),
            "optional" => Ok(Self::Optional),
            "never" => Ok(Self::Never),
            "n/a" => Ok(Self::NA),
            _ => Err(SessionStartParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct SessionStartParseError(String);
impl fmt::Display for SessionStartParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid session_start `{}`; allowed: required, preferred, optional, never, n/a",
            self.0
        )
    }
}
impl std::error::Error for SessionStartParseError {}

// ── RecordKind ──

impl std::str::FromStr for RecordKind {
    type Err = RecordKindParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skill" => Ok(Self::Skill),
            "framework_command" => Ok(Self::FrameworkCommand),
            _ => Err(RecordKindParseError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct RecordKindParseError(String);
impl fmt::Display for RecordKindParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid kind `{}`; allowed: skill, framework_command",
            self.0
        )
    }
}
impl std::error::Error for RecordKindParseError {}

// ---------------------------------------------------------------------------
// YAML / Markdown utility functions (extracted from skill_lint.rs)
// ---------------------------------------------------------------------------

/// Extract a YAML multiline value (after `key: |` or `key: >`) from raw YAML text.
///
/// Returns the indented content lines joined, or None if the key is not found.
pub fn yaml_multiline_value(yaml_text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let mut lines = yaml_text.lines();
    // Find the key line
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with(&prefix) {
            // Check for block scalar indicator (| or >)
            let rest = line.trim_start()[prefix.len()..].trim();
            if rest == "|" || rest == ">" || rest.starts_with("|+") || rest.starts_with(">-") {
                // Collect indented lines
                let mut value_lines = Vec::new();
                let base_indent = line.len() - line.trim_start().len() + 2;
                for next in lines.by_ref() {
                    if next.trim().is_empty() {
                        break;
                    }
                    let indent = next.len() - next.trim_start().len();
                    if indent >= base_indent {
                        value_lines.push(next.trim_start().to_string());
                    } else {
                        break;
                    }
                }
                return Some(value_lines.join("\n"));
            }
        }
    }
    None
}

/// Count the number of `- ` list items under a YAML key.
///
/// Returns 0 if the key is not found.
pub fn count_yaml_list_items(yaml_text: &str, key: &str) -> usize {
    let prefix = format!("{key}:");
    let lines = yaml_text.lines();
    let mut in_list = false;
    let mut count = 0;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            in_list = true;
            continue;
        }
        if in_list {
            if trimmed.starts_with("- ") {
                count += 1;
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') && !line.starts_with(' ') {
                // Non-indented, non-empty line = end of list
                break;
            }
        }
    }
    count
}

/// Check if a Markdown text contains a specific heading (e.g. `## Do not use`).
pub fn contains_heading(text: &str, heading: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('#') && trimmed.contains(heading)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const SAMPLE_SKILL_MD: &str = r#"---
name: test-skill
description: |
  A test skill for unit testing.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints:
  - test
  - 测试
short_description: Test skill for unit testing.
user-invocable: true
---

# test-skill

A test skill for unit testing.
"#;

    #[test]
    fn extract_frontmatter_block_finds_delimiters() {
        let block = extract_frontmatter_block(SAMPLE_SKILL_MD);
        assert!(block.is_some());
        let block = block.unwrap();
        assert!(block.contains("name: test-skill"));
    }

    #[test]
    fn extract_frontmatter_block_returns_none_for_no_frontmatter() {
        let text = "# Just a heading\n\nSome content.";
        assert!(extract_frontmatter_block(text).is_none());
    }

    #[test]
    fn parse_frontmatter_roundtrips() {
        let fm = parse_frontmatter(SAMPLE_SKILL_MD).expect("parse should succeed");
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.routing_layer, RoutingLayer::L2);
        assert_eq!(fm.routing_owner, RoutingOwner::Owner);
        assert_eq!(fm.routing_gate, RoutingGate::None);
        assert_eq!(fm.routing_priority, RoutingPriority::P2);
        assert_eq!(fm.session_start, SessionStart::NA);
        assert_eq!(fm.trigger_hints, vec!["test", "测试"]);
    }

    #[test]
    fn parse_frontmatter_missing_field() {
        let text = r#"---
name: test
description: desc
routing_layer: L2
routing_owner: owner
routing_gate: none
---
"#;
        let err = parse_frontmatter(text).unwrap_err();
        assert!(
            matches!(err, FrontmatterError::ParseError(_)),
            "expected ParseError, got: {err}"
        );
    }

    #[test]
    fn parse_frontmatter_invalid_enum() {
        let text = r#"---
name: test
description: desc
routing_layer: L9
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints: []
---
"#;
        let err = parse_frontmatter(text).unwrap_err();
        assert!(
            matches!(err, FrontmatterError::ParseError(_)),
            "expected ParseError, got: {err}"
        );
    }

    #[test]
    fn validate_frontmatter_warns_empty_triggers() {
        let fm = parse_frontmatter(
            r#"---
name: test
description: desc
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
trigger_hints: []
---
"#,
        )
        .unwrap();
        let warnings = validate_frontmatter(&fm);
        assert!(warnings.contains(&FrontmatterWarning::EmptyTriggerHints));
    }
}
