#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # tool-routing-engine
//!
//! Tool routing engine: scoring pipeline, routing, and search for MCP tools.
//!
//! Part of the Routing Layer. The parallel crate `routing-engine` provides
//! the equivalent for skill routing. Both share `routing-core` primitives.
//!
//! ## Layer responsibilities
//!
//! | Layer | Crate | Role |
//! |---|---|---|
//! | Tool Layer | `mcp-tool-registry` | Tool record types, JSON loading, path hooks |
//! | Routing Layer | `tool-routing-engine` | Scoring pipeline, `route_tool`, `search_tools` |
//! | Routing Layer | `routing-core` | Shared primitives (fuzzy matching) |

pub mod eval;
pub mod fuzzy;
pub mod hooks;
pub mod routing;
pub mod routing_logger;
pub mod scoring_config;
pub mod search;
pub mod types;

/// Maximum query length in bytes to prevent abuse.
pub(crate) const MAX_QUERY_LEN: usize = 4096;

pub use eval::{evaluate_tool_routing_cases, load_tool_routing_eval_cases};
pub use routing::{route_tool, route_tool_from_records};
pub use routing_logger::{init_tool_routing_logger, is_tool_routing_logger_active, log_tool_decision};
pub use search::search_tools;
pub use types::{McpToolDecision, ToolCandidate};
