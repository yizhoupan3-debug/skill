use super::*;
use serde_json::Value;
use std::collections::HashMap;

/// Context for tool call dispatch.
pub struct ToolCallContext {
    pub repo_root: std::path::PathBuf,
    pub host_id: String,
    pub connection_session_id: String,
}

/// Trait for a group of related MCP tools.
pub trait ToolHandler: Send + Sync {
    fn tool_names(&self) -> &[&'static str];
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String>;
}

/// Composite registry that chains multiple ToolHandler implementations.
pub struct CompositeRegistry {
    handlers: Vec<Box<dyn ToolHandler>>,
    name_to_handler: HashMap<&'static str, usize>,
}

impl CompositeRegistry {
    pub fn new() -> Self {
        CompositeRegistry {
            handlers: Vec::new(),
            name_to_handler: HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: impl ToolHandler + 'static) {
        let idx = self.handlers.len();
        for name in handler.tool_names() {
            self.name_to_handler.insert(name, idx);
        }
        self.handlers.push(Box::new(handler));
    }

    pub fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        if let Some(&idx) = self.name_to_handler.get(tool_name) {
            self.handlers[idx].dispatch(tool_name, args, ctx)
        } else {
            Err(format!("Unknown tool: {tool_name}"))
        }
    }
}

// ---------------------------------------------------------------------------
// FrameworkTools (1 tool)
// ---------------------------------------------------------------------------

pub struct FrameworkTools;
impl ToolHandler for FrameworkTools {
    fn tool_names(&self) -> &[&'static str] {
        &["framework_snapshot"]
    }
    fn dispatch(&self, _tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        tool_framework_snapshot(args, &ctx.repo_root)
    }
}

// ---------------------------------------------------------------------------
// RoutingTools (5 tools)
// ---------------------------------------------------------------------------

pub struct RoutingTools;
impl ToolHandler for RoutingTools {
    fn tool_names(&self) -> &[&'static str] {
        &["skill_route", "skill_search", "skill_read", "skill_route_status", "routing_evolution"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "skill_route" => tool_skill_route(args, &ctx.repo_root, &ctx.host_id),
            "skill_search" => tool_skill_search(args, &ctx.repo_root, &ctx.host_id),
            "skill_read" => tool_skill_read(args, &ctx.repo_root),
            "skill_route_status" => tool_skill_route_status(&ctx.repo_root),
            "routing_evolution" => tool_routing_evolution(args, &ctx.repo_root),
            _ => Err(format!("RoutingTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// LifecycleTools (8 tools, including aliases)
// ---------------------------------------------------------------------------

pub struct LifecycleTools;
impl ToolHandler for LifecycleTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "record_evidence",
            "session_checkpoint",
            "closeout_gate",
            "goal_state_read",
            "rfv_loop_status",
            "quality_gate_status",
            "rfv_loop_manage",
            "quality_gate_manage",
            "goal_state_manage",
            "closeout_record_write",
        ]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "record_evidence" => tool_record_evidence(args, &ctx.repo_root),
            "session_checkpoint" => tool_session_checkpoint(args, &ctx.repo_root),
            "closeout_gate" => tool_closeout_gate(args, &ctx.repo_root, &ctx.host_id),
            "goal_state_read" => tool_goal_state_read(args, &ctx.repo_root),
            "rfv_loop_status" | "quality_gate_status" => tool_quality_gate_status(args, &ctx.repo_root),
            "rfv_loop_manage" | "quality_gate_manage" => tool_quality_gate_manage(args, &ctx.repo_root, &ctx.connection_session_id),
            "goal_state_manage" => tool_goal_state_manage(args, &ctx.repo_root, &ctx.connection_session_id),
            "closeout_record_write" => tool_closeout_record_write(args, &ctx.repo_root, &ctx.host_id),
            _ => Err(format!("LifecycleTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// InfraTools (1 tool)
// ---------------------------------------------------------------------------

pub struct InfraTools;
impl ToolHandler for InfraTools {
    fn tool_names(&self) -> &[&'static str] {
        &["web_fetch"]
    }
    fn dispatch(&self, _tool_name: &str, args: &Value, _ctx: &ToolCallContext) -> Result<String, String> {
        tool_web_fetch(args)
    }
}

// ---------------------------------------------------------------------------
// ResearchTools (5 tools — delegates to research-harness, extracted in T1)
// ---------------------------------------------------------------------------

pub struct ResearchTools;
impl ToolHandler for ResearchTools {
    fn tool_names(&self) -> &[&'static str] {
        &[
            "research_aigc_check",
            "research_aigc_humanize",
            "research_review_dimensions",
            "research_claim_drift",
            "research_review_loop",
        ]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, _ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "research_aigc_check" => tool_research_aigc_check(args),
            "research_aigc_humanize" => tool_research_aigc_humanize(args),
            "research_review_dimensions" => tool_research_review_dimensions(args),
            "research_claim_drift" => tool_research_claim_drift(args),
            "research_review_loop" => tool_research_review_loop(args),
            _ => Err(format!("ResearchTools: unknown tool: {tool_name}")),
        }
    }
}


