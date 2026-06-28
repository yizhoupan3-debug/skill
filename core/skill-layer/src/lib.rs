#![deny(clippy::unwrap_used, clippy::expect_used)]
//! Skill layer infrastructure: schema, validation, lifecycle, dependency management.
//!
//! This crate is **self-contained** — all skill-specific logic lives here.
//! Higher layers (runtime-infra, router-rs) delegate via thin wrappers.

pub mod backfill;
pub mod columnar;
pub mod constants;
pub mod discovery;
pub mod frontmatter;
pub mod frontmatter_parser;
pub mod generate;
pub mod health;
pub mod paths;
pub mod refresh;
pub mod validate;

pub use frontmatter::{
    RecordKind, RoutingGate, RoutingLayer, RoutingOwner, RoutingPriority, SessionStart,
    SkillFrontmatter, SkillFrontmatterSpec,
};
pub use frontmatter_parser::{FrontmatterError, FrontmatterWarning, parse_and_validate};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    #[test]
    fn smoke() {
        assert!(true);
    }
}
