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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingOwner {
    Owner,
    Gate,
    User,
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
}

// ---------------------------------------------------------------------------
// SkillDependencies (Phase 6 extensible field)
// ---------------------------------------------------------------------------

/// Optional dependency declaration in SKILL.md frontmatter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDependencies {
    #[serde(default, rename = "requires")]
    pub requires: Vec<String>,
    #[serde(default, rename = "conflicts-with")]
    pub conflicts_with: Vec<String>,
    #[serde(default, rename = "provides-overlay-for")]
    pub provides_overlay_for: Vec<String>,
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
        "user-invocable",
        "disable-model-invocation",
        "metadata",
        "risk",
        "source",
        "trigger_hints_long",
        "allowed_tools",
        "framework_roles",
        "framework_contracts",
        "runtime_requirements",
        "filesystem_scope",
        "network_access",
        "artifact_outputs",
        "approval_required_tools",
        "plan_profile",
        "dependencies",
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
    pub user_invocable: Option<bool>,
    #[serde(default)]
    pub disable_model_invocation: Option<bool>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    pub framework_roles: Option<Vec<String>>,
    #[serde(default)]
    pub framework_contracts: Option<Value>,
    #[serde(default)]
    pub runtime_requirements: Option<Value>,
    #[serde(default)]
    pub filesystem_scope: Option<Vec<String>>,
    #[serde(default)]
    pub network_access: Option<String>,
    #[serde(default)]
    pub artifact_outputs: Option<Vec<String>>,
    #[serde(default)]
    pub approval_required_tools: Option<Vec<String>>,
    #[serde(default)]
    pub plan_profile: Option<String>,
    #[serde(default)]
    pub dependencies: Option<SkillDependencies>,
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
