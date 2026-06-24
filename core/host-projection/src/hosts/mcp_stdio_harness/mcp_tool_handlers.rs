use super::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Context for tool call dispatch.
pub struct ToolCallContext {
    pub repo_root: std::path::PathBuf,
    pub host_id: String,
    pub connection_session_id: Arc<String>,
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
            if self.name_to_handler.contains_key(name) {
                eprintln!(
                    "[router-rs warning] CompositeRegistry: duplicate tool name '{name}' \
                     registered — existing handler will be overwritten"
                );
            }
            self.name_to_handler.insert(name, idx);
        }
        self.handlers.push(Box::new(handler));
    }

    pub fn contains(&self, tool_name: &str) -> bool {
        self.name_to_handler.contains_key(tool_name)
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
        // Registry already matched tool_name; FrameworkTools only registers one tool.
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
            "routing_evolution" => skill_routing_evolution(args, &ctx.repo_root),
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
            "quality_gate_status",

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
            "quality_gate_status" => tool_quality_gate_status(args, &ctx.repo_root),
            "quality_gate_manage" => tool_quality_gate_manage(args, &ctx.repo_root, &ctx.connection_session_id),
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
        // Registry already matched tool_name; InfraTools only registers one tool.
        tool_web_fetch(args)
    }
}

// ---------------------------------------------------------------------------
// ToolDomainTools (tool registry: route, search, status)
// ---------------------------------------------------------------------------

pub struct ToolDomainTools;
impl ToolHandler for ToolDomainTools {
    fn tool_names(&self) -> &[&'static str] {
        &["route_tool", "search_tools", "tool_registry_status"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "route_tool" => tool_route_tool(args, &ctx.repo_root),
            "search_tools" => tool_search_tools(args, &ctx.repo_root),
            "tool_registry_status" => tool_registry_status(),
            _ => Err(format!("ToolDomainTools: unknown tool: {tool_name}")),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskCrudTools (4 tools: task_create, task_list, task_complete, task_focus)
// ---------------------------------------------------------------------------

pub struct TaskCrudTools;
impl ToolHandler for TaskCrudTools {
    fn tool_names(&self) -> &[&'static str] {
        &["task_create", "task_list", "task_complete", "task_focus"]
    }
    fn dispatch(&self, tool_name: &str, args: &Value, ctx: &ToolCallContext) -> Result<String, String> {
        match tool_name {
            "task_create" => tool_task_create(args, &ctx.repo_root),
            "task_list" => tool_task_list(&ctx.repo_root),
            "task_complete" => tool_task_complete(args, &ctx.repo_root),
            "task_focus" => tool_task_focus(args, &ctx.repo_root),
            _ => Err(format!("TaskCrudTools: unknown tool: {tool_name}")),
        }
    }
}

/// Resolve the tool registry path. Uses hooks, falls back to repo_root default.
fn resolve_tool_registry_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    mcp_tool_registry::resolve_tool_registry_path()
        .unwrap_or_else(|| repo_root.join(framework_kernel::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH))
}

/// route_tool: route a natural language query to the best-matching MCP tool.
fn tool_route_tool(args: &Value, ctx_repo_root: &std::path::Path) -> Result<String, String> {
    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or("route_tool: missing 'query' parameter")?;
    let registry_path = resolve_tool_registry_path(ctx_repo_root);
    let decision = mcp_tool_registry::route_tool(query, &registry_path, None)?
        .ok_or_else(|| format!("route_tool: no matching tool found for query '{query}'"))?;
    serde_json::to_string(&decision).map_err(|e| e.to_string())
}

/// search_tools: search the tool registry and return top-k results.
fn tool_search_tools(args: &Value, ctx_repo_root: &std::path::Path) -> Result<String, String> {
    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or("search_tools: missing 'query' parameter")?;
    let top_k = args.get("top_k")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let registry_path = resolve_tool_registry_path(ctx_repo_root);
    let records = mcp_tool_registry::load_tool_records_cached(&registry_path)
        .map_err(|e| format!("search_tools: failed to load registry: {e}"))?;
    let results = mcp_tool_registry::search_tools(query, &records, top_k);
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

/// tool_registry_status: report registry metadata (count, schema version, layers).
fn tool_registry_status() -> Result<String, String> {
    let registry_path = mcp_tool_registry::resolve_tool_registry_path()
        .ok_or("tool_registry_status: registry path not configured (hooks not registered)")?;
    let records = mcp_tool_registry::load_tool_records_cached(&registry_path)
        .map_err(|e| format!("tool_registry_status: failed to load registry: {e}"))?;
    let total = records.len();
    let builtin = records.iter().filter(|r| r.layer == "builtin").count();
    let research = records.iter().filter(|r| r.layer == "research").count();
    let independent = records.iter().filter(|r| r.layer == "independent").count();
    let external = records.iter().filter(|r| r.layer == "external").count();
    let status = serde_json::json!({
        "schema_version": "mcp-tool-registry-v1",
        "total_count": total,
        "builtin_count": builtin,
        "research_count": research,
        "independent_count": independent,
        "external_count": external,
        "registry_path": registry_path,
    });
    serde_json::to_string(&status).map_err(|e| e.to_string())
}

