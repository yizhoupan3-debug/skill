//! Skill frontmatter schema types and validation constants.
//!
//! Defines the strong-typed enums for every frontmatter field observed across
//! the 43 skills, plus the spec that lists required/optional fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Routing enums (constrained value sets)
// ---------------------------------------------------------------------------

/// Valid values for `routing_owner`.
///
/// `Owner` — competes as a domain owner in routing.
/// `Overlay` — secondary pick that can activate alongside the primary owner
///             (never competes for primary ownership).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingOwner {
    Owner,
    Overlay,
}

/// Valid values for `routing_layer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoutingLayer {
    L0,
    L1,
    L2,
    L3,
    L4,
}

/// Valid values for `routing_gate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingGate {
    None,
    Artifact,
    Source,
    Evidence,
    Delegation,
    Approve,
}

/// Valid values for `routing_priority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoutingPriority {
    P1,
    P2,
    P3,
}

/// Valid values for `session_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStart {
    Required,
    Preferred,
    Optional,
    Never,
    #[serde(rename = "n/a")]
    NA,
}

/// Record kind (maps to `kind` column in SKILL_ROUTING_RUNTIME.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Skill,
    FrameworkCommand,
    Reference,
    Runtime,
}

impl RecordKind {
    /// Canonical string representation matching `#[serde(rename_all = "snake_case")]`.
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordKind::Skill => "skill",
            RecordKind::FrameworkCommand => "framework_command",
            RecordKind::Reference => "reference",
            RecordKind::Runtime => "runtime",
        }
    }
}

// ---------------------------------------------------------------------------
// SkillFrontmatterSpec (field catalogue)
// ---------------------------------------------------------------------------

/// Catalogue of required and optional frontmatter fields.
pub struct SkillFrontmatterSpec;

impl SkillFrontmatterSpec {
    /// Fields that MUST be present in every SKILL.md frontmatter.
    pub const REQUIRED_FIELDS: &'static [&'static str] = &[
        "name",
        "description",
        "routing_layer",
        "routing_owner",
        "routing_gate",
        "routing_priority",
        "session_start",
        "trigger_hints",
    ];

    /// Fields that MAY be present; validated if present.
    pub const OPTIONAL_FIELDS: &'static [&'static str] = &[
        "short_description",
        "metadata",
        "risk",
        "source",
        "allowed_tools",
        "runtime_requirements",
        "network_access",
        "approval_required_tools",
        "scene",
        "sub_scene",
    ];
}

// ---------------------------------------------------------------------------
// SkillFrontmatter (parsed representation)
// ---------------------------------------------------------------------------

/// Parsed frontmatter from a SKILL.md file.
///
/// All routing-related fields use strong enums so invalid values are caught at
/// parse time.  Pass-through fields (metadata, risk, etc.) are kept as generic
/// `serde_json::Value` to avoid schema over-specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Skill slug (must match directory name).
    pub name: String,
    /// Multi-line description.
    pub description: String,
    /// Routing layer: L0–L4.
    pub routing_layer: RoutingLayer,
    /// Routing role: `owner`, `gate`, or `user`.
    pub routing_owner: RoutingOwner,
    /// Gate type.
    pub routing_gate: RoutingGate,
    /// Priority level.
    pub routing_priority: RoutingPriority,
    /// Session-start behaviour.
    pub session_start: SessionStart,
    /// NL trigger phrases.
    pub trigger_hints: Vec<String>,

    // -- optional pass-through fields --
    #[serde(default)]
    pub short_description: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    pub runtime_requirements: Option<Value>,
    #[serde(default)]
    pub network_access: Option<String>,
    #[serde(default)]
    pub approval_required_tools: Option<Vec<String>>,
    /// Record kind: `skill` (default) or `framework_command`.
    #[serde(default)]
    pub kind: Option<RecordKind>,
    /// Scene identifier (Wave 6): defaults to "general" if missing.
    #[serde(default)]
    pub scene: Option<String>,
    /// Sub-scene identifier (Wave 6) for granular checker affinity filtering.
    #[serde(default)]
    pub sub_scene: Option<String>,
}

impl SkillFrontmatter {
    /// Return the `routing_layer` as a `&str` for comparison with routing-engine records.
    pub fn layer_str(&self) -> &'static str {
        match self.routing_layer {
            RoutingLayer::L0 => "L0",
            RoutingLayer::L1 => "L1",
            RoutingLayer::L2 => "L2",
            RoutingLayer::L3 => "L3",
            RoutingLayer::L4 => "L4",
        }
    }
}
