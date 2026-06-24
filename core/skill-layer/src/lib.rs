//! Skill layer infrastructure: schema, validation, lifecycle, dependency management.
//!
//! This crate is **self-contained** — all skill-specific logic lives here.
//! Higher layers (runtime-infra, router-rs) delegate via thin wrappers.

pub mod approval;
pub mod columnar;
pub mod constants;
pub mod dependency_graph;
pub mod delete;
pub mod discovery;
pub mod frontmatter;
pub mod frontmatter_parser;
pub mod health;
pub mod paths;
pub mod refresh;
pub mod registry;
pub mod scaffold;
pub mod validate;

pub use frontmatter::{
    RecordKind, RoutingGate, RoutingLayer, RoutingOwner, RoutingPriority, SessionStart,
    SkillDependencies, SkillFrontmatter, SkillFrontmatterSpec,
};
pub use frontmatter_parser::{FrontmatterError, FrontmatterWarning, parse_and_validate};
